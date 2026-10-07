//! The shell's half of the in-client plugin: which loader, the loader winer ships, the plugin
//! bundle, and the bootstrap file that tells the plugin where this session's bridge listens.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use tracing::{info, warn};
use winer_core::{
    CoreError, Service,
    bridge::Bridge,
    plugin::{self, Linked, Loader, PluginStatus},
    service::UiRestart,
};

use crate::{PENGU_CORE, PENGU_VERSION, PLUGIN_BUNDLE, elevation};

/// What every setup and status reads about the machine: where the client is, and which program,
/// if any, Windows starts in place of its interface.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Env {
    pub client_dir: Option<PathBuf>,
    /// The file name of the program IFEO's `Debugger` names for `LeagueClientUx.exe`, the way
    /// Pengu Loader 1.2 loads itself (`read_ifeo`).
    pub foreign: Option<String>,
}

type Probe = Box<dyn Fn(&Service) -> Env + Send + Sync>;

/// Where the loader winer ships lives, and what went wrong the last time it was set up.
pub(crate) struct Host {
    /// winer's own data folder for it: outside the install directory, so updating winer never
    /// has to replace a DLL the client holds open.
    pub dir: PathBuf,
    failure: Mutex<Option<Failure>>,
    /// Read anew by every setup and status, so that all of them see the same machine.
    probe: Probe,
}

/// Why the last setup did not finish.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Failure {
    /// As the system put it.
    message: String,
    /// Only an administrator can create the link, and winer runs without those rights.
    needs_elevation: bool,
}

impl Failure {
    /// A failure no administrator rights would avoid.
    fn other(error: &std::io::Error) -> Self {
        Self {
            message: error.to_string(),
            needs_elevation: false,
        }
    }
}

/// What a setup came to, for its caller to act on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Outcome {
    /// A loader was linked just now; the client loads it when its interface starts again.
    pub linked: bool,
    /// The link was refused for want of administrator rights, which winer runs without.
    pub needs_elevation: bool,
    /// Another program launches the client's interface ([`Env`]).
    pub foreign: Option<String>,
    /// And winer stepped aside for it: the experimental switch is on, so no loader was linked.
    pub yielded: bool,
}

impl Host {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self::probing(
            dir,
            Box::new(|service: &Service| Env {
                client_dir: service.client_dir(),
                foreign: read_ifeo(),
            }),
        )
    }

    fn probing(dir: PathBuf, probe: Probe) -> Self {
        Self {
            dir,
            failure: Mutex::new(None),
            probe,
        }
    }

    /// A host that always sees `env`.
    #[cfg(test)]
    fn with_env(dir: PathBuf, env: Env) -> Self {
        Self::probing(dir, Box::new(move |_: &Service| env.clone()))
    }

    fn env(&self, service: &Service) -> Env {
        (self.probe)(service)
    }

    fn set_failure(&self, failure: Option<Failure>) {
        *self
            .failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = failure;
    }

    fn failure(&self) -> Option<Failure> {
        self.failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

/// Whether winer steps aside for another program launching the client: one does, and the
/// experimental switch for it is on. Off, the program is only reported.
fn yields(service: &Service, env: &Env) -> bool {
    env.foreign.is_some() && service.settings().plugin.pengu_ifeo
}

/// The loader to keep up: the configured folder if it holds one, else wherever the client's
/// `version.dll` link points. While winer steps aside for another program launching the client,
/// only the linked one: the configured folder may well be that program's, and nothing of winer's
/// goes there.
fn loader(service: &Service, env: &Env) -> Option<Loader> {
    let configured = (!yields(service, env))
        .then(|| service.settings().plugin.loader_dir)
        .flatten();
    plugin::find_loader(
        env.client_dir.as_deref(),
        configured.as_deref().map(Path::new),
    )
}

/// The program Windows starts in place of the client's interface: the file name in the IFEO
/// `Debugger` value of `LeagueClientUx.exe`, `None` when there is none. winer reads the key and
/// never writes it.
pub(crate) fn read_ifeo() -> Option<String> {
    #[cfg(windows)]
    {
        ifeo::debugger().as_deref().and_then(program_name)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// The file name of the program a `Debugger` command line starts: the quoted path, else up to and
/// including the first `.exe`, else up to the first space. For `rundll32`, the DLL it starts as
/// well: Pengu Loader 1.2 writes `rundll32 "…\core.dll", #6000` (measured on 1.2.0-dev).
#[cfg_attr(not(windows), allow(dead_code))]
fn program_name(command: &str) -> Option<String> {
    let command = command.trim();
    let (path, rest) = match command.strip_prefix('"') {
        Some(quoted) => quoted.split_once('"').unwrap_or((quoted, "")),
        None => match command.to_ascii_lowercase().find(".exe") {
            Some(at) => command.split_at(at + 4),
            None => command.split_once(' ').unwrap_or((command, "")),
        },
    };
    let name = file_name(path)?;
    let launcher = name
        .strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name);
    if launcher.eq_ignore_ascii_case("rundll32") {
        let rest = rest.trim_start();
        let argument = match rest.strip_prefix('"') {
            Some(quoted) => quoted.split('"').next().unwrap_or(quoted),
            None => rest.split([',', ' ']).next().unwrap_or(rest),
        };
        if let Some(dll) = file_name(argument) {
            return Some(format!("{name} {dll}"));
        }
    }
    Some(name.to_owned())
}

/// The last component of a Windows or Unix path, when there is one.
#[cfg_attr(not(windows), allow(dead_code))]
fn file_name(path: &str) -> Option<&str> {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path).trim();
    (!name.is_empty()).then_some(name)
}

#[cfg(windows)]
mod ifeo {
    #![allow(unsafe_code)]

    use std::{ffi::OsStr, os::windows::ffi::OsStrExt as _, ptr};

    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY_LOCAL_MACHINE, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
            RRF_SUBKEY_WOW6464KEY, RegGetValueW,
        },
    };

    const KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\LeagueClientUx.exe";

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain([0]).collect()
    }

    /// The key's `Debugger` value as written, from the 64-bit view.
    pub(super) fn debugger() -> Option<String> {
        let (key, value) = (wide(KEY), wide("Debugger"));
        let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND | RRF_SUBKEY_WOW6464KEY;
        let mut size = 0u32;
        // SAFETY: both names end in NUL; a null buffer asks for the size alone.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                value.as_ptr(),
                flags,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS || size < 2 {
            return None;
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
        // SAFETY: `buffer` holds at least `size` bytes, the size passed with it.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                value.as_ptr(),
                flags,
                ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let end = buffer
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..end]))
    }
}

pub(crate) fn status(service: &Service, bridge: &Bridge, host: &Host) -> PluginStatus {
    let env = host.env(service);
    let loader = loader(service, &env);
    let client_dir = env.client_dir;
    let failure = host.failure();
    PluginStatus {
        loader_dir: loader
            .as_ref()
            .map(|loader| loader.dir.display().to_string()),
        active: loader.as_ref().is_some_and(|loader| loader.active),
        managed: loader
            .as_ref()
            .is_some_and(|loader| same_dir(&loader.dir, &host.dir)),
        bundled_loader: PENGU_VERSION.into(),
        occupied: client_dir.as_deref().is_some_and(plugin::occupied),
        setup_error: failure.as_ref().map(|failure| failure.message.clone()),
        needs_elevation: failure.is_some_and(|failure| failure.needs_elevation),
        installed_version: loader
            .as_ref()
            .and_then(|loader| plugin::installed_version(&loader.dir)),
        bundled_version: plugin::bundle_version(PLUGIN_BUNDLE).unwrap_or_default(),
        current: loader
            .as_ref()
            .and_then(|loader| plugin::is_current(&loader.dir, PLUGIN_BUNDLE))
            .unwrap_or(false),
        connected: bridge.connected(),
        foreign_activation: env.foreign,
    }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Turns automatic setup on and sets everything up now, if a client is connected. Also returns
/// what that came to, as [`refresh`] does.
pub(crate) fn enable(
    service: &Service,
    bridge: &Bridge,
    host: &Host,
) -> Result<(PluginStatus, Outcome), CoreError> {
    let mut settings = service.settings();
    settings.plugin.auto = true;
    service.set_settings(settings)?;
    let outcome = refresh(service, bridge, host);
    Ok((status(service, bridge, host), outcome))
}

/// Turns automatic setup off, removes the plugin, and unlinks the loader winer ships from the
/// client; a loader the user installed stays linked. Both take effect when the client's interface
/// next starts.
pub(crate) fn disable(
    service: &Service,
    bridge: &Bridge,
    host: &Host,
) -> Result<PluginStatus, CoreError> {
    let mut settings = service.settings();
    settings.plugin.auto = false;
    service.set_settings(settings)?;
    let env = host.env(service);
    if let Some(loader) = loader(service, &env) {
        plugin::uninstall(&loader.dir)?;
        info!(dir = %loader.dir.display(), "plugin removed");
    }
    if let Some(client_dir) = env.client_dir.as_deref()
        && plugin::unlink(client_dir, &host.dir)?
    {
        info!(client = %client_dir.display(), "winer's loader unlinked from the client");
    }
    host.set_failure(None);
    Ok(status(service, bridge, host))
}

/// On every client connection. With automatic setup on, the default: the loader is the one
/// already linked into the client or configured, else the one winer ships, placed in its data
/// folder and linked; the plugin is installed or brought to this build. With it off, only an
/// installed plugin is brought to this build. Either way the plugin learns this session's bridge.
/// Returns whether a loader was newly linked, which the client loads only when its interface
/// starts again, and whether linking one needs winer restarted elevated.
///
/// While another program launches the client (Pengu Loader 1.2 through IFEO) and the experimental
/// switch for it is on, a loader of winer's on top would load twice: none is linked, the configured
/// folder is left alone, and only a loader the client links already is kept up. With the switch
/// off, setup goes on as before and the program is only reported.
pub(crate) fn refresh(service: &Service, bridge: &Bridge, host: &Host) -> Outcome {
    let env = host.env(service);
    let auto = service.settings().plugin.auto;
    let yielded = yields(service, &env);
    let mut outcome = Outcome {
        foreign: env.foreign.clone(),
        yielded,
        ..Outcome::default()
    };
    if yielded {
        info!(program = ?env.foreign, "another program launches the client; winer links no loader");
        host.set_failure(None);
    } else if auto && let Some(client_dir) = env.client_dir.as_deref() {
        match activate(service, host, &env, client_dir) {
            Ok(created) => {
                outcome.linked = created;
                host.set_failure(None);
            }
            Err(failure) => {
                warn!(
                    error = %failure.message,
                    needs_elevation = failure.needs_elevation,
                    "the loader was not set up"
                );
                outcome.needs_elevation = failure.needs_elevation;
                host.set_failure(Some(failure));
            }
        }
    }
    let Some(loader) = loader(service, &env) else {
        return outcome;
    };
    let bundled = plugin::bundle_version(PLUGIN_BUNDLE);
    let installed = plugin::installed_version(&loader.dir);
    let stale = plugin::is_current(&loader.dir, PLUGIN_BUNDLE) != Some(true);
    if stale && (auto || installed.is_some()) {
        match plugin::install(&loader.dir, PLUGIN_BUNDLE) {
            Ok(()) => info!(from = ?installed, to = ?bundled, "plugin installed"),
            Err(error) => warn!(%error, "plugin not installed"),
        }
    }
    match plugin::write_bootstrap(&loader.dir, bridge.port(), bridge.token()) {
        Ok(true) => info!(port = bridge.port(), "plugin bootstrap written"),
        Ok(false) => {}
        Err(error) => warn!(%error, "plugin bootstrap not written"),
    }
    outcome
}

/// Starts a loader linked just now. The client loads it only as its interface starts, so the
/// interface is restarted for it once the client has finished starting with the player idle, and
/// brought up again afterwards (`Service::restart_client_ui_when_idle`). That can take minutes, so
/// it runs on a task of its own: the caller may be a command the window waits on.
pub(crate) fn start_loader(service: &Service) {
    let service = service.clone();
    tauri::async_runtime::spawn(async move {
        match service.restart_client_ui_when_idle().await {
            Ok(UiRestart { plugin_back, shown }) => info!(
                plugin_back = plugin_back,
                shown = shown,
                "client interface restarted to load the new loader"
            ),
            // The client went away first: its next launch loads the loader.
            Err(CoreError::NotConnected) => {
                info!("the client closed before it settled; the loader starts with its next launch")
            }
            Err(error) => warn!(%error, "client interface not restarted"),
        }
    });
}

/// Links a loader into the client unless one already is: the configured folder's, or the one
/// winer ships, written to its data folder first. Returns whether it created the link.
fn activate(service: &Service, host: &Host, env: &Env, client_dir: &Path) -> Result<bool, Failure> {
    if let Some(active) = loader(service, env).filter(|loader| loader.active) {
        // Ours is kept current; a loader the user installed is theirs to update.
        if same_dir(&active.dir, &host.dir)
            && let Err(error) = plugin::place_loader(&host.dir, PENGU_CORE)
        {
            info!(%error, "the loader is in use; it is updated with the client closed");
        }
        return Ok(false);
    }
    let dir = match service.settings().plugin.loader_dir {
        Some(configured) if Path::new(&configured).join("core.dll").is_file() => {
            PathBuf::from(configured)
        }
        _ => {
            plugin::place_loader(&host.dir, PENGU_CORE).map_err(|error| Failure::other(&error))?;
            host.dir.clone()
        }
    };
    // The one write into the client's own folder, and so the one that can need an administrator.
    let linked = plugin::link(client_dir, &dir).map_err(|error| Failure {
        needs_elevation: plugin::needs_elevation(&error, elevation::is_elevated()),
        message: error.to_string(),
    })?;
    match linked {
        Linked::Created => {
            info!(loader = %dir.display(), client = %client_dir.display(), "loader linked");
            Ok(true)
        }
        Linked::Existing => Ok(false),
        Linked::Occupied => {
            warn!(client = %client_dir.display(), "version.dll belongs to something else");
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use sha2::{Digest as _, Sha256};
    use winer_core::{Service, bridge::Bridge, plugin};

    use super::{Env, Failure, Host, PENGU_CORE, disable, enable, program_name, refresh, status};

    /// Every file under `dir` with its bytes, to tell that nothing there changed.
    fn tree(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(next) = pending.pop() {
            for entry in fs::read_dir(&next).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push((path.clone(), fs::read(&path).unwrap()));
                }
            }
        }
        files.sort();
        files
    }

    /// A folder that looks like a loader's: its DLL and a plugins folder.
    fn loader_folder(dir: &Path) -> PathBuf {
        fs::create_dir_all(dir.join("plugins")).unwrap();
        fs::write(dir.join("core.dll"), b"core").unwrap();
        dir.to_path_buf()
    }

    /// A service whose settings name `loader_dir`, with the experimental IFEO switch as `ifeo`.
    fn service_with(root: &Path, loader_dir: &Path, ifeo: bool) -> (Service, Bridge) {
        let service = Service::new(
            root.join("settings.json"),
            tauri::async_runtime::handle().inner().clone(),
        );
        let mut settings = service.settings();
        settings.plugin.loader_dir = Some(loader_dir.display().to_string());
        settings.plugin.pengu_ifeo = ifeo;
        service.set_settings(settings).unwrap();
        let bridge =
            tauri::async_runtime::block_on(Bridge::start(service.clone(), "0.0.0")).unwrap();
        (service, bridge)
    }

    const PENGU: &str = "Pengu Loader.exe";

    #[test]
    fn the_program_a_debugger_value_starts_is_named_by_its_file() {
        assert_eq!(
            program_name(r#""C:\Program Files\Pengu Loader\Pengu Loader.exe" --boot"#).as_deref(),
            Some(PENGU)
        );
        assert_eq!(
            program_name(r"C:\Program Files\Pengu Loader\Pengu Loader.EXE --boot").as_deref(),
            Some("Pengu Loader.EXE")
        );
        assert_eq!(program_name("loader --x").as_deref(), Some("loader"));
        // What Pengu Loader 1.2.0-dev wrote on the QA host, its folder aside.
        assert_eq!(
            program_name(r#"rundll32 "C:\winer-dev\pengu\unpacked\core.dll", #6000"#).as_deref(),
            Some("rundll32 core.dll")
        );
        assert_eq!(
            program_name(r"C:\Windows\System32\rundll32.exe C:\tools\boot.dll,Start").as_deref(),
            Some("rundll32.exe boot.dll")
        );
        assert_eq!(program_name("rundll32").as_deref(), Some("rundll32"));
        assert_eq!(program_name("   "), None);
        assert_eq!(program_name(r#""""#), None);
    }

    /// Without another program launching the client, the configured folder is kept up as before.
    #[test]
    fn a_configured_loader_gets_the_plugin_when_nothing_else_launches_the_client() {
        let root = tempfile::tempdir().unwrap();
        let configured = loader_folder(&root.path().join("own"));
        let (service, bridge) = service_with(root.path(), &configured, false);
        let host = Host::with_env(root.path().join("pengu"), Env::default());
        let outcome = refresh(&service, &bridge, &host);
        assert_eq!(outcome.foreign, None);
        assert!(plugin::installed_version(&configured).is_some());
        let shown = status(&service, &bridge, &host);
        assert_eq!(
            shown.loader_dir.as_deref(),
            Some(configured.display().to_string().as_str())
        );
        assert_eq!(shown.foreign_activation, None);
    }

    /// With the experimental switch off, the default, another program launching the client is
    /// reported and changes nothing: the configured folder gets the plugin as before.
    #[test]
    fn another_launcher_is_only_reported_while_the_experimental_switch_is_off() {
        let root = tempfile::tempdir().unwrap();
        let configured = loader_folder(&root.path().join("own"));
        let (service, bridge) = service_with(root.path(), &configured, false);
        let host = Host::with_env(
            root.path().join("pengu"),
            Env {
                client_dir: None,
                foreign: Some(PENGU.into()),
            },
        );
        let outcome = refresh(&service, &bridge, &host);
        assert_eq!(outcome.foreign.as_deref(), Some(PENGU));
        assert!(!outcome.yielded);
        assert!(plugin::installed_version(&configured).is_some());
        let shown = status(&service, &bridge, &host);
        assert_eq!(
            shown.loader_dir.as_deref(),
            Some(configured.display().to_string().as_str())
        );
        assert_eq!(shown.foreign_activation.as_deref(), Some(PENGU));
    }

    /// Pengu Loader 1.2 launches the client through IFEO; with the experimental switch on, a
    /// configured folder that may be its own is never written, whichever way the setup is reached.
    #[test]
    fn under_another_launcher_the_configured_folder_is_left_alone_by_every_entry() {
        let root = tempfile::tempdir().unwrap();
        let configured = loader_folder(&root.path().join("pengu-1.2"));
        let before = tree(&configured);
        let (service, bridge) = service_with(root.path(), &configured, true);
        let env = Env {
            client_dir: None,
            foreign: Some(PENGU.into()),
        };
        let host = Host::with_env(root.path().join("pengu"), env);

        let outcome = refresh(&service, &bridge, &host);
        assert_eq!(outcome.foreign.as_deref(), Some(PENGU));
        assert!(outcome.yielded && !outcome.linked);
        assert_eq!(tree(&configured), before, "refresh wrote nothing there");

        let (shown, outcome) = enable(&service, &bridge, &host).unwrap();
        assert!(!outcome.linked);
        assert_eq!(tree(&configured), before, "nor did turning the features on");
        assert_eq!(
            shown.loader_dir, None,
            "the configured folder is not the loader"
        );
        assert_eq!(shown.foreign_activation.as_deref(), Some(PENGU));
        assert!(
            !host.dir.exists(),
            "winer's own loader was not placed either"
        );

        let shown = disable(&service, &bridge, &host).unwrap();
        assert_eq!(tree(&configured), before, "nor did turning them off");
        assert_eq!(shown.foreign_activation.as_deref(), Some(PENGU));
    }

    /// A loader the client linked before Pengu Loader 1.2 came is still kept up, and it is the one
    /// the status reports; no new link is made, and the configured folder stays as it was.
    #[cfg(unix)]
    #[test]
    fn under_another_launcher_only_the_linked_loader_is_kept_up() {
        let root = tempfile::tempdir().unwrap();
        let configured = loader_folder(&root.path().join("pengu-1.2"));
        let before = tree(&configured);
        let old = loader_folder(&root.path().join("old"));
        let client = root.path().join("LeagueClient");
        fs::create_dir_all(&client).unwrap();
        std::os::unix::fs::symlink(old.join("core.dll"), client.join("version.dll")).unwrap();
        let (service, bridge) = service_with(root.path(), &configured, true);
        let host = Host::with_env(
            root.path().join("pengu"),
            Env {
                client_dir: Some(client.clone()),
                foreign: Some(PENGU.into()),
            },
        );

        let outcome = refresh(&service, &bridge, &host);
        assert!(!outcome.linked);
        assert!(
            plugin::installed_version(&old).is_some(),
            "the linked loader is kept up"
        );
        assert_eq!(tree(&configured), before);
        assert_eq!(
            fs::read_link(client.join("version.dll")).unwrap(),
            old.join("core.dll"),
            "the link is left as it was"
        );
        let shown = status(&service, &bridge, &host);
        assert_eq!(
            shown.loader_dir.as_deref(),
            Some(old.display().to_string().as_str())
        );
        assert!(shown.active && !shown.managed);
        assert_eq!(shown.foreign_activation.as_deref(), Some(PENGU));
    }

    /// A refused link reaches the window as a request for administrator rights, beside the
    /// system's own words for it, until turning the features off clears it.
    #[test]
    fn a_link_refused_for_want_of_rights_tells_the_window_so() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(
            dir.path().join("settings.json"),
            tauri::async_runtime::handle().inner().clone(),
        );
        let bridge =
            tauri::async_runtime::block_on(Bridge::start(service.clone(), "0.0.0")).unwrap();
        let host = Host::new(dir.path().join("pengu"));
        let refused = "A required privilege is not held by the client. (os error 1314)";
        host.set_failure(Some(Failure {
            message: refused.into(),
            needs_elevation: true,
        }));
        let shown = status(&service, &bridge, &host);
        assert!(shown.needs_elevation);
        assert_eq!(shown.setup_error.as_deref(), Some(refused));

        host.set_failure(Some(Failure::other(&std::io::Error::from_raw_os_error(32))));
        let shown = status(&service, &bridge, &host);
        assert!(!shown.needs_elevation, "a file in use is not about rights");
        assert!(shown.setup_error.is_some());

        let shown = disable(&service, &bridge, &host).unwrap();
        assert!(!shown.needs_elevation && shown.setup_error.is_none());
    }

    /// The DLL the client loads is the release `vendor/pengu-loader/README.md` names, byte for byte.
    #[test]
    fn core_dll_is_the_vendored_release() {
        let hex: String = Sha256::digest(PENGU_CORE)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            hex,
            "d56fcf8f182dd5d392214eb989869c53e6cb7e0bc2a2d111ea0a3b9d3c0a506f"
        );
    }
}
