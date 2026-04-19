//! Win32 window creation and management for the canvas overlay.

use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

pub fn wide_string(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn create_canvas_window(wndproc: WNDPROC) -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class_name = wide_string("WinCanvasClass");

        let bg_brush: HBRUSH = CreateSolidBrush(COLORREF(0x00201820));

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: wndproc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance.into(),
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: bg_brush,
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hIconSm: HICON::default(),
        };

        RegisterClassExW(&wc);

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);

        let title = wide_string("Win Canvas");
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            0,
            0,
            screen_w,
            screen_h,
            None,
            None,
            hinstance,
            None,
        )?;

        set_window_alpha(hwnd, 0);

        let _ = UpdateWindow(hwnd);
        Ok(hwnd)
    }
}

pub fn create_hud_window(wndproc: WNDPROC) -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class_name = wide_string("WinCanvasHUD");

        let bg_brush: HBRUSH = CreateSolidBrush(COLORREF(0x00201820));

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: wndproc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance.into(),
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: bg_brush,
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hIconSm: HICON::default(),
        };

        RegisterClassExW(&wc);

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let hud_w = 120;
        let hud_h = 120;

        let title = wide_string("WC HUD");
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            screen_w - hud_w - 20,
            screen_h - hud_h - 60,
            hud_w,
            hud_h,
            None,
            None,
            hinstance,
            None,
        )?;

        set_window_alpha(hwnd, 220);

        Ok(hwnd)
    }
}

pub fn set_window_alpha(hwnd: HWND, alpha: u8) {
    unsafe {
        const LWA_ALPHA: LAYERED_WINDOW_ATTRIBUTES_FLAGS = LAYERED_WINDOW_ATTRIBUTES_FLAGS(2);
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
    }
}

pub fn capture_screen(screen_w: i32, screen_h: i32) -> HBITMAP {
    unsafe {
        let hdc_screen = GetDC(HWND::default());
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbm = CreateCompatibleBitmap(hdc_screen, screen_w, screen_h);
        let old = SelectObject(hdc_mem, hbm);
        let _ = BitBlt(hdc_mem, 0, 0, screen_w, screen_h, hdc_screen, 0, 0, SRCCOPY);
        SelectObject(hdc_mem, old);
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(HWND::default(), hdc_screen);
        hbm
    }
}

pub fn free_bitmap(hbm: HBITMAP) {
    if !hbm.0.is_null() {
        unsafe {
            let _ = DeleteObject(hbm);
        }
    }
}

pub fn show_canvas(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}

pub fn hide_canvas(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

pub fn show_hud(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
}

pub fn hide_hud(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

pub fn get_screen_size() -> (i32, i32) {
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        (w, h)
    }
}

pub fn activate_window(hwnd: HWND) {
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
}
