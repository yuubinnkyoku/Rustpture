use std::io;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow, PAINTSTRUCT, UpdateWindow,
};
use windows_sys::Win32::System::SystemServices::MK_CONTROL;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetClientRect,
    GetCursorPos, GetWindowLongPtrW, GetWindowRect, HWND_NOTOPMOST, HWND_TOPMOST, IDC_ARROW,
    LWA_ALPHA, LoadCursorW, MB_ICONERROR, MB_OK, MessageBoxW, PostMessageW, RegisterClassExW,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
    SetForegroundWindow, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    WM_CAPTURECHANGED, WM_CLOSE, WM_CONTEXTMENU, WM_DPICHANGED, WM_ERASEBKGND, WM_LBUTTONDBLCLK,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY, WM_PAINT, WNDCLASSEXW,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::geometry::{PointI, RectI, scaled_dimension, zoom_around_point};

use super::bitmap::CapturedBitmap;
use super::menu;
use super::wide::wide_null;
use super::{PIN_CLASS, WM_APP_BEGIN_CAPTURE};

const MIN_SCALE: f64 = 0.10;
const MAX_SCALE: f64 = 8.0;
const DEFAULT_OPACITY: u8 = 255;
const MIN_OPACITY: u8 = 32;

struct PinState {
    bitmap: CapturedBitmap,
    controller: HWND,
    scale: f64,
    opacity: u8,
    dragging: bool,
    drag_cursor: PointI,
    drag_origin: PointI,
    always_on_top: bool,
}

pub unsafe fn register_class(instance: HINSTANCE) -> io::Result<()> {
    let class_name = wide_null(PIN_CLASS);
    let mut class: WNDCLASSEXW = zeroed();
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.style = CS_DBLCLKS;
    class.lpfnWndProc = Some(window_proc);
    class.hInstance = instance;
    class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    class.lpszClassName = class_name.as_ptr();

    if RegisterClassExW(&class) == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub unsafe fn create(
    instance: HINSTANCE,
    controller: HWND,
    bitmap: CapturedBitmap,
    capture_rect: RectI,
) -> io::Result<HWND> {
    let class_name = wide_null(PIN_CLASS);
    let title = wide_null("Rustpture image");
    let window = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP,
        capture_rect.left,
        capture_rect.top,
        bitmap.width(),
        bitmap.height(),
        null_mut(),
        null_mut(),
        instance,
        null(),
    );
    if window.is_null() {
        return Err(io::Error::last_os_error());
    }

    let state = Box::new(PinState {
        bitmap,
        controller,
        scale: 1.0,
        opacity: DEFAULT_OPACITY,
        dragging: false,
        drag_cursor: PointI::default(),
        drag_origin: PointI::default(),
        always_on_top: true,
    });
    SetWindowLongPtrW(window, GWLP_USERDATA, Box::into_raw(state) as isize);

    if SetLayeredWindowAttributes(window, 0, DEFAULT_OPACITY, LWA_ALPHA) == 0 {
        let error = io::Error::last_os_error();
        DestroyWindow(window);
        return Err(error);
    }
    if SetWindowPos(
        window,
        HWND_TOPMOST,
        capture_rect.left,
        capture_rect.top,
        capture_rect.width(),
        capture_rect.height(),
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    ) == 0
    {
        let error = io::Error::last_os_error();
        DestroyWindow(window);
        return Err(error);
    }

    ShowWindow(window, SW_SHOWNOACTIVATE);
    UpdateWindow(window);
    Ok(window)
}

unsafe fn state_ptr(window: HWND) -> *mut PinState {
    GetWindowLongPtrW(window, GWLP_USERDATA) as *mut PinState
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            paint(window);
            0
        }
        WM_ERASEBKGND => 1,
        WM_LBUTTONDOWN => {
            begin_drag(window);
            0
        }
        WM_MOUSEMOVE => {
            continue_drag(window);
            0
        }
        WM_LBUTTONUP => {
            end_drag(window);
            0
        }
        WM_CAPTURECHANGED => {
            let pointer = state_ptr(window);
            if !pointer.is_null() {
                (*pointer).dragging = false;
            }
            0
        }
        WM_LBUTTONDBLCLK => {
            set_scale_from_center(window, 1.0);
            0
        }
        WM_MOUSEWHEEL => {
            handle_mouse_wheel(window, wparam);
            0
        }
        WM_CONTEXTMENU => {
            show_context_menu(window, lparam);
            0
        }
        WM_DPICHANGED => {
            // Bitmap and window sizes are physical pixels. Ignoring the suggested
            // logical-size rectangle preserves the exact visible image region when
            // the window crosses monitors with different scale factors.
            0
        }
        WM_CLOSE => {
            DestroyWindow(window);
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

unsafe fn paint(window: HWND) {
    let mut paint: PAINTSTRUCT = zeroed();
    let dc = BeginPaint(window, &mut paint);
    let pointer = state_ptr(window);
    if !pointer.is_null() {
        let mut client: RECT = zeroed();
        GetClientRect(window, &mut client);
        (*pointer)
            .bitmap
            .paint(dc, client.right - client.left, client.bottom - client.top);
    }
    EndPaint(window, &paint);
}

unsafe fn begin_drag(window: HWND) {
    let pointer = state_ptr(window);
    if pointer.is_null() {
        return;
    }

    SetForegroundWindow(window);
    let mut cursor: POINT = zeroed();
    let mut rect: RECT = zeroed();
    if GetCursorPos(&mut cursor) == 0 || GetWindowRect(window, &mut rect) == 0 {
        return;
    }

    (*pointer).dragging = true;
    (*pointer).drag_cursor = PointI::new(cursor.x, cursor.y);
    (*pointer).drag_origin = PointI::new(rect.left, rect.top);
    SetCapture(window);
}

unsafe fn continue_drag(window: HWND) {
    let pointer = state_ptr(window);
    if pointer.is_null() || !(*pointer).dragging {
        return;
    }

    let drag_cursor = (*pointer).drag_cursor;
    let drag_origin = (*pointer).drag_origin;
    let mut cursor: POINT = zeroed();
    if GetCursorPos(&mut cursor) == 0 {
        return;
    }

    let x = drag_origin.x + cursor.x - drag_cursor.x;
    let y = drag_origin.y + cursor.y - drag_cursor.y;
    SetWindowPos(
        window,
        null_mut(),
        x,
        y,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

unsafe fn end_drag(window: HWND) {
    let pointer = state_ptr(window);
    if !pointer.is_null() {
        (*pointer).dragging = false;
    }
    ReleaseCapture();
}

unsafe fn handle_mouse_wheel(window: HWND, wparam: WPARAM) {
    let delta = ((wparam >> 16) & 0xffff) as u16 as i16 as i32;
    if delta == 0 {
        return;
    }
    let key_state = (wparam & 0xffff) as u32;

    if key_state & MK_CONTROL != 0 {
        let pointer = state_ptr(window);
        if !pointer.is_null() {
            let change = ((delta as f64 / 120.0) * 16.0).round() as i32;
            let opacity = ((*pointer).opacity as i32 + change)
                .clamp(MIN_OPACITY as i32, u8::MAX as i32) as u8;
            (*pointer).opacity = opacity;
            SetLayeredWindowAttributes(window, 0, opacity, LWA_ALPHA);
        }
        return;
    }

    let pointer = state_ptr(window);
    if pointer.is_null() {
        return;
    }
    let mut cursor: POINT = zeroed();
    if GetCursorPos(&mut cursor) == 0 {
        return;
    }
    let factor = 1.1_f64.powf(delta as f64 / 120.0);
    let scale = ((*pointer).scale * factor).clamp(MIN_SCALE, MAX_SCALE);
    apply_scale(window, scale, PointI::new(cursor.x, cursor.y));
}

unsafe fn set_scale_from_center(window: HWND, scale: f64) {
    let mut rect: RECT = zeroed();
    if GetWindowRect(window, &mut rect) == 0 {
        return;
    }
    let center = PointI::new((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2);
    apply_scale(window, scale.clamp(MIN_SCALE, MAX_SCALE), center);
}

unsafe fn apply_scale(window: HWND, scale: f64, anchor: PointI) {
    let pointer = state_ptr(window);
    if pointer.is_null() {
        return;
    }

    let mut old: RECT = zeroed();
    if GetWindowRect(window, &mut old) == 0 {
        return;
    }
    let old = RectI::new(old.left, old.top, old.right, old.bottom);
    let width = scaled_dimension((*pointer).bitmap.width(), scale);
    let height = scaled_dimension((*pointer).bitmap.height(), scale);
    let origin = zoom_around_point(old, anchor, width, height);
    (*pointer).scale = scale;

    SetWindowPos(
        window,
        null_mut(),
        origin.x,
        origin.y,
        width,
        height,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
    InvalidateRect(window, null(), 0);
}

unsafe fn show_context_menu(window: HWND, lparam: LPARAM) {
    let position = if lparam == -1 {
        let mut rect: RECT = zeroed();
        if GetWindowRect(window, &mut rect) == 0 {
            return;
        }
        PointI::new((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2)
    } else {
        PointI::new(
            (lparam as u32 & 0xffff) as u16 as i16 as i32,
            ((lparam as u32 >> 16) & 0xffff) as u16 as i16 as i32,
        )
    };

    let pointer = state_ptr(window);
    if pointer.is_null() {
        return;
    }
    let scale = (*pointer).scale;
    let always_on_top = (*pointer).always_on_top;
    let command = menu::show_pin_menu(window, position, scale, always_on_top);
    execute_menu_command(window, command);
}

unsafe fn execute_menu_command(window: HWND, command: u32) {
    match command {
        menu::CMD_RECAPTURE => {
            let pointer = state_ptr(window);
            if !pointer.is_null() {
                let controller = (*pointer).controller;
                PostMessageW(controller, WM_APP_BEGIN_CAPTURE, 0, 0);
            }
        }
        menu::CMD_COPY => {
            let pointer = state_ptr(window);
            if !pointer.is_null() {
                if let Err(error) = (*pointer).bitmap.copy_to_clipboard(window) {
                    show_error(
                        window,
                        &format!("画像をコピーできませんでした。\n\n{error}"),
                    );
                }
            }
        }
        menu::CMD_FIT => fit_to_monitor(window),
        menu::CMD_RESTORE_ONSCREEN => restore_onscreen(window),
        menu::CMD_ALWAYS_ON_TOP => toggle_always_on_top(window),
        menu::CMD_CLOSE => {
            DestroyWindow(window);
        }
        menu::CMD_EXIT_APP => {
            let pointer = state_ptr(window);
            if !pointer.is_null() {
                let controller = (*pointer).controller;
                PostMessageW(controller, WM_CLOSE, 0, 0);
            }
        }
        menu::CMD_ZOOM_25 => set_scale_from_center(window, 0.25),
        menu::CMD_ZOOM_50 => set_scale_from_center(window, 0.50),
        menu::CMD_ZOOM_75 => set_scale_from_center(window, 0.75),
        menu::CMD_ZOOM_100 => set_scale_from_center(window, 1.00),
        menu::CMD_ZOOM_150 => set_scale_from_center(window, 1.50),
        menu::CMD_ZOOM_200 => set_scale_from_center(window, 2.00),
        _ => {}
    }
}

unsafe fn fit_to_monitor(window: HWND) {
    let pointer = state_ptr(window);
    if pointer.is_null() {
        return;
    }

    let monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
    if monitor.is_null() {
        return;
    }
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) == 0 {
        return;
    }

    let source_width = (*pointer).bitmap.width();
    let source_height = (*pointer).bitmap.height();
    let available_width = info.rcWork.right - info.rcWork.left;
    let available_height = info.rcWork.bottom - info.rcWork.top;
    let scale = (available_width as f64 / source_width as f64)
        .min(available_height as f64 / source_height as f64)
        .clamp(MIN_SCALE, MAX_SCALE);
    let width = scaled_dimension(source_width, scale);
    let height = scaled_dimension(source_height, scale);
    let x = info.rcWork.left + (available_width - width) / 2;
    let y = info.rcWork.top + (available_height - height) / 2;
    (*pointer).scale = scale;

    SetWindowPos(
        window,
        null_mut(),
        x,
        y,
        width,
        height,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
    InvalidateRect(window, null(), 0);
}

unsafe fn restore_onscreen(window: HWND) {
    let monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
    if monitor.is_null() {
        return;
    }
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) == 0 {
        return;
    }

    let mut rect: RECT = zeroed();
    if GetWindowRect(window, &mut rect) == 0 {
        return;
    }
    let window_rect = RectI::new(rect.left, rect.top, rect.right, rect.bottom);
    let work_rect = RectI::new(
        info.rcWork.left,
        info.rcWork.top,
        info.rcWork.right,
        info.rcWork.bottom,
    );
    let origin = window_rect.clamp_origin_inside(work_rect, 32);
    SetWindowPos(
        window,
        null_mut(),
        origin.x,
        origin.y,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

unsafe fn toggle_always_on_top(window: HWND) {
    let pointer = state_ptr(window);
    if pointer.is_null() {
        return;
    }
    (*pointer).always_on_top = !(*pointer).always_on_top;
    let insert_after = if (*pointer).always_on_top {
        HWND_TOPMOST
    } else {
        HWND_NOTOPMOST
    };
    SetWindowPos(
        window,
        insert_after,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
}

unsafe fn show_error(owner: HWND, message: &str) {
    let title = wide_null("Rustpture");
    let message = wide_null(message);
    MessageBoxW(
        owner,
        message.as_ptr(),
        title.as_ptr(),
        MB_OK | MB_ICONERROR,
    );
}
