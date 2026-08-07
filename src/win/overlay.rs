use std::io;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::DwmFlush;
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, FrameRect, HBRUSH, HGDIOBJ,
    InvalidateRect, PAINTSTRUCT, ScreenToClient, UpdateWindow,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetActiveWindow, SetCapture, SetFocus, VK_ESCAPE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetClientRect,
    GetCursorPos, GetSystemMetrics, GetWindowLongPtrW, HWND_TOPMOST, IDC_CROSS, IsWindowVisible,
    LWA_ALPHA, LWA_COLORKEY, LoadCursorW, MB_ICONERROR, MB_OK, MessageBoxW, MoveWindow,
    RDW_NOERASE, RDW_UPDATENOW, RedrawWindow, RegisterClassExW, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_HIDE, SWP_SHOWWINDOW,
    SetForegroundWindow, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    WM_CANCELMODE, WM_CAPTURECHANGED, WM_ERASEBKGND, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCDESTROY, WM_PAINT, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::geometry::{PointI, RectI};

use super::OVERLAY_CLASS;
use super::bitmap::CapturedBitmap;
use super::pin;
use super::wide::wide_null;

const OVERLAY_ALPHA: u8 = 112;
const COLOR_KEY: u32 = rgb(255, 0, 255);
const DIM_COLOR: u32 = rgb(0, 0, 0);
const BORDER_COLOR: u32 = rgb(255, 255, 255);
const BORDER_WIDTH: i32 = 2;

const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | ((green as u32) << 8) | ((blue as u32) << 16)
}

struct OverlayState {
    instance: HINSTANCE,
    controller: HWND,
    virtual_rect: RectI,
    active: bool,
    dragging: bool,
    anchor: PointI,
    current: PointI,
    dim_brush: HBRUSH,
    key_brush: HBRUSH,
    border_brush: HBRUSH,
}

impl Drop for OverlayState {
    fn drop(&mut self) {
        unsafe {
            if !self.dim_brush.is_null() {
                DeleteObject(self.dim_brush as HGDIOBJ);
            }
            if !self.key_brush.is_null() {
                DeleteObject(self.key_brush as HGDIOBJ);
            }
            if !self.border_brush.is_null() {
                DeleteObject(self.border_brush as HGDIOBJ);
            }
        }
    }
}

pub unsafe fn register_class(instance: HINSTANCE) -> io::Result<()> {
    let class_name = wide_null(OVERLAY_CLASS);
    let mut class: WNDCLASSEXW = zeroed();
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.style = CS_DBLCLKS;
    class.lpfnWndProc = Some(window_proc);
    class.hInstance = instance;
    class.hCursor = LoadCursorW(null_mut(), IDC_CROSS);
    class.lpszClassName = class_name.as_ptr();

    if RegisterClassExW(&class) == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub unsafe fn create(instance: HINSTANCE, controller: HWND) -> io::Result<HWND> {
    let virtual_rect = current_virtual_rect();
    let class_name = wide_null(OVERLAY_CLASS);
    let title = wide_null("Rustpture capture overlay");
    let window = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP,
        virtual_rect.left,
        virtual_rect.top,
        virtual_rect.width(),
        virtual_rect.height(),
        null_mut(),
        null_mut(),
        instance,
        null(),
    );
    if window.is_null() {
        return Err(io::Error::last_os_error());
    }

    let dim_brush = CreateSolidBrush(DIM_COLOR);
    let key_brush = CreateSolidBrush(COLOR_KEY);
    let border_brush = CreateSolidBrush(BORDER_COLOR);
    if dim_brush.is_null() || key_brush.is_null() || border_brush.is_null() {
        if !dim_brush.is_null() {
            DeleteObject(dim_brush as HGDIOBJ);
        }
        if !key_brush.is_null() {
            DeleteObject(key_brush as HGDIOBJ);
        }
        if !border_brush.is_null() {
            DeleteObject(border_brush as HGDIOBJ);
        }
        DestroyWindow(window);
        return Err(io::Error::other("GDI brushes could not be created"));
    }

    let state = Box::new(OverlayState {
        instance,
        controller,
        virtual_rect,
        active: false,
        dragging: false,
        anchor: PointI::default(),
        current: PointI::default(),
        dim_brush,
        key_brush,
        border_brush,
    });
    SetWindowLongPtrW(window, GWLP_USERDATA, Box::into_raw(state) as isize);

    if SetLayeredWindowAttributes(window, COLOR_KEY, OVERLAY_ALPHA, LWA_ALPHA | LWA_COLORKEY) == 0 {
        let error = io::Error::last_os_error();
        DestroyWindow(window);
        return Err(error);
    }
    Ok(window)
}

pub unsafe fn begin_capture(window: HWND) {
    let pointer = state_ptr(window);
    if pointer.is_null() {
        return;
    }

    let virtual_rect = current_virtual_rect();
    (*pointer).virtual_rect = virtual_rect;
    (*pointer).active = true;
    (*pointer).dragging = false;
    (*pointer).anchor = PointI::default();
    (*pointer).current = PointI::default();

    SetWindowPos(
        window,
        HWND_TOPMOST,
        virtual_rect.left,
        virtual_rect.top,
        virtual_rect.width(),
        virtual_rect.height(),
        SWP_SHOWWINDOW,
    );
    SetForegroundWindow(window);
    SetActiveWindow(window);
    SetFocus(window);
    InvalidateRect(window, null(), 0);
    UpdateWindow(window);
}

pub unsafe fn refresh_geometry(window: HWND) {
    let pointer = state_ptr(window);
    if pointer.is_null() {
        return;
    }

    if (*pointer).active {
        cancel_capture(window);
    }
    let virtual_rect = current_virtual_rect();
    (*pointer).virtual_rect = virtual_rect;
    MoveWindow(
        window,
        virtual_rect.left,
        virtual_rect.top,
        virtual_rect.width(),
        virtual_rect.height(),
        1,
    );
}

unsafe fn state_ptr(window: HWND) -> *mut OverlayState {
    GetWindowLongPtrW(window, GWLP_USERDATA) as *mut OverlayState
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
            let point = cursor_client_point(window).unwrap_or_else(|| point_from_lparam(lparam));
            let pointer = state_ptr(window);
            if !pointer.is_null() {
                (*pointer).dragging = true;
                (*pointer).anchor = point;
                (*pointer).current = point;
                SetCapture(window);
            }
            0
        }
        WM_MOUSEMOVE => {
            let pointer = state_ptr(window);
            if !pointer.is_null() && (*pointer).dragging {
                let point = point_from_lparam(lparam);
                if point != (*pointer).current {
                    let old_selection = RectI::from_points((*pointer).anchor, (*pointer).current);
                    (*pointer).current = point;
                    let new_selection = RectI::from_points((*pointer).anchor, (*pointer).current);
                    redraw_selection_delta(window, old_selection, new_selection);
                }
            }
            0
        }
        WM_LBUTTONUP => {
            let point = cursor_client_point(window).unwrap_or_else(|| point_from_lparam(lparam));
            finish_selection(window, point);
            0
        }
        WM_KEYDOWN => {
            if wparam as u32 == VK_ESCAPE as u32 {
                cancel_capture(window);
                return 0;
            }
            DefWindowProcW(window, message, wparam, lparam)
        }
        WM_KILLFOCUS => {
            let pointer = state_ptr(window);
            if !pointer.is_null() && (*pointer).active {
                cancel_capture(window);
            }
            0
        }
        WM_CANCELMODE | WM_CAPTURECHANGED => {
            let pointer = state_ptr(window);
            if !pointer.is_null() && (*pointer).dragging {
                let old_selection = RectI::from_points((*pointer).anchor, (*pointer).current);
                (*pointer).dragging = false;
                redraw_selection_delta(window, old_selection, RectI::default());
            }
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
        // During dragging only the pixels whose selection state changed are invalidated.
        // Painting rcPaint instead of the whole virtual desktop keeps pointer tracking
        // responsive even on large multi-monitor setups.
        if paint.rcPaint.right > paint.rcPaint.left && paint.rcPaint.bottom > paint.rcPaint.top {
            FillRect(dc, &paint.rcPaint, (*pointer).dim_brush);
        }

        if (*pointer).dragging {
            let selection = RectI::from_points((*pointer).anchor, (*pointer).current);
            if !selection.is_empty() {
                let rect = to_win_rect(selection);
                FillRect(dc, &rect, (*pointer).key_brush);
                FrameRect(dc, &rect, (*pointer).border_brush);

                let inner = RECT {
                    left: rect.left + 1,
                    top: rect.top + 1,
                    right: rect.right - 1,
                    bottom: rect.bottom - 1,
                };
                if inner.right > inner.left && inner.bottom > inner.top {
                    FrameRect(dc, &inner, (*pointer).border_brush);
                }
            }
        }
    }

    EndPaint(window, &paint);
}

unsafe fn redraw_selection_delta(window: HWND, old: RectI, new: RectI) {
    // Most mouse moves alter only one or two thin strips of the selection. Invalidating
    // the symmetric difference avoids repainting the potentially multi-megapixel
    // overlap on every WM_MOUSEMOVE.
    invalidate_difference(window, old, new);
    invalidate_difference(window, new, old);
    invalidate_frame(window, old);
    invalidate_frame(window, new);

    // WM_MOUSEMOVE messages can arrive faster than ordinary WM_PAINT dispatch. Flush
    // only the already-invalid update region now so the border stays under the cursor.
    RedrawWindow(window, null(), null_mut(), RDW_NOERASE | RDW_UPDATENOW);
}

unsafe fn invalidate_difference(window: HWND, rect: RectI, other: RectI) {
    if rect.is_empty() {
        return;
    }

    let intersection = intersect(rect, other);
    if intersection.is_empty() {
        invalidate(window, rect);
        return;
    }

    invalidate(window, RectI::new(rect.left, rect.top, rect.right, intersection.top));
    invalidate(window, RectI::new(rect.left, intersection.bottom, rect.right, rect.bottom));
    invalidate(
        window,
        RectI::new(rect.left, intersection.top, intersection.left, intersection.bottom),
    );
    invalidate(
        window,
        RectI::new(intersection.right, intersection.top, rect.right, intersection.bottom),
    );
}

unsafe fn invalidate_frame(window: HWND, rect: RectI) {
    if rect.is_empty() {
        return;
    }

    let width = BORDER_WIDTH.min(rect.width()).max(1);
    let height = BORDER_WIDTH.min(rect.height()).max(1);
    invalidate(window, RectI::new(rect.left, rect.top, rect.right, rect.top + height));
    invalidate(
        window,
        RectI::new(rect.left, rect.bottom - height, rect.right, rect.bottom),
    );
    invalidate(window, RectI::new(rect.left, rect.top, rect.left + width, rect.bottom));
    invalidate(
        window,
        RectI::new(rect.right - width, rect.top, rect.right, rect.bottom),
    );
}

unsafe fn invalidate(window: HWND, rect: RectI) {
    if rect.is_empty() {
        return;
    }
    let rect = to_win_rect(rect);
    InvalidateRect(window, &rect, 0);
}

fn intersect(a: RectI, b: RectI) -> RectI {
    RectI::new(
        a.left.max(b.left),
        a.top.max(b.top),
        a.right.min(b.right),
        a.bottom.min(b.bottom),
    )
}

fn to_win_rect(rect: RectI) -> RECT {
    RECT {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}

unsafe fn finish_selection(window: HWND, point: PointI) {
    let pointer = state_ptr(window);
    if pointer.is_null() || !(*pointer).dragging {
        return;
    }

    (*pointer).current = point;
    (*pointer).dragging = false;
    (*pointer).active = false;
    let local_rect = RectI::from_points((*pointer).anchor, (*pointer).current);
    let instance = (*pointer).instance;
    let controller = (*pointer).controller;
    let virtual_rect = (*pointer).virtual_rect;

    ReleaseCapture();
    ShowWindow(window, SW_HIDE);

    if local_rect.width() < 2 || local_rect.height() < 2 {
        return;
    }

    // Ensure the compositor has removed the selection overlay before copying
    // screen pixels, otherwise the dimming layer can appear in the capture.
    DwmFlush();
    let screen_rect = local_rect.translated(virtual_rect.left, virtual_rect.top);
    match CapturedBitmap::capture(screen_rect) {
        Ok(bitmap) => {
            if let Err(error) = pin::create(instance, controller, bitmap, screen_rect) {
                show_error(
                    window,
                    &format!("画像ウィンドウを作成できませんでした。\n\n{error}"),
                );
            }
        }
        Err(error) => {
            show_error(
                window,
                &format!("画面をキャプヅャできませんでした。\n\n{error}"),
            );
        }
    }
}

unsafe fn cancel_capture(window: HWND) {
    let pointer = state_ptr(window);
    if !pointer.is_null() {
        (*pointer).active = false;
        (*pointer).dragging = false;
    }
    ReleaseCapture();
    if IsWindowVisible(window) != 0 {
        ShowWindow(window, SW_HIDE);
    }
}

fn current_virtual_rect() -> RectI {
    unsafe {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1);
        RectI::new(left, top, left + width, top + height)
    }
}

unsafe fn cursor_client_point(window: HWND) -> Option<PointI> {
    let mut point: POINT = zeroed();
    if GetCursorPos(&mut point) == 0 || ScreenToClient(window, &mut point) == 0 {
        return None;
    }
    Some(PointI::new(point.x, point.y))
}

fn point_from_lparam(lparam: LPARAM) -> PointI {
    PointI::new(
        (lparam as u32 & 0xffff) as u16 as i16 as i32,
        ((lparam as u32 >> 16) & 0xffff) as u16 as i16 as i32,
    )
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
