use jni::objects::{JObject, JValue};
use crate::app::str_from_header;
use crate::jni_bridge;

/// Create a TextView. Returns widget handle.
pub fn create(text_ptr: *const u8) -> i64 {
    let text = str_from_header(text_ptr);
    let mut env = jni_bridge::get_env();
    let _ = env.push_local_frame(32);

    let activity = super::get_activity(&mut env);
    let text_view = env.new_object(
        "android/widget/TextView",
        "(Landroid/content/Context;)V",
        &[JValue::Object(&activity)],
    ).expect("Failed to create TextView");

    let jstr = env.new_string(text).expect("Failed to create JNI string");
    let _ = env.call_method(
        &text_view,
        "setText",
        "(Ljava/lang/CharSequence;)V",
        &[JValue::Object(&jstr)],
    );

    let global = env.new_global_ref(text_view).expect("Failed to create global ref");
    let handle = super::register_widget(global);
    unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    handle
}

/// Update the text of an existing TextView.
pub fn set_text_str(handle: i64, text: &str) {
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

/// Update the text of an existing TextView from a StringHeader pointer.
pub fn set_string(handle: i64, text_ptr: *const u8) {
    let text = str_from_header(text_ptr);
    set_text_str(handle, text);
}

/// Set the text color of a TextView (RGBA 0.0-1.0).
pub fn set_color(handle: i64, r: f64, g: f64, b: f64, a: f64) {
    if let Some(view_ref) = super::get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let ai = (a * 255.0) as i32;
        let ri = (r * 255.0) as i32;
        let gi = (g * 255.0) as i32;
        let bi = (b * 255.0) as i32;
        let color = (ai << 24) | (ri << 16) | (gi << 8) | bi;
        let _ = env.call_method(
            view_ref.as_obj(),
            "setTextColor",
            "(I)V",
            &[JValue::Int(color)],
        );
        unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    }
}

/// Set the font size of a TextView (in sp, roughly equivalent to pt on iOS).
pub fn set_font_size(handle: i64, size: f64) {
    if let Some(view_ref) = super::get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        // TypedValue.COMPLEX_UNIT_SP = 2
        let _ = env.call_method(
            view_ref.as_obj(),
            "setTextSize",
            "(IF)V",
            &[JValue::Int(2), JValue::Float(size as f32)],
        );
        unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    }
}

/// Set the font weight of a TextView.
/// weight >= 1.0 means bold (Typeface.BOLD=1), otherwise normal (Typeface.NORMAL=0).
pub fn set_font_weight(handle: i64, _size: f64, weight: f64) {
    if let Some(view_ref) = super::get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let style = if weight >= 1.0 { 1i32 } else { 0i32 }; // Typeface.BOLD=1, NORMAL=0
        let _ = env.call_method(
            view_ref.as_obj(),
            "setTypeface",
            "(Landroid/graphics/Typeface;I)V",
            &[JValue::Object(&JObject::null()), JValue::Int(style)],
        );
        unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    }
}

/// Set whether a TextView is selectable.
pub fn set_selectable(handle: i64, selectable: bool) {
    if let Some(view_ref) = super::get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let _ = env.call_method(
            view_ref.as_obj(),
            "setTextIsSelectable",
            "(Z)V",
            &[JValue::Bool(selectable as u8)],
        );
        unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    }
}
