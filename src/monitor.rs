/// Query X11 for the cursor position and monitor geometries,
/// then return the top-left position to center a window of
/// the given size on the monitor where the cursor is.
#[cfg(target_os = "linux")]
pub fn center_on_cursor_monitor(window_width: f32, window_height: f32) -> Option<(f32, f32)> {
    unsafe {
        let xlib = x11_dl::xlib::Xlib::open().ok()?;
        let xinerama = x11_dl::xinerama::Xlib::open().ok()?;

        let display = (xlib.XOpenDisplay)(std::ptr::null());
        if display.is_null() {
            return None;
        }

        // Query cursor position
        let root = (xlib.XDefaultRootWindow)(display);
        let mut root_return = 0;
        let mut child_return = 0;
        let mut cursor_x = 0;
        let mut cursor_y = 0;
        let mut win_x = 0;
        let mut win_y = 0;
        let mut mask = 0;
        (xlib.XQueryPointer)(
            display,
            root,
            &mut root_return,
            &mut child_return,
            &mut cursor_x,
            &mut cursor_y,
            &mut win_x,
            &mut win_y,
            &mut mask,
        );

        // Query monitor geometries via Xinerama
        let mut num_screens = 0;
        let screens = (xinerama.XineramaQueryScreens)(display, &mut num_screens);

        let mut target: Option<(i16, i16, i16, i16)> = None;
        if !screens.is_null() && num_screens > 0 {
            for i in 0..num_screens {
                let s = *screens.offset(i as isize);
                let sx = s.x_org as i32;
                let sy = s.y_org as i32;
                let sw = s.width as i32;
                let sh = s.height as i32;
                if cursor_x >= sx && cursor_x < sx + sw && cursor_y >= sy && cursor_y < sy + sh {
                    target = Some((s.x_org, s.y_org, s.width, s.height));
                    break;
                }
            }
            (xlib.XFree)(screens as *mut _);
        }

        (xlib.XCloseDisplay)(display);

        let (mx, my, mw, mh) = target?;
        let center_x = mx as f32 + (mw as f32 - window_width) / 2.0;
        let center_y = my as f32 + (mh as f32 - window_height) / 2.0;
        Some((center_x, center_y))
    }
}

#[cfg(not(target_os = "linux"))]
pub fn center_on_cursor_monitor(_window_width: f32, _window_height: f32) -> Option<(f32, f32)> {
    None
}
