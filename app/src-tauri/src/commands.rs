//! The command surface the window calls. Every command is `async`: a synchronous Tauri command
//! runs on the main thread and freezes the webview while it works. File work goes to the
//! blocking pool on top of that.

use tauri::{AppHandle, Manager, Runtime, ipc::Invoke};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_opener::OpenerExt as _;
use winer_core::{
    CoreError, Service, Settings,
    backup::{BackupChannel, BackupInfo},
    bridge::Bridge,
    plugin::PluginStatus,
    profile::{ChallengeProfile, SkinChoice},
    settings::{Audience, CalloutRule, General, Language},
    view::{
        AppInfo, AugmentDetail, ErrorCode, GameData, IpcError, MatchDetail, MatchPage,
        PlayerProfile, PlayerSummary, Presence, Snapshot, UpdateStatus,
    },
};

use crate::{Paths, RELEASES_URL, VERSION, elevation, plugin_host, updater};

type Result<T> = std::result::Result<T, IpcError>;

pub(crate) fn handler<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        get_snapshot,
        get_settings,
        set_settings,
        get_game_data,
        get_match_history,
        get_match_detail,
        get_augment_details,
        find_player,
        get_player_summary,
        get_presence,
        set_availability,
        set_status_message,
        restart_client_ui,
        send_callout,
        preview_callout,
        bench_swap,
        reroll,
        get_plugin_status,
        enable_plugin,
        disable_plugin,
        get_app_info,
        relaunch_elevated,
        reveal_logs,
        get_autostart,
        set_autostart,
        get_update_status,
        check_update,
        install_update,
        open_releases,
        open_docs,
        // The profile tools on the Tools page.
        get_skins,
        get_profile_background,
        set_profile_background,
        get_challenge_profile,
        set_challenge_profile,
        get_game_settings_backups,
        create_game_settings_backup,
        restore_game_settings_backup,
        delete_game_settings_backup,
        import_game_settings_backup,
        reveal_game_settings_backup,
    ]
}

fn service<R: Runtime>(app: &AppHandle<R>) -> Service {
    app.state::<Service>().inner().clone()
}

fn internal(error: impl std::fmt::Display) -> IpcError {
    IpcError {
        code: ErrorCode::Internal,
        message: error.to_string(),
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> std::result::Result<T, CoreError> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(internal)?
        .map_err(IpcError::from)
}

#[tauri::command]
async fn get_snapshot<R: Runtime>(app: AppHandle<R>) -> Result<Snapshot> {
    Ok(service(&app).snapshot())
}

#[tauri::command]
async fn get_settings<R: Runtime>(app: AppHandle<R>) -> Result<Settings> {
    Ok(service(&app).settings())
}

#[tauri::command]
async fn set_settings<R: Runtime>(app: AppHandle<R>, settings: Settings) -> Result<Settings> {
    let service = service(&app);
    blocking(move || service.set_settings(settings)).await
}

#[tauri::command]
async fn get_game_data<R: Runtime>(app: AppHandle<R>) -> Result<Option<GameData>> {
    Ok(service(&app).game_data().map(|data| (*data).clone()))
}

#[tauri::command]
async fn get_match_history<R: Runtime>(
    app: AppHandle<R>,
    puuid: String,
    begin: u32,
    count: u32,
) -> Result<MatchPage> {
    Ok(service(&app).match_history(&puuid, begin, count).await?)
}

#[tauri::command]
async fn get_match_detail<R: Runtime>(app: AppHandle<R>, game_id: i64) -> Result<MatchDetail> {
    Ok(service(&app).match_detail(game_id).await?)
}

/// What each Hextech ARAM augment does (ARAM.GG), empty while the user has switched it off.
#[tauri::command]
async fn get_augment_details<R: Runtime>(app: AppHandle<R>) -> Result<Vec<AugmentDetail>> {
    Ok((*service(&app).augment_details().await?).clone())
}

#[tauri::command]
async fn find_player<R: Runtime>(app: AppHandle<R>, riot_id: String) -> Result<PlayerProfile> {
    Ok(service(&app).find_player(&riot_id).await?)
}

#[tauri::command]
async fn get_player_summary<R: Runtime>(app: AppHandle<R>, puuid: String) -> Result<PlayerSummary> {
    Ok(service(&app).player_summary(&puuid).await?)
}

#[tauri::command]
async fn get_presence<R: Runtime>(app: AppHandle<R>) -> Result<Presence> {
    Ok(service(&app).presence().await?)
}

#[tauri::command]
async fn set_availability<R: Runtime>(app: AppHandle<R>, availability: String) -> Result<()> {
    Ok(service(&app).set_availability(&availability).await?)
}

#[tauri::command]
async fn set_status_message<R: Runtime>(app: AppHandle<R>, message: String) -> Result<()> {
    Ok(service(&app).set_status_message(&message).await?)
}

#[tauri::command]
async fn restart_client_ui<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    Ok(service(&app).restart_client_ui().await?)
}

/// The champ-select callout, to `audience` or the configured one; returns the lines sent.
#[tauri::command]
async fn send_callout<R: Runtime>(app: AppHandle<R>, audience: Option<Audience>) -> Result<u32> {
    Ok(service(&app).send_callout(audience).await?)
}

/// What `rule` would send under `general`, shown with the user's own form; for the settings page.
#[tauri::command]
async fn preview_callout<R: Runtime>(
    app: AppHandle<R>,
    rule: CalloutRule,
    general: General,
) -> Result<Vec<String>> {
    Ok(service(&app).preview_callout(&rule, &general).await?)
}

#[tauri::command]
async fn bench_swap<R: Runtime>(app: AppHandle<R>, champion_id: i64) -> Result<()> {
    Ok(service(&app).bench_swap(champion_id).await?)
}

#[tauri::command]
async fn reroll<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    Ok(service(&app).reroll().await?)
}

#[tauri::command]
async fn get_plugin_status<R: Runtime>(app: AppHandle<R>) -> Result<PluginStatus> {
    let (service, bridge) = (service(&app), app.state::<Bridge>().inner().clone());
    blocking(move || {
        Ok(plugin_host::status(
            &service,
            &bridge,
            &app.state::<plugin_host::Host>(),
        ))
    })
    .await
}

/// Turns the in-client features on. A loader linked just now starts with the client's interface,
/// which is restarted for it while the player is idle.
#[tauri::command]
async fn enable_plugin<R: Runtime>(app: AppHandle<R>) -> Result<PluginStatus> {
    let (service, bridge) = (service(&app), app.state::<Bridge>().inner().clone());
    let (status, linked) = {
        let (app, service) = (app.clone(), service.clone());
        blocking(move || plugin_host::enable(&service, &bridge, &app.state::<plugin_host::Host>()))
            .await?
    };
    if linked && let Err(error) = service.restart_client_ui_when_idle().await {
        tracing::warn!(%error, "client interface not restarted");
    }
    Ok(status)
}

#[tauri::command]
async fn disable_plugin<R: Runtime>(app: AppHandle<R>) -> Result<PluginStatus> {
    let (service, bridge) = (service(&app), app.state::<Bridge>().inner().clone());
    blocking(move || plugin_host::disable(&service, &bridge, &app.state::<plugin_host::Host>()))
        .await
}

#[tauri::command]
async fn get_app_info<R: Runtime>(app: AppHandle<R>) -> Result<AppInfo> {
    let paths = app.state::<Paths>();
    Ok(AppInfo {
        version: VERSION.into(),
        elevated: elevation::is_elevated(),
        log_dir: paths.log_dir.display().to_string(),
        settings_path: paths.settings.display().to_string(),
        notices: include_str!("../../../THIRD_PARTY_NOTICES.md").into(),
    })
}

/// Restarts winer elevated. The current process exits once the user has agreed.
#[tauri::command]
async fn relaunch_elevated<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    tauri::async_runtime::spawn_blocking(elevation::relaunch_elevated)
        .await
        .map_err(internal)?
        .map_err(|error| IpcError {
            code: ErrorCode::Invalid,
            message: error.to_string(),
        })?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn reveal_logs<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    let dir = app.state::<Paths>().log_dir.clone();
    app.opener().reveal_item_in_dir(dir).map_err(internal)
}

#[tauri::command]
async fn get_autostart<R: Runtime>(app: AppHandle<R>) -> Result<bool> {
    app.autolaunch().is_enabled().map_err(internal)
}

#[tauri::command]
async fn set_autostart<R: Runtime>(app: AppHandle<R>, enabled: bool) -> Result<bool> {
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    }
    .map_err(internal)?;
    autolaunch.is_enabled().map_err(internal)
}

#[tauri::command]
async fn get_update_status<R: Runtime>(app: AppHandle<R>) -> Result<UpdateStatus> {
    Ok(app.state::<updater::Updater>().status())
}

#[tauri::command]
async fn check_update<R: Runtime>(app: AppHandle<R>) -> Result<UpdateStatus> {
    updater::check(&app).await
}

#[tauri::command]
async fn install_update<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    updater::install(&app).await
}

/// Opens the release page. The URL is built here: the window never opens one it was handed.
#[tauri::command]
async fn open_releases<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    app.opener()
        .open_url(RELEASES_URL, None::<&str>)
        .map_err(internal)
}

/// A page of the documentation the window may open; it names a page, never a URL.
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DocsPage {
    Home,
    Rating,
}

impl DocsPage {
    /// Chinese is the site's root, English sits under `en/`; the site has clean URLs, so a page
    /// is `rating`, not `rating/`.
    fn url(self, language: Language) -> String {
        let root = match language {
            Language::ZhCn => crate::DOCS_URL.to_owned(),
            Language::En => format!("{}en/", crate::DOCS_URL),
        };
        match self {
            Self::Home => root,
            Self::Rating => format!("{root}rating"),
        }
    }
}

#[tauri::command]
async fn open_docs<R: Runtime>(app: AppHandle<R>, page: DocsPage) -> Result<()> {
    let language = service(&app).settings().general.language;
    app.opener()
        .open_url(page.url(language), None::<&str>)
        .map_err(internal)
}

/// Every skin of every champion, owned or not, for the profile background picker.
#[tauri::command]
async fn get_skins<R: Runtime>(app: AppHandle<R>) -> Result<Vec<SkinChoice>> {
    Ok(service(&app).skins().await?)
}

#[tauri::command]
async fn get_profile_background<R: Runtime>(app: AppHandle<R>) -> Result<Option<i64>> {
    Ok(service(&app).profile_background().await?)
}

/// Returns the background the client reports afterwards, which is the old one if it refused.
#[tauri::command]
async fn set_profile_background<R: Runtime>(
    app: AppHandle<R>,
    skin_id: i64,
) -> Result<Option<i64>> {
    Ok(service(&app).set_profile_background(skin_id).await?)
}

#[tauri::command]
async fn get_challenge_profile<R: Runtime>(app: AppHandle<R>) -> Result<ChallengeProfile> {
    Ok(service(&app).challenge_profile().await?)
}

/// The tokens in slot order and the title; returns what the client reports afterwards.
#[tauri::command]
async fn set_challenge_profile<R: Runtime>(
    app: AppHandle<R>,
    challenge_ids: Vec<i64>,
    title_id: Option<i64>,
) -> Result<ChallengeProfile> {
    Ok(service(&app)
        .set_challenge_profile(challenge_ids, title_id)
        .await?)
}

#[tauri::command]
async fn get_game_settings_backups<R: Runtime>(app: AppHandle<R>) -> Result<Vec<BackupInfo>> {
    let service = service(&app);
    blocking(move || service.game_settings_backups()).await
}

#[tauri::command]
async fn create_game_settings_backup<R: Runtime>(app: AppHandle<R>) -> Result<BackupInfo> {
    Ok(service(&app).back_up_game_settings().await?)
}

/// Refused with `busy` outside the lobby and the home screen.
#[tauri::command]
async fn restore_game_settings_backup<R: Runtime>(
    app: AppHandle<R>,
    id: i64,
    channels: Vec<BackupChannel>,
) -> Result<()> {
    Ok(service(&app).restore_game_settings(id, channels).await?)
}

#[tauri::command]
async fn delete_game_settings_backup<R: Runtime>(app: AppHandle<R>, id: i64) -> Result<()> {
    let service = service(&app);
    blocking(move || service.delete_game_settings_backup(id)).await
}

/// A backup file the user picked in the window, as text.
#[tauri::command]
async fn import_game_settings_backup<R: Runtime>(
    app: AppHandle<R>,
    text: String,
) -> Result<BackupInfo> {
    let service = service(&app);
    blocking(move || service.import_game_settings_backup(&text)).await
}

/// Shows a backup's file in the file manager, so it can be copied elsewhere. The window names a
/// backup, never a path.
#[tauri::command]
async fn reveal_game_settings_backup<R: Runtime>(app: AppHandle<R>, id: i64) -> Result<()> {
    let service = service(&app);
    let path = blocking(move || service.game_settings_backup_path(id)).await?;
    app.opener().reveal_item_in_dir(path).map_err(internal)
}

#[cfg(test)]
mod tests {
    use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
    use winer_core::Service;

    use super::*;

    fn app() -> (tauri::App<MockRuntime>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("a mock app builds");
        let service = Service::new(
            dir.path().join("settings.json"),
            tauri::async_runtime::handle().inner().clone(),
        );
        app.manage(service);
        (app, dir)
    }

    #[test]
    fn docs_pages_open_in_the_windows_language_at_their_clean_urls() {
        assert_eq!(
            DocsPage::Rating.url(Language::ZhCn),
            "https://firlab.app/winer/rating"
        );
        assert_eq!(
            DocsPage::Rating.url(Language::En),
            "https://firlab.app/winer/en/rating"
        );
        assert_eq!(
            DocsPage::Home.url(Language::En),
            "https://firlab.app/winer/en/"
        );
    }

    #[test]
    fn commands_resolve_the_service_from_the_app_handle() {
        let (app, _dir) = app();
        let handle = app.handle().clone();
        tauri::async_runtime::block_on(async move {
            let snapshot = get_snapshot(handle.clone()).await.unwrap();
            assert_eq!(snapshot.rev, 0);

            let mut settings = get_settings(handle.clone()).await.unwrap();
            settings.automation.accept.enabled = true;
            let saved = set_settings(handle.clone(), settings.clone())
                .await
                .unwrap();
            assert_eq!(saved, settings);
            assert!(
                get_settings(handle.clone())
                    .await
                    .unwrap()
                    .automation
                    .accept
                    .enabled
            );

            assert_eq!(get_game_data(handle.clone()).await.unwrap(), None);
            let error = get_match_detail(handle.clone(), 1).await.unwrap_err();
            assert_eq!(error.code, ErrorCode::NotConnected);
            let error = find_player(handle.clone(), "  ".into()).await.unwrap_err();
            assert_eq!(error.code, ErrorCode::Invalid);
            for error in [
                send_callout(handle.clone(), None).await.unwrap_err(),
                preview_callout(handle.clone(), CalloutRule::default(), General::default())
                    .await
                    .unwrap_err(),
                bench_swap(handle.clone(), 1).await.unwrap_err(),
                reroll(handle).await.unwrap_err(),
            ] {
                assert_eq!(error.code, ErrorCode::NotConnected);
            }
        });
    }

    #[test]
    fn the_profile_tools_answer_without_a_client() {
        let (app, dir) = app();
        app.state::<Service>()
            .set_backup_dir(dir.path().join("game-settings"));
        let handle = app.handle().clone();
        tauri::async_runtime::block_on(async move {
            for error in [
                get_skins(handle.clone()).await.unwrap_err(),
                get_profile_background(handle.clone()).await.unwrap_err(),
                set_profile_background(handle.clone(), 103015)
                    .await
                    .unwrap_err(),
                get_challenge_profile(handle.clone()).await.unwrap_err(),
                set_challenge_profile(handle.clone(), vec![101304], None)
                    .await
                    .unwrap_err(),
                create_game_settings_backup(handle.clone())
                    .await
                    .unwrap_err(),
                restore_game_settings_backup(handle.clone(), 1, vec![BackupChannel::General])
                    .await
                    .unwrap_err(),
            ] {
                assert_eq!(error.code, ErrorCode::NotConnected);
            }
            // The kept snapshots are files: they list and import with no client at all.
            assert!(
                get_game_settings_backups(handle.clone())
                    .await
                    .unwrap()
                    .is_empty()
            );
            let text = r#"{"format": "winer-game-settings", "version": 1, "takenAt": 1, "inputSettings": {"GameEvents": {}}}"#;
            let imported = import_game_settings_backup(handle.clone(), text.into())
                .await
                .unwrap();
            assert_eq!(
                get_game_settings_backups(handle.clone()).await.unwrap(),
                vec![imported.clone()]
            );
            delete_game_settings_backup(handle.clone(), imported.id)
                .await
                .unwrap();
            let error = reveal_game_settings_backup(handle, imported.id)
                .await
                .unwrap_err();
            assert_eq!(error.code, ErrorCode::Invalid);
        });
    }
}
