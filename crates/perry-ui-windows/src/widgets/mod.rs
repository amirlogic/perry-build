//! Widget registry — Vec<WidgetEntry> with 1-based handles.
//! Each widget has an HWND (on Windows), a kind, children list, and layout info.

pub mod text;
pub mod button;
pub mod vstack;
pub mod hstack;
pub mod spacer;
pub mod divider;
pub mod textfield;
pub mod toggle;
pub mod slider;
pub mod scrollview;

use std::cell::RefCell;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Debug, PartialEq)]
pub enum WidgetKind {
    Text,
    Button,
    VStack,
    HStack,
    Spacer,
    Divider,
    TextField,
    Toggle,
    Slider,
    ScrollView,
}

pub struct WidgetEntry {
    #[cfg(target_os = "windows")]
    pub hwnd: HWND,
    #[cfg(not(target_os = "windows"))]
    pub hwnd: isize,
    pub kind: WidgetKind,
    pub children: Vec<i64>,
    pub spacing: f64,
    pub insets: (f64, f64, f64, f64), // top, left, bottom, right
    pub hidden: bool,
    /// Win32 control ID (for WM_COMMAND routing)
    pub control_id: u16,
}

/// Info returned by get_widget_info (clone-safe subset)
pub struct WidgetInfo {
    pub kind: WidgetKind,
    pub children: Vec<i64>,
    pub spacing: f64,
    pub insets: (f64, f64, f64, f64),
    pub hidden: bool,
}

thread_local! {
    static WIDGETS: RefCell<Vec<WidgetEntry>> = RefCell::new(Vec::new());
    static NEXT_CONTROL_ID: RefCell<u16> = RefCell::new(1000);
}

/// Allocate a new control ID.
pub fn alloc_control_id() -> u16 {
    NEXT_CONTROL_ID.with(|id| {
        let mut id = id.borrow_mut();
        let current = *id;
        *id += 1;
        current
    })
}

/// Register a widget entry and return its 1-based handle.
#[cfg(target_os = "windows")]
pub fn register_widget(hwnd: HWND, kind: WidgetKind, control_id: u16) -> i64 {
    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        widgets.push(WidgetEntry {
            hwnd,
            kind,
            children: Vec::new(),
            spacing: 0.0,
            insets: (0.0, 0.0, 0.0, 0.0),
            hidden: false,
            control_id,
        });
        widgets.len() as i64
    })
}

#[cfg(not(target_os = "windows"))]
pub fn register_widget(hwnd: isize, kind: WidgetKind, control_id: u16) -> i64 {
    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        widgets.push(WidgetEntry {
            hwnd,
            kind,
            children: Vec::new(),
            spacing: 0.0,
            insets: (0.0, 0.0, 0.0, 0.0),
            hidden: false,
            control_id,
        });
        widgets.len() as i64
    })
}

/// Register a widget with spacing and insets (for stacks).
#[cfg(target_os = "windows")]
pub fn register_widget_with_layout(hwnd: HWND, kind: WidgetKind, spacing: f64, insets: (f64, f64, f64, f64)) -> i64 {
    let control_id = alloc_control_id();
    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        widgets.push(WidgetEntry {
            hwnd,
            kind,
            children: Vec::new(),
            spacing,
            insets,
            hidden: false,
            control_id,
        });
        widgets.len() as i64
    })
}

#[cfg(not(target_os = "windows"))]
pub fn register_widget_with_layout(hwnd: isize, kind: WidgetKind, spacing: f64, insets: (f64, f64, f64, f64)) -> i64 {
    let control_id = alloc_control_id();
    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        widgets.push(WidgetEntry {
            hwnd,
            kind,
            children: Vec::new(),
            spacing,
            insets,
            hidden: false,
            control_id,
        });
        widgets.len() as i64
    })
}

/// Get the HWND for a widget handle.
#[cfg(target_os = "windows")]
pub fn get_hwnd(handle: i64) -> Option<HWND> {
    WIDGETS.with(|w| {
        let widgets = w.borrow();
        let idx = (handle - 1) as usize;
        if idx < widgets.len() {
            Some(widgets[idx].hwnd)
        } else {
            None
        }
    })
}

#[cfg(not(target_os = "windows"))]
pub fn get_hwnd(handle: i64) -> Option<isize> {
    WIDGETS.with(|w| {
        let widgets = w.borrow();
        let idx = (handle - 1) as usize;
        if idx < widgets.len() {
            Some(widgets[idx].hwnd)
        } else {
            None
        }
    })
}

/// Get widget info (clone-safe subset).
pub fn get_widget_info(handle: i64) -> Option<WidgetInfo> {
    WIDGETS.with(|w| {
        let widgets = w.borrow();
        let idx = (handle - 1) as usize;
        if idx < widgets.len() {
            Some(WidgetInfo {
                kind: widgets[idx].kind.clone(),
                children: widgets[idx].children.clone(),
                spacing: widgets[idx].spacing,
                insets: widgets[idx].insets,
                hidden: widgets[idx].hidden,
            })
        } else {
            None
        }
    })
}

/// Find the widget handle that owns a given HWND.
#[cfg(target_os = "windows")]
pub fn find_handle_by_hwnd(hwnd: HWND) -> i64 {
    WIDGETS.with(|w| {
        let widgets = w.borrow();
        for (i, widget) in widgets.iter().enumerate() {
            if widget.hwnd == hwnd {
                return (i + 1) as i64;
            }
        }
        0
    })
}

#[cfg(not(target_os = "windows"))]
pub fn find_handle_by_hwnd(_hwnd: isize) -> i64 { 0 }

/// Find widget handle by control ID.
pub fn find_handle_by_control_id(id: u16) -> i64 {
    WIDGETS.with(|w| {
        let widgets = w.borrow();
        for (i, widget) in widgets.iter().enumerate() {
            if widget.control_id == id {
                return (i + 1) as i64;
            }
        }
        0
    })
}

/// Add a child widget to a parent container.
pub fn add_child(parent_handle: i64, child_handle: i64) {
    #[cfg(target_os = "windows")]
    {
        // Re-parent the child HWND
        if let (Some(parent_hwnd), Some(child_hwnd)) = (get_hwnd(parent_handle), get_hwnd(child_handle)) {
            unsafe {
                let _ = SetParent(child_hwnd, Some(parent_hwnd));
                let style = GetWindowLongW(child_hwnd, GWL_STYLE) as u32;
                SetWindowLongW(child_hwnd, GWL_STYLE, (style | WS_CHILD.0 | WS_VISIBLE.0) as i32);
            }
        }
    }

    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        let idx = (parent_handle - 1) as usize;
        if idx < widgets.len() {
            widgets[idx].children.push(child_handle);
        }
    });
}

/// Add a child widget at a specific index.
pub fn add_child_at(parent_handle: i64, child_handle: i64, index: i64) {
    #[cfg(target_os = "windows")]
    {
        if let (Some(parent_hwnd), Some(child_hwnd)) = (get_hwnd(parent_handle), get_hwnd(child_handle)) {
            unsafe {
                let _ = SetParent(child_hwnd, Some(parent_hwnd));
                let style = GetWindowLongW(child_hwnd, GWL_STYLE) as u32;
                SetWindowLongW(child_hwnd, GWL_STYLE, (style | WS_CHILD.0 | WS_VISIBLE.0) as i32);
            }
        }
    }

    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        let idx = (parent_handle - 1) as usize;
        if idx < widgets.len() {
            let insert_at = (index as usize).min(widgets[idx].children.len());
            widgets[idx].children.insert(insert_at, child_handle);
        }
    });
}

/// Remove all children from a container widget.
pub fn clear_children(handle: i64) {
    let children: Vec<i64> = WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        let idx = (handle - 1) as usize;
        if idx < widgets.len() {
            widgets[idx].children.drain(..).collect()
        } else {
            Vec::new()
        }
    });

    #[cfg(target_os = "windows")]
    {
        for child in &children {
            if let Some(child_hwnd) = get_hwnd(*child) {
                unsafe {
                    let _ = ShowWindow(child_hwnd, SW_HIDE);
                    let _ = SetParent(child_hwnd, None);
                }
            }
        }
    }

    let _ = children;
}

/// Set the hidden state of a widget.
pub fn set_hidden(handle: i64, hidden: bool) {
    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        let idx = (handle - 1) as usize;
        if idx < widgets.len() {
            widgets[idx].hidden = hidden;

            #[cfg(target_os = "windows")]
            {
                let hwnd = widgets[idx].hwnd;
                unsafe {
                    let _ = ShowWindow(hwnd, if hidden { SW_HIDE } else { SW_SHOW });
                }
            }
        }
    });
}

/// Handle WM_COMMAND from WndProc — dispatch to button/textfield/toggle callbacks.
#[cfg(target_os = "windows")]
pub fn handle_command(control_id: u16, notify_code: u16, _lparam: LPARAM) {
    // BN_CLICKED = 0
    if notify_code == 0 {
        // Could be a button click or toggle click
        let handle = find_handle_by_control_id(control_id);
        if handle > 0 {
            let kind = WIDGETS.with(|w| {
                let widgets = w.borrow();
                let idx = (handle - 1) as usize;
                if idx < widgets.len() {
                    Some(widgets[idx].kind.clone())
                } else {
                    None
                }
            });
            match kind {
                Some(WidgetKind::Button) => button::handle_click(handle),
                Some(WidgetKind::Toggle) => toggle::handle_click(handle),
                _ => {}
            }
        }
    }
    // EN_CHANGE = 0x0300
    if notify_code == 0x0300 {
        let handle = find_handle_by_control_id(control_id);
        if handle > 0 {
            textfield::handle_change(handle);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn handle_command(_control_id: u16, _notify_code: u16, _lparam: isize) {}

/// Handle WM_HSCROLL/WM_VSCROLL — dispatch to slider or scrollview.
#[cfg(target_os = "windows")]
pub fn handle_scroll(wparam: WPARAM, lparam: LPARAM) {
    let child_hwnd = HWND(lparam.0 as *mut _);
    let handle = find_handle_by_hwnd(child_hwnd);
    if handle > 0 {
        let kind = WIDGETS.with(|w| {
            let widgets = w.borrow();
            let idx = (handle - 1) as usize;
            if idx < widgets.len() {
                Some(widgets[idx].kind.clone())
            } else {
                None
            }
        });
        match kind {
            Some(WidgetKind::Slider) => slider::handle_scroll(handle),
            _ => {}
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn handle_scroll(_wparam: usize, _lparam: isize) {}
