//! Input handling — mouse and keyboard events for the canvas.

/// Decoded mouse input event.
#[derive(Debug)]
pub enum MouseEvent {
    LeftDown { x: f64, y: f64 },
    LeftUp { x: f64, y: f64 },
    RightDown { x: f64, y: f64 },
    RightUp { x: f64, y: f64 },
    Move { x: f64, y: f64 },
    Wheel { x: f64, y: f64, delta: f64 },
}

/// Extract mouse coordinates from LPARAM.
pub fn mouse_coords(lparam: isize) -> (f64, f64) {
    let x = (lparam & 0xFFFF) as i16 as f64;
    let y = ((lparam >> 16) & 0xFFFF) as i16 as f64;
    (x, y)
}

/// Extract wheel delta from WPARAM.
pub fn wheel_delta(wparam: usize) -> f64 {
    let delta = ((wparam >> 16) & 0xFFFF) as i16;
    delta as f64 / 120.0
}
