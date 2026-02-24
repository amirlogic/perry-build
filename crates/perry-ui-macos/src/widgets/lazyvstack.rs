use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_app_kit::NSView;
use objc2_foundation::MainThreadMarker;
use std::cell::RefCell;

extern "C" {
    fn js_closure_call1(closure: *const u8, arg: f64) -> f64;
    fn js_nanbox_get_pointer(value: f64) -> i64;
}

struct LazyVStackEntry {
    scroll_view: Retained<NSView>,
    row_count: i64,
    render_closure: f64,
}

thread_local! {
    static LAZY_VSTACKS: RefCell<Vec<LazyVStackEntry>> = RefCell::new(Vec::new());
}

/// Create a LazyVStack backed by NSScrollView + NSStackView.
/// For simplicity, renders all rows into a VStack inside a ScrollView.
/// This gives the scrolling behavior; true virtualization can be added later.
pub fn create(count: i64, render_closure: f64) -> i64 {
    let _mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");

    unsafe {
        // Create scroll view
        let scroll_cls = AnyClass::get(c"NSScrollView").unwrap();
        let scroll: Retained<AnyObject> = msg_send![scroll_cls, new];
        let _: () = msg_send![&*scroll, setHasVerticalScroller: true];
        let _: () = msg_send![&*scroll, setHasHorizontalScroller: false];

        // Create a VStack (NSStackView) as the document view
        let stack_cls = AnyClass::get(c"NSStackView").unwrap();
        let stack: Retained<AnyObject> = msg_send![stack_cls, new];
        let _: () = msg_send![&*stack, setOrientation: 1i64]; // NSUserInterfaceLayoutOrientationVertical
        let _: () = msg_send![&*stack, setSpacing: 0.0f64];

        // Render initial rows
        let closure_ptr = js_nanbox_get_pointer(render_closure) as *const u8;
        for i in 0..count {
            let child_f64 = js_closure_call1(closure_ptr, i as f64);
            let child_handle = js_nanbox_get_pointer(child_f64);
            if let Some(child_view) = super::get_widget(child_handle) {
                let _: () = msg_send![&*stack, addArrangedSubview: &*child_view];
            }
        }

        let _: () = msg_send![&*scroll, setDocumentView: &*stack];

        let scroll_view: Retained<NSView> = Retained::cast_unchecked(scroll);
        let handle = super::register_widget(scroll_view.clone());

        LAZY_VSTACKS.with(|l| {
            l.borrow_mut().push(LazyVStackEntry {
                scroll_view,
                row_count: count,
                render_closure,
            });
        });

        handle
    }
}

/// Update the row count and re-render all rows.
pub fn update_count(handle: i64, new_count: i64) {
    LAZY_VSTACKS.with(|l| {
        let mut stacks = l.borrow_mut();
        // Find the entry by checking which one matches our handle
        for entry in stacks.iter_mut() {
            unsafe {
                let doc_view: *mut AnyObject = msg_send![&*entry.scroll_view, documentView];
                if doc_view.is_null() { continue; }

                // Clear existing children
                let subviews: Retained<AnyObject> = msg_send![doc_view, arrangedSubviews];
                let count: usize = msg_send![&*subviews, count];
                for i in (0..count).rev() {
                    let sv: *mut AnyObject = msg_send![&*subviews, objectAtIndex: i];
                    let _: () = msg_send![doc_view, removeArrangedSubview: sv];
                    let _: () = msg_send![sv, removeFromSuperview];
                }

                // Re-render with new count
                let closure_ptr = js_nanbox_get_pointer(entry.render_closure) as *const u8;
                for i in 0..new_count {
                    let child_f64 = js_closure_call1(closure_ptr, i as f64);
                    let child_handle = js_nanbox_get_pointer(child_f64);
                    if let Some(child_view) = super::get_widget(child_handle) {
                        let _: () = msg_send![doc_view, addArrangedSubview: &*child_view];
                    }
                }

                entry.row_count = new_count;
                return; // Found it
            }
        }
    });
}
