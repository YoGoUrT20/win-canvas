//! Win-Canvas: An infinite canvas for managing open windows.
//!
//! Press Ctrl+Alt+Space to toggle the canvas overlay.
//! Features: wallpaper background, fade-in animation, persistent layout.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod canvas;
mod dwm;
mod enumerate;
mod hotkey;
mod input;
mod state;
mod window;

use std::cell::RefCell;
use std::fs;
use std::io::Write;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

use canvas::{Canvas, SourceInfo};
use dwm::Thumbnail;

// Animation constants
const TIMER_FADE_IN: usize = 1;
const ANIM_INTERVAL_MS: u32 = 16;
const ANIM_STEPS: u32 = 18;
const TARGET_ALPHA: u8 = 240;

fn ease_out(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

/// Simple debug logger
fn log_debug(msg: &str) {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let log_dir = std::path::PathBuf::from(&appdata).join("win-canvas");
    let _ = fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("debug.log");
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = writeln!(f, "{}", msg);
    }
    #[cfg(debug_assertions)]
    eprintln!("{}", msg);
}

struct AppState {
    canvas: Canvas,
    thumbnails: Vec<Thumbnail>,
    visible: bool,
    canvas_hwnd: HWND,
    drag_moved: bool,
    click_target: Option<usize>,
    bg_bitmap: HBITMAP,
    anim_step: u32,
    anim_active: bool,
    current_alpha: u8,
}

impl AppState {
    fn new(screen_w: i32, screen_h: i32) -> Self {
        Self {
            canvas: Canvas::new(screen_w, screen_h),
            thumbnails: Vec::new(),
            visible: false,
            canvas_hwnd: HWND::default(),
            drag_moved: false,
            click_target: None,
            bg_bitmap: HBITMAP::default(),
            anim_step: 0,
            anim_active: false,
            current_alpha: 0,
        }
    }

    fn refresh(&mut self) {
        self.thumbnails.clear();
        self.canvas.windows.clear();

        let windows = enumerate::enumerate_windows();
        log_debug(&format!("Enumerated {} windows", windows.len()));

        let mut source_infos = Vec::new();

        for winfo in &windows {
            if winfo.hwnd == self.canvas_hwnd {
                continue;
            }
            match Thumbnail::register(self.canvas_hwnd, winfo.hwnd) {
                Ok(thumb) => {
                    let idx = self.thumbnails.len();
                    source_infos.push(SourceInfo {
                        thumb_index: idx,
                        width: thumb.source_width,
                        height: thumb.source_height,
                        title: winfo.title.clone(),
                    });
                    self.thumbnails.push(thumb);
                }
                Err(e) => {
                    log_debug(&format!(
                        "Failed to register thumbnail for '{}': {:?}",
                        winfo.title, e
                    ));
                }
            }
        }

        log_debug(&format!("Registered {} thumbnails", self.thumbnails.len()));

        let saved = state::load_state();
        self.canvas.layout_grid(&source_infos, saved.as_ref());
        self.update_all_thumbnails();
    }

    fn update_all_thumbnails(&self) {
        let scale = if self.anim_active {
            let t = self.anim_step as f64 / ANIM_STEPS as f64;
            0.92 + 0.08 * ease_out(t)
        } else {
            1.0
        };

        for cw in &self.canvas.windows {
            if cw.thumb_index < self.thumbnails.len() {
                let rect = self.canvas.canvas_to_screen_rect(cw, scale);
                if rect.right > 0
                    && rect.bottom > 0
                    && rect.left < self.canvas.screen_w
                    && rect.top < self.canvas.screen_h
                {
                    let _ = self.thumbnails[cw.thumb_index]
                        .update(rect, self.current_alpha, false);
                } else {
                    let _ = self.thumbnails[cw.thumb_index].hide();
                }
            }
        }
    }

    fn toggle(&mut self) {
        log_debug(&format!("Toggle called, visible={}", self.visible));
        if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    fn show(&mut self) {
        log_debug("show() called");
        self.visible = true;

        // Capture the current screen as background
        if !self.bg_bitmap.0.is_null() {
            window::free_bitmap(self.bg_bitmap);
            self.bg_bitmap = HBITMAP::default();
        }
        self.bg_bitmap =
            window::capture_screen(self.canvas.screen_w, self.canvas.screen_h);
        log_debug(&format!("Screen captured: bitmap={:?}", self.bg_bitmap.0));

        self.current_alpha = 0;
        window::set_window_alpha(self.canvas_hwnd, 0);

        self.refresh();
        window::show_canvas(self.canvas_hwnd);

        // Start fade-in animation
        self.anim_step = 0;
        self.anim_active = true;
        unsafe {
            SetTimer(self.canvas_hwnd, TIMER_FADE_IN, ANIM_INTERVAL_MS, None);
        }
        log_debug("show() complete, animation started");
    }

    fn hide(&mut self) {
        log_debug("hide() called");
        let saved = self.canvas.to_saved_state();
        state::save_state(&saved);

        self.visible = false;
        self.anim_active = false;
        unsafe {
            let _ = KillTimer(self.canvas_hwnd, TIMER_FADE_IN);
        }

        for thumb in &self.thumbnails {
            let _ = thumb.hide();
        }
        window::hide_canvas(self.canvas_hwnd);
    }

    fn tick_animation(&mut self) {
        self.anim_step += 1;
        if self.anim_step >= ANIM_STEPS {
            self.anim_step = ANIM_STEPS;
            self.anim_active = false;
            unsafe {
                let _ = KillTimer(self.canvas_hwnd, TIMER_FADE_IN);
            }
        }

        let t = self.anim_step as f64 / ANIM_STEPS as f64;
        let eased = ease_out(t);
        self.current_alpha = (TARGET_ALPHA as f64 * eased) as u8;

        window::set_window_alpha(self.canvas_hwnd, self.current_alpha);
        self.update_all_thumbnails();

        unsafe {
            let _ = InvalidateRect(self.canvas_hwnd, None, true);
        }
    }
}

thread_local! {
    static APP_STATE: RefCell<Option<AppState>> = RefCell::new(None);
}

fn with_state<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AppState) -> R,
{
    APP_STATE.with(|cell| {
        if let Ok(mut opt) = cell.try_borrow_mut() {
            opt.as_mut().map(|state| f(state))
        } else {
            // State is currently borrowed (re-entrant call), skip
            None
        }
    })
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            if wparam.0 as i32 == hotkey::HOTKEY_TOGGLE_CANVAS {
                with_state(|s| s.toggle());
            }
            LRESULT(0)
        }

        WM_TIMER => {
            if wparam.0 == TIMER_FADE_IN {
                with_state(|s| s.tick_animation());
            }
            LRESULT(0)
        }

        WM_KEYDOWN => {
            if wparam.0 as u32 == 0x1B {
                with_state(|s| s.hide());
            }
            LRESULT(0)
        }

        WM_LBUTTONDOWN => {
            let (x, y) = input::mouse_coords(lparam.0);
            with_state(|s| {
                let hit = s.canvas.hit_test(x, y);
                s.click_target = hit;
                s.drag_moved = false;
                if let Some(idx) = hit {
                    s.canvas.start_drag(idx, x, y);
                    SetCapture(hwnd);
                }
            });
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            with_state(|s| {
                if !s.drag_moved {
                    if let Some(idx) = s.click_target {
                        if idx < s.canvas.windows.len() {
                            let ti = s.canvas.windows[idx].thumb_index;
                            if ti < s.thumbnails.len() {
                                let target = s.thumbnails[ti].source_hwnd;
                                s.hide();
                                window::activate_window(target);
                            }
                        }
                    }
                }
                s.canvas.end_drag();
                s.click_target = None;
                let _ = ReleaseCapture();
                s.update_all_thumbnails();
                let _ = InvalidateRect(hwnd, None, true);
            });
            LRESULT(0)
        }

        WM_RBUTTONDOWN => {
            let (x, y) = input::mouse_coords(lparam.0);
            with_state(|s| {
                s.canvas.start_pan(x, y);
                SetCapture(hwnd);
            });
            LRESULT(0)
        }

        WM_RBUTTONUP => {
            with_state(|s| {
                s.canvas.end_pan();
                let _ = ReleaseCapture();
                s.update_all_thumbnails();
                let _ = InvalidateRect(hwnd, None, true);
            });
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            let (x, y) = input::mouse_coords(lparam.0);
            with_state(|s| {
                if s.canvas.drag_target.is_some() {
                    s.drag_moved = true;
                    s.canvas.update_drag(x, y);
                    s.update_all_thumbnails();
                    let _ = InvalidateRect(hwnd, None, true);
                } else if s.canvas.panning {
                    s.canvas.update_pan(x, y);
                    s.update_all_thumbnails();
                    let _ = InvalidateRect(hwnd, None, true);
                }
            });
            LRESULT(0)
        }

        WM_MOUSEWHEEL => {
            let (x, y) = input::mouse_coords(lparam.0);
            let delta = input::wheel_delta(wparam.0);
            with_state(|s| {
                let mut pt = POINT {
                    x: x as i32,
                    y: y as i32,
                };
                let _ = ScreenToClient(hwnd, &mut pt);
                s.canvas.zoom_at(pt.x as f64, pt.y as f64, delta);
                s.update_all_thumbnails();
                let _ = InvalidateRect(hwnd, None, true);
            });
            LRESULT(0)
        }

        WM_ERASEBKGND => {
            let hdc = HDC(wparam.0 as *mut _);
            let painted = with_state(|s| {
                if !s.bg_bitmap.0.is_null() {
                    let hdc_mem = CreateCompatibleDC(hdc);
                    let old = SelectObject(hdc_mem, s.bg_bitmap);
                    let _ = BitBlt(
                        hdc, 0, 0,
                        s.canvas.screen_w, s.canvas.screen_h,
                        hdc_mem, 0, 0, SRCCOPY,
                    );
                    SelectObject(hdc_mem, old);
                    let _ = DeleteDC(hdc_mem);
                    true
                } else {
                    false
                }
            });
            if painted == Some(true) {
                LRESULT(1)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }

        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            with_state(|s| {
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x00E0E0E0));

                let font_name = window::wide_string("Segoe UI");
                let font = CreateFontW(
                    18, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0,
                    PCWSTR(font_name.as_ptr()),
                );
                let old_font = SelectObject(hdc, font);

                let scale = if s.anim_active {
                    let t = s.anim_step as f64 / ANIM_STEPS as f64;
                    0.92 + 0.08 * ease_out(t)
                } else {
                    1.0
                };

                for cw in &s.canvas.windows {
                    let rect = s.canvas.canvas_to_screen_rect(cw, scale);

                    let pen = CreatePen(PS_SOLID, 2, COLORREF(0x00707070));
                    let old_pen = SelectObject(hdc, pen);
                    let null_brush = GetStockObject(NULL_BRUSH);
                    let old_brush = SelectObject(hdc, null_brush);
                    let _ = Rectangle(
                        hdc,
                        rect.left - 1, rect.top - 1,
                        rect.right + 1, rect.bottom + 1,
                    );
                    SelectObject(hdc, old_pen);
                    SelectObject(hdc, old_brush);
                    let _ = DeleteObject(pen);

                    let mut tw: Vec<u16> = cw.title.encode_utf16().collect();
                    let mut tr = RECT {
                        left: rect.left,
                        top: rect.bottom + 4,
                        right: rect.right,
                        bottom: rect.bottom + 26,
                    };
                    DrawTextW(
                        hdc, &mut tw, &mut tr,
                        DT_CENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );
                }

                SelectObject(hdc, old_font);
                let _ = DeleteObject(font);

                // Zoom indicator
                let zoom_text = format!("{:.0}%", s.canvas.zoom * 100.0);
                let mut zw: Vec<u16> = zoom_text.encode_utf16().collect();
                let bf = CreateFontW(
                    24, 0, 0, 0, 300, 0, 0, 0, 0, 0, 0, 0, 0,
                    PCWSTR(font_name.as_ptr()),
                );
                let of2 = SelectObject(hdc, bf);
                SetTextColor(hdc, COLORREF(0x00808080));
                let mut zr = RECT {
                    left: s.canvas.screen_w - 120,
                    top: s.canvas.screen_h - 40,
                    right: s.canvas.screen_w - 10,
                    bottom: s.canvas.screen_h - 10,
                };
                DrawTextW(hdc, &mut zw, &mut zr, DT_RIGHT | DT_SINGLELINE | DT_NOPREFIX);
                SelectObject(hdc, of2);
                let _ = DeleteObject(bf);

                // Help bar
                let help = "Ctrl+Alt+Space: Toggle | Scroll: Zoom | Right-drag: Pan | Left-drag: Move | Click: Switch | Esc: Close";
                let mut hw: Vec<u16> = help.encode_utf16().collect();
                let sf = CreateFontW(
                    14, 0, 0, 0, 300, 0, 0, 0, 0, 0, 0, 0, 0,
                    PCWSTR(font_name.as_ptr()),
                );
                let of3 = SelectObject(hdc, sf);
                SetTextColor(hdc, COLORREF(0x00909090));
                let mut hr = RECT {
                    left: 10,
                    top: s.canvas.screen_h - 30,
                    right: s.canvas.screen_w - 130,
                    bottom: s.canvas.screen_h - 10,
                };
                DrawTextW(hdc, &mut hw, &mut hr, DT_LEFT | DT_SINGLELINE | DT_NOPREFIX);
                SelectObject(hdc, of3);
                let _ = DeleteObject(sf);
            });

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        WM_DESTROY => {
            hotkey::unregister_hotkey(hwnd);
            with_state(|s| {
                if !s.bg_bitmap.0.is_null() {
                    window::free_bitmap(s.bg_bitmap);
                    s.bg_bitmap = HBITMAP::default();
                }
            });
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    // Set up panic hook to log panics
    std::panic::set_hook(Box::new(|info| {
        log_debug(&format!("PANIC: {}", info));
    }));

    log_debug("=== Win-Canvas starting ===");

    let (screen_w, screen_h) = window::get_screen_size();
    log_debug(&format!("Screen: {}x{}", screen_w, screen_h));

    let mut app_state = AppState::new(screen_w, screen_h);

    let hwnd = match window::create_canvas_window(Some(wndproc)) {
        Ok(h) => {
            log_debug(&format!("Window created: {:?}", h.0));
            h
        }
        Err(e) => {
            log_debug(&format!("Failed to create window: {:?}", e));
            return;
        }
    };
    app_state.canvas_hwnd = hwnd;

    match hotkey::register_hotkey(hwnd) {
        Ok(_) => log_debug("Hotkey Ctrl+Alt+Space registered successfully"),
        Err(e) => {
            log_debug(&format!("Failed to register hotkey: {:?}", e));
            // Try alternative: Ctrl+Alt+Tab
            log_debug("Hotkey registration failed! Another app may have Ctrl+Alt+Space.");
            return;
        }
    }

    APP_STATE.with(|cell| {
        *cell.borrow_mut() = Some(app_state);
    });

    log_debug("Entering message loop...");

    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    log_debug("=== Win-Canvas exiting ===");
}
