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

use jni::objects::{GlobalRef, JObject, JValue};
use std::cell::RefCell;

use crate::jni_bridge;

extern "C" {
    fn __android_log_print(prio: i32, tag: *const u8, fmt: *const u8, ...) -> i32;
}

thread_local! {
    /// Map from widget handle (1-based) to Android View (JNI global ref).
    static WIDGETS: RefCell<Vec<GlobalRef>> = RefCell::new(Vec::new());
}

/// Store an Android View and return its handle (1-based i64).
pub fn register_widget(view: GlobalRef) -> i64 {
    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        widgets.push(view);
        widgets.len() as i64
    })
}

/// Retrieve the JNI GlobalRef for a given widget handle.
pub fn get_widget(handle: i64) -> Option<GlobalRef> {
    WIDGETS.with(|w| {
        let widgets = w.borrow();
        let idx = (handle - 1) as usize;
        widgets.get(idx).cloned()
    })
}

/// Set the hidden state of a widget (View.VISIBLE=0, View.GONE=8).
pub fn set_hidden(handle: i64, hidden: bool) {
    if let Some(view_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let visibility = if hidden { 8i32 } else { 0i32 }; // View.GONE=8, View.VISIBLE=0
        let _ = env.call_method(
            view_ref.as_obj(),
            "setVisibility",
            "(I)V",
            &[JValue::Int(visibility)],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Remove all child views from a ViewGroup container.
pub fn clear_children(handle: i64) {
    unsafe {
        __android_log_print(
            3, b"PerryWidgets\0".as_ptr(),
            b"clear_children: handle=%lld\0".as_ptr(),
            handle,
        );
    }
    if let Some(parent_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let _ = env.call_method(
            parent_ref.as_obj(),
            "removeAllViews",
            "()V",
            &[],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Add a child view to a parent ViewGroup.
pub fn add_child(parent_handle: i64, child_handle: i64) {
    if let (Some(parent_ref), Some(child_ref)) = (get_widget(parent_handle), get_widget(child_handle)) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let _ = env.call_method(
            parent_ref.as_obj(),
            "addView",
            "(Landroid/view/View;)V",
            &[JValue::Object(child_ref.as_obj())],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Add a child view to a parent ViewGroup at a specific index.
pub fn add_child_at(parent_handle: i64, child_handle: i64, index: i64) {
    if let (Some(parent_ref), Some(child_ref)) = (get_widget(parent_handle), get_widget(child_handle)) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let _ = env.call_method(
            parent_ref.as_obj(),
            "addView",
            "(Landroid/view/View;I)V",
            &[JValue::Object(child_ref.as_obj()), JValue::Int(index as i32)],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Get the Activity context via PerryBridge.
pub fn get_activity<'a>(env: &mut jni::JNIEnv<'a>) -> JObject<'a> {
    let bridge_class = jni_bridge::with_cache(|c| {
        env.new_local_ref(c.perry_bridge_class.as_obj()).unwrap()
    });
    let bridge_cls: &jni::objects::JClass = (&bridge_class).into();
    let result = env.call_static_method(
        bridge_cls,
        "getActivity",
        "()Landroid/app/Activity;",
        &[],
    ).expect("Failed to get Activity");
    result.l().expect("Activity is not an object")
}

/// Convert dp to pixels via PerryBridge.
pub fn dp_to_px(env: &mut jni::JNIEnv, dp: f32) -> i32 {
    let bridge_class = jni_bridge::with_cache(|c| {
        env.new_local_ref(c.perry_bridge_class.as_obj()).unwrap()
    });
    let bridge_cls: &jni::objects::JClass = (&bridge_class).into();
    let result = env.call_static_method(
        bridge_cls,
        "dpToPx",
        "(F)I",
        &[JValue::Float(dp)],
    ).expect("Failed to convert dp to px");
    result.i().expect("dpToPx did not return int")
}
