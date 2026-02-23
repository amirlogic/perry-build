use jni::objects::JValue;
use std::cell::RefCell;

use crate::jni_bridge;
use crate::widgets;

thread_local! {
    static PENDING_CONFIG: RefCell<Option<AppConfig>> = RefCell::new(None);
    static PENDING_BODY: RefCell<Option<i64>> = RefCell::new(None);
}

struct AppConfig {
    _title: String,
    _width: f64,
    _height: f64,
}

/// Extract a &str from a *const StringHeader pointer (Perry runtime string format).
pub fn str_from_header(ptr: *const u8) -> &'static str {
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

/// Create an app. Stores config for deferred creation. Returns app handle (i64).
pub fn app_create(title_ptr: *const u8, width: f64, height: f64) -> i64 {
    let title = if title_ptr.is_null() {
        "Perry App".to_string()
    } else {
        str_from_header(title_ptr).to_string()
    };

    let w = if width > 0.0 { width } else { 400.0 };
    let h = if height > 0.0 { height } else { 300.0 };

    PENDING_CONFIG.with(|c| {
        *c.borrow_mut() = Some(AppConfig {
            _title: title,
            _width: w,
            _height: h,
        });
    });

    1 // Single app handle
}

/// Set the root widget (body) of the app.
pub fn app_set_body(_app_handle: i64, root_handle: i64) {
    PENDING_BODY.with(|b| {
        *b.borrow_mut() = Some(root_handle);
    });
}

/// Attach the root widget to the Activity's content view.
/// Called from the native init thread after all widgets are built.
/// Posts to the UI thread to add the root view to the FrameLayout.
fn attach_root_to_activity() {
    PENDING_BODY.with(|b| {
        if let Some(root_handle) = b.borrow().as_ref() {
            let root_handle = *root_handle;
            let mut env = jni_bridge::get_env();

            // Get the root View jobject
            if let Some(root_ref) = widgets::get_widget(root_handle) {
                // Call PerryBridge.setContentView(View) on UI thread
                let root_obj = root_ref.as_obj();
                let bridge_class = jni_bridge::with_cache(|c| {
                    env.new_local_ref(c.perry_bridge_class.as_obj()).unwrap()
                });
                let bridge_cls: &jni::objects::JClass = (&bridge_class).into();
                let _ = env.call_static_method(
                    bridge_cls,
                    "setContentView",
                    "(Landroid/view/View;)V",
                    &[JValue::Object(root_obj)],
                );
            }
        }
    });
}

/// Run the app event loop.
/// On Android, the event loop is the Activity lifecycle managed by the system.
/// This just attaches the root widget to the Activity and returns.
/// Unlike macOS/iOS, this does NOT block — the Activity keeps running.
pub fn app_run(_app_handle: i64) {
    // Attach the root widget to the Activity
    attach_root_to_activity();

    // On Android we run on the UI thread, so we must NOT block.
    // The Activity lifecycle IS the event loop.
}

/// Called when the Activity is destroyed. No-op since App() doesn't block on Android.
pub fn signal_shutdown() {
    // Nothing to do — App() returns immediately on Android.
}

/// Set minimum window size (no-op on Android).
pub fn set_min_size(_app_handle: i64, _w: f64, _h: f64) {
    // No-op on Android
}

/// Set maximum window size (no-op on Android).
pub fn set_max_size(_app_handle: i64, _w: f64, _h: f64) {
    // No-op on Android
}

/// Add a keyboard shortcut.
/// On Android, this is handled via dispatchKeyEvent in the Activity.
/// For now, store the binding and the Activity will check against it.
pub fn add_keyboard_shortcut(_key_ptr: *const u8, _modifiers: f64, _callback: f64) {
    // Stub — Android hardware keyboard shortcuts are uncommon.
    // Could be implemented via onKeyDown in PerryActivity if needed.
}
