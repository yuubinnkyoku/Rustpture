use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CWPSTRUCT, CallNextHookEx, GetClassNameW, SetWindowsHookExW, WH_CALLWNDPROC, WM_CREATE,
    WM_SETTINGCHANGE, WM_THEMECHANGED,
};

use super::PIN_CLASS;
use super::wide::wide_null;

// DWMWA_USE_IMMERSIVE_DARK_MODE. The documented ABI value is 20 on Windows 11.
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
const CLASS_NAME_CAPACITY: usize = 64;

pub unsafe fn install_current_thread_hook() {
    // The hook intentionally lives for the GUI thread's entire lifetime. Windows
    // removes thread hooks automatically when the owning thread exits.
    let _ = unsafe {
        SetWindowsHookExW(
            WH_CALLWNDPROC,
            Some(call_wnd_proc),
            null_mut(),
            GetCurrentThreadId(),
        )
    };
}

unsafe extern "system" fn call_wnd_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && lparam != 0 {
        let info = unsafe { &*(lparam as *const CWPSTRUCT) };
        if matches!(info.message, WM_CREATE | WM_SETTINGCHANGE | WM_THEMECHANGED)
            && unsafe { is_pin_window(info.hwnd) }
        {
            unsafe { apply_title_bar_theme(info.hwnd) };
        }
    }

    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

unsafe fn is_pin_window(window: HWND) -> bool {
    let mut class_name = [0u16; CLASS_NAME_CAPACITY];
    let length = unsafe {
        GetClassNameW(
            window,
            class_name.as_mut_ptr(),
            class_name.len().try_into().unwrap_or(i32::MAX),
        )
    };
    if length <= 0 {
        return false;
    }

    let expected = wide_null(PIN_CLASS);
    class_name[..length as usize] == expected[..expected.len() - 1]
}

unsafe fn apply_title_bar_theme(window: HWND) {
    let value: i32 = if unsafe { system_prefers_dark_apps() } {
        1
    } else {
        0
    };

    // Failure is deliberately non-fatal. Unsupported Windows versions simply
    // keep the normal light title bar while the rest of Rustpture keeps working.
    let _ = unsafe {
        DwmSetWindowAttribute(
            window,
            DWMWA_USE_IMMERSIVE_DARK_MODE as _,
            (&value as *const i32).cast::<c_void>(),
            size_of::<i32>() as u32,
        )
    };
}

unsafe fn system_prefers_dark_apps() -> bool {
    // This is a raw string, so registry separators are written as a single '\\'.
    let path = wide_null(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let name = wide_null("AppsUseLightTheme");
    let mut value = 1u32;
    let mut bytes = size_of::<u32>() as u32;

    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&mut value as *mut u32).cast::<c_void>(),
            &mut bytes,
        )
    };

    status == 0 && value == 0
}
