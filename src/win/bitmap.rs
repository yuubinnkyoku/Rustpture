use std::io;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{HANDLE, HWND};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CAPTUREBLT, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
    HALFTONE, HBITMAP, HDC, HGDIOBJ, ReleaseDC, SRCCOPY, SelectObject, SetBrushOrgEx,
    SetStretchBltMode, StretchBlt,
};
use windows_sys::Win32::System::DataExchange::{
    CF_BITMAP, CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{CopyImage, IMAGE_BITMAP, LR_CREATEDIBSECTION};

use crate::geometry::RectI;

pub struct CapturedBitmap {
    handle: HBITMAP,
    width: i32,
    height: i32,
}

impl CapturedBitmap {
    pub unsafe fn capture(rect: RectI) -> io::Result<Self> {
        if rect.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "capture rectangle is empty",
            ));
        }

        let screen_dc = ScreenDc::acquire(null_mut())?;
        let memory_dc = MemoryDc::create(screen_dc.handle())?;
        let bitmap = CreateCompatibleBitmap(screen_dc.handle(), rect.width(), rect.height());
        if bitmap.is_null() {
            return Err(io::Error::last_os_error());
        }

        let old_object = SelectObject(memory_dc.handle(), bitmap as HGDIOBJ);
        let copied = BitBlt(
            memory_dc.handle(),
            0,
            0,
            rect.width(),
            rect.height(),
            screen_dc.handle(),
            rect.left,
            rect.top,
            SRCCOPY | CAPTUREBLT,
        );
        SelectObject(memory_dc.handle(), old_object);

        if copied == 0 {
            DeleteObject(bitmap as HGDIOBJ);
            return Err(io::Error::last_os_error());
        }

        Ok(Self {
            handle: bitmap,
            width: rect.width(),
            height: rect.height(),
        })
    }

    pub const fn width(&self) -> i32 {
        self.width
    }

    pub const fn height(&self) -> i32 {
        self.height
    }

    pub unsafe fn paint(&self, destination: HDC, width: i32, height: i32) {
        if destination.is_null() || width <= 0 || height <= 0 {
            return;
        }

        let Ok(memory_dc) = MemoryDc::create(destination) else {
            return;
        };
        let old_object = SelectObject(memory_dc.handle(), self.handle as HGDIOBJ);

        SetStretchBltMode(destination, HALFTONE);
        SetBrushOrgEx(destination, 0, 0, null_mut());
        StretchBlt(
            destination,
            0,
            0,
            width,
            height,
            memory_dc.handle(),
            0,
            0,
            self.width,
            self.height,
            SRCCOPY,
        );

        SelectObject(memory_dc.handle(), old_object);
    }

    pub unsafe fn copy_to_clipboard(&self, owner: HWND) -> io::Result<()> {
        let copy = CopyImage(
            self.handle as HANDLE,
            IMAGE_BITMAP,
            0,
            0,
            LR_CREATEDIBSECTION,
        );
        if copy.is_null() {
            return Err(io::Error::last_os_error());
        }

        if OpenClipboard(owner) == 0 {
            DeleteObject(copy as HGDIOBJ);
            return Err(io::Error::last_os_error());
        }

        let result = if EmptyClipboard() == 0 {
            Err(io::Error::last_os_error())
        } else if SetClipboardData(CF_BITMAP, copy).is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        };

        CloseClipboard();
        if result.is_err() {
            DeleteObject(copy as HGDIOBJ);
        }
        result
    }
}

impl Drop for CapturedBitmap {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                DeleteObject(self.handle as HGDIOBJ);
            }
        }
    }
}

struct ScreenDc {
    owner: HWND,
    handle: HDC,
}

impl ScreenDc {
    unsafe fn acquire(owner: HWND) -> io::Result<Self> {
        let handle = GetDC(owner);
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { owner, handle })
    }

    const fn handle(&self) -> HDC {
        self.handle
    }
}

impl Drop for ScreenDc {
    fn drop(&mut self) {
        unsafe {
            ReleaseDC(self.owner, self.handle);
        }
    }
}

struct MemoryDc(HDC);

impl MemoryDc {
    unsafe fn create(compatible_with: HDC) -> io::Result<Self> {
        let handle = CreateCompatibleDC(compatible_with);
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(handle))
    }

    const fn handle(&self) -> HDC {
        self.0
    }
}

impl Drop for MemoryDc {
    fn drop(&mut self) {
        unsafe {
            DeleteDC(self.0);
        }
    }
}
