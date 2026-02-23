//! Divider widget — STATIC control with SS_ETCHEDHORZ style (etched horizontal line)

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::*;
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use super::{WidgetKind, alloc_control_id, register_widget};

#[cfg(target_os = "windows")]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Create a Divider. Returns widget handle.
pub fn create() -> i64 {
    let control_id = alloc_control_id();

    #[cfg(target_os = "windows")]
    {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::PCWSTR(to_wide("STATIC").as_ptr()),
                windows::core::PCWSTR(to_wide("").as_ptr()),
                WINDOW_STYLE(SS_ETCHEDHORZ as u32 | WS_CHILD.0 | WS_VISIBLE.0),
                0, 0, 100, 2,
                None,
                HMENU(control_id as *mut _),
                Some(hinstance.into()),
                None,
            ).unwrap();

            register_widget(hwnd, WidgetKind::Divider, control_id)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        register_widget(0, WidgetKind::Divider, control_id)
    }
}
