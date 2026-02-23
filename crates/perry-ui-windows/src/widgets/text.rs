//! Text widget — STATIC control (SS_LEFT) with custom color/font support

use std::cell::RefCell;
use std::collections::HashMap;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::*;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::*;
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

/// Per-widget text color (COLORREF) and background brush
#[cfg(target_os = "windows")]
struct TextStyle {
    color: u32,         // COLORREF (0x00BBGGRR)
    bg_brush: HBRUSH,
    font: HFONT,
}

#[cfg(not(target_os = "windows"))]
struct TextStyle {
    color: u32,
}

thread_local! {
    static TEXT_STYLES: RefCell<HashMap<i64, TextStyle>> = RefCell::new(HashMap::new());

    // Map from HWND (as isize) -> widget handle for fast WM_CTLCOLORSTATIC lookup
    static HWND_TO_HANDLE: RefCell<HashMap<isize, i64>> = RefCell::new(HashMap::new());
}

/// Create a Text label. Returns widget handle.
pub fn create(text_ptr: *const u8) -> i64 {
    let text = str_from_header(text_ptr);
    let control_id = alloc_control_id();

    #[cfg(target_os = "windows")]
    {
        let wide = to_wide(text);
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::PCWSTR(to_wide("STATIC").as_ptr()),
                windows::core::PCWSTR(wide.as_ptr()),
                WINDOW_STYLE(SS_LEFT as u32 | WS_CHILD.0 | WS_VISIBLE.0),
                0, 0, 100, 20,
                None,
                HMENU(control_id as *mut _),
                Some(hinstance.into()),
                None,
            ).unwrap();

            let handle = register_widget(hwnd, WidgetKind::Text, control_id);

            HWND_TO_HANDLE.with(|m| {
                m.borrow_mut().insert(hwnd.0 as isize, handle);
            });

            handle
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = text;
        register_widget(0, WidgetKind::Text, control_id)
    }
}

/// Set the text string of a Text widget from a raw string pointer.
pub fn set_string(handle: i64, text_ptr: *const u8) {
    let text = str_from_header(text_ptr);
    set_text_str(handle, text);
}

/// Set the text string of a Text widget from a &str (used by state bindings).
pub fn set_text_str(handle: i64, text: &str) {
    #[cfg(target_os = "windows")]
    {
        if let Some(hwnd) = super::get_hwnd(handle) {
            let wide = to_wide(text);
            unsafe {
                let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(wide.as_ptr()));
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, text);
    }
}

/// Set the text color (RGBA 0.0-1.0).
pub fn set_color(handle: i64, r: f64, g: f64, b: f64, _a: f64) {
    let cr = ((r * 255.0) as u32)
        | (((g * 255.0) as u32) << 8)
        | (((b * 255.0) as u32) << 16);

    #[cfg(target_os = "windows")]
    {
        // Get or create a null brush for transparent background
        let bg_brush = unsafe { GetStockObject(NULL_BRUSH) };
        let bg_brush = HBRUSH(bg_brush.0);

        TEXT_STYLES.with(|styles| {
            let mut styles = styles.borrow_mut();
            let entry = styles.entry(handle).or_insert(TextStyle {
                color: cr,
                bg_brush,
                font: HFONT::default(),
            });
            entry.color = cr;
            entry.bg_brush = bg_brush;
        });

        // Force repaint
        if let Some(hwnd) = super::get_hwnd(handle) {
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, cr);
    }
}

/// Set the font size of a Text widget.
pub fn set_font_size(handle: i64, size: f64) {
    #[cfg(target_os = "windows")]
    {
        let font = create_font(size as i32, 400); // FW_NORMAL = 400
        apply_font(handle, font);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, size);
    }
}

/// Set the font weight of a Text widget (size + weight).
pub fn set_font_weight(handle: i64, size: f64, weight: f64) {
    #[cfg(target_os = "windows")]
    {
        // Perry weight: 1.0 = bold. Map to Win32 weight (400=normal, 700=bold).
        let win32_weight = if weight >= 1.0 { 700 } else { 400 };
        let font = create_font(size as i32, win32_weight);
        apply_font(handle, font);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, size, weight);
    }
}

/// Set whether a Text widget is selectable.
pub fn set_selectable(handle: i64, _selectable: bool) {
    // Win32 STATIC controls are not selectable by default.
    // To make text selectable, we'd need to use an EDIT control with ES_READONLY.
    // For now, this is a no-op — selectable text can be implemented later by
    // swapping the STATIC with an ES_READONLY EDIT control.
    let _ = handle;
}

/// Handle WM_CTLCOLORSTATIC — set text color and background for styled text widgets.
#[cfg(target_os = "windows")]
pub fn handle_ctlcolor(hdc: HDC, child_hwnd: HWND) -> Option<LRESULT> {
    let handle = HWND_TO_HANDLE.with(|m| {
        m.borrow().get(&(child_hwnd.0 as isize)).copied()
    });

    let handle = handle?;

    TEXT_STYLES.with(|styles| {
        let styles = styles.borrow();
        if let Some(style) = styles.get(&handle) {
            unsafe {
                SetTextColor(hdc, COLORREF(style.color));
                SetBkMode(hdc, TRANSPARENT);
            }
            if !style.font.is_invalid() {
                unsafe { SelectObject(hdc, style.font); }
            }
            // Return the background brush
            Some(LRESULT(unsafe { GetStockObject(NULL_BRUSH) }.0 as isize))
        } else {
            None
        }
    })
}

#[cfg(target_os = "windows")]
fn create_font(size: i32, weight: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -size,              // nHeight (negative = character height)
            0,                  // nWidth (0 = default)
            0,                  // nEscapement
            0,                  // nOrientation
            weight,             // fnWeight
            0,                  // fdwItalic
            0,                  // fdwUnderline
            0,                  // fdwStrikeOut
            0,                  // fdwCharSet (DEFAULT_CHARSET)
            0,                  // fdwOutputPrecision
            0,                  // fdwClipPrecision
            0,                  // fdwQuality
            0,                  // fdwPitchAndFamily
            windows::core::PCWSTR(to_wide("Segoe UI").as_ptr()),
        )
    }
}

#[cfg(target_os = "windows")]
fn apply_font(handle: i64, font: HFONT) {
    TEXT_STYLES.with(|styles| {
        let mut styles = styles.borrow_mut();
        let entry = styles.entry(handle).or_insert(TextStyle {
            color: 0,
            bg_brush: HBRUSH::default(),
            font: HFONT::default(),
        });
        // Clean up old font
        if !entry.font.is_invalid() {
            unsafe { let _ = DeleteObject(entry.font); }
        }
        entry.font = font;
    });

    if let Some(hwnd) = super::get_hwnd(handle) {
        unsafe {
            SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        }
    }
}
