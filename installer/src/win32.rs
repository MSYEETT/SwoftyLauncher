use std::ffi::c_void;

const TH32CS_SNAPPROCESS: u32 = 0x2;
const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;
const MAX_PATH: usize = 260;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

#[repr(C)]
struct ProcessEntry32W {
    size: u32,
    usage: u32,
    process_id: u32,
    default_heap_id: usize,
    module_id: u32,
    threads: u32,
    parent_process_id: u32,
    priority: i32,
    flags: u32,
    exe_file: [u16; MAX_PATH],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
    fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn QueryFullProcessImageNameW(process: *mut c_void, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

pub fn processes() -> Vec<(u32, String)> {
    // SAFETY: the snapshot handle is checked before use and closed once;
    // the entry is zeroed with its size set, as Process32FirstW requires,
    // and exe_file is read only up to its NUL inside the array.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
            return Vec::new();
        }
        let mut entry: ProcessEntry32W = std::mem::zeroed();
        entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
        let mut out = Vec::new();
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let len = entry.exe_file.iter().position(|&c| c == 0).unwrap_or(MAX_PATH);
            out.push((entry.process_id, String::from_utf16_lossy(&entry.exe_file[..len])));
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        out
    }
}

pub fn image(pid: u32) -> Option<String> {
    let mut name = vec![0u16; 32_768];
    let mut len = name.len() as u32;
    // SAFETY: the handle is checked before use and closed once, and `len`
    // tells the call how many u16s `name` holds.
    let ok = unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let ok = QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut len);
        CloseHandle(process);
        ok
    };
    (ok != 0).then(|| String::from_utf16_lossy(&name[..len as usize]))
}

/// The full image paths of the running processes whose exe file is `name`.
pub fn images(name: &str) -> Vec<String> {
    processes()
        .into_iter()
        .filter(|(_, file)| file.eq_ignore_ascii_case(name))
        .filter_map(|(pid, _)| image(pid))
        .collect()
}
