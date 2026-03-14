# Win-Canvas 🖼️

An infinite canvas for managing open application windows, built with Rust and the Windows DWM Thumbnail API.

## Features

- **🎯 Global Hotkey** — Press `Ctrl+Alt+Space` to toggle the canvas overlay
- **📐 Live Window Thumbnails** — See all your open windows rendered live by the Windows compositor
- **🔍 Infinite Zoom** — Scroll wheel to zoom in/out without any quality loss (DWM renders at native resolution)
- **✋ Pan Canvas** — Right-click drag to pan around the canvas
- **🖱️ Move Windows** — Left-click drag to rearrange window thumbnails on the canvas
- **⚡ Quick Switch** — Click any window thumbnail to instantly switch to it

## How It Works

Win-Canvas leverages the **DWM (Desktop Window Manager) Thumbnail API** to display live, resolution-independent previews of all open windows. Unlike screenshot-based approaches:

- Thumbnails are composited directly by Windows — **zero quality loss** at any zoom level
- Content updates **in real-time** — you see live window content, not snapshots
- **Zero-copy** — no pixel data is transferred; DWM handles all rendering

## Controls

| Action | Input |
|--------|-------|
| Toggle canvas | `Ctrl + Alt + Space` |
| Zoom in/out | Scroll wheel |
| Pan canvas | Right-click + drag |
| Move window | Left-click + drag |
| Switch to window | Left-click (no drag) |
| Close canvas | `Escape` |

## Building

```bash
cargo build --release
```

The binary will be at `target/release/win-canvas.exe`.

## Running

```bash
cargo run --release
```

The app runs in the background with no visible window. Press `Ctrl+Alt+Space` to open the canvas.

## Requirements

- Windows 10/11 (requires DWM, which is enabled by default)
- Desktop composition must be enabled (default on modern Windows)

## Architecture

```
src/
├── main.rs          # Entry point, message loop, WM_PAINT rendering
├── canvas.rs        # Canvas state: pan, zoom, window layout, coordinate transforms
├── dwm.rs           # DWM thumbnail registration and management
├── enumerate.rs     # Enumerate visible top-level windows
├── hotkey.rs        # Global hotkey (Ctrl+Alt+Space)
├── input.rs         # Mouse input helpers
└── window.rs        # Win32 overlay window creation
```

## Tech Stack

- **Rust** — Memory safety, no GC
- **`windows` crate v0.58** — Native Win32 API bindings
- **DWM Thumbnail API** — Live, compositor-rendered window previews
- **GDI** — Title labels, borders, UI text (lightweight, no heavy rendering framework needed)
