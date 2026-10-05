//! Game data: the names and icons behind every id the views carry.

use lcu::Lcu;
use serde::de::DeserializeOwned;

use crate::{
    model::{ChampionSummary, ClientAugment, Item, Perk, PerkStyles, Queue, SummonerSpell},
    view::{AssetInfo, AugmentInfo, ChampionInfo, GameData, QueueInfo, Rarity},
};

pub async fn load(lcu: &Lcu) -> GameData {
    async fn fetch<T: DeserializeOwned + Default>(lcu: &Lcu, path: &str) -> T {
        lcu.get(path).await.unwrap_or_else(|error| {
            tracing::warn!(%error, "game data unavailable");
            T::default()
        })
    }
    let (champions, items, spells, perks, styles, queues, augments) = tokio::join!(
        fetch::<Vec<ChampionSummary>>(lcu, "/lol-game-data/assets/v1/champion-summary.json"),
        fetch::<Vec<Item>>(lcu, "/lol-game-data/assets/v1/items.json"),
        fetch::<Vec<SummonerSpell>>(lcu, "/lol-game-data/assets/v1/summoner-spells.json"),
        fetch::<Vec<Perk>>(lcu, "/lol-game-data/assets/v1/perks.json"),
        fetch::<PerkStyles>(lcu, "/lol-game-data/assets/v1/perkstyles.json"),
        fetch::<Vec<Queue>>(lcu, "/lol-game-queues/v1/queues"),
        fetch::<Vec<ClientAugment>>(lcu, "/lol-game-data/assets/v1/cherry-augments.json"),
    );
    let mut data = build(champions, items, spells, perks, styles, queues);
    data.augments = augments_of(augments);
    data
}

/// Arena's and Hextech ARAM's augments: the client keeps both in `cherry-augments.json`.
pub fn augments_of(augments: Vec<ClientAugment>) -> Vec<AugmentInfo> {
    augments
        .into_iter()
        .filter(|augment| augment.id > 0)
        .map(|augment| AugmentInfo {
            id: augment.id,
            rarity: Rarity::parse(&augment.rarity),
            name: augment.name,
            icon: augment.augment_small_icon_path,
        })
        .collect()
}

pub fn build(
    champions: Vec<ChampionSummary>,
    items: Vec<Item>,
    spells: Vec<SummonerSpell>,
    perks: Vec<Perk>,
    styles: PerkStyles,
    queues: Vec<Queue>,
) -> GameData {
    let asset = |id: i64, name: String, icon: String| AssetInfo { id, name, icon };
    let mut champions: Vec<ChampionInfo> = champions
        .into_iter()
        .filter(|champion| champion.id > 0)
        .map(|champion| ChampionInfo {
            id: champion.id,
            short_name: if champion.description.is_empty() {
                champion.name.clone()
            } else {
                champion.description
            },
            name: champion.name,
            alias: champion.alias,
            icon: champion.square_portrait_path,
        })
        .collect();
    champions.sort_by(|a, b| a.alias.cmp(&b.alias));

    GameData {
        champions,
        items: items
            .into_iter()
            .filter(|item| item.id > 0)
            .map(|item| asset(item.id, item.name, item.icon_path))
            .collect(),
        spells: spells
            .into_iter()
            .map(|spell| asset(spell.id, spell.name, spell.icon_path))
            .collect(),
        perks: perks
            .into_iter()
            .map(|perk| asset(perk.id, perk.name, perk.icon_path))
            .chain(
                styles
                    .styles
                    .into_iter()
                    .map(|style| asset(style.id, style.name, style.icon_path)),
            )
            .collect(),
        augments: Vec::new(),
        queues: queues
            .into_iter()
            .map(|queue| QueueInfo {
                id: queue.id,
                name: [queue.name, queue.description, queue.short_name]
                    .into_iter()
                    .find(|name| !name.is_empty())
                    .unwrap_or_default(),
                game_mode: queue.game_mode,
                ranked: queue.is_ranked,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;

    #[test]
    fn the_live_catalog_names_champions_the_way_the_client_does() {
        let data = build(
            fixture("live/static/champion-summary.json"),
            fixture("live/static/items.json"),
            fixture("live/static/summoner-spells.json"),
            fixture("live/static/perks.json"),
            fixture("live/static/perkstyles.json"),
            fixture("live/ranked/queues.json"),
        );
        let annie = data
            .champions
            .iter()
            .find(|champion| champion.id == 1)
            .expect("Annie");
        assert_eq!(
            (
                annie.name.as_str(),
                annie.short_name.as_str(),
                annie.alias.as_str()
            ),
            ("黑暗之女", "安妮", "Annie")
        );
        assert_eq!(annie.icon, "/lol-game-data/assets/v1/champion-icons/1.png");
        assert!(data.champions.iter().all(|champion| champion.id > 0));
        assert!(
            data.perks.iter().any(|perk| perk.id == 8000),
            "rune styles are included"
        );
        let solo = data
            .queues
            .iter()
            .find(|queue| queue.id == 420)
            .expect("solo queue");
        assert_eq!(solo.name, "排位赛 单排/双排");
        assert!(data.items.len() > 100 && data.spells.len() > 10);
    }

    #[test]
    fn augments_read_their_text_ids_and_the_clients_rarity() {
        let rows: Vec<ClientAugment> = serde_json::from_value(serde_json::json!([
            {"id": "1004", "nameTRA": "回归基本功", "augmentSmallIconPath": "/lol-game-data/assets/ASSETS/UX/Cherry/Augments/Icons/BackToBasics_small.png", "rarity": "kPrismatic"},
            {"id": 2103, "nameTRA": "狙神飞星", "augmentSmallIconPath": "/x.png", "rarity": "kGold"},
            {"id": "", "nameTRA": "?", "rarity": "kSilver"}
        ]))
        .unwrap();
        let augments = augments_of(rows);
        assert_eq!(augments.len(), 2, "a row without an id is dropped");
        assert_eq!(
            (
                augments[0].id,
                augments[0].name.as_str(),
                augments[0].rarity
            ),
            (1004, "回归基本功", Some(Rarity::Prismatic))
        );
        assert_eq!(
            (augments[1].id, augments[1].rarity),
            (2103, Some(Rarity::Gold))
        );
    }
}
