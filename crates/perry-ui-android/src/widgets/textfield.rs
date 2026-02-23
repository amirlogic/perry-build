use jni::objects::JValue;
use crate::app::str_from_header;
use crate::callback;
use crate::jni_bridge;

/// Create an EditText with placeholder and onChange callback. Returns widget handle.
pub fn create(placeholder_ptr: *const u8, on_change: f64) -> i64 {
    let placeholder = str_from_header(placeholder_ptr);
    let mut env = jni_bridge::get_env();
    let _ = env.push_local_frame(32);

    let activity = super::get_activity(&mut env);
    let edit_text = env.new_object(
        "android/widget/EditText",
        "(Landroid/content/Context;)V",
        &[JValue::Object(&activity)],
    ).expect("Failed to create EditText");

    // Set hint (placeholder)
    let hint_str = env.new_string(placeholder).expect("Failed to create JNI string");
    let _ = env.call_method(
        &edit_text,
        "setHint",
        "(Ljava/lang/CharSequence;)V",
        &[JValue::Object(&hint_str)],
    );

    // Single line by default
    let _ = env.call_method(
        &edit_text,
        "setSingleLine",
        "(Z)V",
        &[JValue::Bool(1)],
    );

    // MATCH_PARENT width, WRAP_CONTENT height
    let params = env.new_object(
        "android/widget/LinearLayout$LayoutParams",
        "(II)V",
        &[JValue::Int(-1), JValue::Int(-2)],
    ).expect("Failed to create LayoutParams");
    let _ = env.call_method(
        &edit_text,
        "setLayoutParams",
        "(Landroid/view/ViewGroup$LayoutParams;)V",
        &[JValue::Object(&params)],
    );

    // Register callback and set up TextWatcher via PerryBridge
    let cb_key = callback::register(on_change);
    let bridge_class = jni_bridge::with_cache(|c| {
        env.new_local_ref(c.perry_bridge_class.as_obj()).unwrap()
    });
    let bridge_cls: &jni::objects::JClass = (&bridge_class).into();
    let _ = env.call_static_method(
        bridge_cls,
        "setTextChangedCallback",
        "(Landroid/widget/EditText;J)V",
        &[JValue::Object(&edit_text), JValue::Long(cb_key)],
    );

    let global = env.new_global_ref(edit_text).expect("Failed to create global ref");
    let handle = super::register_widget(global);
    unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    handle
}

/// Focus an EditText (request focus).
pub fn focus(handle: i64) {
    if let Some(view_ref) = super::get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let _ = env.call_method(
            view_ref.as_obj(),
            "requestFocus",
            "()Z",
            &[],
        );
        unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    }
}

/// Set the text of an EditText from a StringHeader pointer.
pub fn set_string_value(handle: i64, text_ptr: *const u8) {
    let text = str_from_header(text_ptr);
    if let Some(view_ref) = super::get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let jstr = env.new_string(text).expect("Failed to create JNI string");
        let _ = env.call_method(
            view_ref.as_obj(),
            "setText",
            "(Ljava/lang/CharSequence;)V",
            &[JValue::Object(&jstr)],
        );
        unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    }
}
