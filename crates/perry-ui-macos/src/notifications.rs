use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::NSString;

fn str_from_header(ptr: *const u8) -> &'static str {
    if ptr.is_null() { return ""; }
    unsafe {
        let header = ptr as *const crate::string_header::StringHeader;
        let len = (*header).length as usize;
        let data = ptr.add(std::mem::size_of::<crate::string_header::StringHeader>());
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(data, len))
    }
}

/// Send a local notification with title and body.
/// Note: On macOS, the app must be bundled (.app) for notifications to display.
pub fn send(title_ptr: *const u8, body_ptr: *const u8) {
    let title = str_from_header(title_ptr);
    let body = str_from_header(body_ptr);

    unsafe {
        // Create UNMutableNotificationContent
        let content_cls = AnyClass::get(c"UNMutableNotificationContent");
        if content_cls.is_none() {
            // UNUserNotificationCenter not available — fall back to NSUserNotification (deprecated but works unbundled)
            send_legacy(title, body);
            return;
        }
        let content_cls = content_cls.unwrap();
        let content: Retained<AnyObject> = msg_send![content_cls, new];

        let ns_title = NSString::from_str(title);
        let _: () = msg_send![&*content, setTitle: &*ns_title];

        let ns_body = NSString::from_str(body);
        let _: () = msg_send![&*content, setBody: &*ns_body];

        // Create trigger (immediate)
        let trigger_cls = AnyClass::get(c"UNTimeIntervalNotificationTrigger").unwrap();
        let trigger: Retained<AnyObject> = msg_send![trigger_cls, triggerWithTimeInterval: 0.1f64, repeats: false];

        // Create request
        let request_cls = AnyClass::get(c"UNNotificationRequest").unwrap();
        let ident = NSString::from_str("perry_notification");
        let request: Retained<AnyObject> = msg_send![request_cls, requestWithIdentifier: &*ident, content: &*content, trigger: &*trigger];

        // Get notification center and add request
        let center_cls = AnyClass::get(c"UNUserNotificationCenter").unwrap();
        let center: Retained<AnyObject> = msg_send![center_cls, currentNotificationCenter];

        // Request authorization first
        let _: () = msg_send![&*center, requestAuthorizationWithOptions: 7i64, completionHandler: std::ptr::null::<AnyObject>()];

        let _: () = msg_send![&*center, addNotificationRequest: &*request, withCompletionHandler: std::ptr::null::<AnyObject>()];
    }
}

/// Fallback for non-bundled apps using deprecated NSUserNotification
unsafe fn send_legacy(title: &str, body: &str) {
    let notif_cls = AnyClass::get(c"NSUserNotification");
    if notif_cls.is_none() { return; }
    let notif: Retained<AnyObject> = msg_send![notif_cls.unwrap(), new];

    let ns_title = NSString::from_str(title);
    let _: () = msg_send![&*notif, setTitle: &*ns_title];

    let ns_body = NSString::from_str(body);
    let _: () = msg_send![&*notif, setInformativeText: &*ns_body];

    let center_cls = AnyClass::get(c"NSUserNotificationCenter");
    if center_cls.is_none() { return; }
    let center: *mut AnyObject = msg_send![center_cls.unwrap(), defaultUserNotificationCenter];
    if !center.is_null() {
        let _: () = msg_send![center, deliverNotification: &*notif];
    }
}
