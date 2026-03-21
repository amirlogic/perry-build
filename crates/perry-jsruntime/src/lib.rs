//! V8 JavaScript Runtime for Perry
//!
//! This crate provides V8 JavaScript runtime support for running npm modules
//! that cannot be natively compiled. It serves as a fallback when:
//! - A module is pure JavaScript (not TypeScript)
//! - A module uses dynamic features incompatible with AOT compilation
//!
//! The runtime is opt-in and requires explicit configuration.

mod bridge;
mod interop;
mod modules;
mod ops;

pub use bridge::{native_to_v8, v8_to_native, store_js_handle, get_js_handle, release_js_handle,
    is_js_handle, get_handle_id, make_js_handle_value};
pub use interop::{
    js_call_function, js_call_method, js_get_export, js_load_module, js_register_native_function,
    js_runtime_init, js_runtime_shutdown, js_handle_object_get_property, js_set_property,
    js_new_instance, js_new_from_handle, js_create_callback,
    js_handle_array_get, js_handle_array_length,
};
// Re-export deno_core's ModuleLoader trait for external use
pub use deno_core::ModuleLoader;

// Re-export perry-stdlib to include all its symbols in this staticlib
pub use perry_stdlib;

use deno_core::{JsRuntime, RuntimeOptions};
use once_cell::sync::OnceCell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::runtime::Runtime as TokioRuntime;

/// Global Tokio runtime for async operations
static TOKIO_RUNTIME: OnceCell<TokioRuntime> = OnceCell::new();

thread_local! {
    /// Thread-local V8 runtime instance
    /// JsRuntime is not Send, so it must be thread-local
    static JS_RUNTIME: RefCell<Option<JsRuntimeState>> = const { RefCell::new(None) };
}

/// State for the JS runtime
pub struct JsRuntimeState {
    pub runtime: JsRuntime,
    /// Map of loaded module paths to their V8 module IDs
    pub loaded_modules: HashMap<PathBuf, deno_core::ModuleId>,
    /// Whether the runtime has been initialized
    pub initialized: bool,
}

impl JsRuntimeState {
    fn new() -> Self {
        let mut runtime = JsRuntime::new(RuntimeOptions {
            module_loader: Some(std::rc::Rc::new(modules::NodeModuleLoader::new())),
            extensions: vec![ops::perry_ops::init_ops()],
            ..Default::default()
        });

        // Set V8 stack limit based on actual thread stack bounds.
        // Previously set to 0x10000 which disabled V8's stack overflow detection entirely,
        // causing SIGBUS on arm64 when deep call chains (module init → async → V8 eval)
        // overflowed past the stack guard page.
        //
        // The Rust v8 bindings (v8 0.106) don't expose Isolate::SetStackLimit,
        // so we call the C++ function directly via its Itanium ABI mangled name.
        {
            extern "C" {
                #[link_name = "_ZN2v87Isolate13SetStackLimitEm"]
                fn v8_isolate_set_stack_limit(isolate: *mut std::ffi::c_void, stack_limit: usize);
            }
            let isolate: &mut deno_core::v8::Isolate = runtime.v8_isolate();
            let isolate_ptr: *mut std::ffi::c_void = (isolate as *mut deno_core::v8::Isolate).cast();

            // Compute stack limit from actual thread stack bounds
            #[cfg(target_os = "macos")]
            let stack_limit = {
                extern "C" {
                    fn pthread_self() -> *mut std::ffi::c_void;
                    fn pthread_get_stackaddr_np(thread: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
                    fn pthread_get_stacksize_np(thread: *mut std::ffi::c_void) -> usize;
                }
                let thread = unsafe { pthread_self() };
                let stack_addr = unsafe { pthread_get_stackaddr_np(thread) } as usize;
                let stack_size = unsafe { pthread_get_stacksize_np(thread) };
                let stack_bottom = stack_addr - stack_size;
                // Reserve 64KB above stack bottom as safety margin for V8's stack check
                stack_bottom + 64 * 1024
            };
            #[cfg(not(target_os = "macos"))]
            let stack_limit: usize = 0x10000;

            unsafe { v8_isolate_set_stack_limit(isolate_ptr, stack_limit); }
        }

        // Set up Node.js global polyfills before any modules are loaded
        runtime.execute_script("<node-polyfills>", deno_core::ascii_str_include!("node_polyfills.js"))
            .expect("Failed to initialize Node.js polyfills");

        Self {
            runtime,
            loaded_modules: HashMap::new(),
            initialized: true,
        }
    }
}

/// Initialize the Tokio runtime for async operations
pub fn get_tokio_runtime() -> &'static TokioRuntime {
    TOKIO_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    })
}

/// Initialize the JS runtime for the current thread
pub fn ensure_runtime_initialized() {
    JS_RUNTIME.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            *opt = Some(JsRuntimeState::new());
        }
    });
}

/// Execute a closure with the JS runtime
pub fn with_runtime<F, R>(f: F) -> R
where
    F: FnOnce(&mut JsRuntimeState) -> R,
{
    ensure_runtime_initialized();
    JS_RUNTIME.with(|cell| {
        let mut opt = cell.borrow_mut();
        let state = opt.as_mut().expect("Runtime should be initialized");
        f(state)
    })
}

/// Execute an async closure with the JS runtime
pub fn with_runtime_async<F, Fut, R>(f: F) -> R
where
    F: FnOnce(&mut JsRuntimeState) -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let tokio_rt = get_tokio_runtime();
    tokio_rt.block_on(async {
        ensure_runtime_initialized();
        JS_RUNTIME.with(|cell| {
            let mut opt = cell.borrow_mut();
            let state = opt.as_mut().expect("Runtime should be initialized");
            // Use a dedicated current-thread Tokio runtime to avoid thread pool starvation deadlock.
            // The outer block_on holds a worker thread; using Handle::current().block_on() would
            // create a nested block_on on the same runtime, deadlocking if async JS operations
            // spawn Tokio tasks (e.g., ethers.js HTTP calls).
            tokio::task::block_in_place(|| {
                let local_rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create local Tokio runtime");
                local_rt.block_on(f(state))
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_init() {
        js_runtime_init();
        // Should not panic on double init
        js_runtime_init();
    }
}
