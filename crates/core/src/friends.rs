//! Friends' presence, as `/lol-chat/v1/friends` and its events report it: who is in a game, in
//! which mode and since when, and which friends play together.
//!
//! The presence is what each friend's own client publishes under `lol` (`gameStatus`, `queueId`,
//! `timeStamp`, `gameId`, `isObservable`, `pty`), strings on the wire. Offline friends carry
//! `lol: {}`; `gameId` stays set after a game ends, so it only means "this game" while the status
//! says one is running.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::view::{FriendStatus, FriendView, FriendsView, QueueInfo, RiotId};

/// The friends list and the prefix of every one of its events.
pub const FRIENDS: &str = "/lol-chat/v1/friends";

/// One row of `/lol-chat/v1/friends`, and the data of a friend's event.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Friend {
    /// The chat id, `<puuid>@<host>`: what a friend's event URI ends with.
    pub id: String,
    pub pid: String,
    pub puuid: String,
    pub game_name: String,
    pub game_tag: String,
    /// The summoner name of old; empty on Tencent shards.
    pub name: String,
    pub icon: i64,
    /// `chat`, `away`, `dnd`, `mobile` or `offline`.
    pub availability: String,
    /// The game's presence fields, each as text: they are strings on the wire, and anything else is
    /// kept as its JSON.
    #[serde(deserialize_with = "text_map")]
    pub lol: HashMap<String, String>,
}

impl Friend {
    fn field(&self, key: &str) -> &str {
        self.lol.get(key).map_or("", |value| value.trim())
    }

    fn number(&self, key: &str) -> i64 {
        self.field(key).parse().unwrap_or(0)
    }

    /// Whether `id`, the last segment of an event's URI, names this friend.
    fn is(&self, id: &str) -> bool {
        [&self.id, &self.pid, &self.puuid]
            .iter()
            .any(|own| !own.is_empty() && own.eq_ignore_ascii_case(id))
    }

    /// The party the presence names, when it names one (`pty` is a JSON document of its own).
    fn party(&self) -> Option<String> {
        let party: Value = serde_json::from_str(self.lol.get("pty")?).ok()?;
        let id = match party.get("partyId")? {
            Value::String(id) => id.trim().to_owned(),
            Value::Number(id) => id.to_string(),
            _ => return None,
        };
        (!id.is_empty()).then_some(id)
    }
}

fn text_map<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<HashMap<String, String>, D::Error> {
    let Value::Object(map) = Value::deserialize(deserializer)? else {
        return Ok(HashMap::new());
    };
    Ok(map
        .into_iter()
        .filter_map(|(key, value)| {
            let text = match value {
                Value::Null => return None,
                Value::String(text) => text,
                other => other.to_string(),
            };
            Some((key, text))
        })
        .collect())
}

/// What `friend` is doing, with the mode named from `queues`; `None` while they are offline.
pub fn status(friend: &Friend, queues: &[QueueInfo]) -> Option<FriendStatus> {
    let availability = friend.availability.trim();
    if availability.is_empty() || availability.eq_ignore_ascii_case("offline") {
        return None;
    }
    let queue_id = friend.number("queueId");
    let since = friend.number("timeStamp").max(0);
    let mode = || {
        mode_label(
            queue_id,
            friend.field("gameQueueType"),
            friend.field("gameMode"),
            queues,
        )
    };
    let game_status = friend.field("gameStatus").to_ascii_lowercase();
    Some(match game_status.as_str() {
        "inprogress" | "ingame" => FriendStatus::InGame {
            mode: mode(),
            queue_id,
            started_at: since,
            observable: observable(friend.field("isObservable")),
        },
        "championselect" => FriendStatus::ChampSelect {
            mode: mode(),
            queue_id,
            since,
        },
        "inqueue" => FriendStatus::InQueue {
            mode: mode(),
            queue_id,
            since,
        },
        // `outOfGame`, `hosting_*` (a lobby), `spectating`, `tutorial` and whatever comes next.
        _ => FriendStatus::OutOfGame,
    })
}

/// The queue's name in the client's catalog, else the presence's own words for it.
fn mode_label(queue_id: i64, queue_type: &str, game_mode: &str, queues: &[QueueInfo]) -> String {
    queues
        .iter()
        .find(|queue| queue_id > 0 && queue.id == queue_id && !queue.name.is_empty())
        .map(|queue| queue.name.clone())
        .or_else(|| (!queue_type.is_empty()).then(|| queue_type.to_owned()))
        .unwrap_or_else(|| game_mode.to_owned())
}

/// `isObservable`: `ALL` (anyone may watch) or a friends-only variant; `NONE` otherwise.
fn observable(value: &str) -> bool {
    let value = value.to_ascii_uppercase();
    value == "ALL" || value == "TRUE" || value.contains("FRIEND")
}

/// What `friend` plays in with others: a running game, by its id, else a party, when the presence
/// names one.
fn together(friend: &Friend, status: &FriendStatus) -> Option<String> {
    let party = || friend.party().map(|party| format!("party:{party}"));
    match status {
        FriendStatus::InGame { .. } => Some(friend.number("gameId"))
            .filter(|&id| id > 0)
            .map(|id| format!("game:{id}"))
            .or_else(party),
        FriendStatus::ChampSelect { .. }
        | FriendStatus::InQueue { .. }
        | FriendStatus::OutOfGame => party(),
    }
}

/// Where a friend sorts: playing first, and the longest at it first within each state.
fn order(status: &FriendStatus) -> (u8, i64) {
    let since = |at: i64| if at > 0 { at } else { i64::MAX };
    match status {
        FriendStatus::InGame { started_at, .. } => (0, since(*started_at)),
        FriendStatus::ChampSelect { since: at, .. } => (1, since(*at)),
        FriendStatus::InQueue { since: at, .. } => (2, since(*at)),
        FriendStatus::OutOfGame => (3, 0),
    }
}

/// The friends signed in, sorted in game first, with a group number shared by the friends in one
/// game or party. The numbers go to the groups in the order they appear, so the colours stay put
/// while nothing ahead of a group changes.
pub fn view(friends: &[Friend], queues: &[QueueInfo]) -> FriendsView {
    let mut rows: Vec<(FriendView, Option<String>)> = friends
        .iter()
        .filter(|friend| !friend.puuid.is_empty())
        .filter_map(|friend| {
            let status = status(friend, queues)?;
            let key = together(friend, &status);
            let name = RiotId::new(&friend.game_name, &friend.game_tag)
                .or_else(|| RiotId::new(&friend.name, ""));
            Some((
                FriendView {
                    puuid: friend.puuid.clone(),
                    name,
                    icon_id: friend.icon,
                    availability: friend.availability.clone(),
                    status,
                    group: None,
                },
                key,
            ))
        })
        .collect();
    // Each friend's client stamps the start on its own, a few milliseconds apart: a group sorts by
    // its earliest member, so it stays together.
    let mut anchors: HashMap<String, (u8, i64)> = HashMap::new();
    for (row, key) in &rows {
        if let Some(key) = key {
            let own = order(&row.status);
            anchors
                .entry(key.clone())
                .and_modify(|anchor| *anchor = (*anchor).min(own))
                .or_insert(own);
        }
    }
    let place = |row: &FriendView, key: &Option<String>| {
        let own = order(&row.status);
        key.as_ref()
            .and_then(|key| anchors.get(key))
            .map_or(own, |anchor| (own.0, anchor.1.min(own.1)))
    };
    let name = |row: &FriendView| {
        row.name
            .as_ref()
            .map(|name| name.game_name.to_lowercase())
            .unwrap_or_default()
    };
    rows.sort_by(|(a, a_key), (b, b_key)| {
        place(a, a_key)
            .cmp(&place(b, b_key))
            .then_with(|| a_key.cmp(b_key))
            .then_with(|| name(a).cmp(&name(b)))
            .then_with(|| a.puuid.cmp(&b.puuid))
    });

    let mut sizes: HashMap<&str, usize> = HashMap::new();
    for key in rows.iter().filter_map(|(_, key)| key.as_deref()) {
        *sizes.entry(key).or_default() += 1;
    }
    let mut numbers: HashMap<String, u8> = HashMap::new();
    let mut groups = Vec::with_capacity(rows.len());
    for (_, key) in &rows {
        let group = key
            .as_deref()
            .filter(|key| sizes.get(key).is_some_and(|&size| size > 1))
            .map(|key| {
                let next = numbers.len().min(usize::from(u8::MAX - 1)) as u8 + 1;
                *numbers.entry(key.to_owned()).or_insert(next)
            });
        groups.push(group);
    }
    FriendsView {
        friends: rows
            .into_iter()
            .zip(groups)
            .map(|((row, _), group)| FriendView { group, ..row })
            .collect(),
    }
}

/// Applies one event of the friends topic to `list`: the whole list again, one friend changed,
/// or one friend gone (`data` is `null`). Returns whether it was one of those.
pub fn apply(list: &mut Vec<Friend>, uri: &str, data: Value) -> bool {
    if uri == FRIENDS {
        return match data {
            Value::Null => {
                list.clear();
                true
            }
            data => match serde_json::from_value::<Vec<Friend>>(data) {
                Ok(friends) => {
                    *list = friends;
                    true
                }
                Err(error) => {
                    tracing::debug!(%error, "friends list did not decode");
                    false
                }
            },
        };
    }
    let Some(id) = uri
        .strip_prefix(FRIENDS)
        .and_then(|rest| rest.strip_prefix('/'))
        .filter(|id| !id.is_empty() && !id.contains('/'))
    else {
        return false;
    };
    let id = percent_decoded(id);
    if data.is_null() {
        list.retain(|friend| !friend.is(&id));
        return true;
    }
    let friend: Friend = match serde_json::from_value(data) {
        Ok(friend) => friend,
        Err(error) => {
            tracing::debug!(%error, "friend event did not decode");
            return false;
        }
    };
    match list
        .iter_mut()
        .find(|known| known.is(&id) || (!friend.puuid.is_empty() && known.puuid == friend.puuid))
    {
        Some(known) => *known = friend,
        None => list.push(friend),
    }
    true
}

/// `%40` and friends back to their characters; an id's `@` may arrive encoded.
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = |at: usize| bytes.get(at).and_then(|byte| (*byte as char).to_digit(16));
        match (bytes[index], hex(index + 1), hex(index + 2)) {
            (b'%', Some(high), Some(low)) => {
                out.push((high * 16 + low) as u8);
                index += 3;
            }
            (byte, _, _) => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn queues() -> Vec<QueueInfo> {
        vec![
            QueueInfo {
                id: 450,
                name: "极地大乱斗".into(),
                game_mode: "ARAM".into(),
                ranked: false,
            },
            QueueInfo {
                id: 420,
                name: "排位赛 单排/双排".into(),
                game_mode: "CLASSIC".into(),
                ranked: true,
            },
        ]
    }

    /// A friend as the list carries them; `lol` as the presence's text fields.
    fn friend(puuid: &str, name: &str, availability: &str, lol: Value) -> Friend {
        serde_json::from_value(json!({
            "id": format!("{puuid}@nj100.pvp.net"),
            "pid": format!("{puuid}@nj100.pvp.net"),
            "puuid": puuid,
            "gameName": name,
            "gameTag": "10001",
            "name": "",
            "icon": 29,
            "availability": availability,
            "lol": lol,
        }))
        .unwrap()
    }

    fn in_game(puuid: &str, name: &str, game: &str, started: &str) -> Friend {
        friend(
            puuid,
            name,
            "dnd",
            json!({"gameStatus": "inProgress", "queueId": "450", "gameQueueType": "ARAM_UNRANKED_5x5", "timeStamp": started, "gameId": game, "isObservable": "ALL"}),
        )
    }

    #[test]
    fn the_presence_says_what_a_friend_plays_and_since_when() {
        let playing = in_game("a", "Ann", "9001", "1791195911713");
        assert_eq!(
            status(&playing, &queues()),
            Some(FriendStatus::InGame {
                mode: "极地大乱斗".into(),
                queue_id: 450,
                started_at: 1_791_195_911_713,
                observable: true
            })
        );
        let unknown_queue = friend(
            "b",
            "Bo",
            "dnd",
            json!({"gameStatus": "inProgress", "queueId": "2400", "gameQueueType": "KIWI", "isObservable": "NONE"}),
        );
        assert_eq!(
            status(&unknown_queue, &queues()),
            Some(FriendStatus::InGame {
                mode: "KIWI".into(),
                queue_id: 2400,
                started_at: 0,
                observable: false
            }),
            "a queue the catalog lacks is named by the presence, a missing start is zero"
        );
        let picking = friend(
            "c",
            "Cy",
            "dnd",
            json!({"gameStatus": "championSelect", "queueId": "420", "timeStamp": "100"}),
        );
        assert_eq!(
            status(&picking, &queues()),
            Some(FriendStatus::ChampSelect {
                mode: "排位赛 单排/双排".into(),
                queue_id: 420,
                since: 100
            })
        );
        let lobby = friend(
            "d",
            "Di",
            "chat",
            json!({"gameStatus": "hosting_RANKED_SOLO_5x5", "gameId": "77"}),
        );
        assert_eq!(status(&lobby, &queues()), Some(FriendStatus::OutOfGame));
        let offline = friend("e", "Ed", "offline", json!({}));
        assert_eq!(status(&offline, &queues()), None);
    }

    #[test]
    fn friends_in_one_game_share_a_group_and_a_lone_friend_has_none() {
        let list = vec![
            friend(
                "z",
                "Zed",
                "chat",
                json!({"gameStatus": "outOfGame", "gameId": "9001"}),
            ),
            // Bo's client stamped the start a moment after Ann's; Dee's game began in between.
            in_game("b", "Bo", "9001", "2003"),
            in_game("d", "Dee", "8000", "2001"),
            in_game("c", "Cy", "7000", "1000"),
            in_game("a", "Ann", "9001", "2000"),
            friend("o", "Off", "offline", json!({})),
            friend(
                "q",
                "Que",
                "dnd",
                json!({"gameStatus": "inQueue", "queueId": "420", "timeStamp": "50", "pty": "{\"partyId\":\"P1\",\"summoners\":[1,2]}"}),
            ),
            friend(
                "r",
                "Rae",
                "dnd",
                json!({"gameStatus": "inQueue", "queueId": "420", "timeStamp": "50", "pty": "{\"partyId\":\"P1\"}"}),
            ),
        ];
        let view = view(&list, &queues());
        let rows: Vec<(&str, Option<u8>)> = view
            .friends
            .iter()
            .map(|row| (row.puuid.as_str(), row.group))
            .collect();
        assert_eq!(
            rows,
            vec![
                ("c", None),
                ("a", Some(1)),
                ("b", Some(1)),
                ("d", None),
                ("q", Some(2)),
                ("r", Some(2)),
                ("z", None),
            ],
            "the longest game first, one game's friends together, the offline left out; an old \
             game id out of game groups nobody"
        );
        assert_eq!(
            view.friends[1].name,
            Some(RiotId {
                game_name: "Ann".into(),
                tag_line: "10001".into()
            })
        );
    }

    #[test]
    fn events_change_one_friend_or_the_whole_list() {
        let mut list = vec![in_game("a", "Ann", "1", "1")];
        let changed = json!({"id": "a@nj100.pvp.net", "puuid": "a", "gameName": "Ann", "availability": "chat", "lol": {"gameStatus": "outOfGame"}});
        assert!(apply(
            &mut list,
            "/lol-chat/v1/friends/a@nj100.pvp.net",
            changed
        ));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].availability, "chat");

        let new =
            json!({"id": "b@nj100.pvp.net", "puuid": "b", "availability": "mobile", "lol": {}});
        assert!(apply(
            &mut list,
            "/lol-chat/v1/friends/b%40nj100.pvp.net",
            new
        ));
        assert_eq!(list.len(), 2);
        assert!(apply(
            &mut list,
            "/lol-chat/v1/friends/a%40nj100.pvp.net",
            Value::Null
        ));
        assert_eq!(
            list.iter().map(|f| f.puuid.as_str()).collect::<Vec<_>>(),
            ["b"]
        );

        assert!(apply(
            &mut list,
            FRIENDS,
            json!([{"puuid": "x"}, {"puuid": "y"}])
        ));
        assert_eq!(list.len(), 2);
        assert!(!apply(&mut list, "/lol-chat/v1/friend-groups", json!([])));
        assert!(!apply(&mut list, "/lol-chat/v1/friends/x/notes", json!({})));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn presence_fields_of_any_json_type_read_as_text() {
        let friend: Friend = serde_json::from_value(json!({
            "puuid": "a",
            "availability": "dnd",
            "lol": {"gameStatus": "inProgress", "queueId": 450, "timeStamp": 12, "gameId": null}
        }))
        .unwrap();
        assert_eq!(friend.number("queueId"), 450);
        assert_eq!(friend.number("timeStamp"), 12);
        assert!(!friend.lol.contains_key("gameId"));
        let offline: Friend = serde_json::from_value(json!({"puuid": "o", "lol": null})).unwrap();
        assert!(offline.lol.is_empty());
        assert_eq!(percent_decoded("a%40b%2"), "a@b%2");
    }
}
