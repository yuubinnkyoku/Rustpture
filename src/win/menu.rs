use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, HMENU, MF_CHECKED, MF_POPUP, MF_SEPARATOR,
    MF_STRING, PostMessageW, SetForegroundWindow, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TPM_TOPALIGN, TrackPopupMenuEx, WM_NULL,
};

use crate::geometry::PointI;

use super::wide::wide_null;

pub const CMD_RECAPTURE: u32 = 100;
pub const CMD_COPY: u32 = 101;
pub const CMD_FIT: u32 = 102;
pub const CMD_RESTORE_ONSCREEN: u32 = 103;
pub const CMD_ALWAYS_ON_TOP: u32 = 104;
pub const CMD_CLOSE: u32 = 105;
pub const CMD_EXIT_APP: u32 = 106;

pub const CMD_ZOOM_25: u32 = 125;
pub const CMD_ZOOM_50: u32 = 150;
pub const CMD_ZOOM_75: u32 = 175;
pub const CMD_ZOOM_100: u32 = 200;
pub const CMD_ZOOM_150: u32 = 250;
pub const CMD_ZOOM_200: u32 = 300;

pub unsafe fn show_pin_menu(
    window: HWND,
    position: PointI,
    scale: f64,
    always_on_top: bool,
) -> u32 {
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return 0;
    }

    append_text(menu, CMD_RECAPTURE, "再キャプチャ(&R)", false);
    append_text(menu, CMD_COPY, "画像をクリップボードへコピー(&C)", false);
    AppendMenuW(menu, MF_SEPARATOR, 0, null());

    let zoom_menu = CreatePopupMenu();
    if !zoom_menu.is_null() {
        append_text(zoom_menu, CMD_ZOOM_25, "25%", near(scale, 0.25));
        append_text(zoom_menu, CMD_ZOOM_50, "50%", near(scale, 0.50));
        append_text(zoom_menu, CMD_ZOOM_75, "75%", near(scale, 0.75));
        append_text(zoom_menu, CMD_ZOOM_100, "100%", near(scale, 1.00));
        append_text(zoom_menu, CMD_ZOOM_150, "150%", near(scale, 1.50));
        append_text(zoom_menu, CMD_ZOOM_200, "200%", near(scale, 2.00));
        if !append_submenu(menu, zoom_menu, "ズーム(&Z)") {
            DestroyMenu(zoom_menu);
        }
    }

    append_text(menu, CMD_FIT, "画面に合わせる(&F)", false);
    append_text(menu, CMD_ALWAYS_ON_TOP, "常に手前に表示(&T)", always_on_top);
    append_text(menu, CMD_RESTORE_ONSCREEN, "画面内に戻す(&D)", false);
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    append_text(menu, CMD_CLOSE, "この画像を閉じる(&Q)\tAlt+F4", false);
    append_text(menu, CMD_EXIT_APP, "Rustptureを終了(&X)", false);

    SetForegroundWindow(window);
    let command = TrackPopupMenuEx(
        menu,
        TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
        position.x,
        position.y,
        window,
        null_mut(),
    ) as u32;
    DestroyMenu(menu);

    // Required by TrackPopupMenu's foreground-window behavior. It prevents the
    // next click from being swallowed after the popup closes.
    PostMessageW(window, WM_NULL, 0, 0);
    command
}

fn near(value: f64, expected: f64) -> bool {
    (value - expected).abs() < 0.005
}

unsafe fn append_text(menu: HMENU, command: u32, text: &str, checked: bool) -> bool {
    let text = wide_null(text);
    let flags = MF_STRING | if checked { MF_CHECKED } else { 0 };
    AppendMenuW(menu, flags, command as usize, text.as_ptr()) != 0
}

unsafe fn append_submenu(menu: HMENU, submenu: HMENU, text: &str) -> bool {
    let text = wide_null(text);
    AppendMenuW(menu, MF_POPUP | MF_STRING, submenu as usize, text.as_ptr()) != 0
}
