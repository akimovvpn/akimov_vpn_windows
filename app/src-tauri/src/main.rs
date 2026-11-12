#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod cabinet;
mod config;
mod core;
mod hwid;
mod process;
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
fn cabinet_boot(c: Cab, e: Eng) -> Result<Value, String> {
    c.set_proxy(e.proxy_port());
    c.boot()
}

#[tauri::command]
async fn update_check(app: tauri::AppHandle) -> Result<Value, String> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app
        .updater()
        .map_err(|_| "Updates are unavailable.")?
        .check()
        .await
        .map_err(|_| "Could not check for updates.")?;
    Ok(match update {
        Some(update) => serde_json::json!({"available": true, "version": update.version}),
        None => serde_json::json!({"available": false}),
    })
}

#[tauri::command]
async fn update_install(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app
        .updater()
        .map_err(|_| "Updates are unavailable.")?
        .check()
        .await
        .map_err(|_| "Could not check for updates.")?
        .ok_or("You already have the latest version.")?;
    // download() verifies the signature against the public key built into this app; nothing unsigned is kept
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|_| "The update could not be downloaded or verified.")?;
    // the installer replaces the core: disconnect and stop it first
    let engine = app.state::<Arc<core::Engine>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.shutdown())
        .await
        .map_err(|_| "Could not stop the VPN before updating.")?;
    update
        .install(bytes)
        .map_err(|_| "The update could not be installed. Restart the app and try again.")?;
    app.exit(0);
    Ok(())
}

/// One check shortly after start; the window decides how to show it.
fn announce_update(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        use tauri_plugin_updater::UpdaterExt;
        std::thread::sleep(std::time::Duration::from_secs(20));
        let found =
            tauri::async_runtime::block_on(async { app.updater().ok()?.check().await.ok()? });
        if let Some(update) = found {
            let _ = app.emit(
                "update-available",
                serde_json::json!({"version": update.version}),
            );
        }
    });
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
/// Runs inside Telegram's page, in a window that cannot call any app command. Telegram's page
/// answers the page that opened it; here that answer is carried out as a navigation the app stops.
const TELEGRAM_BRIDGE: &str = r#"(function(){
  if(location.hostname!=="oauth.telegram.org")return;
  var sent=false;
  function done(data){
    if(sent)return;sent=true;
    location.href="https://akimov-login.invalid/?"+(data===undefined?"closed":"d="+encodeURIComponent(typeof data==="string"?data:JSON.stringify(data)));
  }
  var opener={closed:false,focus:function(){},postMessage:function(data){done(data)}};
  try{Object.defineProperty(window,"opener",{configurable:true,get:function(){return opener},set:function(){}})}catch(e){}
  window.close=function(){done()};
})();"#;

/// Telegram's official sign-in, the same one the cabinet uses, in its own window.
/// Returns the account, or null when the window was closed without signing in.
#[tauri::command(async)]
fn cabinet_telegram_login(
    app: tauri::AppHandle,
    c: Cab,
    e: Eng,
    gate: Gate,
) -> Result<Value, String> {
    const REFUSED: &str = "Telegram sign-in was not completed. Try again.";
    c.set_proxy(e.proxy_port());
    let challenge = c.telegram_challenge()?;
    if let Some(old) = app.get_webview_window("telegram") {
        let _ = old.destroy();
    }
    let (done, answers) = std::sync::mpsc::channel::<Option<String>>();
    let closed = done.clone();
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        "telegram",
        tauri::WebviewUrl::External(challenge.url),
    )
    .title("Sign in with Telegram")
    .inner_size(520.0, 720.0)
    .center()
    .initialization_script(TELEGRAM_BRIDGE)
    .on_navigation(move |url| {
        if url.host_str() == Some("akimov-login.invalid") {
            let answer = url.query_pairs().find(|(key, _)| key == "d");
            let _ = done.send(answer.map(|(_, value)| value.into_owned()));
            return false;
        }
        // The window stays on Telegram's own pages.
        url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|host| host == "telegram.org" || host.ends_with(".telegram.org"))
    })
    .build()
    .map_err(|_| "Could not open the Telegram sign-in window.")?;
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            let _ = closed.send(None);
        }
    });
    let answer = answers
        .recv_timeout(std::time::Duration::from_secs(challenge.expires))
        .ok()
        .flatten();
    let _ = window.destroy();
    let Some(answer) = answer else {
        return Ok(Value::Null);
    };
    let message: Value = serde_json::from_str(&answer).map_err(|_| REFUSED)?;
    if message["event"] != "auth_result" || !message["error"].is_null() {
        return Err(REFUSED.into());
    }
    let _guard = gate.0.lock().map_err(|_| "Account operation failed.")?;
    let me = c.telegram_complete(&challenge.token, &message["result"])?;
    let stopped = e.disconnect();
    let forgotten = e.forget_subscription();
    stopped?;
    forgotten?;
    Ok(me)
}
#[tauri::command(async)]
fn cabinet_request(
    c: Cab,
    e: Eng,
    method: String,
    path: String,
    body: Option<Value>,
) -> Result<Value, String> {
    c.set_proxy(e.proxy_port());
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
fn connect(e: Eng, c: Cab, gate: Gate, app: tauri::AppHandle) -> Result<Value, String> {
    let _guard = gate.0.lock().map_err(|_| "Account operation failed.")?;
    c.require_session()?;
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
fn test_latency(e: Eng, gate: Gate) -> Result<Value, String> {
    let _guard = gate.0.lock().map_err(|_| "Account operation failed.")?;
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
    if let Some(result) = process::helper() {
        std::process::exit(if result.is_ok() { 0 } else { 1 });
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
            cabinet_telegram_login,
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
            update_check,
            update_install,
            watch
        ])
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            announce_update(app.handle().clone());
            use windows_sys::Win32::Graphics::Dwm::{
                DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
            };
            if let Some(window) = app.get_webview_window("main") {
                let hwnd = window.hwnd()?.0;
                // COLORREF is BGR: match the fixed Dust background #07070A.
                for (attribute, value) in [
                    (DWMWA_USE_IMMERSIVE_DARK_MODE, 1u32),
                    (DWMWA_CAPTION_COLOR, 0x000A0707),
                    (DWMWA_TEXT_COLOR, 0x00F0F0F0),
                ] {
                    // Older Windows versions keep the configured native dark theme.
                    unsafe {
                        DwmSetWindowAttribute(
                            hwnd as _,
                            attribute as u32,
                            std::ptr::from_ref(&value).cast(),
                            std::mem::size_of_val(&value) as u32,
                        );
                    }
                }
            }
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
