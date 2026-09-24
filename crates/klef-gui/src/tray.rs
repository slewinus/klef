//! Menu bar icon and popover placement.

use tauri::{
    Emitter as _, Manager as _,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// Monochrome glyph (shield with the K knocked out). As a template image
/// macOS tints it from its alpha channel, so it matches the menu bar in
/// light mode, dark mode, and when highlighted — like native icons.
const TRAY_ICON: &[u8] = include_bytes!("../icons/tray-template.png");

/// Build the tray icon; a left click toggles the popover.
///
/// # Errors
/// Propagates icon decoding / tray creation failures.
pub fn build(app: &tauri::App) -> tauri::Result<()> {
    TrayIconBuilder::with_id("main")
        .icon(tauri::image::Image::from_bytes(TRAY_ICON)?)
        .icon_as_template(true)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Place the window centered under the tray icon, querying the icon's
/// screen rect directly from Tauri (works even before the user has clicked
/// the tray for the first time, unlike `Position::TrayCenter` from the
/// positioner plugin which depends on `on_tray_event` having fired).
/// Returns true when the position was successfully computed and applied.
fn place_under_tray(app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> bool {
    let Some(tray) = app.tray_by_id("main") else {
        return false;
    };
    let Ok(Some(rect)) = tray.rect() else {
        return false;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    // Tauri 2's Rect uses Physical-or-Logical enums; convert for set_position.
    let pos = rect.position.to_physical::<f64>(scale);
    let size = rect.size.to_physical::<f64>(scale);
    let win = window.outer_size().unwrap_or_default();
    let tray_center_x = pos.x + size.width / 2.0;
    #[allow(clippy::cast_possible_truncation)]
    let x = (tray_center_x - f64::from(win.width) / 2.0).round() as i32;
    #[allow(clippy::cast_possible_truncation)]
    // 8px gap from the tray icon — close enough to feel anchored, far
    // enough that macOS' menu bar shadow doesn't visually flatten the
    // popover's top rounded corners.
    let y = (pos.y + size.height + 8.0).round() as i32;
    let _ = window.set_position(tauri::PhysicalPosition::new(x.max(0), y.max(0)));
    true
}

/// Last-resort fallback: top-right of the current monitor with margins.
/// Used only when `tray.rect()` is unavailable (e.g. the tray icon is
/// not yet attached to a screen by the OS).
fn place_top_right_with_margin(window: &tauri::WebviewWindow) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let screen = monitor.size();
    let win = window.outer_size().unwrap_or_default();
    let x = i32::try_from(screen.width.saturating_sub(win.width)).unwrap_or(0) - 12;
    let _ = window.set_position(tauri::PhysicalPosition::new(x.max(0), 32));
}

pub fn toggle_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        if !place_under_tray(app, &window) {
            place_top_right_with_margin(&window);
        }
        let _ = window.show();
        let _ = window.set_focus();
        // Notify the frontend that the popover just opened so it can
        // refresh data and refocus the search bar. The DOM `focus` event
        // isn't reliable on Tauri's webview when toggling visibility — the
        // OS-level show/hide doesn't always propagate as a JS focus event.
        let _ = window.emit("popover-shown", ());
    }
}
