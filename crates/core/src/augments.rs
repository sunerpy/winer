//! What Hextech ARAM's augments do, in words. The client names its augments and draws their icons
//! (`catalog`), but describes none of them; the descriptions come from ARAM.GG, a third-party host,
//! fetched once per run and only while the user allows it (`General::augment_details`).

use std::{collections::HashMap, time::Duration};

use reqwest::header::ACCEPT;

use crate::{model::PublishedAugment, net, settings::Language, view::AugmentDetail};

/// The documented per-request bound for this host.
const TIMEOUT: Duration = Duration::from_secs(10);

pub fn url(language: Language) -> String {
    let locale = match language {
        Language::ZhCn => "zh_cn",
        Language::En => "en_us",
    };
    format!("https://aramgg.com/data/aram-mayhem-augments.{locale}.json")
}

pub async fn fetch(language: Language) -> Result<Vec<AugmentDetail>, String> {
    let response = net::client()?
        .get(url(language))
        .header(ACCEPT, "application/json")
        .timeout(TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| error.to_string())?;
    // Through serde_json, not reqwest's `json` feature: that one is only on when the whole
    // workspace builds together.
    let body = response.bytes().await.map_err(|error| error.to_string())?;
    let published: HashMap<String, PublishedAugment> =
        serde_json::from_slice(&body).map_err(|error| error.to_string())?;
    Ok(details(published))
}

/// One plain-text description per augment, by id. The in-game tooltip is preferred; the shorter
/// description stands in where it is missing.
pub fn details(published: HashMap<String, PublishedAugment>) -> Vec<AugmentDetail> {
    let mut details: Vec<AugmentDetail> = published
        .into_iter()
        .filter_map(|(id, augment)| {
            let id = id.trim().parse::<i64>().ok().filter(|&id| id > 0)?;
            let text = if augment.tooltip.trim().is_empty() {
                augment.description
            } else {
                augment.tooltip
            };
            let description = plain_text(&text);
            (!description.is_empty()).then_some(AugmentDetail { id, description })
        })
        .collect();
    details.sort_by_key(|detail| detail.id);
    details
}

/// The game's tooltip markup as text: `<br>` breaks the line, every other tag and inline icon goes,
/// the few entities it uses are decoded, and runs of blanks collapse.
pub fn plain_text(markup: &str) -> String {
    let mut text = String::with_capacity(markup.len());
    let mut rest = markup;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            // A lone `<` is text.
            text.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let tag = rest[start + 1..start + end].trim().to_ascii_lowercase();
        if tag == "br" || tag == "br/" || tag == "br /" {
            text.push('\n');
        }
        rest = &rest[start + end + 1..];
    }
    text.push_str(rest);
    // `%i:scaleCrit%` draws an icon in the game's own tooltip; as text it is noise.
    while let Some(start) = text.find("%i:") {
        let Some(length) = text[start + 3..].find('%') else {
            break;
        };
        text.replace_range(start..start + 3 + length + 1, "");
    }
    let text = text
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_becomes_text_with_its_line_breaks() {
        let tooltip = "你的终极技能已被封印。获得35%技能伤害、<healing>治疗效果</healing>和<scaleAbilityHaste>70技能急速</scaleAbilityHaste>。<br><br><font color='#F0C200'>奖励：</font>一颗流星";
        assert_eq!(
            plain_text(tooltip),
            "你的终极技能已被封印。获得35%技能伤害、治疗效果和70技能急速。\n\n奖励：一颗流星"
        );
        assert_eq!(plain_text("a  b &amp; c <br/> d"), "a b & c\nd");
        assert_eq!(plain_text("x < y"), "x < y", "a lone bracket is text");
        assert_eq!(
            plain_text("获得%i:scaleCrit%25%暴击几率"),
            "获得25%暴击几率",
            "icon tokens go"
        );
    }

    #[test]
    fn details_prefer_the_tooltip_and_skip_what_cannot_be_used() {
        let published: HashMap<String, PublishedAugment> =
            serde_json::from_value(serde_json::json!({
                "1116": {"description": "短", "tooltip": "你的闪现有3层充能。"},
                "2103": {"description": "<b>只有</b>描述", "tooltip": ""},
                "abc": {"description": "无效 id", "tooltip": ""},
                "7": {"description": "", "tooltip": ""}
            }))
            .unwrap();
        assert_eq!(
            details(published),
            vec![
                AugmentDetail {
                    id: 1116,
                    description: "你的闪现有3层充能。".into()
                },
                AugmentDetail {
                    id: 2103,
                    description: "只有描述".into()
                },
            ]
        );
        assert!(url(Language::ZhCn).ends_with("aram-mayhem-augments.zh_cn.json"));
    }
}
