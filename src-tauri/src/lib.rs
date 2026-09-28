mod platform;
mod timer;

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use timer::{Mode, Timer};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    work_minutes: u32,
    break_minutes: u32,
    sound: bool,
    autostart: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            work_minutes: 120,
            break_minutes: 5,
            sound: true,
            autostart: false,
        }
    }
}
impl Settings {
    fn validate(&self) -> Result<(), String> {
        if !(1..=1440).contains(&self.work_minutes) || !(1..=180).contains(&self.break_minutes) {
            return Err("作業時間は1〜1440分、休憩時間は1〜180分で指定してください。".into());
        }
        Ok(())
    }
    fn durations(&self, fast: bool) -> (u64, u64) {
        if fast {
            (60_000, 30_000)
        } else {
            (
                self.work_minutes as u64 * 60_000,
                self.break_minutes as u64 * 60_000,
            )
        }
    }
}
struct AppState {
    timer: Timer,
    settings: Settings,
    fast: bool,
    announced: Mode,
    break_visible: bool,
    break_chimed: bool,
    notice_until: u64,
    warning: String,
}
struct TrayItems {
    pause: MenuItem<tauri::Wry>,
    reset: MenuItem<tauri::Wry>,
    settings: MenuItem<tauri::Wry>,
}

fn settings_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|p| p.join("settings.json"))
        .map_err(|e| e.to_string())
}
fn load_settings_from_disk(app: &AppHandle) -> (Settings, String) {
    let result = (|| -> Result<Settings, String> {
        let mut path = settings_path(app)?;
        if !path.exists() && path.with_file_name("settings.backup.json").exists() {
            path = path.with_file_name("settings.backup.json");
        }
        if !path.exists() {
            return Ok(Settings::default());
        }
        let settings: Settings =
            serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        settings.validate()?;
        Ok(settings)
    })();
    match result {
        Ok(settings) => (settings, String::new()),
        Err(_) => (
            Settings::default(),
            "設定ファイルを読み込めなかったため、初期設定で開始しました。".into(),
        ),
    }
}
fn persist(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app)?;
    let parent = path.parent().ok_or("設定保存先が見つかりません。")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let text = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    let temp = parent.join("settings.tmp");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
        f.write_all(&text).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    // Windows rename cannot replace an existing file. Keep a backup until commit succeeds.
    let backup = parent.join("settings.backup.json");
    let existed = path.exists();
    if existed {
        if backup.exists() {
            std::fs::remove_file(&backup).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&path, &backup).map_err(|e| e.to_string())?;
    }
    if let Err(e) = std::fs::rename(&temp, &path) {
        if existed {
            let _ = std::fs::rename(&backup, &path);
        }
        return Err(e.to_string());
    }
    if existed {
        let _ = std::fs::remove_file(backup);
    }
    Ok(())
}
#[cfg(windows)]
fn autostart_set_reg(name: &str, exe: &std::path::Path, enable: bool) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .create_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run")
        .map(|(key, _)| key)
        .map_err(|e| e.to_string())?;
    if enable {
        key.set_value(name, &format!("\"{}\"", exe.display()))
            .map_err(|e| e.to_string())?;
    } else {
        match key.delete_value(name) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn autostart_reg_is_enabled(name: &str) -> bool {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", KEY_READ)
        .ok()
        .and_then(|k| k.get_value::<String, _>(name).ok())
        .is_some()
}

#[cfg(windows)]
fn autostart_set(_app: &AppHandle, enable: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    autostart_set_reg("Sit Break 120-5", &exe, enable)
}

#[cfg(windows)]
fn autostart_is_enabled(_app: &AppHandle) -> bool {
    autostart_reg_is_enabled("Sit Break 120-5")
}

#[cfg(not(windows))]
fn autostart_set(_: &AppHandle, _: bool) -> Result<(), String> {
    Err("Windowsでのみ利用できます。".into())
}
#[cfg(not(windows))]
fn autostart_is_enabled(_: &AppHandle) -> bool {
    false
}

fn sync_timer(st: &mut AppState) {
    let now = platform::now_ms();
    let blocked = !platform::interactive();
    st.timer.set_blocked(now, blocked);
}
fn system_blocked(app: &AppHandle, blocked: bool) {
    if let Some(state) = app.try_state::<Mutex<AppState>>() {
        state
            .lock()
            .unwrap()
            .timer
            .set_blocked(platform::now_ms(), blocked);
    }
}
fn snapshot(st: &AppState) -> serde_json::Value {
    json!({"mode": if st.timer.mode == Mode::Work { "work" } else { "break" },
        "remaining": st.timer.remaining(), "paused": st.timer.paused,
        "blocked": st.timer.blocked, "emergency_remaining": st.timer.emergency_remaining(),
        "break_minutes": st.settings.break_minutes, "test_mode": st.fast, "warning": st.warning })
}

// Called on Tauri's main thread. Keep all window transitions ordered with timer commands.
fn publish(app: &AppHandle) {
    let (payload, mode, visible, visibility_changed, transitioned, sound, pause, notice_until) = {
        let state = app.state::<Mutex<AppState>>();
        let mut st = state.lock().unwrap();
        let transitioned = st.announced != st.timer.mode;
        st.announced = st.timer.mode;
        if transitioned {
            st.break_chimed = false;
        }
        let visible = st.timer.mode == Mode::Break && !st.timer.blocked;
        let visibility_changed = st.break_visible != visible;
        st.break_visible = visible;
        let sound = visible && !st.break_chimed && st.settings.sound;
        if visible {
            st.break_chimed = true;
        }
        if transitioned && st.timer.mode == Mode::Work {
            st.notice_until = platform::now_ms() + 3000;
        }
        (
            snapshot(&st),
            st.timer.mode,
            visible,
            visibility_changed,
            transitioned,
            sound,
            st.timer.paused,
            st.notice_until,
        )
    };
    if visibility_changed {
        if let Some(w) = app.get_webview_window("reminder") {
            if visible {
                for label in ["panel", "settings", "notice"] {
                    if let Some(other) = app.get_webview_window(label) {
                        let _ = other.hide();
                    }
                }
                let _ = w.center();
                let _ = w.show();
                let _ = w.set_focus();
                if sound {
                    platform::chime();
                }
            } else {
                let _ = w.hide();
            }
        }
    }
    if mode == Mode::Break && visible {
        // Recover from Win+D / minimize without continuously stealing keyboard focus.
        if let Some(w) = app.get_webview_window("reminder") {
            if w.is_minimized().unwrap_or(false) {
                let _ = w.unminimize();
            }
            if !w.is_visible().unwrap_or(true) {
                let _ = w.show();
            }
        }
    }
    if let Some(w) = app.get_webview_window("notice") {
        if transitioned && mode == Mode::Work {
            let _ = w.show();
        }
        if platform::now_ms() >= notice_until {
            let _ = w.hide();
        }
    }
    if let Some(items) = app.try_state::<TrayItems>() {
        let working = mode == Mode::Work;
        let _ = items.pause.set_enabled(working);
        let _ = items.reset.set_enabled(working);
        let _ = items.settings.set_enabled(working);
        let _ = items
            .pause
            .set_text(if pause { "再開" } else { "一時停止" });
    }
    if let Some(tray) = app.tray_by_id("main-tray") {
        let remaining = payload["remaining"].as_u64().unwrap_or(0);
        let label = if mode == Mode::Break {
            "休憩中"
        } else if pause {
            "一時停止中"
        } else {
            "次の休憩まで"
        };
        let _ = tray.set_tooltip(Some(format!(
            "Sit Break · {label} {:02}:{:02}:{:02}",
            remaining / 3600,
            remaining / 60 % 60,
            remaining % 60
        )));
    }
    let _ = app.emit("tick", payload);
}

fn show_status(app: &AppHandle) {
    let breaking = app.state::<Mutex<AppState>>().lock().unwrap().timer.mode == Mode::Break;
    let label = if breaking { "reminder" } else { "panel" };
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    app.state::<Mutex<AppState>>()
        .lock()
        .unwrap()
        .settings
        .clone()
}
#[tauri::command]
fn timer_info(app: AppHandle) -> serde_json::Value {
    // Read only: transitions belong to the timer loop or explicit commands.
    snapshot(&app.state::<Mutex<AppState>>().lock().unwrap())
}
fn act(
    app: &AppHandle,
    action: impl FnOnce(&mut Timer, u64) -> Result<(), &'static str>,
) -> Result<(), String> {
    let result = {
        let state = app.state::<Mutex<AppState>>();
        let mut st = state.lock().unwrap();
        sync_timer(&mut st);
        action(&mut st.timer, platform::now_ms()).map_err(str::to_string)
    };
    publish(app);
    result
}
#[tauri::command]
fn toggle_pause(app: AppHandle) -> Result<(), String> {
    act(&app, |t, now| t.toggle_pause(now))
}
#[tauri::command]
fn reset_timer(app: AppHandle) -> Result<(), String> {
    act(&app, |t, now| t.reset(now))
}
#[tauri::command]
fn request_emergency(app: AppHandle) -> Result<(), String> {
    act(&app, |t, now| t.request_emergency(now))
}
#[tauri::command]
fn confirm_emergency(app: AppHandle) -> Result<(), String> {
    act(&app, |t, now| t.confirm_emergency(now))
}
#[tauri::command]
fn cancel_emergency(app: AppHandle) -> Result<(), String> {
    act(&app, |t, _| {
        t.cancel_emergency();
        Ok(())
    })
}
#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), String> {
    act(&app, |t, _| t.require_work())?;
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = app.emit("settings_open", ());
    }
    Ok(())
}
#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    settings.validate()?;
    let result = (|| {
        let state = app.state::<Mutex<AppState>>();
        let mut st = state.lock().unwrap();
        sync_timer(&mut st);
        st.timer.require_work().map_err(str::to_string)?;
        let (work, rest) = settings.durations(st.fast);
        let mut next_timer = st.timer.clone();
        next_timer
            .configure(platform::now_ms(), work, rest)
            .map_err(str::to_string)?;
        let previous_autostart = autostart_is_enabled(&app);
        if settings.autostart != previous_autostart {
            autostart_set(&app, settings.autostart)?;
        }
        if let Err(e) = persist(&app, &settings) {
            if settings.autostart != previous_autostart {
                let _ = autostart_set(&app, previous_autostart);
            }
            return Err(format!("設定を保存できませんでした: {e}"));
        }
        st.timer = next_timer;
        st.settings = settings;
        st.warning.clear();
        Ok(())
    })();
    publish(&app);
    if result.is_ok() {
        let _ = app.emit("settings_changed", ());
    }
    result
}

fn run_timer(app: AppHandle) {
    std::thread::spawn(move || loop {
        // One outstanding UI tick at most. The clock, not this sleep, measures elapsed time.
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let handle = app.clone();
        if app
            .run_on_main_thread(move || {
                {
                    let state = handle.state::<Mutex<AppState>>();
                    sync_timer(&mut state.lock().unwrap());
                }
                publish(&handle);
                let _ = tx.send(());
            })
            .is_err()
        {
            break;
        }
        if rx.recv().is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_status(app)
        }))
        .invoke_handler(tauri::generate_handler![
            get_settings,
            timer_info,
            save_settings,
            toggle_pause,
            reset_timer,
            request_emergency,
            confirm_emergency,
            cancel_emergency,
            open_settings
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let (mut settings, warning) = load_settings_from_disk(&handle);
            settings.autostart = autostart_is_enabled(&handle);
            // Environment override is compiled out of release builds.
            let fast = cfg!(debug_assertions)
                && std::env::var("SIT_BREAK_TEST_MODE").as_deref() == Ok("1");
            let (work, rest) = settings.durations(fast);
            let mut timer = Timer::new(platform::now_ms(), work, rest);
            timer.blocked = !platform::interactive();
            handle.manage(Mutex::new(AppState {
                timer,
                settings,
                fast,
                announced: Mode::Work,
                break_visible: false,
                break_chimed: false,
                notice_until: 0,
                warning,
            }));
            let panel =
                WebviewWindowBuilder::new(&handle, "panel", WebviewUrl::App("panel.html".into()))
                    .title("Sit Break 120-5")
                    .inner_size(360.0, 340.0)
                    .visible(false)
                    .resizable(false)
                    .center()
                    .build()?;
            let reminder = WebviewWindowBuilder::new(
                &handle,
                "reminder",
                WebviewUrl::App("reminder.html".into()),
            )
            .title("休憩時間です")
            .inner_size(680.0, 540.0)
            .visible(false)
            .decorations(false)
            .always_on_top(true)
            .resizable(false)
            .minimizable(false)
            .maximizable(false)
            .closable(false)
            .center()
            .build()?;
            let settings_window = WebviewWindowBuilder::new(
                &handle,
                "settings",
                WebviewUrl::App("settings.html".into()),
            )
            .title("Sit Break 設定")
            .inner_size(420.0, 490.0)
            .visible(false)
            .resizable(false)
            .center()
            .build()?;
            WebviewWindowBuilder::new(&handle, "notice", WebviewUrl::App("notice.html".into()))
                .title("休憩終了")
                .inner_size(300.0, 120.0)
                .visible(false)
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .focused(false)
                .focusable(false)
                .center()
                .build()?;
            platform::install(&handle, &panel).map_err(std::io::Error::other)?;
            for window in [panel, settings_window] {
                window.on_window_event({
                    let w = window.clone();
                    move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();
                            let _ = w.hide();
                        }
                    }
                });
            }
            reminder.on_window_event({
                let w = reminder.clone();
                move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.show(); // Alt+F4 never skips a break.
                    }
                }
            });
            let status = MenuItem::with_id(&handle, "status", "状態を表示", true, None::<&str>)?;
            let pause = MenuItem::with_id(&handle, "pause", "一時停止", true, None::<&str>)?;
            let reset = MenuItem::with_id(
                &handle,
                "reset",
                "タイマーを最初から開始",
                true,
                None::<&str>,
            )?;
            let settings_item = MenuItem::with_id(&handle, "settings", "設定", true, None::<&str>)?;
            let quit = MenuItem::with_id(&handle, "quit", "終了", true, None::<&str>)?;
            let menu =
                Menu::with_items(&handle, &[&status, &pause, &reset, &settings_item, &quit])?;
            handle.manage(TrayItems {
                pause,
                reset,
                settings: settings_item,
            });
            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().expect("bundled icon").clone())
                .menu(&menu)
                .tooltip("Sit Break · 次の休憩まで 02:00:00")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "status" => show_status(app),
                    "pause" => {
                        let _ = toggle_pause(app.clone());
                    }
                    "reset" => {
                        let _ = reset_timer(app.clone());
                    }
                    "settings" => {
                        let _ = open_settings(app.clone());
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_status(tray.app_handle());
                    }
                })
                .build(&handle)?;
            publish(&handle);
            run_timer(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Sit Breakを起動できませんでした");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_validation() {
        let mut s = Settings::default();
        assert_eq!(s.durations(false), (7_200_000, 300_000));
        assert!(s.sound);
        assert!(!s.autostart);
        assert!(s.validate().is_ok());
        s.work_minutes = 0;
        assert!(s.validate().is_err());
        s.work_minutes = 120;
        s.break_minutes = 181;
        assert!(s.validate().is_err());
    }
    #[test]
    fn debug_durations_are_explicit() {
        assert_eq!(Settings::default().durations(true), (60_000, 30_000));
    }
    #[cfg(windows)]
    #[test]
    fn quoted_autostart_registry_roundtrip() {
        let exe = std::env::current_exe().unwrap();
        let name = "Sit Break 120-5 Test";
        autostart_set_reg(name, &exe, true).unwrap();
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run")
            .unwrap();
        let value: String = key.get_value(name).unwrap();
        autostart_set_reg(name, &exe, false).unwrap();
        assert!(value.starts_with('"') && value.ends_with('"'));
        assert!(!autostart_reg_is_enabled(name));
    }
}
