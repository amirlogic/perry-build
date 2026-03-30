//! Image widget — Win32 STATIC control with SS_BITMAP for file images,
//! or system icon display for symbol images.

use std::cell::RefCell;
use std::collections::HashMap;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::*;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::InvalidateRect;
#[cfg(target_os = "windows")]
use windows::Win32::System::SystemServices::{SS_BITMAP, SS_ICON};
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use super::{WidgetKind, alloc_control_id, register_widget};

fn str_from_header(ptr: *const u8) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    unsafe {
        let header = ptr as *const perry_runtime::string::StringHeader;
        let len = (*header).length as usize;
        let data = ptr.add(std::mem::size_of::<perry_runtime::string::StringHeader>());
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(data, len))
    }
}

#[cfg(target_os = "windows")]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// STM_SETIMAGE message
#[cfg(target_os = "windows")]
const STM_SETIMAGE: u32 = 0x0172;

/// Per-widget tint color (limited use on Win32 — stored for potential custom draw)
struct ImageTint {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

thread_local! {
    static IMAGE_TINTS: RefCell<HashMap<i64, ImageTint>> = RefCell::new(HashMap::new());
    /// Store file paths so we can reload at different sizes
    static IMAGE_PATHS: RefCell<HashMap<i64, String>> = RefCell::new(HashMap::new());
}

/// Load an image file (PNG, JPEG, etc.) via GDI+ and return as HBITMAP.
/// `bg_color` is the COLORREF used to fill transparent areas (default: white).
#[cfg(target_os = "windows")]
fn load_image_gdiplus(wide_path: &[u16], bg_color: u32) -> Option<windows::Win32::Graphics::Gdi::HBITMAP> {
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::Graphics::GdiPlus::*;

    unsafe {
        // Initialize GDI+
        let mut token: usize = 0;
        let input = GdiplusStartupInput {
            GdiplusVersion: 1,
            ..Default::default()
        };
        let status = GdiplusStartup(&mut token, &input, std::ptr::null_mut());
        if status.0 != 0 {
            return None;
        }

        // Load image from file
        let mut gp_image: *mut GpImage = std::ptr::null_mut();
        let status = GdipLoadImageFromFile(windows::core::PCWSTR(wide_path.as_ptr()), &mut gp_image);
        if status.0 != 0 || gp_image.is_null() {
            GdiplusShutdown(token);
            return None;
        }

        // Get dimensions
        let mut width: u32 = 0;
        let mut height: u32 = 0;
        GdipGetImageWidth(gp_image, &mut width);
        GdipGetImageHeight(gp_image, &mut height);
        if width == 0 || height == 0 {
            GdipDisposeImage(gp_image);
            GdiplusShutdown(token);
            return None;
        }

        // Create a GDI+ graphics context on a memory DC and draw the image.
        // Pre-fill with bg_color so PNG transparency composites against the parent.
        let screen_dc = GetDC(None);
        let mem_dc = CreateCompatibleDC(screen_dc);
        let hbitmap = CreateCompatibleBitmap(screen_dc, width as i32, height as i32);
        let old_bmp = SelectObject(mem_dc, hbitmap);

        let bg_rect = RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 };
        let bg_brush = CreateSolidBrush(COLORREF(bg_color));
        FillRect(mem_dc, &bg_rect, bg_brush);
        let _ = DeleteObject(bg_brush);

        let mut graphics: *mut GpGraphics = std::ptr::null_mut();
        GdipCreateFromHDC(mem_dc, &mut graphics);
        if !graphics.is_null() {
            GdipDrawImageRectI(graphics, gp_image, 0, 0, width as i32, height as i32);
            GdipDeleteGraphics(graphics);
        }

        SelectObject(mem_dc, old_bmp);
        DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);
        GdipDisposeImage(gp_image);
        GdiplusShutdown(token);

        Some(hbitmap)
    }
}

/// Load an image scaled to specific dimensions via GDI+.
/// `bg_color` is the COLORREF used to fill transparent areas.
#[cfg(target_os = "windows")]
fn load_image_gdiplus_scaled(wide_path: &[u16], target_w: i32, target_h: i32, bg_color: u32) -> Option<windows::Win32::Graphics::Gdi::HBITMAP> {
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::Graphics::GdiPlus::*;

    unsafe {
        let mut token: usize = 0;
        let input = GdiplusStartupInput { GdiplusVersion: 1, ..Default::default() };
        let status = GdiplusStartup(&mut token, &input, std::ptr::null_mut());
        if status.0 != 0 { return None; }

        let mut gp_image: *mut GpImage = std::ptr::null_mut();
        let status = GdipLoadImageFromFile(windows::core::PCWSTR(wide_path.as_ptr()), &mut gp_image);
        if status.0 != 0 || gp_image.is_null() {
            GdiplusShutdown(token);
            return None;
        }

        let screen_dc = GetDC(None);
        let mem_dc = CreateCompatibleDC(screen_dc);
        let hbitmap = CreateCompatibleBitmap(screen_dc, target_w, target_h);
        let old_bmp = SelectObject(mem_dc, hbitmap);

        let bg_rect = RECT { left: 0, top: 0, right: target_w, bottom: target_h };
        let bg_brush = CreateSolidBrush(COLORREF(bg_color));
        FillRect(mem_dc, &bg_rect, bg_brush);
        let _ = DeleteObject(bg_brush);

        let mut graphics: *mut GpGraphics = std::ptr::null_mut();
        GdipCreateFromHDC(mem_dc, &mut graphics);
        if !graphics.is_null() {
            // Set high quality interpolation for scaling
            GdipSetInterpolationMode(graphics, InterpolationMode(7)); // HighQualityBicubic
            GdipDrawImageRectI(graphics, gp_image, 0, 0, target_w, target_h);
            GdipDeleteGraphics(graphics);
        }

        SelectObject(mem_dc, old_bmp);
        DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);
        GdipDisposeImage(gp_image);
        GdiplusShutdown(token);

        Some(hbitmap)
    }
}

/// Resolve a relative asset path against the executable's directory first,
/// falling back to the path as-is (relative to cwd). Matches macOS/GTK behavior.
#[cfg(target_os = "windows")]
fn resolve_asset_path(path: &str) -> String {
    if std::path::Path::new(path).is_absolute() {
        return path.to_string();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidate = exe_dir.join(path);
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }
    path.to_string()
}

/// Create an Image from a file path. Returns widget handle.
pub fn create_file(path_ptr: *const u8) -> i64 {
    let path = str_from_header(path_ptr);
    let control_id = alloc_control_id();

    #[cfg(target_os = "windows")]
    {
        // Resolve relative paths against the exe directory (parity with macOS/GTK)
        let resolved = resolve_asset_path(path);

        let class_name = to_wide("STATIC");
        let window_text = to_wide("");
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::PCWSTR(class_name.as_ptr()),
                windows::core::PCWSTR(window_text.as_ptr()),
                WINDOW_STYLE(SS_BITMAP.0 | WS_CHILD.0 | WS_VISIBLE.0),
                0, 0, 100, 100,
                super::get_parking_hwnd(),
                HMENU(control_id as *mut _),
                HINSTANCE::from(hinstance),
                None,
            )
            .unwrap();

            // Load the image from file — try GDI+ for PNG/JPEG support,
            // fall back to LoadImageW for BMP/ICO.
            // At creation time the widget has no parent yet, so use white fallback.
            let wide_path = to_wide(&resolved);
            let hbitmap = load_image_gdiplus(&wide_path, 0x00FFFFFF);

            // Fall back to LoadImageW for BMP/ICO
            let hbitmap_handle = hbitmap.map(|b| b.0 as isize).or_else(|| {
                LoadImageW(
                    None,
                    windows::core::PCWSTR(wide_path.as_ptr()),
                    IMAGE_BITMAP,
                    0, 0,
                    LR_LOADFROMFILE | LR_DEFAULTSIZE,
                ).ok().map(|h| h.0 as isize)
            });

            if let Some(hbmp) = hbitmap_handle {
                SendMessageW(
                    hwnd,
                    STM_SETIMAGE,
                    WPARAM(IMAGE_BITMAP.0 as usize),
                    LPARAM(hbmp),
                );
            }

            let handle = register_widget(hwnd, WidgetKind::Image, control_id);
            // Store the resolved path so reload_bitmap_scaled can find the file
            IMAGE_PATHS.with(|p| p.borrow_mut().insert(handle, resolved));
            handle
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        register_widget(0, WidgetKind::Image, control_id)
    }
}

/// Create an Image from a system symbol/icon name. Returns widget handle.
pub fn create_symbol(name_ptr: *const u8) -> i64 {
    let name = str_from_header(name_ptr);
    let control_id = alloc_control_id();

    #[cfg(target_os = "windows")]
    {
        let class_name = to_wide("STATIC");
        let window_text = to_wide("");
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::PCWSTR(class_name.as_ptr()),
                windows::core::PCWSTR(window_text.as_ptr()),
                WINDOW_STYLE(SS_ICON.0 | WS_CHILD.0 | WS_VISIBLE.0),
                0, 0, 32, 32,
                super::get_parking_hwnd(),
                HMENU(control_id as *mut _),
                HINSTANCE::from(hinstance),
                None,
            )
            .unwrap();

            // Map common symbol names to system icons
            let icon_id = match name {
                "exclamationmark.triangle" | "warning" => IDI_WARNING,
                "info.circle" | "info" => IDI_INFORMATION,
                "xmark.circle" | "error" => IDI_ERROR,
                "questionmark.circle" | "question" => IDI_QUESTION,
                "app" | "application" => IDI_APPLICATION,
                "shield" | "shield.fill" => IDI_SHIELD,
                _ => IDI_APPLICATION,
            };

            let hicon = LoadIconW(None, icon_id);
            if let Ok(hicon) = hicon {
                SendMessageW(
                    hwnd,
                    STM_SETIMAGE,
                    WPARAM(IMAGE_ICON.0 as usize),
                    LPARAM(hicon.0 as isize),
                );
            }

            register_widget(hwnd, WidgetKind::Image, control_id)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = name;
        register_widget(0, WidgetKind::Image, control_id)
    }
}

/// Reload the bitmap scaled to the given pixel dimensions.
/// Called by `set_size` and by the layout engine after `MoveWindow`.
/// Uses the nearest ancestor's background color for transparency compositing
/// so the image blends with its parent (gradient or solid) instead of showing white.
#[cfg(target_os = "windows")]
pub fn reload_bitmap_scaled(handle: i64, w: i32, h: i32) {
    if w <= 0 || h <= 0 { return; }
    let path = IMAGE_PATHS.with(|p| p.borrow().get(&handle).cloned());
    if let Some(path) = path {
        if let Some(hwnd) = super::get_hwnd(handle) {
            let bg_color = super::find_ancestor_hwnd_bg_color(hwnd).unwrap_or(0x00FFFFFF);
            let wide_path = to_wide(&path);
            if let Some(hbitmap) = load_image_gdiplus_scaled(&wide_path, w, h, bg_color) {
                unsafe {
                    SendMessageW(hwnd, STM_SETIMAGE,
                        WPARAM(IMAGE_BITMAP.0 as usize),
                        LPARAM(hbitmap.0 as isize));
                }
            }
        }
    }
}

/// Set the size of an Image widget. Reloads the bitmap scaled to the new size.
pub fn set_size(handle: i64, width: f64, height: f64) {
    // Also set fixed dimensions so the layout engine uses these
    super::set_fixed_width(handle, width as i32);
    super::set_fixed_height(handle, height as i32);

    #[cfg(target_os = "windows")]
    {
        if let Some(hwnd) = super::get_hwnd(handle) {
            let w = width as i32;
            let h = height as i32;
            unsafe {
                let _ = SetWindowPos(hwnd, None, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER);
            }
            reload_bitmap_scaled(handle, w, h);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, width, height);
    }
}

/// Set the tint color for an Image widget.
/// On Win32, tinting is limited — we store the color for potential custom-draw use.
pub fn set_tint(handle: i64, r: f64, g: f64, b: f64, a: f64) {
    IMAGE_TINTS.with(|tints| {
        tints.borrow_mut().insert(handle, ImageTint {
            r: (r * 255.0) as u8,
            g: (g * 255.0) as u8,
            b: (b * 255.0) as u8,
            a: (a * 255.0) as u8,
        });
    });

    #[cfg(target_os = "windows")]
    {
        // Force repaint (custom-draw could use the tint if implemented)
        if let Some(hwnd) = super::get_hwnd(handle) {
            unsafe {
                let _ = InvalidateRect(hwnd, None, true);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
    }
}
