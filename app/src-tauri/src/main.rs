#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod cabinet;
mod config;
mod core;
mod hwid;
mod profile;
mod sysproxy;

use serde_json::Value;
use std::sync::Arc;
use tauri::{Emitter, Manager};

type Eng<'a> = tauri::State<'a, Arc<core::Engine>>;
type Cab<'a> = tauri::State<'a, cabinet::Cabinet>;
struct AccountGate(std::sync::Mutex<()>);
type Gate<'a> = tauri::State<'a, AccountGate>;

#[tauri::command(async)]
fn cabinet_boot(c: Cab) -> Result<Value, String> {
    c.boot()
}
#[tauri::command(async)]
fn cabinet_login(
    c: Cab,
    e: Eng,
    gate: Gate,
    email: String,
    password: String,
) -> Result<Value, String> {
    let _guard = gate.0.lock().map_err(|_| "Account operation failed.")?;
    let result = c.login(&email, &password)?;
    let stopped = e.disconnect();
    let forgotten = e.forget_subscription();
    stopped?;
    forgotten?;
    Ok(result)
}
#[tauri::command(async)]
fn cabinet_request(
    c: Cab,
    method: String,
    path: String,
    body: Option<Value>,
) -> Result<Value, String> {
    c.request(&method, &path, body)
}
#[tauri::command(async)]
fn cabinet_auth(c: Cab, e: Eng, gate: Gate, action: String, body: Value) -> Result<Value, String> {
    let _guard = gate
        .0
        .lock()
        .map_err(|_| "Something went wrong. Try again later.")?;
    let result = c.auth(&action, body)?;
    if result.get("me").is_some() {
        let stopped = e.disconnect();
        let forgotten = e.forget_subscription();
        stopped?;
        forgotten?;
    }
    Ok(result)
}
#[tauri::command(async)]
fn cabinet_logout(c: Cab, e: Eng, gate: Gate) -> Result<Value, String> {
    let _guard = gate.0.lock().map_err(|_| "Account operation failed.")?;
    let stopped = e.disconnect();
    let result = c.logout();
    let forgotten = e.forget_subscription();
    stopped?;
    forgotten?;
    result
}
#[tauri::command(async)]
fn cabinet_use_subscription(c: Cab, e: Eng, gate: Gate, sub_id: u64) -> Result<Value, String> {
    let _guard = gate.0.lock().map_err(|_| "Account operation failed.")?;
    let result = c.request("GET", &format!("/subscription/link?sub_id={sub_id}"), None)?;
    let url = result["link"]
        .as_str()
        .ok_or("The subscription link is missing.")?;
    e.set_subscription(url)?;
    e.set_subscription_id(sub_id)
}
#[tauri::command(async)]
fn clear_subscription(e: Eng, gate: Gate, expected_sub_id: Option<u64>) -> Result<(), String> {
    let _guard = gate.0.lock().map_err(|_| "Account operation failed.")?;
    if let Some(id) = expected_sub_id {
        if e.boot()?["sub_id"].as_u64() != Some(id) {
            return Ok(());
        }
    }
    let stopped = e.disconnect();
    let forgotten = e.forget_subscription();
    stopped?;
    forgotten
}
#[tauri::command(async)]
fn open_external(url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|_| "Invalid link.")?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Only HTTPS links can be opened.".into());
    }
    let operation: Vec<u16> = "open\0".encode_utf16().collect();
    let file: Vec<u16> = parsed.as_str().encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as usize <= 32 {
        return Err("Could not open the browser.".into());
    }
    Ok(())
}

// Every command runs off the UI thread: the window never freezes while the core starts or a server is slow.
#[tauri::command(async)]
fn boot(e: Eng) -> Result<Value, String> {
    e.boot()
}
#[tauri::command(async)]
fn set_subscription(e: Eng, gate: Gate, url: String) -> Result<Value, String> {
    let _guard = gate.0.lock().map_err(|_| "Account operation failed.")?;
    e.set_subscription(&url)
}
#[tauri::command(async)]
fn refresh_subscription(e: Eng, gate: Gate) -> Result<Value, String> {
    let _guard = gate
        .0
        .lock()
        .map_err(|_| "Something went wrong. Try again later.")?;
    e.refresh_subscription()
}
#[tauri::command(async)]
fn connect(e: Eng, gate: Gate, app: tauri::AppHandle) -> Result<Value, String> {
    let _guard = gate.0.lock().map_err(|_| "Account operation failed.")?;
    let result = e.inner().connect(app.clone());
    tray_sync(&app);
    result
}
#[tauri::command(async)]
fn disconnect(e: Eng, app: tauri::AppHandle) -> Result<Value, String> {
    let result = e.disconnect();
    tray_sync(&app);
    result
}

struct TrayToggle(tauri::menu::MenuItem<tauri::Wry>);

/// The tray reads the engine's real state, so its label cannot drift from what the core is doing.
fn tray_sync(app: &tauri::AppHandle) {
    let connected = app
        .state::<Arc<core::Engine>>()
        .boot()
        .map(|v| v["connected"] == true)
        .unwrap_or(false);
    if let Some(item) = app.try_state::<TrayToggle>() {
        let _ = item
            .0
            .set_text(if connected { "Disconnect" } else { "Connect" });
    }
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(if connected {
            "akimov vpn · connected"
        } else {
            "akimov vpn · not connected"
        }));
    }
}

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let open = MenuItem::with_id(app, "open", "Open akimov vpn", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "Connect", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open, &toggle, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    app.manage(TrayToggle(toggle));
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("akimov vpn · not connected")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app),
            // the window owns the connect flow (consent, errors, animation); the tray only asks it to run
            "toggle" => {
                let _ = app.emit("tray-toggle", ());
            }
            "quit" => {
                let app = app.clone();
                std::thread::spawn(move || {
                    app.state::<Arc<core::Engine>>().shutdown();
                    app.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // any click refreshes the label before the menu opens; a left click also brings the window back
            tray_sync(tray.app_handle());
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
#[tauri::command(async)]
fn proxies(e: Eng) -> Result<Value, String> {
    e.proxies()
}
#[tauri::command(async)]
fn select_node(e: Eng, name: String) -> Result<Value, String> {
    e.select_node(name)
}
#[tauri::command(async)]
fn test_latency(e: Eng) -> Result<Value, String> {
    e.test_latency()
}
#[tauri::command(async)]
fn set_rules(e: Eng, rules: Vec<config::Rule>) -> Result<Value, String> {
    e.set_rules(rules)
}
#[tauri::command(async)]
fn set_mode(e: Eng, mode: String) -> Result<Value, String> {
    e.set_mode(mode)
}

#[tauri::command(async)]
fn set_close_to_tray(e: Eng, on: bool) -> Result<Value, String> {
    e.set_close_to_tray(on)
}

#[tauri::command(async)]
fn elevate(app: tauri::AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let operation = wide("runas");
    let file = wide(&exe.to_string_lossy());
    let args = wide(&format!("--wait-parent={} --connect", std::process::id()));
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            args.as_ptr(),
            std::ptr::null(),
            1,
        )
    };
    if result as usize <= 32 {
        return Err(
            "Administrator access was not granted. TUN needs it to create the network adapter."
                .into(),
        );
    }
    app.exit(0);
    Ok(())
}
#[tauri::command(async)]
fn watch(e: Eng, on: bool) -> Result<(), String> {
    e.watch(on);
    Ok(())
}

fn main() {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows_sys::Win32::{
        Foundation::{GetLastError, ERROR_ALREADY_EXISTS},
        System::Threading::CreateMutexW,
    };
    if let Some(pid) = std::env::args().find_map(|arg| {
        arg.strip_prefix("--wait-parent=")
            .and_then(|s| s.parse::<u32>().ok())
    }) {
        unsafe {
            let parent = windows_sys::Win32::System::Threading::OpenProcess(0x0010_0000, 0, pid);
            if !parent.is_null() {
                windows_sys::Win32::System::Threading::WaitForSingleObject(parent, 10000);
                windows_sys::Win32::Foundation::CloseHandle(parent);
            }
        }
    }
    let name: Vec<u16> = "Local\\akimov-vpn-app\0".encode_utf16().collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return;
    }
    let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let _instance = unsafe { OwnedHandle::from_raw_handle(handle as _) };
    if exists {
        return;
    }
    let engine = Arc::new(core::Engine::new());
    let on_close = engine.clone();
    tauri::Builder::default()
        .manage(engine)
        .manage(cabinet::Cabinet::new(core::data_dir()))
        .manage(AccountGate(std::sync::Mutex::new(())))
        .invoke_handler(tauri::generate_handler![
            boot,
            cabinet_boot,
            cabinet_login,
            cabinet_auth,
            cabinet_request,
            cabinet_logout,
            cabinet_use_subscription,
            clear_subscription,
            open_external,
            set_subscription,
            refresh_subscription,
            connect,
            disconnect,
            proxies,
            select_node,
            test_latency,
            set_rules,
            set_mode,
            set_close_to_tray,
            elevate,
            watch
        ])
        .setup(|app| {
            build_tray(app)?;
            Ok(())
        })
        // closing the window hides it to the tray by default; with that setting off it quits:
        // restore the system proxy, close every connection, stop the core
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if on_close.close_to_tray() {
                    let _ = window.hide();
                    return;
                }
                let engine = on_close.clone();
                let app = window.app_handle().clone();
                std::thread::spawn(move || {
                    engine.shutdown();
                    app.exit(0);
                });
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to start akimov vpn")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<Arc<core::Engine>>().shutdown();
            }
        });
}
