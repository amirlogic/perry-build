use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{define_class, msg_send, AnyThread, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSEventModifierFlags,
    NSLayoutConstraint, NSMenu, NSMenuItem, NSWindow, NSWindowStyleMask,
};
use objc2_core_foundation::{CGPoint, CGSize, CGRect};
use objc2_foundation::{NSObject, NSString, MainThreadMarker};

use std::cell::RefCell;
use std::collections::HashMap;

use crate::widgets;

thread_local! {
    static APPS: RefCell<Vec<AppEntry>> = RefCell::new(Vec::new());
    /// Buffered keyboard shortcuts registered before the menu bar exists.
    static PENDING_SHORTCUTS: RefCell<Vec<PendingShortcut>> = RefCell::new(Vec::new());
}

struct PendingShortcut {
    key_ptr: *const u8,
    modifiers: f64,
    callback: f64,
}

thread_local! {
    static ON_TERMINATE_CALLBACK: RefCell<Option<f64>> = RefCell::new(None);
    static ON_ACTIVATE_CALLBACK: RefCell<Option<f64>> = RefCell::new(None);
    static WINDOWS: RefCell<Vec<WindowEntry>> = RefCell::new(Vec::new());
}

struct WindowEntry {
    window: Retained<NSWindow>,
}

struct AppEntry {
    window: Retained<NSWindow>,
    _root_widget: Option<i64>,
}

/// Extract a &str from a *const StringHeader pointer.
fn str_from_header(ptr: *const u8) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    unsafe {
        let header = ptr as *const crate::string_header::StringHeader;
        let len = (*header).length as usize;
        let data = ptr.add(std::mem::size_of::<crate::string_header::StringHeader>());
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(data, len))
    }
}

/// Create an app with title, width, height.
pub fn app_create(title_ptr: *const u8, width: f64, height: f64) -> i64 {
    let title = if title_ptr.is_null() {
        "Perry App"
    } else {
        str_from_header(title_ptr)
    };

    let w = if width > 0.0 { width } else { 400.0 };
    let h = if height > 0.0 { height } else { 300.0 };

    let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");

    unsafe {
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;

        let frame = CGRect::new(CGPoint::new(200.0, 200.0), CGSize::new(w, h));

        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        );

        let ns_title = NSString::from_str(title);
        window.setTitle(&ns_title);

        APPS.with(|a| {
            let mut apps = a.borrow_mut();
            apps.push(AppEntry {
                window,
                _root_widget: None,
            });
            apps.len() as i64 // 1-based handle
        })
    }
}

/// Set the root widget (body) of the app.
pub fn app_set_body(app_handle: i64, root_handle: i64) {
    APPS.with(|a| {
        let mut apps = a.borrow_mut();
        let idx = (app_handle - 1) as usize;
        if idx < apps.len() {
            apps[idx]._root_widget = Some(root_handle);

            if let Some(view) = widgets::get_widget(root_handle) {
                // Use autoresizing mask (the default) so the window
                // automatically sizes the content view to fill its bounds.
                // NOTE: we intentionally do NOT disable autoresizing here —
                // setContentView already manages the frame for us.
                apps[idx].window.setContentView(Some(&view));
            }
        }
    });
}

/// Create a standard macOS menu bar with Quit (Cmd+Q) support.
fn setup_menu_bar(app: &NSApplication, mtm: MainThreadMarker) {
    unsafe {
        // Main menu bar
        let menu_bar = NSMenu::new(mtm);

        // App menu (the first menu, shown as the app name)
        let app_menu_item = NSMenuItem::new(mtm);
        let app_menu = NSMenu::new(mtm);

        // Quit menu item: "Quit" with Cmd+Q
        let quit_title = NSString::from_str("Quit");
        let quit_key = NSString::from_str("q");
        let quit_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &quit_title,
            Some(Sel::register(c"terminate:")),
            &quit_key,
        );
        quit_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        app_menu.addItem(&quit_item);

        app_menu_item.setSubmenu(Some(&app_menu));
        menu_bar.addItem(&app_menu_item);

        app.setMainMenu(Some(&menu_bar));
    }
}

/// Run the application event loop (blocks).
pub fn app_run(_app_handle: i64) {
    let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    // Set up menu bar with Cmd+Q support
    setup_menu_bar(&app, mtm);

    // Install any keyboard shortcuts that were registered before the menu existed
    flush_pending_shortcuts(mtm);

    APPS.with(|a| {
        let apps = a.borrow();
        for entry in apps.iter() {
            entry.window.center();
            entry.window.makeKeyAndOrderFront(None);
        }
    });

    // Activate the app (bring to front)
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    app.run();
}

/// Set the minimum window size.
pub fn set_min_size(app_handle: i64, w: f64, h: f64) {
    APPS.with(|a| {
        let apps = a.borrow();
        let idx = (app_handle - 1) as usize;
        if idx < apps.len() {
            apps[idx].window.setMinSize(CGSize::new(w, h));
        }
    });
}

/// Set the maximum window size.
pub fn set_max_size(app_handle: i64, w: f64, h: f64) {
    APPS.with(|a| {
        let apps = a.borrow();
        let idx = (app_handle - 1) as usize;
        if idx < apps.len() {
            apps[idx].window.setMaxSize(CGSize::new(w, h));
        }
    });
}

// ============================================
// Keyboard Shortcuts
// ============================================

thread_local! {
    static SHORTCUT_CALLBACKS: RefCell<HashMap<usize, f64>> = RefCell::new(HashMap::new());
}

extern "C" {
    fn js_closure_call0(closure: *const u8) -> f64;
    fn js_nanbox_get_pointer(value: f64) -> i64;
}

pub struct PerryShortcutTargetIvars {
    callback_key: std::cell::Cell<usize>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "PerryShortcutTarget"]
    #[ivars = PerryShortcutTargetIvars]
    pub struct PerryShortcutTarget;

    impl PerryShortcutTarget {
        #[unsafe(method(shortcutFired:))]
        fn shortcut_fired(&self, _sender: &AnyObject) {
            let key = self.ivars().callback_key.get();
            // Extract closure pointer and drop the borrow BEFORE calling it,
            // because the callback may modify widgets (re-entrant borrow).
            let closure_f64 = SHORTCUT_CALLBACKS.with(|cbs| {
                cbs.borrow().get(&key).copied()
            });
            if let Some(closure_f64) = closure_f64 {
                let closure_ptr = unsafe { js_nanbox_get_pointer(closure_f64) };
                unsafe {
                    js_closure_call0(closure_ptr as *const u8);
                }
            }
        }
    }
);

impl PerryShortcutTarget {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(PerryShortcutTargetIvars {
            callback_key: std::cell::Cell::new(0),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// Add a keyboard shortcut to the app menu.
/// `key_ptr` is a StringHeader pointer to the key character (e.g., "s" for Cmd+S).
/// `modifiers` is a bitfield: 1=Cmd, 2=Shift, 4=Option, 8=Control.
/// `callback` is a NaN-boxed closure pointer.
///
/// If the menu bar doesn't exist yet (called before `app_run`), the shortcut
/// is buffered and will be installed once the menu is created.
pub fn add_keyboard_shortcut(key_ptr: *const u8, modifiers: f64, callback: f64) {
    let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");
    let app = NSApplication::sharedApplication(mtm);

    // If the menu bar exists, install immediately; otherwise buffer for later.
    let has_menu = unsafe { app.mainMenu().is_some() };
    if has_menu {
        install_keyboard_shortcut(key_ptr, modifiers, callback, mtm);
    } else {
        PENDING_SHORTCUTS.with(|ps| {
            ps.borrow_mut().push(PendingShortcut { key_ptr, modifiers, callback });
        });
    }
}

/// Install a single keyboard shortcut into the existing app menu.
fn install_keyboard_shortcut(key_ptr: *const u8, modifiers: f64, callback: f64, mtm: MainThreadMarker) {
    let key_str = str_from_header(key_ptr);
    let app = NSApplication::sharedApplication(mtm);

    unsafe {
        let target = PerryShortcutTarget::new();
        let target_addr = Retained::as_ptr(&target) as usize;
        target.ivars().callback_key.set(target_addr);

        SHORTCUT_CALLBACKS.with(|cbs| {
            cbs.borrow_mut().insert(target_addr, callback);
        });

        let ns_key = NSString::from_str(key_str);
        let title = NSString::from_str(&format!("Shortcut {}", key_str));
        let item = NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &title,
            Some(Sel::register(c"shortcutFired:")),
            &ns_key,
        );

        // Build modifier flags
        let mod_bits = modifiers as u64;
        let mut flags = NSEventModifierFlags::empty();
        if mod_bits & 1 != 0 { flags |= NSEventModifierFlags::Command; }
        if mod_bits & 2 != 0 { flags |= NSEventModifierFlags::Shift; }
        if mod_bits & 4 != 0 { flags |= NSEventModifierFlags::Option; }
        if mod_bits & 8 != 0 { flags |= NSEventModifierFlags::Control; }
        item.setKeyEquivalentModifierMask(flags);

        item.setTarget(Some(&target));
        std::mem::forget(target);

        // Add to the app menu (first menu item's submenu)
        if let Some(main_menu) = app.mainMenu() {
            if main_menu.numberOfItems() > 0 {
                if let Some(app_menu_item) = main_menu.itemAtIndex(0) {
                    if let Some(submenu) = app_menu_item.submenu() {
                        submenu.addItem(&item);
                    }
                }
            }
        }
    }
}

/// Flush any keyboard shortcuts that were registered before the menu bar existed.
fn flush_pending_shortcuts(mtm: MainThreadMarker) {
    PENDING_SHORTCUTS.with(|ps| {
        let pending: Vec<PendingShortcut> = ps.borrow_mut().drain(..).collect();
        for shortcut in pending {
            install_keyboard_shortcut(shortcut.key_ptr, shortcut.modifiers, shortcut.callback, mtm);
        }
    });
}

// ============================================
// Timer
// ============================================

thread_local! {
    static TIMER_CALLBACKS: RefCell<HashMap<usize, f64>> = RefCell::new(HashMap::new());
}

extern "C" {
    fn js_stdlib_process_pending();
    fn js_promise_run_microtasks() -> i32;
}

pub struct PerryTimerTargetIvars {
    callback_key: std::cell::Cell<usize>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "PerryTimerTarget"]
    #[ivars = PerryTimerTargetIvars]
    pub struct PerryTimerTarget;

    impl PerryTimerTarget {
        #[unsafe(method(timerFired:))]
        fn timer_fired(&self, _sender: &AnyObject) {
            // Drain resolved promises, then run microtasks (.then callbacks)
            unsafe {
                js_stdlib_process_pending();
                js_promise_run_microtasks();
            }

            let key = self.ivars().callback_key.get();
            let closure_f64 = TIMER_CALLBACKS.with(|cbs| {
                cbs.borrow().get(&key).copied()
            });
            if let Some(closure_f64) = closure_f64 {
                let closure_ptr = unsafe { js_nanbox_get_pointer(closure_f64) };
                unsafe {
                    js_closure_call0(closure_ptr as *const u8);
                }
            }
        }
    }
);

impl PerryTimerTarget {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(PerryTimerTargetIvars {
            callback_key: std::cell::Cell::new(0),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// Set a recurring timer. interval_ms is in milliseconds.
/// The timer calls js_stdlib_process_pending() then invokes the callback.
pub fn set_timer(interval_ms: f64, callback: f64) {
    let interval_secs = interval_ms / 1000.0;

    unsafe {
        let target = PerryTimerTarget::new();
        let target_addr = Retained::as_ptr(&target) as usize;
        target.ivars().callback_key.set(target_addr);

        TIMER_CALLBACKS.with(|cbs| {
            cbs.borrow_mut().insert(target_addr, callback);
        });

        // NSTimer.scheduledTimerWithTimeInterval:target:selector:userInfo:repeats:
        let sel = Sel::register(c"timerFired:");
        let _: Retained<AnyObject> = msg_send![
            objc2::class!(NSTimer),
            scheduledTimerWithTimeInterval: interval_secs,
            target: &*target,
            selector: sel,
            userInfo: std::ptr::null::<AnyObject>(),
            repeats: true
        ];

        // Keep the target alive
        std::mem::forget(target);
    }
}

// ============================================
// Lifecycle Hooks
// ============================================

/// Register an onTerminate callback.
pub fn register_on_terminate(callback: f64) {
    ON_TERMINATE_CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(callback);
    });
}

/// Register an onActivate callback.
pub fn register_on_activate(callback: f64) {
    ON_ACTIVATE_CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(callback);
    });
}

// ============================================
// Multi-Window
// ============================================

/// Create a new window. Returns 1-based handle.
pub fn window_create(title_ptr: *const u8, width: f64, height: f64) -> i64 {
    let title = if title_ptr.is_null() { "Window" } else { str_from_header(title_ptr) };
    let w = if width > 0.0 { width } else { 400.0 };
    let h = if height > 0.0 { height } else { 300.0 };

    let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");

    unsafe {
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;

        let frame = CGRect::new(CGPoint::new(200.0, 200.0), CGSize::new(w, h));

        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        );

        let ns_title = NSString::from_str(title);
        window.setTitle(&ns_title);

        WINDOWS.with(|w| {
            let mut windows = w.borrow_mut();
            windows.push(WindowEntry { window });
            windows.len() as i64
        })
    }
}

/// Set the root widget of a window.
pub fn window_set_body(window_handle: i64, widget_handle: i64) {
    WINDOWS.with(|w| {
        let windows = w.borrow();
        let idx = (window_handle - 1) as usize;
        if idx < windows.len() {
            if let Some(view) = crate::widgets::get_widget(widget_handle) {
                windows[idx].window.setContentView(Some(&view));
            }
        }
    });
}

/// Show a window (make key and order front).
pub fn window_show(window_handle: i64) {
    WINDOWS.with(|w| {
        let windows = w.borrow();
        let idx = (window_handle - 1) as usize;
        if idx < windows.len() {
            windows[idx].window.center();
            windows[idx].window.makeKeyAndOrderFront(None);
        }
    });
}

/// Close a window.
pub fn window_close(window_handle: i64) {
    WINDOWS.with(|w| {
        let windows = w.borrow();
        let idx = (window_handle - 1) as usize;
        if idx < windows.len() {
            windows[idx].window.close();
        }
    });
}
