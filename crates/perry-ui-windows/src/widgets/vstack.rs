//! VStack widget — custom window class for vertical layout container

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::*;
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use super::{WidgetKind, register_widget_with_layout};

#[cfg(target_os = "windows")]
static VSTACK_CLASS_REGISTERED: std::sync::Once = std::sync::Once::new();

#[cfg(target_os = "windows")]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn ensure_class_registered() {
    VSTACK_CLASS_REGISTERED.call_once(|| {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let class_name = to_wide("PerryVStack");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(container_wnd_proc),
                hInstance: hinstance.into(),
                hbrBackground: HBRUSH(std::ptr::null_mut()), // transparent
                lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            RegisterClassExW(&wc);
        }
    });
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn container_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Create a VStack. Returns widget handle.
pub fn create(spacing: f64) -> i64 {
    create_with_insets(spacing, 0.0, 0.0, 0.0, 0.0)
}

/// Create a VStack with custom insets. Returns widget handle.
pub fn create_with_insets(spacing: f64, top: f64, left: f64, bottom: f64, right: f64) -> i64 {
    #[cfg(target_os = "windows")]
    {
        ensure_class_registered();
        let class_name = to_wide("PerryVStack");
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::PCWSTR(class_name.as_ptr()),
                windows::core::PCWSTR(to_wide("").as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
                0, 0, 100, 100,
                None,
                None,
                Some(hinstance.into()),
                None,
            ).unwrap();

            register_widget_with_layout(hwnd, WidgetKind::VStack, spacing, (top, left, bottom, right))
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        register_widget_with_layout(0, WidgetKind::VStack, spacing, (top, left, bottom, right))
    }
}
