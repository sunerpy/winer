//! Running as administrator. The Tencent client is launched elevated, and an elevated process
//! does not give its command line (where the LCU credentials are) to a normal one.

/// Passed to the elevated copy: the pid it replaces, so it starts only once that one is gone.
const REPLACE: &str = "--replace=";

#[cfg(windows)]
mod imp {
    #![allow(unsafe_code)]

    use std::{ffi::OsStr, io, mem, os::windows::ffi::OsStrExt as _, ptr};

    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
        System::Threading::{
            GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_SYNCHRONIZE,
            WaitForSingleObject,
        },
        UI::{
            Shell::{SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    };

    pub(super) fn is_elevated() -> bool {
        let mut token: HANDLE = ptr::null_mut();
        // SAFETY: the current-process pseudo handle is always valid; `token` is closed below.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return false;
        }
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
        // SAFETY: the token handle was opened above.
        unsafe { CloseHandle(token) };
        ok != 0 && elevation.TokenIsElevated != 0
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
    pub(super) fn is_elevated() -> bool {
        false
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

/// Starts an elevated copy; the caller exits once this returns `Ok`.
pub(crate) fn relaunch_elevated() -> std::io::Result<()> {
    imp::launch_elevated(&format!("{REPLACE}{}", std::process::id()))
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
