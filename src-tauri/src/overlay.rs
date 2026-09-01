use tauri::{AppHandle, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

/// Logical size of the floating control bar, matching the overlay template.
const OVERLAY_WIDTH: f64 = 460.0;
const OVERLAY_HEIGHT: f64 = 132.0;

const BOTTOM_MARGIN: f64 = 36.0;

fn bar_position(app: &AppHandle) -> Option<LogicalPosition<f64>> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let origin = monitor.position().to_logical::<f64>(scale);

    Some(LogicalPosition::new(
        origin.x + (size.width - OVERLAY_WIDTH) / 2.0,
        origin.y + size.height - OVERLAY_HEIGHT - BOTTOM_MARGIN,
    ))
}

#[tauri::command]
pub async fn open_overlay(app: AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("overlay") {
        existing.show().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // The bar is deliberately small rather than fullscreen: a fullscreen
    // overlay swallows every click on the desktop being recorded.
    let mut builder =
        WebviewWindowBuilder::new(&app, "overlay", WebviewUrl::App("#/overlay".into()))
            .title("ScreenR Recording Controls")
            .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
            .resizable(false)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            // Taking focus would pull the user out of whatever they are recording.
            .focused(false);

    if let Some(position) = bar_position(&app) {
        builder = builder.position(position.x, position.y);
    }

    builder.build().map_err(|e| e.to_string())?;
    Ok(())
}

/// Close the bar if it is open. Safe to call when it is not.
pub fn close(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.close();
    }
}

#[tauri::command]
pub async fn close_overlay(app: AppHandle) -> Result<(), String> {
    close(&app);
    Ok(())
}
