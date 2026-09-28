//! Small Win32 boundary. No hooks, input logging, services or network access.
#[cfg(windows)]
mod native {
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use tauri::{AppHandle, WebviewWindow};
    type Handle = *mut c_void;
    type SubclassProc = unsafe extern "system" fn(Handle, u32, usize, isize, usize, usize) -> isize;
    #[link(name = "kernel32")]
    extern "system" {
        fn QueryUnbiasedInterruptTime(time: *mut u64) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn OpenInputDesktop(flags: u32, inherit: i32, access: u32) -> Handle;
        fn CloseDesktop(desktop: Handle) -> i32;
        fn GetUserObjectInformationW(
            object: Handle,
            index: i32,
            info: *mut c_void,
            len: u32,
            needed: *mut u32,
        ) -> i32;
        fn MessageBeep(kind: u32) -> i32;
    }
    #[link(name = "wtsapi32")]
    extern "system" {
        fn WTSRegisterSessionNotification(hwnd: Handle, flags: u32) -> i32;
        fn WTSUnRegisterSessionNotification(hwnd: Handle) -> i32;
        fn WTSQuerySessionInformationW(
            server: Handle,
            session: u32,
            class: u32,
            buffer: *mut *mut u16,
            bytes: *mut u32,
        ) -> i32;
        fn WTSFreeMemory(memory: Handle);
    }
    #[link(name = "comctl32")]
    extern "system" {
        fn SetWindowSubclass(
            hwnd: Handle,
            callback: Option<SubclassProc>,
            id: usize,
            data: usize,
        ) -> i32;
        fn RemoveWindowSubclass(hwnd: Handle, callback: Option<SubclassProc>, id: usize) -> i32;
        fn DefSubclassProc(hwnd: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
    }
    static APP: OnceLock<AppHandle> = OnceLock::new();

    pub fn now_ms() -> u64 {
        let mut time = 0;
        // Available since Windows 7. An API failure must not invent elapsed work.
        unsafe {
            QueryUnbiasedInterruptTime(&mut time);
        }
        time / 10_000
    }

    pub fn interactive() -> bool {
        unsafe {
            let mut buffer = std::ptr::null_mut();
            let mut bytes = 0;
            // WTS_CURRENT_SESSION / WTSConnectState. Query is local to this session.
            if WTSQuerySessionInformationW(
                std::ptr::null_mut(),
                u32::MAX,
                8,
                &mut buffer,
                &mut bytes,
            ) == 0
            {
                return false;
            }
            let active = !buffer.is_null() && bytes >= 4 && *(buffer as *const u32) == 0;
            WTSFreeMemory(buffer.cast());
            if !active {
                return false;
            }
            let desktop = OpenInputDesktop(0, 0, 0x0001); // DESKTOP_READOBJECTS
            if desktop.is_null() {
                return false;
            }
            let mut name = [0u16; 64];
            let mut needed = 0;
            let ok =
                GetUserObjectInformationW(desktop, 2, name.as_mut_ptr().cast(), 128, &mut needed)
                    != 0;
            CloseDesktop(desktop);
            let end = name.iter().position(|&ch| ch == 0).unwrap_or(name.len());
            ok && String::from_utf16_lossy(&name[..end]).eq_ignore_ascii_case("default")
        }
    }

    unsafe extern "system" fn session_event(
        hwnd: Handle,
        message: u32,
        wparam: usize,
        lparam: isize,
        _id: usize,
        _data: usize,
    ) -> isize {
        let blocked = match (message, wparam) {
            (0x02B1, 2 | 4 | 6 | 7) => Some(true), // disconnect, logoff, lock
            (0x02B1, 1 | 3 | 5 | 8) => Some(!interactive()),
            (0x0218, 4) => Some(true),                // PBT_APMSUSPEND
            (0x0218, 7 | 18) => Some(!interactive()), // resume
            _ => None,
        };
        if let (Some(app), Some(blocked)) = (APP.get(), blocked) {
            // Only touch the pure timer here; window operations happen on the next UI tick.
            super::super::system_blocked(app, blocked);
        }
        if message == 0x0082 {
            // WM_NCDESTROY
            WTSUnRegisterSessionNotification(hwnd);
            RemoveWindowSubclass(hwnd, Some(session_event), 1);
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }

    pub fn install(app: &AppHandle, window: &WebviewWindow) -> Result<(), String> {
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0;
        APP.set(app.clone())
            .map_err(|_| "セッション監視の二重初期化")?;
        unsafe {
            if SetWindowSubclass(hwnd, Some(session_event), 1, 0) == 0 {
                return Err("Windowsセッション監視を開始できませんでした。".into());
            }
            // Polling remains as a fallback if Terminal Services is still starting.
            WTSRegisterSessionNotification(hwnd, 0);
        }
        Ok(())
    }
    pub fn chime() {
        unsafe {
            MessageBeep(0x00000040);
        }
    }
}
#[cfg(windows)]
pub use native::*;

#[cfg(not(windows))]
pub fn now_ms() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}
#[cfg(not(windows))]
pub fn interactive() -> bool {
    true
}
#[cfg(not(windows))]
pub fn install(_: &tauri::AppHandle, _: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}
#[cfg(not(windows))]
pub fn chime() {}
