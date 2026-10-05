//! What discovery needs from a process winer does not own: its image path and its command line,
//! both through `PROCESS_QUERY_LIMITED_INFORMATION`, the one right an elevated client still grants.
#![allow(unsafe_code)]

use std::{ffi::OsString, io, mem, os::windows::ffi::OsStringExt, path::PathBuf, ptr, slice};

use windows_sys::{
    Wdk::System::Threading::{NtQueryInformationProcess, ProcessCommandLineInformation},
    Win32::{
        Foundation::{
            CloseHandle, HANDLE, INVALID_HANDLE_VALUE, NTSTATUS, STATUS_ACCESS_DENIED,
            UNICODE_STRING,
        },
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
    },
};

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful Win32 call and is closed exactly once.
        unsafe { CloseHandle(self.0) };
    }
}

/// Pids of every process whose executable file name is `name`, compared case-insensitively.
pub(crate) fn pids_named(name: &str) -> io::Result<Vec<u32>> {
    // SAFETY: no pointers are passed; the result is checked before use.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let snapshot = Handle(snapshot);
    // SAFETY: PROCESSENTRY32W is plain data, for which all-zero is a valid value.
    let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
    entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;

    let mut pids = Vec::new();
    // SAFETY: `entry` is a writable PROCESSENTRY32W whose `dwSize` is set.
    let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
    while more {
        let len = entry
            .szExeFile
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(entry.szExeFile.len());
        if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case(name) {
            pids.push(entry.th32ProcessID);
        }
        // SAFETY: as above.
        more = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
    }
    Ok(pids)
}

pub(crate) struct Process(Handle);

impl Process {
    pub(crate) fn open(pid: u32) -> io::Result<Self> {
        // SAFETY: no pointers are passed; a null handle is checked below.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(Handle(handle)))
        }
    }

    pub(crate) fn image_path(&self) -> Option<PathBuf> {
        let mut buffer = vec![0u16; 32_768];
        let mut len = buffer.len() as u32;
        // SAFETY: `buffer` holds `len` writable UTF-16 units, and `len` is updated in place.
        let ok = unsafe {
            QueryFullProcessImageNameW(self.0.0, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut len)
        };
        (ok != 0).then(|| PathBuf::from(OsString::from_wide(&buffer[..len as usize])))
    }

    /// The command line through `ProcessCommandLineInformation`. Reading it out of the PEB, as
    /// WMI does, needs `PROCESS_VM_READ`, which an elevated process denies to a normal one.
    pub(crate) fn command_line(&self) -> io::Result<String> {
        let mut needed = 0u32;
        // SAFETY: a zero-length query only reports the size it needs through `needed`.
        let status = unsafe {
            NtQueryInformationProcess(
                self.0.0,
                ProcessCommandLineInformation,
                ptr::null_mut(),
                0,
                &mut needed,
            )
        };
        if needed == 0 {
            return Err(nt_error(status));
        }
        // `u64` storage keeps the UNICODE_STRING header at the alignment it needs.
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
        // SAFETY: `buffer` is at least `needed` bytes long and suitably aligned.
        let status = unsafe {
            NtQueryInformationProcess(
                self.0.0,
                ProcessCommandLineInformation,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        };
        if status < 0 {
            return Err(nt_error(status));
        }
        // SAFETY: on success the buffer starts with a UNICODE_STRING whose `Buffer` points into the
        // same allocation and holds `Length` bytes.
        let header = unsafe { &*buffer.as_ptr().cast::<UNICODE_STRING>() };
        if header.Length == 0 || header.Buffer.is_null() {
            return Ok(String::new());
        }
        // SAFETY: see above; the buffer outlives this borrow.
        let units = unsafe { slice::from_raw_parts(header.Buffer, usize::from(header.Length) / 2) };
        Ok(String::from_utf16_lossy(units))
    }
}

fn nt_error(status: NTSTATUS) -> io::Error {
    if status == STATUS_ACCESS_DENIED {
        io::Error::from(io::ErrorKind::PermissionDenied)
    } else {
        io::Error::other(format!("NtQueryInformationProcess returned {status:#010x}"))
    }
}
