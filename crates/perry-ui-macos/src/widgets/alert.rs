use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{MainThreadMarker, NSString};

extern "C" {
    fn js_closure_call1(closure: *const u8, arg: f64) -> f64;
    fn js_nanbox_get_pointer(value: f64) -> i64;
}

fn str_from_header(ptr: *const u8) -> &'static str {
    if ptr.is_null() { return ""; }
    unsafe {
        let header = ptr as *const crate::string_header::StringHeader;
        let len = (*header).length as usize;
        let data = ptr.add(std::mem::size_of::<crate::string_header::StringHeader>());
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(data, len))
    }
}

/// Show an alert dialog with title, message, buttons array, and callback.
/// buttons_ptr is a NaN-boxed pointer to a JS array of strings.
/// callback receives the button index (0-based).
pub fn show(title_ptr: *const u8, message_ptr: *const u8, buttons_ptr: i64, callback: f64) {
    let _mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");
    let title = str_from_header(title_ptr);
    let message = str_from_header(message_ptr);

    unsafe {
        let alert_cls = AnyClass::get(c"NSAlert").unwrap();
        let alert: Retained<AnyObject> = msg_send![alert_cls, new];

        let ns_title = NSString::from_str(title);
        let _: () = msg_send![&*alert, setMessageText: &*ns_title];

        let ns_message = NSString::from_str(message);
        let _: () = msg_send![&*alert, setInformativeText: &*ns_message];

        // Extract button labels from JS array
        extern "C" {
            fn js_array_get_length(arr: i64) -> i64;
            fn js_array_get_element(arr: i64, index: i64) -> f64;
            fn js_get_string_pointer_unified(val: f64) -> i64;
        }

        let arr_ptr = js_nanbox_get_pointer(f64::from_bits(buttons_ptr as u64));
        let len = js_array_get_length(arr_ptr);
        for i in 0..len {
            let elem = js_array_get_element(arr_ptr, i);
            let str_ptr = js_get_string_pointer_unified(elem) as *const u8;
            let label = str_from_header(str_ptr);
            let ns_label = NSString::from_str(label);
            let _: Retained<AnyObject> = msg_send![&*alert, addButtonWithTitle: &*ns_label];
        }

        // Run modal
        let response: isize = msg_send![&*alert, runModal];
        // NSAlertFirstButtonReturn = 1000
        let button_index = (response - 1000) as f64;

        let closure_ptr = js_nanbox_get_pointer(callback) as *const u8;
        js_closure_call1(closure_ptr, button_index);
    }
}
