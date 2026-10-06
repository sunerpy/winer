//! The TypeScript declarations of every type the window and the plugin receive, as one file:
//! `packages/shared/src/bindings.ts`. `cargo test -p winer-core` fails when it is stale;
//! `UPDATE_BINDINGS=1 cargo test -p winer-core bindings` rewrites it.

use ts_rs::{Config, TS};

use crate::{backup, bridge, builds, loadout, plugin, profile, settings, view};

pub fn typescript() -> String {
    let config = Config::new().with_large_int("number");
    let mut out = String::from(
        "// Generated from crates/core by `UPDATE_BINDINGS=1 cargo test -p winer-core bindings`. Do not edit.\n",
    );
    macro_rules! declare {
        ($($ty:ty),+ $(,)?) => {$(
            out.push_str("\nexport ");
            out.push_str(&<$ty as TS>::decl(&config));
            out.push('\n');
        )+};
    }
    declare!(
        view::Snapshot,
        view::Update,
        view::Patch,
        view::Event,
        view::Connection,
        view::Phase,
        view::RiotId,
        view::Me,
        view::Tier,
        view::Rank,
        view::Ranked,
        view::Position,
        view::ChampSelectView,
        view::TimerView,
        view::GameView,
        view::Seat,
        view::SeatRating,
        view::PlayerStats,
        view::PlayerSummary,
        view::RecentForm,
        view::RecentMatch,
        view::ChampionForm,
        view::MatchPage,
        view::HistorySource,
        view::Side,
        view::MatchSummary,
        view::MatchDetail,
        view::TeamDetail,
        view::PlayerLine,
        view::Award,
        view::Feat,
        view::GameData,
        view::ChampionInfo,
        view::AssetInfo,
        view::AugmentInfo,
        view::Rarity,
        view::AugmentDetail,
        view::QueueInfo,
        view::Notice,
        view::NoticeKind,
        view::PlayerProfile,
        view::Presence,
        view::AppInfo,
        view::UpdateStatus,
        view::IpcError,
        view::ErrorCode,
        // Social.
        view::FriendsView,
        view::FriendView,
        view::FriendStatus,
        view::LobbyView,
        view::LobbyMember,
        view::LanePreference,
        view::HotkeyStatus,
        settings::Settings,
        settings::Appearance,
        settings::Theme,
        settings::Accent,
        settings::Density,
        settings::General,
        settings::Language,
        settings::Automation,
        settings::AcceptRule,
        settings::PickRule,
        settings::BanRule,
        settings::CalloutRule,
        settings::Audience,
        settings::TierSet,
        settings::BenchRule,
        settings::Scopes,
        settings::Mode,
        settings::ChampionPool,
        settings::PluginSettings,
        // The profile tools: background, challenges, rank disguise, remembered status, backups.
        settings::ProfileSettings,
        settings::RankDisguise,
        settings::DisguiseQueue,
        settings::Division,
        settings::PresenceRule,
        profile::SkinChoice,
        profile::ChallengeProfile,
        profile::ChallengeToken,
        profile::TitleChoice,
        profile::BannerChoice,
        profile::BannerKind,
        backup::BackupInfo,
        backup::BackupChannel,
        plugin::PluginStatus,
        bridge::BridgeMessage,
        bridge::PluginMessage,
        bridge::LogLevel,
    );
    // Runes, spells, builds and item sets.
    declare!(
        settings::LoadoutRule,
        settings::BuildSettings,
        settings::RiftSource,
        loadout::LoadoutSummary,
        loadout::PageOutcome,
        builds::BuildSource,
        builds::Build,
        builds::Rates,
        builds::SpellOption,
        builds::RunePage,
        builds::RuneOption,
        builds::ItemOption,
        builds::Ability,
        builds::SkillOrder,
        builds::Matchup,
        builds::Matchups,
        builds::AugmentTier,
        builds::AugmentOption,
    );
    // Storage: what winer keeps on disk and in memory, and the cleanup.
    declare!(
        view::DiskUse,
        view::MemoryUse,
        view::StorageLimits,
        view::StorageReport,
        view::CleanupReport,
    );
    // The history panel in the client.
    declare!(bridge::PanelHistory, bridge::PanelGame);
    // The callout's shortcut, the game's chat and whose lines are typed there.
    declare!(
        view::CalloutHotkeyStatus,
        view::CalloutSkip,
        settings::GameTeams
    );
    // History: what the numbers count, custom games, a player rated alone.
    declare!(
        view::GameKind,
        view::FormScope,
        view::PlayerStanding,
        settings::HistorySettings,
    );
    out
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    #[test]
    fn bindings_are_current() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/shared/src/bindings.ts");
        let expected = super::typescript();
        if std::env::var_os("UPDATE_BINDINGS").is_some() {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, &expected).unwrap();
        }
        let actual = fs::read_to_string(&path).unwrap_or_default();
        assert!(
            actual == expected,
            "{} is stale; run `UPDATE_BINDINGS=1 cargo test -p winer-core bindings`",
            path.display()
        );
    }
}
