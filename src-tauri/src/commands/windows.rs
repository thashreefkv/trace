use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, Position, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

/// Bring the main Trace window to front and activate the app on macOS.
///
/// tao's `window.show()` + `window.set_focus()` has a race condition: the
/// `set_focus` implementation checks `isVisible` synchronously, but `show()`
/// dispatches `makeKeyAndOrderFront` asynchronously to the main thread, so
/// the visibility check always sees `false` and `activateIgnoringOtherApps`
/// is never called.
///
/// Fix: run both calls synchronously in `run_on_main_thread`.
#[cfg(target_os = "macos")]
fn activate_main_window_macos(app: &AppHandle, win_ptr: usize) {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_app_kit::NSWindow;

    let _ = app.run_on_main_thread(move || unsafe {
        let ns_app_cls = AnyClass::get(c"NSApplication").unwrap();
        let ns_app: *mut AnyObject = msg_send![ns_app_cls, sharedApplication];

        let is_hidden: bool = msg_send![ns_app, isHidden];
        if is_hidden {
            let _: () = msg_send![ns_app, unhide: std::ptr::null::<AnyObject>()];
        }

        let win = &*(win_ptr as *const NSWindow);
        // `orderFrontRegardless` bypasses macOS's "active app's windows take
        // priority" z-order rule, guaranteeing the window surfaces above Chrome
        // / Claude Desktop before we activate Trace. Without this, Cmd+Shift+M
        // from another app can leave the main window invisible behind it.
        let _: () = msg_send![win, orderFrontRegardless];
        win.makeKeyAndOrderFront(None);
        let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
    });
}

// Define the tray popup NSPanel subclass and its window-delegate event handler.
//
// TrayPanel
//   • can_become_key_window: true  — user can type immediately without a first click.
//   • is_floating_panel: true      — floats above regular app windows.
//
// TrayPanelEventHandler
//   • window_did_resign_key — fires when the user clicks in another app;
//     we call panel.hide() here (no JS blur-timer hacks needed).
//
// The critical non-activating behaviour is provided by tauri-nspanel converting
// the NSWindow to a real NSPanel subclass via object_setClass before first show.
#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(TrayPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })

    panel_event!(TrayPanelEventHandler {
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

const CAPTURE_WINDOW_LABEL: &str = "capture";
const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_WINDOW_LABEL: &str = "tray";
const SPOTLIGHT_WINDOW_LABEL: &str = "spotlight";
const SPOTLIGHT_WIDTH: f64 = 720.0;
const SPOTLIGHT_HEIGHT: f64 = 520.0;

// ---------------------------------------------------------------------------
// Public Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn show_capture_window(app: AppHandle) -> Result<(), String> {
    show_capture_window_inner(&app)
}

#[tauri::command]
pub async fn hide_capture_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(CAPTURE_WINDOW_LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn show_main_window(app: AppHandle, route: Option<String>) -> Result<(), String> {
    show_main_window_inner(&app, route.as_deref())
}

#[tauri::command]
pub async fn show_spotlight_window(app: AppHandle) -> Result<(), String> {
    show_spotlight_window_inner(&app)
}

#[tauri::command]
pub async fn hide_spotlight_window(app: AppHandle) -> Result<(), String> {
    hide_spotlight_window_inner(&app)
}

#[tauri::command]
pub async fn toggle_spotlight_window_cmd(app: AppHandle) -> Result<(), String> {
    toggle_spotlight_window(&app)
}

// ---------------------------------------------------------------------------
// Capture window
// ---------------------------------------------------------------------------

pub(crate) fn ensure_capture_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(CAPTURE_WINDOW_LABEL) {
        return Ok(window);
    }

    WebviewWindowBuilder::new(
        app,
        CAPTURE_WINDOW_LABEL,
        WebviewUrl::App("index.html#/capture".into()),
    )
    .title("Capture")
    .inner_size(420.0, 320.0)
    .min_inner_size(420.0, 320.0)
    .resizable(false)
    .always_on_top(true)
    .visible(false)
    .focused(false)
    .build()
    .map_err(|error| error.to_string())
}

pub(crate) fn show_capture_window_inner(app: &AppHandle) -> Result<(), String> {
    let window = ensure_capture_window(app)?;
    window.unminimize().map_err(|error| error.to_string())?;

    #[cfg(target_os = "macos")]
    if let Ok(ptr) = window.ns_window() {
        // Capture window is a floating popup — activate the app so the user
        // can type immediately without having to click first.
        activate_main_window_macos(app, ptr as usize);
        return Ok(());
    }

    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Main window
// ---------------------------------------------------------------------------

pub(crate) fn show_main_window_inner(app: &AppHandle, route: Option<&str>) -> Result<(), String> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Err("main window not found".to_string());
    };

    // Reverse a prior `hide()` (which uses NSWindow `orderOut:`). Without this
    // an `orderFrontRegardless` from the activate path can briefly leave the
    // webview unrendered — visible chrome but blank contents — when the main
    // window is surfaced from a non-activating context like the spotlight
    // panel.
    let _ = window.show();
    window.unminimize().map_err(|error| error.to_string())?;

    #[cfg(target_os = "macos")]
    if let Ok(ptr) = window.ns_window() {
        activate_main_window_macos(app, ptr as usize);
    }

    #[cfg(not(target_os = "macos"))]
    {
        window.set_focus().map_err(|error| error.to_string())?;
    }

    if let Some(route) = route.filter(|value| !value.trim().is_empty()) {
        app.emit_to(MAIN_WINDOW_LABEL, "navigate-to", route.trim())
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

/// Show the main window and open the inline quick-capture overlay.
pub(crate) fn show_main_window_with_capture(app: &AppHandle) -> Result<(), String> {
    show_main_window_inner(app, None)?;
    app.emit_to(MAIN_WINDOW_LABEL, "show-capture", ())
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tray popup
// ---------------------------------------------------------------------------

pub(crate) fn toggle_tray_window(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;

        // If the panel was already created, just toggle visibility.
        if let Ok(panel) = app.get_webview_panel(TRAY_WINDOW_LABEL) {
            if panel.is_visible() {
                panel.hide();
            } else {
                position_tray_window(app)?;
                force_show_tray_panel(app);
            }
            return Ok(());
        }

        // First click — create and show the NSPanel.
        return show_tray_window_inner(app);
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app.get_webview_window(TRAY_WINDOW_LABEL) {
            if window.is_visible().unwrap_or(false) {
                window.hide().map_err(|error| error.to_string())?;
                return Ok(());
            }
        }
        show_tray_window_inner(app)
    }
}

/// Force the tray panel to appear above the windows of any other app.
///
/// This is the only path that reliably works when an inactive app (Trace) needs
/// to surface a window over the active app (Chrome, Claude Desktop).  The
/// trick is two specific NSWindow calls in this exact order:
///
///   1. `orderFrontRegardless` — bypasses macOS's "active app's windows take
///      priority" z-order rule.  `makeKeyAndOrderFront:` (which is what
///      `panel.show_and_make_key()` calls) does NOT bypass it: an inactive
///      app's panel called via `makeKeyAndOrderFront:` may render BEHIND the
///      active app's windows, even at high window levels.  This is the bug
///      that made the popup "open in another window" for the user.
///
///   2. `makeKeyWindow` — makes the panel the key window so the user can
///      type immediately.  Works without activating Trace because the panel
///      has the `NSWindowStyleMaskNonactivatingPanel` bit set.
#[cfg(target_os = "macos")]
fn force_show_tray_panel(app: &AppHandle) {
    let Some(window) = app.get_webview_window(TRAY_WINDOW_LABEL) else {
        return;
    };
    let Ok(ptr) = window.ns_window() else {
        return;
    };
    unsafe {
        use objc2::msg_send;
        use objc2_app_kit::NSWindow;
        let ns_win = &*(ptr as *const NSWindow);
        let _: () = msg_send![ns_win, orderFrontRegardless];
        let _: () = msg_send![ns_win, makeKeyWindow];
    }
}

/// Move the tray window to sit directly below the menu-bar icon (macOS only).
#[cfg(target_os = "macos")]
fn position_tray_window(app: &AppHandle) -> Result<(), String> {
    // The WebviewWindow wrapper remains accessible via get_webview_window even
    // after to_panel() converts the underlying NSWindow to an NSPanel.
    let Some(window) = app.get_webview_window(TRAY_WINDOW_LABEL) else {
        return Ok(());
    };
    if let Some(tray) = app.tray_by_id("project-manager-tray") {
        if let Ok(Some(rect)) = tray.rect() {
            let sf = window.scale_factor().unwrap_or(1.0);
            let pos = rect.position.to_physical::<i32>(sf);
            let size = rect.size.to_physical::<u32>(sf);

            // Center the 300 px-wide window under the tray icon, then clamp so
            // it never runs off the right edge of the screen (which is what
            // happens with right-side tray icons + a 300 px wide popup).
            let win_width_phys = (300.0 * sf) as i32;
            let mut x = pos.x + (size.width as i32 / 2) - (win_width_phys / 2);

            // Keep at least 8 px from the right edge of the screen.
            if let Some(monitor) = window
                .current_monitor()
                .ok()
                .flatten()
                .or_else(|| window.primary_monitor().ok().flatten())
            {
                let m_size = monitor.size();
                let m_pos = monitor.position();
                let right_edge = m_pos.x + m_size.width as i32;
                let max_x = right_edge - win_width_phys - (8.0 * sf) as i32;
                if x > max_x {
                    x = max_x;
                }
            }

            // 6 px below the menu bar — standard macOS popover gap.
            let y = pos.y + size.height as i32 + (6.0 * sf) as i32;

            window
                .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                    x, y,
                )))
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

/// Create (or reuse) the tray popup and show it.
///
/// On macOS the WebviewWindow is converted to a real `NSPanel` via
/// `tauri-nspanel` so the popup:
///   1. Appears above every other app's windows at the floating level.
///   2. Does NOT activate the Trace app when it appears.
///   3. Dismisses itself natively when `windowDidResignKey` fires (user clicks
///      in Chrome, Finder, etc.) — no JS blur-timer hacks needed.
fn show_tray_window_inner(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::{ManagerExt, WebviewWindowExt};

        // Reuse an already-converted panel if one somehow exists.
        let panel = if let Ok(panel) = app.get_webview_panel(TRAY_WINDOW_LABEL) {
            panel
        } else {
            // Build the underlying WebviewWindow (kept invisible until shown as panel).
            let window = WebviewWindowBuilder::new(
                app,
                TRAY_WINDOW_LABEL,
                WebviewUrl::App("index.html#/tray".into()),
            )
            .title("Trace Widget")
            // 300 × 300 — card fills the whole window.  Native macOS shadow
            // (shadow=true) renders around the visible alpha on Sonoma+ and
            // respects rounded corners, so we don't need the CSS-shadow hack.
            .inner_size(300.0, 300.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .background_color(tauri::webview::Color(0, 0, 0, 0))
            .always_on_top(true)
            .visible(false)
            .shadow(true)
            .skip_taskbar(true)
            .build()
            .map_err(|error| error.to_string())?;

            // Capture the NSWindow pointer BEFORE to_panel() consumes `window`.
            let ns_win_ptr = window.ns_window().ok().map(|p| p as usize);

            // Convert NSWindow → NSPanel via object_setClass.
            let panel = window
                .to_panel::<TrayPanel>()
                .map_err(|error| error.to_string())?;

            // ── CRITICAL: apply NSPanel flags synchronously, before first show ──
            //
            // We are on the main thread (called from the tray-icon event handler).
            // Using run_on_main_thread here would be async — the flags would be
            // set AFTER the first show, making them useless.
            //
            //  • styleMask |= NSWindowStyleMaskNonactivatingPanel (1 << 7 = 128)
            //      The panel can become key WITHOUT activating Trace, so Chrome
            //      / Claude Desktop stay the active app when the popup appears.
            //      Only valid on NSPanel subclasses (this is, after to_panel()).
            //
            //  • setLevel: NSPopUpMenuWindowLevel (101)
            //      A high enough level to float above any normal app window
            //      including Chrome / Claude / Slack.  Higher than the
            //      NSFloatingWindowLevel (3) that `always_on_top(true)` sets.
            //
            //  • setCollectionBehavior:
            //      CanJoinAllSpaces (1) | Transient (8) | FullScreenAuxiliary (256)
            //      = 265.  Appears on whichever Space the user is currently on
            //      (including fullscreen apps), without pinning to any Space.
            // ───────────────────────────────────────────────────────────────────
            if let Some(ptr) = ns_win_ptr {
                unsafe {
                    use objc2::msg_send;
                    use objc2_app_kit::NSWindow;
                    let ns_win = &*(ptr as *const NSWindow);
                    let mask: usize = msg_send![ns_win, styleMask];
                    let _: () = msg_send![ns_win, setStyleMask: mask | 128usize];
                    let _: () = msg_send![ns_win, setLevel: 101i64];
                    let _: () = msg_send![ns_win, setCollectionBehavior: 265usize];
                    // Don't hide the panel when Trace is deactivated — we
                    // dismiss only on windowDidResignKey (user clicks elsewhere).
                    let _: () = msg_send![ns_win, setHidesOnDeactivate: false];
                }
            }

            // Wire up the native dismiss handler.
            let handle = app.clone();
            let handler = TrayPanelEventHandler::new();
            handler.window_did_resign_key(move |_| {
                if let Ok(p) = handle.get_webview_panel(TRAY_WINDOW_LABEL) {
                    p.hide();
                }
            });
            panel.set_event_handler(Some(handler.as_ref()));

            panel
        };

        // `panel` is required to keep the handle alive after registration;
        // the actual show goes through orderFrontRegardless (see helper).
        let _ = panel;
        position_tray_window(app)?;
        force_show_tray_panel(app);
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let window = if let Some(window) = app.get_webview_window(TRAY_WINDOW_LABEL) {
            window
        } else {
            WebviewWindowBuilder::new(
                app,
                TRAY_WINDOW_LABEL,
                WebviewUrl::App("index.html#/tray".into()),
            )
            .title("Trace Widget")
            // 300 × 300 — card fills the whole window.  Native macOS shadow
            // (shadow=true) renders around the visible alpha on Sonoma+ and
            // respects rounded corners, so we don't need the CSS-shadow hack.
            .inner_size(300.0, 300.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .background_color(tauri::webview::Color(0, 0, 0, 0))
            .always_on_top(true)
            .visible(false)
            .shadow(true)
            .skip_taskbar(true)
            .build()
            .map_err(|error| error.to_string())?
        };

        if let Some(tray) = app.tray_by_id("project-manager-tray") {
            if let Ok(Some(rect)) = tray.rect() {
                let sf = window.scale_factor().unwrap_or(1.0);
                let pos = rect.position.to_physical::<i32>(sf);
                let size = rect.size.to_physical::<u32>(sf);
                let x = pos.x + (size.width as i32 / 2) - ((300.0 * sf) as i32 / 2);
                let y = pos.y + size.height as i32 + 8;
                window
                    .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                        x, y,
                    )))
                    .map_err(|error| error.to_string())?;
            }
        }

        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Spotlight overlay
//
// A floating, non-activating NSPanel that mimics macOS Spotlight: pressing
// ⌘⇧Space from any app brings it forward, centered ~22% from the top of
// whichever screen the user is currently working on. Trace does not become
// the active app; the other app's menu bar stays in place.
//
// Reuses the same `TrayPanel` subclass as the tray widget.
//
// THREAD SAFETY
// =============
// Every NSPanel / NSScreen / NSWindow AppKit API must be called on the macOS
// main thread. The global-shortcut handler and `tauri::command` executors both
// run on tokio worker threads, so every public entry-point in this section
// dispatches its macOS work via `run_on_main_thread` and returns `Ok(())`
// immediately. `ensure_spotlight_window` is the sole exception — it is called
// from `setup()`, which IS the main thread, so it can call the inner helper
// directly.
// ---------------------------------------------------------------------------

/// Create the spotlight NSPanel and configure all AppKit properties.
///
/// **MUST be called on the macOS main thread.**
#[cfg(target_os = "macos")]
fn create_spotlight_panel_on_main(app: &AppHandle) -> Result<(), String> {
    use tauri_nspanel::{ManagerExt, WebviewWindowExt};

    // Already created — nothing to do.
    if app.get_webview_panel(SPOTLIGHT_WINDOW_LABEL).is_ok() {
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        app,
        SPOTLIGHT_WINDOW_LABEL,
        WebviewUrl::App("index.html#/spotlight".into()),
    )
    .title("Trace Spotlight")
    .inner_size(SPOTLIGHT_WIDTH, SPOTLIGHT_HEIGHT)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .background_color(tauri::webview::Color(0, 0, 0, 0))
    .always_on_top(true)
    .visible(false)
    .shadow(false)
    .skip_taskbar(true)
    .build()
    .map_err(|error| error.to_string())?;

    let ns_win_ptr = window.ns_window().ok().map(|p| p as usize);

    let panel = window
        .to_panel::<TrayPanel>()
        .map_err(|error| error.to_string())?;

    if let Some(ptr) = ns_win_ptr {
        unsafe {
            use objc2::msg_send;
            use objc2_app_kit::NSWindow;
            let ns_win = &*(ptr as *const NSWindow);
            let mask: usize = msg_send![ns_win, styleMask];
            let _: () = msg_send![ns_win, setStyleMask: mask | 128usize];
            let _: () = msg_send![ns_win, setLevel: 101i64];
            let _: () = msg_send![ns_win, setCollectionBehavior: 265usize];
            let _: () = msg_send![ns_win, setHidesOnDeactivate: false];
        }
    }

    let handle = app.clone();
    let handler = TrayPanelEventHandler::new();
    handler.window_did_resign_key(move |_| {
        // AppKit delegates fire on the main thread, so panel.hide() is safe here.
        if let Ok(p) = handle.get_webview_panel(SPOTLIGHT_WINDOW_LABEL) {
            p.hide();
        }
        // Let the JS side cancel any in-flight Ask run so we don't keep
        // streaming Gemini tokens into a hidden window.
        let _ = handle.emit_to(SPOTLIGHT_WINDOW_LABEL, "spotlight:dismissed", ());
    });
    panel.set_event_handler(Some(handler.as_ref()));

    let _ = panel;
    Ok(())
}

/// Position the spotlight panel then bring it to front and make it key.
///
/// **MUST be called on the macOS main thread** — both `position_spotlight_window`
/// (NSScreen query) and `force_show_spotlight_panel` (orderFrontRegardless /
/// makeKeyWindow) require it.
#[cfg(target_os = "macos")]
fn show_spotlight_on_main(app: &AppHandle) -> Result<(), String> {
    position_spotlight_window(app)?;
    let _ = app.emit_to(SPOTLIGHT_WINDOW_LABEL, "spotlight:opening", ());
    force_show_spotlight_panel(app);
    Ok(())
}

pub(crate) fn ensure_spotlight_window(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if app.get_webview_panel(SPOTLIGHT_WINDOW_LABEL).is_ok() {
            return Ok(());
        }
        // Called from setup() which runs on the main thread — invoke directly.
        create_spotlight_panel_on_main(app)
    }

    #[cfg(not(target_os = "macos"))]
    {
        if app.get_webview_window(SPOTLIGHT_WINDOW_LABEL).is_some() {
            return Ok(());
        }
        WebviewWindowBuilder::new(
            app,
            SPOTLIGHT_WINDOW_LABEL,
            WebviewUrl::App("index.html#/spotlight".into()),
        )
        .title("Trace Spotlight")
        .inner_size(SPOTLIGHT_WIDTH, SPOTLIGHT_HEIGHT)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .background_color(tauri::webview::Color(0, 0, 0, 0))
        .always_on_top(true)
        .visible(false)
        .skip_taskbar(true)
        .build()
        .map(|_| ())
        .map_err(|error| error.to_string())
    }
}

pub(crate) fn toggle_spotlight_window(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let app2 = app.clone();
        // The global-shortcut handler and tauri::command executors run on tokio
        // worker threads. All NSPanel / AppKit APIs must run on the macOS main
        // thread — dispatch the entire toggle body here and return immediately.
        let _ = app.run_on_main_thread(move || {
            use tauri_nspanel::ManagerExt;
            if let Ok(panel) = app2.get_webview_panel(SPOTLIGHT_WINDOW_LABEL) {
                if panel.is_visible() {
                    panel.hide();
                    let _ = app2.emit_to(SPOTLIGHT_WINDOW_LABEL, "spotlight:dismissed", ());
                    return;
                }
            } else {
                // Panel absent (e.g. setup() raced with an early keypress) —
                // create it now that we are on the main thread.
                let _ = create_spotlight_panel_on_main(&app2);
            }
            let _ = show_spotlight_on_main(&app2);
        });
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app.get_webview_window(SPOTLIGHT_WINDOW_LABEL) {
            if window.is_visible().unwrap_or(false) {
                window.hide().map_err(|error| error.to_string())?;
                let _ = app.emit_to(SPOTLIGHT_WINDOW_LABEL, "spotlight:dismissed", ());
                return Ok(());
            }
        }
        show_spotlight_window_inner(app)
    }
}

pub(crate) fn show_spotlight_window_inner(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            use tauri_nspanel::ManagerExt;
            if app2.get_webview_panel(SPOTLIGHT_WINDOW_LABEL).is_err() {
                let _ = create_spotlight_panel_on_main(&app2);
            }
            let _ = show_spotlight_on_main(&app2);
        });
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let Some(window) = app.get_webview_window(SPOTLIGHT_WINDOW_LABEL) else {
            return Ok(());
        };
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        Ok(())
    }
}

pub(crate) fn hide_spotlight_window_inner(app: &AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            use tauri_nspanel::ManagerExt;
            if let Ok(panel) = app2.get_webview_panel(SPOTLIGHT_WINDOW_LABEL) {
                panel.hide();
            }
            let _ = app2.emit_to(SPOTLIGHT_WINDOW_LABEL, "spotlight:dismissed", ());
        });
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app.get_webview_window(SPOTLIGHT_WINDOW_LABEL) {
            window.hide().map_err(|error| error.to_string())?;
        }
        let _ = app.emit_to(SPOTLIGHT_WINDOW_LABEL, "spotlight:dismissed", ());
        Ok(())
    }
}

/// Force the spotlight panel to surface above any other app's windows.
///
/// Same trick used by the tray panel: `orderFrontRegardless` bypasses macOS's
/// active-app z-order priority, and `makeKeyWindow` lets the user start typing
/// immediately — the `NSWindowStyleMaskNonactivatingPanel` bit means Trace
/// stays in the background while the panel takes the keyboard.
///
/// **MUST be called on the macOS main thread.**
#[cfg(target_os = "macos")]
fn force_show_spotlight_panel(app: &AppHandle) {
    let Some(window) = app.get_webview_window(SPOTLIGHT_WINDOW_LABEL) else {
        return;
    };
    let Ok(ptr) = window.ns_window() else {
        return;
    };
    unsafe {
        use objc2::msg_send;
        use objc2_app_kit::NSWindow;
        let ns_win = &*(ptr as *const NSWindow);
        let _: () = msg_send![ns_win, orderFrontRegardless];
        let _: () = msg_send![ns_win, makeKeyWindow];
    }
}

/// Place the spotlight panel centered horizontally at ~22% from the top of
/// whichever screen the user is currently working on.
///
/// Uses `NSScreen.mainScreen()` — the screen containing the key window of the
/// currently active app — so the panel always appears where the user is looking.
/// `current_monitor()` on a hidden panel returns garbage before first show, so
/// we bypass Tauri's monitor API and go straight to AppKit.
///
/// **MUST be called on the macOS main thread.**
fn position_spotlight_window(app: &AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window(SPOTLIGHT_WINDOW_LABEL) else {
        return Ok(());
    };

    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyClass;
        use objc2_app_kit::NSScreen;
        use objc2_foundation::NSRect;

        let frame: Option<NSRect> = unsafe {
            let cls = AnyClass::get(c"NSScreen").ok_or("NSScreen class not found")?;
            let main_screen: *mut NSScreen = msg_send![cls, mainScreen];
            if main_screen.is_null() {
                None
            } else {
                let screen: &NSScreen = &*main_screen;
                Some(msg_send![screen, visibleFrame])
            }
        };

        if let Some(rect) = frame {
            // visibleFrame is in macOS screen coordinates (origin bottom-left).
            // We want the panel at ~22% from the visible top.
            let screen_x = rect.origin.x;
            let screen_y = rect.origin.y;
            let screen_w = rect.size.width;
            let screen_h = rect.size.height;

            let x = screen_x + (screen_w - SPOTLIGHT_WIDTH) / 2.0;
            let top_offset = screen_h * 0.22;
            // Tauri's `set_position` expects top-left in flipped (Cocoa-y)
            // logical points. Convert from bottom-left visibleFrame to the
            // top of the window.
            let y_top_in_screen = screen_y + screen_h - top_offset - SPOTLIGHT_HEIGHT;

            window
                .set_position(Position::Logical(LogicalPosition::new(x, y_top_in_screen)))
                .map_err(|error| error.to_string())?;
            return Ok(());
        }
    }

    // Fallback: use Tauri's monitor info. Less ideal on macOS for hidden
    // panels but fine on other platforms.
    if let Some(monitor) = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
    {
        let sf = window.scale_factor().unwrap_or(1.0);
        let m_pos = monitor.position();
        let m_size = monitor.size();
        let m_w_logical = m_size.width as f64 / sf;
        let m_h_logical = m_size.height as f64 / sf;
        let m_x_logical = m_pos.x as f64 / sf;
        let m_y_logical = m_pos.y as f64 / sf;
        let x = m_x_logical + (m_w_logical - SPOTLIGHT_WIDTH) / 2.0;
        let y = m_y_logical + m_h_logical * 0.22;
        window
            .set_position(Position::Logical(LogicalPosition::new(x, y)))
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}
