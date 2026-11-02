//! Running as administrator. The Tencent client is launched elevated, and an elevated process
//! does not give its command line (where the LCU credentials are) to a normal one. Nor does it take
//! a normal one's keys: the system drops input synthesized into a window of a process that
//! outranks the sender (UIPI), without an error to say so, and a game the client starts runs with
//! the client's rights.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Runtime};
use tracing::{info, warn};

/// Passed to the elevated copy: the pid it replaces, so it starts only once that one is gone.
const REPLACE: &str = "--replace=";

/// Whether this process has asked for elevation on its own already; a refusal is not repeated.
static ASKED: AtomicBool = AtomicBool::new(false);

/// What winer can read of another process's rights (`token_of`).
// Off Windows nothing is read, and only `Unreadable` is ever made.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenRead {
    /// It runs elevated, as administrator.
    Elevated,
    /// It runs without elevation.
    NotElevated,
    /// The process opened but its token refused winer, as an elevated process's token refuses a
    /// process without elevation.
    Denied,
    /// The process did not open (it is gone, or protected), or its token did not say: its rights
    /// are unknown.
    Unreadable,
}

#[cfg(windows)]
mod imp {
    #![allow(unsafe_code)]

    use std::{ffi::OsStr, io, mem, os::windows::ffi::OsStrExt as _, ptr};

    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_ACCESS_DENIED, GetLastError, HANDLE},
        Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
        System::Threading::{
            GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE, WaitForSingleObject,
        },
        UI::{
            Shell::{SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    };

    use super::TokenRead;

    /// Whether `token`, opened for `TOKEN_QUERY`, is elevated; `None` when it does not say.
    fn elevation_of(token: HANDLE) -> Option<bool> {
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size = 0u32;
        // SAFETY: `elevation` is a TOKEN_ELEVATION and its size is passed alongside it.
        let ok = unsafe {
            GetTokenInformation(
                token,
                TokenElevation,
                (&raw mut elevation).cast(),
                mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            )
        };
        (ok != 0).then_some(elevation.TokenIsElevated != 0)
    }

    pub(super) fn is_elevated() -> bool {
        let mut token: HANDLE = ptr::null_mut();
        // SAFETY: the current-process pseudo handle is always valid; `token` is closed below.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return false;
        }
        let elevated = elevation_of(token);
        // SAFETY: the token handle was opened above.
        unsafe { CloseHandle(token) };
        elevated == Some(true)
    }

    /// The process is opened with the least right there is, which a process without elevation
    /// still gets on an elevated one (the client's command line is read with it); the token is
    /// where the two part.
    pub(super) fn token_of(pid: u32) -> TokenRead {
        // SAFETY: a process that does not open yields null, checked here; it is closed below.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return TokenRead::Unreadable;
        }
        let mut token: HANDLE = ptr::null_mut();
        // SAFETY: `process` is open; the error is read before another call can replace it.
        let (opened, refused) = unsafe {
            let opened = OpenProcessToken(process, TOKEN_QUERY, &mut token) != 0;
            (opened, !opened && GetLastError() == ERROR_ACCESS_DENIED)
        };
        // SAFETY: the process handle was opened above.
        unsafe { CloseHandle(process) };
        if !opened {
            return if refused {
                TokenRead::Denied
            } else {
                TokenRead::Unreadable
            };
        }
        let elevated = elevation_of(token);
        // SAFETY: the token handle was opened above.
        unsafe { CloseHandle(token) };
        match elevated {
            Some(true) => TokenRead::Elevated,
            Some(false) => TokenRead::NotElevated,
            None => TokenRead::Unreadable,
        }
    }

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain([0]).collect()
    }

    /// Asks for consent and starts an elevated copy of this executable. Returns once the user has
    /// answered; an error includes the user declining.
    pub(super) fn launch_elevated(parameters: &str) -> io::Result<()> {
        let exe = std::env::current_exe()?;
        let (verb, file, parameters) = (
            wide(OsStr::new("runas")),
            wide(exe.as_os_str()),
            wide(OsStr::new(parameters)),
        );
        // SAFETY: SHELLEXECUTEINFOW is plain data, for which all-zero is a valid value.
        let mut info: SHELLEXECUTEINFOW = unsafe { mem::zeroed() };
        info.cbSize = mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = parameters.as_ptr();
        info.nShow = SW_SHOWNORMAL;
        // SAFETY: every string outlives the call and is NUL-terminated.
        if unsafe { ShellExecuteExW(&mut info) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub(super) fn wait_for_exit(pid: u32) {
        // SAFETY: plain Win32 calls on a handle closed below; a missing process yields null.
        unsafe {
            let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if !process.is_null() {
                WaitForSingleObject(process, 10_000);
                CloseHandle(process);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::TokenRead;

    pub(super) fn is_elevated() -> bool {
        false
    }

    pub(super) fn token_of(_pid: u32) -> TokenRead {
        TokenRead::Unreadable
    }

    pub(super) fn launch_elevated(_parameters: &str) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "elevation is a Windows feature",
        ))
    }

    pub(super) fn wait_for_exit(_pid: u32) {}
}

pub(crate) fn is_elevated() -> bool {
    imp::is_elevated()
}

/// The rights of process `pid`, as far as winer can read them.
pub(crate) fn token_of(pid: u32) -> TokenRead {
    imp::token_of(pid)
}

/// Whether a process whose rights read `target` outranks winer, elevated or not
/// (`winer_elevated`): the system drops every key winer presses into its windows. Only a winer
/// without elevation is outranked, by an elevated process or one whose token it may not read. One
/// whose rights are unknown is not known to outrank it, so typing into it is still tried.
pub(crate) fn outranks(winer_elevated: bool, target: TokenRead) -> bool {
    !winer_elevated && matches!(target, TokenRead::Elevated | TokenRead::Denied)
}

/// Starts an elevated copy; the caller exits once this returns `Ok`.
pub(crate) fn relaunch_elevated() -> std::io::Result<()> {
    imp::launch_elevated(&format!("{REPLACE}{}", std::process::id()))
}

/// winer turned out to need administrator rights it runs without (`reason`, for the log): the
/// client runs elevated, only an administrator can link the loader into it, or a game the client
/// starts would drop every key winer types. Ask Windows, once per process, to restart winer
/// elevated, and quit when the user agrees. Only the system's consent prompt shows. A refusal is
/// final for this run; the window keeps its own button for it.
pub(crate) fn ask_once<R: Runtime>(app: &AppHandle<R>, reason: &'static str) {
    if is_elevated() || ASKED.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || match relaunch_elevated() {
        Ok(()) => {
            info!(reason, "restarting elevated");
            app.exit(0);
        }
        Err(error) => warn!(%error, reason, "not restarted elevated"),
    });
}

/// Whether this run has asked for elevation already: it is not asked again, whatever the reason.
pub(crate) fn asked() -> bool {
    ASKED.load(Ordering::Acquire)
}

/// The elevated copy waits for the instance it replaces to exit, or the single-instance guard
/// would hand it straight back to the process that is quitting.
pub(crate) fn wait_for_replaced_instance() {
    let pid =
        std::env::args().find_map(|arg| arg.strip_prefix(REPLACE).and_then(|pid| pid.parse().ok()));
    if let Some(pid) = pid {
        imp::wait_for_exit(pid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_winer_without_elevation_is_outranked_and_only_by_rights_it_cannot_match() {
        use TokenRead::{Denied, Elevated, NotElevated, Unreadable};
        // winer elevated, the other process's rights, whether that process outranks winer.
        for (winer_elevated, target, outranked) in [
            (false, Elevated, true),
            (false, Denied, true),
            (false, NotElevated, false),
            (false, Unreadable, false),
            (true, Elevated, false),
            (true, Denied, false),
            (true, NotElevated, false),
            (true, Unreadable, false),
        ] {
            assert_eq!(
                outranks(winer_elevated, target),
                outranked,
                "winer elevated: {winer_elevated}, the other: {target:?}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn winer_reads_its_own_rights_and_none_of_a_process_that_does_not_open() {
        let own = if is_elevated() {
            TokenRead::Elevated
        } else {
            TokenRead::NotElevated
        };
        assert_eq!(token_of(std::process::id()), own);
        // The System Idle Process never opens.
        assert_eq!(token_of(0), TokenRead::Unreadable);
    }

    #[cfg(not(windows))]
    #[test]
    fn no_process_is_read_off_windows() {
        assert_eq!(token_of(std::process::id()), TokenRead::Unreadable);
        assert!(!outranks(is_elevated(), token_of(1)));
    }
}
