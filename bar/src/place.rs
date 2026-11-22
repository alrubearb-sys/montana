/// Top left corner for a panel of this width, below the menu bar and centered on the mouse.
#[cfg(target_os = "macos")]
pub fn below_menu_bar(width: f64) -> Option<(f64, f64)> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSScreen};

    const MARGIN: f64 = 8.0;

    let main_thread = MainThreadMarker::new()?;
    let mouse = NSEvent::mouseLocation();
    let screens = NSScreen::screens(main_thread);
    let primary = screens.firstObject()?.frame();
    let visible = screens
        .iter()
        .find(|screen| {
            let frame = screen.frame();
            (frame.origin.x..=frame.origin.x + frame.size.width).contains(&mouse.x)
                && (frame.origin.y..=frame.origin.y + frame.size.height).contains(&mouse.y)
        })
        .map_or(primary, |screen| screen.visibleFrame());

    let first = visible.origin.x + MARGIN;
    let last = visible.origin.x + visible.size.width - width - MARGIN;
    let left = (mouse.x - width / 2.0).min(last).max(first);
    // AppKit measures upwards from the bottom of the primary screen.
    let top = primary.size.height - (visible.origin.y + visible.size.height);
    Some((left, top))
}

#[cfg(not(target_os = "macos"))]
pub fn below_menu_bar(_width: f64) -> Option<(f64, f64)> {
    None
}

/// Makes this the active app, so that a click anywhere else takes the focus away from the panel.
#[cfg(target_os = "macos")]
pub fn activate() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSColorSpace};

    let Some(main_thread) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(main_thread);
    #[expect(
        deprecated,
        reason = "the new method may leave a menu bar app inactive"
    )]
    app.activateIgnoringOtherApps(true);
    // Without it, macOS converts every pixel to the colors of the screen on each redraw.
    let colors = NSColorSpace::sRGBColorSpace();
    for window in app.windows() {
        window.setColorSpace(Some(&colors));
    }
}

#[cfg(not(target_os = "macos"))]
pub fn activate() {}
