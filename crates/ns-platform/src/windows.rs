//! Win32 backend.
//!
//! Foreground window via `GetForegroundWindow`, idle via `GetLastInputInfo`,
//! session changes via `WTSRegisterSessionNotification` and
//! `RegisterSuspendResumeNotification` on a message-only window that runs on
//! its own thread.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ns_core::{ProcessInfo, SessionEvent, SessionState, Timestamp, WindowInfo};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_SUCCESS, FILETIME, HANDLE, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::Storage::Packaging::Appx::GetPackageFamilyName;
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::RegisterSuspendResumeNotification;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW,
    RegSetKeyValueW,
};
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
};
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Shell::{
    QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DEVICE_NOTIFY_WINDOW_HANDLE, DefWindowProcW, DispatchMessageW,
    EnumChildWindows, GetDesktopWindow, GetForegroundWindow, GetMessageW, GetShellWindow,
    GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, HWND_MESSAGE,
    MB_ICONEXCLAMATION, MSG, PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND, RegisterClassW,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_POWERBROADCAST, WM_WTSSESSION_CHANGE,
    WNDCLASSW, WTS_SESSION_LOCK, WTS_SESSION_LOGOFF, WTS_SESSION_LOGON, WTS_SESSION_UNLOCK,
};
use windows::core::{BOOL, PCWSTR, PWSTR, w};

use crate::{Caps, Platform, PlatformError, Result, SessionSink, SteamInfo};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: PCWSTR = w!("nagsworth");
const STEAM_KEY: PCWSTR = w!("Software\\Valve\\Steam");

static SESSION_STATE: AtomicU8 = AtomicU8::new(0);
static SESSION_SINK: OnceLock<SessionSink> = OnceLock::new();

pub struct WindowsPlatform {
    /// Process info keyed by pid, validated by process creation time so a
    /// reused pid does not return a stale entry.
    cache: Mutex<HashMap<u32, (u64, ProcessInfo)>>,
}

impl Default for WindowsPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsPlatform {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn window_info(&self, hwnd: HWND) -> Option<WindowInfo> {
        let mut pid = 0u32;
        // SAFETY: hwnd may be stale; the call then returns 0 and leaves pid 0.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == 0 {
            return None;
        }
        let mut process = self
            .process_info(pid)
            .unwrap_or_else(|_| ProcessInfo::unknown(pid));
        // UWP apps are hosted by ApplicationFrameHost; the real app owns a child window.
        if process
            .exe_name
            .eq_ignore_ascii_case("ApplicationFrameHost.exe")
            && let Some(child_pid) = uwp_child_pid(hwnd, pid)
            && let Ok(real) = self.process_info(child_pid)
        {
            process = real;
        }
        let (is_fullscreen, monitor) = fullscreen_and_monitor(hwnd);
        Some(WindowInfo {
            hwnd: hwnd.0 as usize as u64,
            title: window_title(hwnd),
            process,
            is_fullscreen,
            monitor,
        })
    }
}

impl Platform for WindowsPlatform {
    fn foreground_window(&self) -> Result<Option<WindowInfo>> {
        // SAFETY: no preconditions.
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_invalid() {
            return Ok(None);
        }
        Ok(self.window_info(hwnd))
    }

    fn idle_time(&self) -> Result<Duration> {
        let mut info = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        // SAFETY: info is a valid, sized LASTINPUTINFO.
        if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
            return Err(os_error("GetLastInputInfo"));
        }
        // Both are 32-bit tick counts; wrapping_sub handles the 49.7-day wrap.
        let now = unsafe { GetTickCount() };
        Ok(Duration::from_millis(now.wrapping_sub(info.dwTime) as u64))
    }

    fn process_info(&self, pid: u32) -> Result<ProcessInfo> {
        // SAFETY: the handle is closed by HandleGuard.
        let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
            Ok(h) => HandleGuard(h),
            Err(_) => {
                // Protected and some elevated processes refuse even limited access.
                return Ok(ProcessInfo {
                    exe_name: "<elevated>".into(),
                    is_elevated: true,
                    ..ProcessInfo::unknown(pid)
                });
            }
        };
        let created = creation_time(handle.0);
        if let Some((t, info)) = self.cache.lock().unwrap().get(&pid)
            && *t == created
        {
            return Ok(info.clone());
        }
        let exe_path = image_path(handle.0);
        let exe_name = exe_path
            .as_deref()
            .and_then(|p| p.rsplit(['\\', '/']).next())
            .map(str::to_string)
            .unwrap_or_else(|| "<unknown>".into());
        let info = ProcessInfo {
            pid,
            exe_name,
            exe_path,
            is_elevated: is_elevated(handle.0),
            package_id: package_family(handle.0),
        };
        let mut cache = self.cache.lock().unwrap();
        if cache.len() > 512 {
            cache.clear();
        }
        cache.insert(pid, (created, info.clone()));
        Ok(info)
    }

    fn session_events(&self, sink: SessionSink) -> Result<()> {
        if SESSION_SINK.set(sink).is_err() {
            return Err(PlatformError::Os("session events already started".into()));
        }
        std::thread::Builder::new()
            .name("an-session-events".into())
            .spawn(run_message_window)
            .map_err(|e| PlatformError::Os(e.to_string()))?;
        Ok(())
    }

    fn session_state(&self) -> SessionState {
        match SESSION_STATE.load(Ordering::SeqCst) {
            1 => SessionState::Locked,
            2 => SessionState::Suspended,
            _ => SessionState::Active,
        }
    }

    fn autostart(&self) -> Result<bool> {
        Ok(read_reg_string(RUN_KEY, RUN_VALUE).is_some())
    }

    fn set_autostart(&self, enabled: bool) -> Result<()> {
        // SAFETY: all strings are valid null-terminated wide strings.
        let status = if enabled {
            let exe = std::env::current_exe().map_err(|e| PlatformError::Os(e.to_string()))?;
            let value: Vec<u16> = format!("\"{}\" --autostart", exe.display())
                .encode_utf16()
                .chain([0])
                .collect();
            unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    RUN_KEY,
                    RUN_VALUE,
                    REG_SZ.0,
                    Some(value.as_ptr().cast()),
                    (value.len() * 2) as u32,
                )
            }
        } else {
            unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE) }
        };
        if status == ERROR_SUCCESS || (!enabled && status.0 == 2) {
            Ok(())
        } else {
            Err(PlatformError::Os(format!("registry error {}", status.0)))
        }
    }

    fn lock_workstation(&self) -> Result<()> {
        // SAFETY: no preconditions.
        unsafe { LockWorkStation() }.map_err(|e| PlatformError::Os(e.to_string()))
    }

    fn steam_info(&self) -> Option<SteamInfo> {
        let running_app_id = read_reg_dword(STEAM_KEY, w!("RunningAppID"))?;
        Some(SteamInfo {
            running_app_id,
            steam_path: read_reg_string(STEAM_KEY, w!("SteamPath")),
        })
    }

    fn exclusive_fullscreen(&self) -> bool {
        // SAFETY: plain query.
        matches!(
            unsafe { SHQueryUserNotificationState() },
            Ok(s) if s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE
        )
    }

    fn alert(&self) {
        // SAFETY: plain call; failure only means no sound.
        let _ = unsafe { MessageBeep(MB_ICONEXCLAMATION) };
    }

    fn caps(&self) -> Caps {
        Caps {
            session_events: true,
            lock_workstation: true,
            autostart: true,
            steam: true,
        }
    }
}

struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenProcess/OpenProcessToken and is owned.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn os_error(what: &str) -> PlatformError {
    PlatformError::Os(format!("{what}: {}", windows::core::Error::from_thread()))
}

fn window_title(hwnd: HWND) -> String {
    // SAFETY: the buffer is sized from GetWindowTextLengthW plus the terminator.
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}

fn image_path(handle: HANDLE) -> Option<String> {
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: len holds the buffer capacity in characters.
    unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    }
    .ok()?;
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

fn creation_time(handle: HANDLE) -> u64 {
    let mut times = [FILETIME::default(); 4];
    let [c, e, k, u] = &mut times;
    // SAFETY: four valid FILETIME out pointers.
    if unsafe { GetProcessTimes(handle, c, e, k, u) }.is_err() {
        return 0;
    }
    ((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64
}

fn is_elevated(handle: HANDLE) -> bool {
    let mut token = HANDLE::default();
    // SAFETY: token is closed by HandleGuard; the out buffer is a TOKEN_ELEVATION.
    unsafe {
        if OpenProcessToken(handle, TOKEN_QUERY, &mut token).is_err() {
            // Medium-integrity callers cannot open elevated processes' tokens.
            return true;
        }
        let token = HandleGuard(token);
        let mut elevation = TOKEN_ELEVATION::default();
        let mut ret = 0u32;
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret,
        )
        .is_ok()
            && elevation.TokenIsElevated != 0
    }
}

fn package_family(handle: HANDLE) -> Option<String> {
    let mut len = 0u32;
    // SAFETY: first call queries the length, second fills a buffer of that length.
    unsafe {
        let _ = GetPackageFamilyName(handle, &mut len, None);
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        if GetPackageFamilyName(handle, &mut len, Some(PWSTR(buf.as_mut_ptr()))) != ERROR_SUCCESS {
            return None;
        }
        Some(String::from_utf16_lossy(
            &buf[..len.saturating_sub(1) as usize],
        ))
    }
}

fn uwp_child_pid(hwnd: HWND, host_pid: u32) -> Option<u32> {
    struct Search {
        host: u32,
        found: u32,
    }
    unsafe extern "system" fn visit(child: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam points at the Search below for the duration of the enumeration.
        let search = unsafe { &mut *(lparam.0 as *mut Search) };
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(child, Some(&mut pid)) };
        if pid != 0 && pid != search.host {
            search.found = pid;
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut search = Search {
        host: host_pid,
        found: 0,
    };
    // SAFETY: the callback only touches `search`, which outlives the call.
    unsafe {
        let _ = EnumChildWindows(
            Some(hwnd),
            Some(visit),
            LPARAM(&mut search as *mut Search as isize),
        );
    }
    (search.found != 0).then_some(search.found)
}

fn fullscreen_and_monitor(hwnd: HWND) -> (bool, u64) {
    // SAFETY: plain queries on a window handle; failures leave zeroed rects.
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let mut rect = RECT::default();
        let is_shell = hwnd == GetShellWindow() || hwnd == GetDesktopWindow();
        let full = !is_shell
            && GetMonitorInfoW(monitor, &mut mi).as_bool()
            && GetWindowRect(hwnd, &mut rect).is_ok()
            && rect.left <= mi.rcMonitor.left
            && rect.top <= mi.rcMonitor.top
            && rect.right >= mi.rcMonitor.right
            && rect.bottom >= mi.rcMonitor.bottom;
        (full, monitor.0 as usize as u64)
    }
}

fn read_reg_dword(key: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: data is a u32 and size says so.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    (status == ERROR_SUCCESS).then_some(data)
}

fn read_reg_string(key: PCWSTR, value: PCWSTR) -> Option<String> {
    let mut size = 0u32;
    // SAFETY: first call queries the size in bytes, second fills a buffer of that size.
    unsafe {
        if RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            value,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let mut buf = vec![0u16; (size as usize).div_ceil(2)];
        if RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            value,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    }
}

fn now_ms() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as Timestamp)
        .unwrap_or(0)
}

fn emit(state: SessionState, event: SessionEvent) {
    SESSION_STATE.store(
        match state {
            SessionState::Active => 0,
            SessionState::Locked => 1,
            SessionState::Suspended => 2,
        },
        Ordering::SeqCst,
    );
    if let Some(sink) = SESSION_SINK.get() {
        sink(event);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let at = now_ms();
    match msg {
        WM_WTSSESSION_CHANGE => match wparam.0 as u32 {
            WTS_SESSION_LOCK | WTS_SESSION_LOGOFF => {
                emit(SessionState::Locked, SessionEvent::Locked { at })
            }
            WTS_SESSION_UNLOCK | WTS_SESSION_LOGON => {
                emit(SessionState::Active, SessionEvent::Unlocked { at })
            }
            _ => {}
        },
        WM_POWERBROADCAST => match wparam.0 as u32 {
            PBT_APMSUSPEND => emit(SessionState::Suspended, SessionEvent::Suspending { at }),
            PBT_APMRESUMEAUTOMATIC => {
                emit(SessionState::Active, SessionEvent::Resumed { at });
            }
            _ => {}
        },
        _ => {}
    }
    // SAFETY: forwarding the message unchanged.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn run_message_window() {
    // SAFETY: standard message-only window setup on this thread; the window
    // and its registrations live until the process exits.
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            tracing::warn!("GetModuleHandleW failed; no session events");
            return;
        };
        let class = w!("nagsworth_session_events");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: module.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(module.into()),
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("cannot create session window: {e}");
                return;
            }
        };
        if let Err(e) = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) {
            tracing::warn!("WTSRegisterSessionNotification failed: {e}");
        }
        if let Err(e) =
            RegisterSuspendResumeNotification(HANDLE(hwnd.0), DEVICE_NOTIFY_WINDOW_HANDLE)
        {
            tracing::warn!("RegisterSuspendResumeNotification failed: {e}");
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_own_process_and_idle() {
        let p = WindowsPlatform::new();
        let me = p.process_info(std::process::id()).unwrap();
        assert!(me.exe_name.to_ascii_lowercase().ends_with(".exe"), "{me:?}");
        assert!(me.exe_path.is_some());
        // Cached path returns the same thing.
        assert_eq!(p.process_info(std::process::id()).unwrap(), me);
        assert!(p.idle_time().is_ok());
        // May be None on a hidden desktop or in CI.
        let _ = p.foreground_window().unwrap();
        let _ = p.exclusive_fullscreen();
    }
}
