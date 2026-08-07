use std::io;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetWindowLongPtrW,
    IDC_ARROW, IDI_APPLICATION, LoadCursorW, LoadIconW, PostQuitMessage, RegisterClassExW,
    SetWindowLongPtrW, WM_CLOSE, WM_DESTROY, WM_DISPLAYCHANGE, WM_NCDESTROY, WNDCLASSEXW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};

use super::overlay;
use super::wide::wide_null;
use super::{APP_TITLE, CONTROLLER_CLASS, WM_APP_BEGIN_CAPTURE};

struct ControllerState {
    overlay: HWND,
}

#[allow(clippy::manual_dangling_ptr)]
pub unsafe fn register_class(instance: HINSTANCE) -> io::Result<()> {
    let class_name = wide_null(CONTROLLER_CLASS);
    // Win32's MAKEINTRESOURCEW convention encodes a numeric resource ID
    // in a pointer value; the pointer is never dereferenced.
    let embedded_icon = LoadIconW(instance, 1usize as *const u16);
    let icon = if embedded_icon.is_null() {
        LoadIconW(null_mut(), IDI_APPLICATION)
    } else {
        embedded_icon
    };

    let mut class: WNDCLASSEXW = zeroed();
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.style = CS_DBLCLKS;
    class.lpfnWndProc = Some(window_proc);
    class.hInstance = instance;
    class.hIcon = icon;
    class.hIconSm = icon;
    class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    class.lpszClassName = class_name.as_ptr();

    if RegisterClassExW(&class) == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub unsafe fn create(instance: HINSTANCE) -> io::Result<HWND> {
    let class_name = wide_null(CONTROLLER_CLASS);
    let title = wide_null(APP_TITLE);

    // The controller only exists for single-instance routing and display-change
    // notifications. Keeping it as a hidden tool window preserves the warm resident
    // process without creating a phantom taskbar button after all pin windows close.
    let window = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP,
        0,
        0,
        0,
        0,
        null_mut(),
        null_mut(),
        instance,
        null(),
    );
    if window.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(window)
}

pub unsafe fn attach_state(window: HWND, overlay: HWND) {
    let state = Box::new(ControllerState { overlay });
    SetWindowLongPtrW(window, GWLP_USERDATA, Box::into_raw(state) as isize);
}

unsafe fn state_ptr(window: HWND) -> *mut ControllerState {
    GetWindowLongPtrW(window, GWLP_USERDATA) as *mut ControllerState
}

unsafe fn begin_capture(window: HWND) {
    let pointer = state_ptr(window);
    if pointer.is_null() {
        return;
    }
    overlay::begin_capture((*pointer).overlay);
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_APP_BEGIN_CAPTURE => {
            begin_capture(window);
            0
        }
        WM_DISPLAYCHANGE => {
            let pointer = state_ptr(window);
            if !pointer.is_null() {
                overlay::refresh_geometry((*pointer).overlay);
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(window);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        WM_NCDESTROY => {
            let pointer = state_ptr(window);
            SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            if !pointer.is_null() {
                drop(Box::from_raw(pointer));
            }
            DefWindowProcW(window, message, wparam, lparam)
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}
