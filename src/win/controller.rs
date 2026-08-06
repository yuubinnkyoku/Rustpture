use std::io;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, LoadCursorW,
    LoadIconW, PostQuitMessage, RegisterClassExW, SetWindowLongPtrW, ShowWindow,
    WNDCLASSEXW, CS_DBLCLKS, CW_USEDEFAULT, GWLP_USERDATA, IDC_ARROW, IDI_APPLICATION,
    SC_MAXIMIZE, SC_RESTORE, SW_SHOWMINNOACTIVE, WM_CLOSE, WM_DESTROY, WM_DISPLAYCHANGE,
    WM_NCDESTROY, WM_PAINT, WM_SYSCOMMAND, WS_OVERLAPPEDWINDOW,
};

use super::overlay;
use super::wide::wide_null;
use super::{APP_TITLE, CONTROLLER_CLASS, WM_APP_BEGIN_CAPTURE};

struct ControllerState {
    overlay: HWND,
}

pub unsafe fn register_class(instance: HINSTANCE) -> io::Result<()> {
    let class_name = wide_null(CONTROLLER_CLASS);
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
    let window = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        360,
        160,
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

pub unsafe fn show_resident(window: HWND) {
    ShowWindow(window, SW_SHOWMINNOACTIVE);
}

unsafe fn state_ptr(window: HWND) -> *mut ControllerState {
    GetWindowLongPtrW(window, GWLP_USERDATA) as *mut ControllerState
}

unsafe fn begin_capture(window: HWND) {
    let pointer = state_ptr(window);
    if pointer.is_null() {
        return;
    }
    let overlay = (*pointer).overlay;
    overlay::begin_capture(overlay);
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
        WM_SYSCOMMAND => {
            let command = (wparam as u32) & 0xfff0;
            if command == SC_RESTORE || command == SC_MAXIMIZE {
                begin_capture(window);
                return 0;
            }
            DefWindowProcW(window, message, wparam, lparam)
        }
        WM_DISPLAYCHANGE => {
            let pointer = state_ptr(window);
            if !pointer.is_null() {
                let overlay = (*pointer).overlay;
                overlay::refresh_geometry(overlay);
            }
            0
        }
        WM_PAINT => {
            let mut paint: PAINTSTRUCT = zeroed();
            BeginPaint(window, &mut paint);
            EndPaint(window, &paint);
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
