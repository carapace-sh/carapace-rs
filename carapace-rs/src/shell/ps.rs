//! Shell determination by parent process name (port of Go `pkg/ps`).

/// The executable names of the ancestor chain, starting at the parent
/// of `start` (Go `ps.DetermineShell`'s walk; callers match the first
/// known shell).
pub(crate) fn parents(start: u32) -> impl Iterator<Item = String> {
    #[cfg(windows)]
    return windows_parents(start);
    #[cfg(not(windows))]
    unix_parents(start)
}

#[cfg(unix)]
fn unix_parents(start: u32) -> impl Iterator<Item = String> {
    UnixWalk {
        pid: crate::shell::process_parent(start),
    }
}
#[cfg(unix)]
struct UnixWalk {
    pid: Option<u32>,
}

#[cfg(unix)]
impl Iterator for UnixWalk {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        let pid = self.pid?;
        self.pid = crate::shell::process_parent(pid);
        let mut name = crate::shell::process_name(pid);
        if matches!(name.as_str(), "linker" | "linker64") {
            // Termux rewrites execve through the system linker; the
            // real binary is the first cmdline entry
            name = std::fs::read_to_string(format!("/proc/{pid}/cmdline"))
                .ok()
                .and_then(|data| {
                    data.split('\0')
                        .next()
                        .map(|path| path.rsplit('/').next().unwrap_or(path).to_owned())
                })
                .unwrap_or(name);
        }
        Some(name)
    }
}

#[cfg(windows)]
fn windows_parents(start: u32) -> impl Iterator<Item = String> {
    let entries = snapshot().unwrap_or_default();
    let first = entries.get(&start).map(|(parent, _)| *parent);
    WindowsWalk {
        pid: first,
        entries,
    }
}

#[cfg(windows)]
struct WindowsWalk {
    pid: Option<u32>,
    entries: std::collections::HashMap<u32, (u32, String)>,
}

#[cfg(windows)]
impl Iterator for WindowsWalk {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        let (parent, name) = self.entries.remove(&self.pid?)?;
        self.pid = Some(parent);
        Some(name)
    }
}

/// One process snapshot covering the whole chain (the `go-ps`
/// `CreateToolhelp32Snapshot` equivalent); `None` when the API fails.
#[cfg(windows)]
fn snapshot() -> Option<std::collections::HashMap<u32, (u32, String)>> {
    use std::collections::HashMap;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if handle == INVALID_HANDLE_VALUE {
        return None;
    }
    let mut entries = HashMap::new();
    let result = unsafe {
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(handle, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&unit| unit == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                entries.insert(entry.th32ProcessID, (entry.th32ParentProcessID, name));
                if Process32NextW(handle, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(handle)
    };
    if result == 0 && entries.is_empty() {
        return None;
    }
    Some(entries)
}
