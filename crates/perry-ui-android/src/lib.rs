pub mod app;
pub mod callback;
pub mod clipboard;
pub mod file_dialog;
pub mod jni_bridge;
pub mod json;
pub mod menu;
pub mod state;
pub mod stdlib_stubs;
pub mod widgets;

// =============================================================================
// JNI lifecycle
// =============================================================================

extern "C" {
    fn __android_log_print(prio: i32, tag: *const u8, fmt: *const u8, ...) -> i32;
    fn mallopt(param: i32, value: i32) -> i32;
}

/// Catch panics from widget functions, log them, and return 0 instead of aborting.
fn catch_panic(name: &str, f: impl FnOnce() -> i64 + std::panic::UnwindSafe) -> i64 {
    match std::panic::catch_unwind(f) {
        Ok(h) => h,
        Err(e) => {
            let detail = if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "<unknown>".to_string()
            };
            let msg = format!("{} panicked: {}\0", name, detail);
            unsafe {
                __android_log_print(6, b"PerryJNI\0".as_ptr(), b"%s\0".as_ptr(), msg.as_ptr());
            }
            0
        }
    }
}

/// Catch panics from void widget functions, log them instead of aborting.
fn catch_panic_void(name: &str, f: impl FnOnce() + std::panic::UnwindSafe) {
    if let Err(e) = std::panic::catch_unwind(f) {
        let detail = if let Some(s) = e.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = e.downcast_ref::<String>() {
            s.clone()
        } else {
            "<unknown>".to_string()
        };
        let msg = format!("{} panicked: {}\0", name, detail);
        unsafe {
            __android_log_print(6, b"PerryJNI\0".as_ptr(), b"%s\0".as_ptr(), msg.as_ptr());
        }
    }
}

/// Called by the JVM when the native library is loaded via System.loadLibrary().
#[no_mangle]
pub extern "C" fn JNI_OnLoad(vm: jni::JavaVM, _reserved: *mut std::ffi::c_void) -> jni::sys::jint {
    unsafe {
        __android_log_print(
            3, b"PerryJNI\0".as_ptr(),
            b"JNI_OnLoad: starting\0".as_ptr(),
        );
    }

    // Disable MTE (Memory Tagging Extension) tagged addresses.
    // Perry's NaN-boxing uses 48-bit pointers (POINTER_MASK = 0x0000_FFFF_FFFF_FFFF).
    // Android's MTE puts a tag in the top byte, making pointers 56 bits.
    // When NaN-boxed pointers are extracted, the MTE tag is lost, causing crashes.
    // Disabling tagged addresses makes the allocator use standard 48-bit pointers.
    // Disable heap tagging (MTE/TBI) for the allocator.
    // Perry's NaN-boxing uses 48-bit pointers (POINTER_MASK = 0x0000_FFFF_FFFF_FFFF).
    // Android's scudo allocator tags pointers with a top byte (e.g., 0xb4...),
    // which breaks NaN-boxing when the tag is stripped.
    // mallopt(M_BIONIC_SET_HEAP_TAGGING_LEVEL, 0) disables tagging for NEW allocations
    // without breaking the JVM (which keeps its own tagged pointers).
    #[cfg(target_os = "android")]
    unsafe {
        // M_BIONIC_SET_HEAP_TAGGING_LEVEL = -204, level 0 = no tagging
        let ret = mallopt(-204, 0);
        __android_log_print(
            3, b"PerryJNI\0".as_ptr(),
            b"JNI_OnLoad: mallopt(-204, 0) returned %d\0".as_ptr(),
            ret,
        );
    }

    jni_bridge::init_vm(vm);
    unsafe {
        __android_log_print(
            3, b"PerryJNI\0".as_ptr(),
            b"JNI_OnLoad: done\0".as_ptr(),
        );
    }
    jni::sys::JNI_VERSION_1_6
}

/// Called from PerryActivity after the native library is loaded.
/// Initializes the JNI cache on the calling thread.
#[no_mangle]
pub extern "C" fn Java_com_perry_app_PerryBridge_nativeInit(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
) {
    jni_bridge::init_cache(&mut env);
}

/// Called from PerryActivity when the Activity is being destroyed.
#[no_mangle]
pub extern "C" fn Java_com_perry_app_PerryBridge_nativeShutdown(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) {
    app::signal_shutdown();
}

extern "C" {
    fn main();
}

// =============================================================================
// Stdlib stubs — perry-stdlib can't be cross-compiled for Android (OpenSSL dep).
// The codegen always emits calls to these at startup. Provide no-op versions.
// =============================================================================

/// No-op: native module dispatch isn't needed without perry-stdlib.
#[no_mangle]
pub extern "C" fn js_stdlib_init_dispatch() {}

/// No-op: returns 0 (no pending events to process).
#[no_mangle]
pub extern "C" fn js_stdlib_process_pending() -> i32 {
    0
}

/// Called from the native thread to run the compiled TypeScript entry point.
/// This wraps the compiler-generated `main()` function as a JNI method on PerryBridge,
/// so the Activity doesn't need its own native method (which would require package-specific JNI names).
#[no_mangle]
pub extern "C" fn Java_com_perry_app_PerryBridge_nativeMain(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) {
    unsafe {
        __android_log_print(
            3, b"PerryJNI\0".as_ptr(),
            b"nativeMain: calling main()\0".as_ptr(),
        );
        main();
        __android_log_print(
            3, b"PerryJNI\0".as_ptr(),
            b"nativeMain: main() returned\0".as_ptr(),
        );
    }
}

// =============================================================================
// FFI exports — identical signatures to perry-ui-macos and perry-ui-ios
// =============================================================================

#[no_mangle]
pub extern "C" fn perry_ui_app_create(title_ptr: i64, width: f64, height: f64) -> i64 {
    app::app_create(title_ptr as *const u8, width, height)
}

#[no_mangle]
pub extern "C" fn perry_ui_app_set_body(app_handle: i64, root_handle: i64) {
    app::app_set_body(app_handle, root_handle);
}

#[no_mangle]
pub extern "C" fn perry_ui_app_run(app_handle: i64) {
    app::app_run(app_handle);
}

#[no_mangle]
pub extern "C" fn perry_ui_text_create(text_ptr: i64) -> i64 {
    catch_panic("perry_ui_text_create", || widgets::text::create(text_ptr as *const u8))
}

#[no_mangle]
pub extern "C" fn perry_ui_button_create(label_ptr: i64, on_press: f64) -> i64 {
    catch_panic("perry_ui_button_create", || widgets::button::create(label_ptr as *const u8, on_press))
}

#[no_mangle]
pub extern "C" fn perry_ui_vstack_create(spacing: f64) -> i64 {
    catch_panic("perry_ui_vstack_create", || widgets::vstack::create(spacing))
}

#[no_mangle]
pub extern "C" fn perry_ui_hstack_create(spacing: f64) -> i64 {
    catch_panic("perry_ui_hstack_create", || widgets::hstack::create(spacing))
}

#[no_mangle]
pub extern "C" fn perry_ui_widget_add_child(parent_handle: i64, child_handle: i64) {
    catch_panic_void("perry_ui_widget_add_child", || widgets::add_child(parent_handle, child_handle));
}

#[no_mangle]
pub extern "C" fn perry_ui_state_create(initial: f64) -> i64 {
    state::state_create(initial)
}

#[no_mangle]
pub extern "C" fn perry_ui_state_get(state_handle: i64) -> f64 {
    state::state_get(state_handle)
}

#[no_mangle]
pub extern "C" fn perry_ui_state_set(state_handle: i64, value: f64) {
    state::state_set(state_handle, value);
}

#[no_mangle]
pub extern "C" fn perry_ui_state_bind_text_numeric(state_handle: i64, text_handle: i64, prefix_ptr: i64, suffix_ptr: i64) {
    state::bind_text_numeric(state_handle, text_handle, prefix_ptr as *const u8, suffix_ptr as *const u8);
}

#[no_mangle]
pub extern "C" fn perry_ui_spacer_create() -> i64 {
    widgets::spacer::create()
}

#[no_mangle]
pub extern "C" fn perry_ui_divider_create() -> i64 {
    widgets::divider::create()
}

#[no_mangle]
pub extern "C" fn perry_ui_textfield_create(placeholder_ptr: i64, on_change: f64) -> i64 {
    widgets::textfield::create(placeholder_ptr as *const u8, on_change)
}

#[no_mangle]
pub extern "C" fn perry_ui_toggle_create(label_ptr: i64, on_change: f64) -> i64 {
    widgets::toggle::create(label_ptr as *const u8, on_change)
}

#[no_mangle]
pub extern "C" fn perry_ui_slider_create(min: f64, max: f64, initial: f64, on_change: f64) -> i64 {
    widgets::slider::create(min, max, initial, on_change)
}

// =============================================================================
// Phase 4: Advanced Reactive UI
// =============================================================================

#[no_mangle]
pub extern "C" fn perry_ui_state_bind_slider(state_handle: i64, slider_handle: i64) {
    state::bind_slider(state_handle, slider_handle);
}

#[no_mangle]
pub extern "C" fn perry_ui_state_bind_toggle(state_handle: i64, toggle_handle: i64) {
    state::bind_toggle(state_handle, toggle_handle);
}

#[no_mangle]
pub extern "C" fn perry_ui_state_bind_text_template(
    text_handle: i64,
    num_parts: i32,
    types_ptr: i64,
    values_ptr: i64,
) {
    state::bind_text_template(text_handle, num_parts, types_ptr as *const i32, values_ptr as *const i64);
}

#[no_mangle]
pub extern "C" fn perry_ui_state_bind_visibility(state_handle: i64, show_handle: i64, hide_handle: i64) {
    state::bind_visibility(state_handle, show_handle, hide_handle);
}

#[no_mangle]
pub extern "C" fn perry_ui_set_widget_hidden(handle: i64, hidden: i64) {
    widgets::set_hidden(handle, hidden != 0);
}

#[no_mangle]
pub extern "C" fn perry_ui_for_each_init(container_handle: i64, state_handle: i64, render_closure: f64) {
    state::for_each_init(container_handle, state_handle, render_closure);
}

#[no_mangle]
pub extern "C" fn perry_ui_widget_clear_children(handle: i64) {
    widgets::clear_children(handle);
}

// =============================================================================
// Phase A.1: Text Mutation & Layout Control
// =============================================================================

#[no_mangle]
pub extern "C" fn perry_ui_text_set_string(handle: i64, text_ptr: i64) {
    widgets::text::set_string(handle, text_ptr as *const u8);
}

#[no_mangle]
pub extern "C" fn perry_ui_vstack_create_with_insets(spacing: f64, top: f64, left: f64, bottom: f64, right: f64) -> i64 {
    widgets::vstack::create_with_insets(spacing, top, left, bottom, right)
}

#[no_mangle]
pub extern "C" fn perry_ui_hstack_create_with_insets(spacing: f64, top: f64, left: f64, bottom: f64, right: f64) -> i64 {
    widgets::hstack::create_with_insets(spacing, top, left, bottom, right)
}

// =============================================================================
// Phase A.2: ScrollView, Clipboard & Keyboard Shortcuts
// =============================================================================

#[no_mangle]
pub extern "C" fn perry_ui_scrollview_create() -> i64 {
    unsafe {
        __android_log_print(
            3, b"PerryJNI\0".as_ptr(),
            b"perry_ui_scrollview_create: called\0".as_ptr(),
        );
    }
    let h = widgets::scrollview::create();
    unsafe {
        __android_log_print(
            3, b"PerryJNI\0".as_ptr(),
            b"perry_ui_scrollview_create: returned handle=%lld\0".as_ptr(),
            h,
        );
    }
    h
}

#[no_mangle]
pub extern "C" fn perry_ui_scrollview_set_child(scroll_handle: i64, child_handle: i64) {
    widgets::scrollview::set_child(scroll_handle, child_handle);
}

#[no_mangle]
pub extern "C" fn perry_ui_clipboard_read() -> f64 {
    clipboard::read()
}

#[no_mangle]
pub extern "C" fn perry_ui_clipboard_write(text_ptr: i64) {
    clipboard::write(text_ptr as *const u8);
}

#[no_mangle]
pub extern "C" fn perry_ui_add_keyboard_shortcut(key_ptr: i64, modifiers: f64, callback: f64) {
    app::add_keyboard_shortcut(key_ptr as *const u8, modifiers, callback);
}

// =============================================================================
// Phase A.3: Text Styling & Button Styling
// =============================================================================

#[no_mangle]
pub extern "C" fn perry_ui_text_set_color(handle: i64, r: f64, g: f64, b: f64, a: f64) {
    widgets::text::set_color(handle, r, g, b, a);
}

#[no_mangle]
pub extern "C" fn perry_ui_text_set_font_size(handle: i64, size: f64) {
    widgets::text::set_font_size(handle, size);
}

#[no_mangle]
pub extern "C" fn perry_ui_text_set_font_weight(handle: i64, size: f64, weight: f64) {
    widgets::text::set_font_weight(handle, size, weight);
}

#[no_mangle]
pub extern "C" fn perry_ui_text_set_selectable(handle: i64, selectable: f64) {
    widgets::text::set_selectable(handle, selectable != 0.0);
}

#[no_mangle]
pub extern "C" fn perry_ui_button_set_bordered(handle: i64, bordered: f64) {
    widgets::button::set_bordered(handle, bordered != 0.0);
}

#[no_mangle]
pub extern "C" fn perry_ui_button_set_title(handle: i64, title_ptr: i64) {
    widgets::button::set_title(handle, title_ptr as *const u8);
}

// =============================================================================
// Phase A.4: Focus & Scroll-To
// =============================================================================

#[no_mangle]
pub extern "C" fn perry_ui_textfield_focus(handle: i64) {
    widgets::textfield::focus(handle);
}

#[no_mangle]
pub extern "C" fn perry_ui_scrollview_scroll_to(scroll_handle: i64, child_handle: i64) {
    widgets::scrollview::scroll_to(scroll_handle, child_handle);
}

#[no_mangle]
pub extern "C" fn perry_ui_scrollview_get_offset(scroll_handle: i64) -> f64 {
    widgets::scrollview::get_offset(scroll_handle)
}

#[no_mangle]
pub extern "C" fn perry_ui_scrollview_set_offset(scroll_handle: i64, offset: f64) {
    widgets::scrollview::set_offset(scroll_handle, offset);
}

// =============================================================================
// Phase A.5: Context Menus, File Dialog & Window Sizing
// =============================================================================

#[no_mangle]
pub extern "C" fn perry_ui_menu_create() -> i64 {
    menu::create()
}

#[no_mangle]
pub extern "C" fn perry_ui_menu_add_item(menu_handle: i64, title_ptr: i64, callback: f64) {
    menu::add_item(menu_handle, title_ptr as *const u8, callback);
}

#[no_mangle]
pub extern "C" fn perry_ui_widget_set_context_menu(widget_handle: i64, menu_handle: i64) {
    menu::set_context_menu(widget_handle, menu_handle);
}

#[no_mangle]
pub extern "C" fn perry_ui_open_file_dialog(callback: f64) {
    file_dialog::open_dialog(callback);
}

#[no_mangle]
pub extern "C" fn perry_ui_app_set_min_size(app_handle: i64, w: f64, h: f64) {
    app::set_min_size(app_handle, w, h);
}

#[no_mangle]
pub extern "C" fn perry_ui_app_set_max_size(app_handle: i64, w: f64, h: f64) {
    app::set_max_size(app_handle, w, h);
}

#[no_mangle]
pub extern "C" fn perry_ui_textfield_set_string(handle: i64, text_ptr: i64) {
    widgets::textfield::set_string_value(handle, text_ptr as *const u8);
}

#[no_mangle]
pub extern "C" fn perry_ui_widget_add_child_at(parent_handle: i64, child_handle: i64, index: f64) {
    widgets::add_child_at(parent_handle, child_handle, index as i64);
}
