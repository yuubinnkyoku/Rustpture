mod bitmap;
mod controller;
mod menu;
mod overlay;
mod pin;
mod wide;

use std::io;
use std::mem::zeroed;
use std::ptr::{null, null_mut};

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, DispatchMessageW, FindWindowW, GetMessageW,
    GetWindowThreadProcessId, MessageBoxW, PostMessageW, TranslateMessage, MB_ICONERROR, MB_OK,
    MSG, WM_CLOSE,
};

use self::wide::wide_null;

pub const CONTROLLER_CLASS: &str = "Rustpture.Controller.0.1";
pub const OVERLAY_CLASS: &str = "Rustpture.Overlay.0.1";
pub const PIN_CLASS: &str = "Rustpture.Pin.0.1";
pub const APP_TITLE: &str = "Rustpture — クリックしてキャプチャ";

pub const WM_APP_BEGIN_CAPTURE: u32 = 0x8000 + 1;

pub fn run() -> io::Result<()> {
    let command = Command::parse();
    let controller_class = wide_null(CONTROLLER_CLASS);

    unsafe {
        let existing = FindWindowW(controller_class.as_ptr(), null());
        if !existing.is_null() {
            match command {
                Command::Background => {}
                Command::Quit => {
                    PostMessageW(existing, WM_CLOSE, 0, 0);
                }
                Command::Capture => {
                    // A taskbar-launched helper is usually allowed to transfer
                    // foreground activation to the resident process. This keeps
                    // the pre-created overlay focused so Esc works immediately.
                    let mut process_id = 0;
                    GetWindowThreadProcessId(existing, &mut process_id);
                    if process_id != 0 {
                        AllowSetForegroundWindow(process_id);
                    }
                    PostMessageW(existing, WM_APP_BEGIN_CAPTURE, 0, 0);
                }
            }
            return Ok(());
        }

        if command == Command::Quit {
            return Ok(());
        }

        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err(io::Error::last_os_error());
        }

        controller::register_class(instance)?;
        overlay::register_class(instance)?;
        pin::register_class(instance)?;

        let controller = controller::create(instance)?;
        let overlay = match overlay::create(instance, controller) {
            Ok(overlay) => overlay,
            Err(error) => {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(controller);
                return Err(error);
            }
        };
        controller::attach_state(controller, overlay);
        controller::show_resident(controller);

        if command == Command::Capture {
            PostMessageW(controller, WM_APP_BEGIN_CAPTURE, 0, 0);
        }

        let mut message: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result == -1 {
                return Err(io::Error::last_os_error());
            }
            if result == 0 {
                break;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    Ok(())
}

pub fn show_fatal_error(message: &str) {
    let title = wide_null("Rustpture - エラー");
    let message = wide_null(message);
    unsafe {
        MessageBoxW(
            null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Capture,
    Background,
    Quit,
}

impl Command {
    fn parse() -> Self {
        let mut command = Self::Capture;
        for argument in std::env::args().skip(1) {
            match argument.as_str() {
                "--background" | "--resident" => command = Self::Background,
                "--capture" => command = Self::Capture,
                "--quit" => command = Self::Quit,
                _ => {}
            }
        }
        command
    }
}
