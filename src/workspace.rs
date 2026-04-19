use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::enumerate;
use crate::log_debug;

pub const GRID_COLS: i32 = 3;
pub const GRID_ROWS: i32 = 3;
const TILE_PAD: i32 = 10;
const TASKBAR_H: i32 = 48;
const LERP_SPEED: f64 = 6.0;
const MIN_MOVE_PIXELS: f64 = 2.0;

fn is_system_window(class_name: &str) -> bool {
    matches!(
        class_name,
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Progman" | "WorkerW"
    )
}

#[derive(Clone)]
pub struct WsWindow {
    pub hwnd: HWND,
    pub title: String,
    pub vx: i32,
    pub vy: i32,
    pub vw: i32,
    pub vh: i32,
    pub orig_placement: WINDOWPLACEMENT,
}

pub struct Workspace {
    pub active: bool,
    pub viewport_x: f64,
    pub viewport_y: f64,
    pub target_x: f64,
    pub target_y: f64,
    pub screen_w: i32,
    pub screen_h: i32,
    pub windows: Vec<WsWindow>,
    pub last_update_ms: i64,
}

impl Workspace {
    pub fn new(screen_w: i32, screen_h: i32) -> Self {
        Self {
            active: false,
            viewport_x: 0.0,
            viewport_y: 0.0,
            target_x: 0.0,
            target_y: 0.0,
            screen_w,
            screen_h,
            windows: Vec::new(),
            last_update_ms: 0,
        }
    }

    pub fn activate(&mut self, canvas_hwnd: HWND) {
        if self.active {
            return;
        }

        self.active = true;
        self.viewport_x = 0.0;
        self.viewport_y = 0.0;
        self.target_x = 0.0;
        self.target_y = 0.0;
        self.last_update_ms = current_time_ms();

        let infos = enumerate::enumerate_windows_ext();
        self.windows.clear();

        for wi in infos {
            if wi.hwnd == canvas_hwnd {
                continue;
            }
            if is_system_window(&wi.class_name) {
                continue;
            }

            let mut placement = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            unsafe {
                let _ = GetWindowPlacement(wi.hwnd, &mut placement);
            }

            self.windows.push(WsWindow {
                hwnd: wi.hwnd,
                title: wi.title,
                vx: 0,
                vy: 0,
                vw: 0,
                vh: 0,
                orig_placement: placement,
            });
        }

        self.distribute_and_tile();

        for ww in &self.windows {
            unsafe {
                if IsZoomed(ww.hwnd).as_bool() {
                    let _ = ShowWindow(ww.hwnd, SW_RESTORE);
                }
            }
        }

        self.apply_positions();

        log_debug(&format!(
            "Workspace activated: {} windows across {}x{} grid",
            self.windows.len(),
            GRID_COLS,
            GRID_ROWS
        ));
    }

    pub fn deactivate(&mut self) {
        if !self.active {
            return;
        }

        self.active = false;

        for ww in &self.windows {
            unsafe {
                if IsWindow(ww.hwnd).as_bool() {
                    let _ = SetWindowPlacement(ww.hwnd, &ww.orig_placement);
                }
            }
        }

        self.windows.clear();
        log_debug("Workspace deactivated, windows restored");
    }

    fn distribute_and_tile(&mut self) {
        let count = self.windows.len();
        if count == 0 {
            return;
        }

        let total_cells = (GRID_COLS * GRID_ROWS) as usize;
        let per_cell = ((count + total_cells - 1) / total_cells).max(1);

        for i in 0..count {
            let cell_idx = (i / per_cell).min(total_cells - 1);
            let cell_col = cell_idx as i32 % GRID_COLS;
            let cell_row = cell_idx as i32 / GRID_COLS;

            let cell_start = cell_idx * per_cell;
            let cell_end = ((cell_idx + 1) * per_cell).min(count);
            let cell_count = cell_end - cell_start;
            let in_cell = i - cell_start;

            let cols = (cell_count as f64).sqrt().ceil() as i32;
            let rows = (cell_count as i32 + cols - 1) / cols;

            let avail_w = self.screen_w - TILE_PAD * 2;
            let avail_h = self.screen_h - TASKBAR_H - TILE_PAD * 2;
            let tw = avail_w / cols;
            let th = avail_h / rows;

            let c = in_cell as i32 % cols;
            let r = in_cell as i32 / cols;

            let base_x = cell_col * self.screen_w;
            let base_y = cell_row * self.screen_h;

            self.windows[i].vx = base_x + TILE_PAD + c * tw + TILE_PAD;
            self.windows[i].vy = base_y + TILE_PAD + r * th + TILE_PAD;
            self.windows[i].vw = tw - TILE_PAD * 2;
            self.windows[i].vh = th - TILE_PAD * 2;
        }
    }

    pub fn apply_positions(&self) {
        let vx = self.viewport_x.round() as i32;
        let vy = self.viewport_y.round() as i32;
        let margin = self.screen_w.max(self.screen_h);

        for ww in &self.windows {
            let x = ww.vx - vx;
            let y = ww.vy - vy;

            if x + ww.vw < -margin
                || x > self.screen_w + margin
                || y + ww.vh < -margin
                || y > self.screen_h + margin
            {
                continue;
            }

            unsafe {
                if !IsWindow(ww.hwnd).as_bool() {
                    continue;
                }
                let _ = SetWindowPos(
                    ww.hwnd,
                    HWND::default(),
                    x,
                    y,
                    ww.vw,
                    ww.vh,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
    }

    fn clamp_target(&mut self) {
        let max_x = ((GRID_COLS - 1) * self.screen_w) as f64;
        let max_y = ((GRID_ROWS - 1) * self.screen_h) as f64;
        self.target_x = self.target_x.clamp(0.0, max_x);
        self.target_y = self.target_y.clamp(0.0, max_y);
    }

    pub fn navigate(&mut self, dx: f64, dy: f64) {
        if !self.active {
            return;
        }

        self.target_x += dx;
        self.target_y += dy;
        self.clamp_target();
    }

    pub fn jump_to(&mut self, canvas_x: f64, canvas_y: f64) {
        if !self.active {
            return;
        }

        self.target_x = canvas_x - self.screen_w as f64 / 2.0;
        self.target_y = canvas_y - self.screen_h as f64 / 2.0;
        self.clamp_target();
    }

    pub fn tick(&mut self) -> bool {
        let now = current_time_ms();
        let dt = (now - self.last_update_ms).max(1) as f64 / 1000.0;
        self.last_update_ms = now;

        let dx = self.target_x - self.viewport_x;
        let dy = self.target_y - self.viewport_y;

        let dist = (dx * dx + dy * dy).sqrt();
        if dist < MIN_MOVE_PIXELS {
            self.viewport_x = self.target_x;
            self.viewport_y = self.target_y;
            self.apply_positions();
            return true;
        }

        let factor = 1.0 - (-LERP_SPEED * dt).exp();
        self.viewport_x += dx * factor;
        self.viewport_y += dy * factor;

        self.apply_positions();
        false
    }

    pub fn is_moving(&self) -> bool {
        let dx = (self.target_x - self.viewport_x).abs();
        let dy = (self.target_y - self.viewport_y).abs();
        dx > MIN_MOVE_PIXELS || dy > MIN_MOVE_PIXELS
    }

    pub fn current_cell(&self) -> (i32, i32) {
        let cx = self.viewport_x + self.screen_w as f64 / 2.0;
        let cy = self.viewport_y + self.screen_h as f64 / 2.0;
        let col = (cx / self.screen_w as f64).floor() as i32;
        let row = (cy / self.screen_h as f64).floor() as i32;
        (col.clamp(0, GRID_COLS - 1), row.clamp(0, GRID_ROWS - 1))
    }
}

fn current_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
