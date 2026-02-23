//! Button widget — Win32 BUTTON control (BS_PUSHBUTTON)

use std::cell::RefCell;
use std::collections::HashMap;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::*;
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use super::{WidgetKind, alloc_control_id, register_widget};

extern "C" {
    fn js_closure_call0(closure: *const u8) -> f64;
    fn js_nanbox_get_pointer(value: f64) -> i64;
}

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

thread_local! {
    // Map from widget handle -> callback pointer
    static BUTTON_CALLBACKS: RefCell<HashMap<i64, *const u8>> = RefCell::new(HashMap::new());
}

/// Create a Button. Returns widget handle.
pub fn create(label_ptr: *const u8, on_press: f64) -> i64 {
    let label = str_from_header(label_ptr);
    let callback_ptr = unsafe { js_nanbox_get_pointer(on_press) } as *const u8;
    let control_id = alloc_control_id();

    #[cfg(target_os = "windows")]
    {
        let wide = to_wide(label);
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::PCWSTR(to_wide("BUTTON").as_ptr()),
                windows::core::PCWSTR(wide.as_ptr()),
                WINDOW_STYLE(BS_PUSHBUTTON as u32 | WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0),
                0, 0, 80, 30,
                None,
                HMENU(control_id as *mut _),
                Some(hinstance.into()),
                None,
            ).unwrap();

            let handle = register_widget(hwnd, WidgetKind::Button, control_id);
            BUTTON_CALLBACKS.with(|cb| {
                cb.borrow_mut().insert(handle, callback_ptr);
            });
            handle
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = label;
        let handle = register_widget(0, WidgetKind::Button, control_id);
        BUTTON_CALLBACKS.with(|cb| {
            cb.borrow_mut().insert(handle, callback_ptr);
        });
        handle
    }
}

/// Handle button click (BN_CLICKED).
pub fn handle_click(handle: i64) {
    BUTTON_CALLBACKS.with(|cb| {
        let callbacks = cb.borrow();
        if let Some(&ptr) = callbacks.get(&handle) {
            unsafe { js_closure_call0(ptr) };
        }
    });
}

/// Set whether a Button has a visible border.
pub fn set_bordered(handle: i64, bordered: bool) {
    #[cfg(target_os = "windows")]
    {
        if let Some(hwnd) = super::get_hwnd(handle) {
            unsafe {
                let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
                let new_style = if bordered {
                    style | BS_PUSHBUTTON as u32
                } else {
                    // Use BS_FLAT for a borderless look
                    (style & !(BS_PUSHBUTTON as u32)) | BS_FLAT as u32
                };
                SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, bordered);
    }
}

/// Set the title text of a Button.
pub fn set_title(handle: i64, title_ptr: *const u8) {
    let title = str_from_header(title_ptr);

    #[cfg(target_os = "windows")]
    {
        if let Some(hwnd) = super::get_hwnd(handle) {
            let wide = to_wide(title);
            unsafe {
                let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(wide.as_ptr()));
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, title);
    }
}
