use std::collections::HashMap;
use std::ptr::{null, null_mut};
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CWPSTRUCT, CallNextHookEx, FindWindowW, GetClassNameW, GetClientRect, GetWindowRect,
    PostMessageW, SWP_NOSIZE, SetWindowsHookExW, WH_CALLWNDPROC, WINDOWPOS, WM_CREATE,
    WM_NCDESTROY, WM_SHOWWINDOW, WM_SIZING, WM_WINDOWPOSCHANGED, WM_WINDOWPOSCHANGING,
    WMSZ_BOTTOM, WMSZ_BOTTOMLEFT, WMSZ_BOTTOMRIGHT, WMSZ_LEFT, WMSZ_RIGHT, WMSZ_TOP,
    WMSZ_TOPLEFT, WMSZ_TOPRIGHT,
};

use super::wide::wide_null;
use super::{
    CONTROLLER_CLASS, OVERLAY_CLASS, PIN_CLASS, WM_APP_CAPTURE_FINISHED, WM_APP_PIN_CLOSED,
    WM_APP_PIN_OPENED,
};

const CLASS_NAME_CAPACITY: usize = 64;
static PIN_ASPECT_RATIOS: OnceLock<Mutex<HashMap<usize, f64>>> = OnceLock::new();

pub unsafe fn install_current_thread_hook() {
    // This hook only observes windows created on Rustpture's GUI thread. Windows
    // removes it automatically when that thread exits.
    let _ = SetWindowsHookExW(
        WH_CALLWNDPROC,
        Some(call_wnd_proc),
        null_mut(),
        GetCurrentThreadId(),
    );
}

unsafe extern "system" fn call_wnd_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && lparam != 0 {
        let info = &*(lparam as *const CWPSTRUCT);
        let class = window_class(info.hwnd);

        if class.as_deref() == Some(PIN_CLASS) {
            match info.message {
                WM_CREATE => {
                    // Count the pin immediately and remember its original client-area
                    // aspect ratio before the user can resize or maximize it.
                    notify_controller(WM_APP_PIN_OPENED);
                    remember_pin_aspect_ratio(info.hwnd);
                }
                WM_WINDOWPOSCHANGING if info.lParam != 0 => {
                    // Catch non-drag resizes too: maximize, Snap layouts and any other
                    // SetWindowPos-based path. The window itself is fitted inside the
                    // proposed bounds, so the image never needs letterboxing or crop.
                    constrain_pin_windowpos(info.hwnd, info.lParam as *mut WINDOWPOS);
                }
                WM_WINDOWPOSCHANGED => {
                    // WM_CREATE can arrive before the final client geometry exists on
                    // some Windows configurations, so take one more opportunity to
                    // record the ratio. Existing entries are never overwritten.
                    remember_pin_aspect_ratio(info.hwnd);
                }
                WM_SIZING if info.lParam != 0 => {
                    constrain_pin_sizing(info.hwnd, info.wParam as u32, info.lParam as *mut RECT);
                }
                WM_NCDESTROY => {
                    forget_pin_aspect_ratio(info.hwnd);
                    notify_controller(WM_APP_PIN_CLOSED);
                }
                _ => {}
            }
        } else if class.as_deref() == Some(OVERLAY_CLASS)
            && info.message == WM_SHOWWINDOW
            && info.wParam == 0
        {
            // The overlay is hidden for both a completed selection and a cancel.
            // The controller performs a deferred idle check so a pin created by a
            // successful selection can announce itself first.
            notify_controller(WM_APP_CAPTURE_FINISHED);
        }
    }

    CallNextHookEx(null_mut(), code, wparam, lparam)
}

fn aspect_ratios() -> &'static Mutex<HashMap<usize, f64>> {
    PIN_ASPECT_RATIOS.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe fn remember_pin_aspect_ratio(window: HWND) {
    let key = window as usize;
    let mut ratios = aspect_ratios()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if ratios.contains_key(&key) {
        return;
    }

    let mut client: RECT = std::mem::zeroed();
    if GetClientRect(window, &mut client) == 0 {
        return;
    }
    let width = client.right - client.left;
    let height = client.bottom - client.top;
    if width <= 0 || height <= 0 {
        return;
    }

    ratios.insert(key, width as f64 / height as f64);
}

fn pin_aspect_ratio(window: HWND) -> Option<f64> {
    let ratios = aspect_ratios()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    ratios.get(&(window as usize)).copied()
}

fn forget_pin_aspect_ratio(window: HWND) {
    let mut ratios = aspect_ratios()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    ratios.remove(&(window as usize));
}

unsafe fn current_frame_size(window: HWND) -> Option<(i32, i32)> {
    let mut outer: RECT = std::mem::zeroed();
    let mut client: RECT = std::mem::zeroed();
    if GetWindowRect(window, &mut outer) == 0 || GetClientRect(window, &mut client) == 0 {
        return None;
    }

    Some((
        (outer.right - outer.left) - (client.right - client.left),
        (outer.bottom - outer.top) - (client.bottom - client.top),
    ))
}

unsafe fn constrain_pin_windowpos(window: HWND, proposed: *mut WINDOWPOS) {
    if proposed.is_null() {
        return;
    }
    let position = &mut *proposed;
    if position.flags & SWP_NOSIZE != 0 || position.cx <= 0 || position.cy <= 0 {
        return;
    }

    let Some(ratio) = pin_aspect_ratio(window) else {
        return;
    };
    if !ratio.is_finite() || ratio <= 0.0 {
        return;
    }
    let Some((frame_width, frame_height)) = current_frame_size(window) else {
        return;
    };

    let available_client_width = (position.cx - frame_width).max(1);
    let available_client_height = (position.cy - frame_height).max(1);
    let available_ratio = available_client_width as f64 / available_client_height as f64;

    let (client_width, client_height) = if available_ratio > ratio {
        (
            (available_client_height as f64 * ratio).round().max(1.0) as i32,
            available_client_height,
        )
    } else {
        (
            available_client_width,
            (available_client_width as f64 / ratio).round().max(1.0) as i32,
        )
    };

    let target_width = client_width + frame_width;
    let target_height = client_height + frame_height;
    position.x += (position.cx - target_width) / 2;
    position.y += (position.cy - target_height) / 2;
    position.cx = target_width;
    position.cy = target_height;
}

unsafe fn constrain_pin_sizing(window: HWND, edge: u32, proposed: *mut RECT) {
    if proposed.is_null() {
        return;
    }

    let Some(ratio) = pin_aspect_ratio(window) else {
        remember_pin_aspect_ratio(window);
        return;
    };
    if !ratio.is_finite() || ratio <= 0.0 {
        return;
    }
    let Some((frame_width, frame_height)) = current_frame_size(window) else {
        return;
    };

    let rect = &mut *proposed;
    let client_width = ((rect.right - rect.left) - frame_width).max(1);
    let client_height = ((rect.bottom - rect.top) - frame_height).max(1);

    let width_drives = match edge {
        WMSZ_LEFT | WMSZ_RIGHT => true,
        WMSZ_TOP | WMSZ_BOTTOM => false,
        WMSZ_TOPLEFT | WMSZ_TOPRIGHT | WMSZ_BOTTOMLEFT | WMSZ_BOTTOMRIGHT => {
            let height_from_width = (client_width as f64 / ratio).round() as i32;
            let width_from_height = (client_height as f64 * ratio).round() as i32;
            (height_from_width - client_height).abs() <= (width_from_height - client_width).abs()
        }
        _ => return,
    };

    if width_drives {
        let target_client_height = (client_width as f64 / ratio).round().max(1.0) as i32;
        let target_height = target_client_height + frame_height;
        if matches!(edge, WMSZ_TOP | WMSZ_TOPLEFT | WMSZ_TOPRIGHT) {
            rect.top = rect.bottom - target_height;
        } else {
            rect.bottom = rect.top + target_height;
        }
    } else {
        let target_client_width = (client_height as f64 * ratio).round().max(1.0) as i32;
        let target_width = target_client_width + frame_width;
        if matches!(edge, WMSZ_LEFT | WMSZ_TOPLEFT | WMSZ_BOTTOMLEFT) {
            rect.left = rect.right - target_width;
        } else {
            rect.right = rect.left + target_width;
        }
    }
}

unsafe fn notify_controller(message: u32) {
    let class_name = wide_null(CONTROLLER_CLASS);
    let controller = FindWindowW(class_name.as_ptr(), null());
    if !controller.is_null() {
        PostMessageW(controller, message, 0, 0);
    }
}

unsafe fn window_class(window: HWND) -> Option<String> {
    let mut buffer = [0u16; CLASS_NAME_CAPACITY];
    let length = GetClassNameW(
        window,
        buffer.as_mut_ptr(),
        buffer.len().try_into().unwrap_or(i32::MAX),
    );
    if length <= 0 {
        return None;
    }
    String::from_utf16(&buffer[..length as usize]).ok()
}
