use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CWPSTRUCT, CallNextHookEx, FindWindowW, GetClassNameW, PostMessageW, SetWindowsHookExW,
    WH_CALLWNDPROC, WM_CREATE, WM_NCDESTROY, WM_SHOWWINDOW,
};

use super::wide::wide_null;
use super::{
    CONTROLLER_CLASS, OVERLAY_CLASS, PIN_CLASS, WM_APP_CAPTURE_FINISHED, WM_APP_PIN_CLOSED,
    WM_APP_PIN_OPENED,
};

const CLASS_NAME_CAPACITY: usize = 64;

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
            if info.message == WM_CREATE {
                notify_controller(WM_APP_PIN_OPENED);
            } else if info.message == WM_NCDESTROY {
                notify_controller(WM_APP_PIN_CLOSED);
            }
        } else if class.as_deref() == Some(OVERLAY_CLASS)
            && info.message == WM_SHOWWINDOW
            && info.wParam == 0
        {
            notify_controller(WM_APP_CAPTURE_FINISHED);
        }
    }

    CallNextHookEx(null_mut(), code, wparam, lparam)
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
