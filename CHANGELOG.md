# Changelog

Detailed changelog for Perry. See CLAUDE.md for concise summaries.

## v0.2.147
- **Mark-sweep garbage collection** for bounded memory in long-running programs
  - New `crates/perry-runtime/src/gc.rs`: full GC infrastructure
    - 8-byte `GcHeader` prepended to every heap allocation (obj_type, gc_flags, size)
    - Conservative stack scanning: `setjmp` captures registers, walks stack with NaN-boxing tag validation
    - Type-specific object tracing: arrays (elements), objects (fields + keys), closures (captures), promises (value/callbacks/chain), errors (message/name/stack)
    - Iterative worklist-based marking (no recursion — safe for deep object graphs)
    - Sweep: malloc objects freed via `dealloc`; arena objects added to free list for reuse
  - Arena integration (`arena.rs`):
    - `arena_alloc_gc(size, align, obj_type)`: allocates with GcHeader, checks free list first
    - `arena_walk_objects(callback)`: linear block walking for zero-cost arena object discovery
    - GC trigger check only on new block allocation (~every 8MB), not per-allocation
  - All allocation sites instrumented:
    - Arena: arrays (`js_array_alloc*`, `js_array_grow`), objects (`js_object_alloc*`) → `arena_alloc_gc`
    - Malloc: strings (`js_string_from_bytes*`, `js_string_concat`, `js_string_append`) → `gc_malloc`/`gc_realloc`
    - Malloc: closures (`js_closure_alloc`), promises (`js_promise_new`), bigints, errors → `gc_malloc`
  - Root scanning: promise task queue, timer callbacks, exception state, module-level global variables
  - Codegen: `gc()` callable from TypeScript, `js_gc_init()` in entry module, `js_gc_register_global_root()` for module globals
  - HIR: `gc` added to `is_builtin_function()` for ExternFuncRef resolution
  - `js_object_free()` and `js_promise_free()` made no-op (GC handles deallocation)
  - **Performance**: Zero overhead for compute-heavy benchmarks; <5% for allocation-heavy code (8 extra bytes per alloc)

## v0.2.146
- Fix i64 → f64 type mismatches when passing local object variables as arguments to NativeMethodCall
  - Root cause: i64 was passed directly without NaN-boxing in default argument handling
  - Both `_ => arg_vals.clone()` cases now use `inline_nanbox_pointer` for i64 values
- Fix fs module NativeMethodCall using wrong argument types (ensure_i64 instead of ensure_f64)

## v0.2.145
- Fix i64 → f64 type mismatches when passing object parameters to cross-module function calls
  - Use `inline_nanbox_pointer()` instead of `bitcast` for i64→f64 conversions in 8 locations

## v0.2.144
- Fix duplicate symbol linker errors when using jsruntime
  - Only add stub symbols when `!use_jsruntime`

## v0.2.143
- Fix fs.readFileSync() SIGSEGV crash - NaN-boxed string pointers were dereferenced directly
  - Changed all fs functions to accept `f64` (NaN-boxed) and extract raw pointer via `& POINTER_MASK`

## v0.2.142
- Shape-cached object literal allocation eliminates per-object key array construction
  - `js_object_alloc_with_shape(shape_id, field_count, packed_keys, len)` + SHAPE_CACHE
  - **object_create benchmark: 11-13ms → 2ms (5-6x faster, now 3x faster than Node's 5-7ms)**

## v0.2.141
- Fix stub generator including runtime functions already defined in libperry_jsruntime.a

## v0.2.140
- Inline NaN-box string operations to eliminate FFI overhead in string hot paths
  - `inline_nanbox_string` / `inline_get_string_pointer`: pure Cranelift IR replacing FFI calls
  - **string_concat benchmark: 2ms (Perry) vs 4-5ms (Node) — 2x faster**
- Add i32 shadow variables for integer function parameters

## v0.2.139
- Fix keyboard shortcuts registered before App() (added PENDING_SHORTCUTS buffer)

## v0.2.138
- **iOS support**: perry-ui-ios crate + `--target ios-simulator`/`--target ios` CLI flag
  - Complete UIKit implementation of all 47 `perry_ui_*` FFI functions
  - perry-runtime feature gates: `default = ["full"]`, `--no-default-features` for iOS

## v0.2.137
- Fix arena allocator crash on large allocations (>8MB)
  - `alloc_block(min_size)` now rounds up to next multiple of 8MB

## v0.2.136
- Comprehensive perry/ui smoke test (`test-files/test_ui_comprehensive.ts`)

## v0.2.135
- Module-scoped cross-module symbols for large multi-module compilation (183+ modules)
- Stub generation for unresolved external dependencies (`generate_stub_object()`)

## v0.2.134
- Perry UI Phase A: 24 new FFI functions (styling, scrolling, clipboard, keyboard shortcuts, menus, file dialog)

## v0.2.133
- Move array allocation from system malloc to arena bump allocator
- Fix `new Array(n)` pre-allocation

## v0.2.132
- Advanced Reactive UI Phase 4: Multi-state text, two-way binding, conditional rendering, ForEach

## v0.2.131
- Eliminate js_is_truthy FFI calls from if/for/while conditions (inline truthiness check)
- i32 shadow variables for integer function parameters

## v0.2.130
- Generalized reactive state text bindings (prefix+suffix patterns)

## v0.2.129
- Loop-Invariant Code Motion (LICM) for nested loops
  - **nested_loops: ~26ms → ~21ms, matrix_multiply: ~46ms → ~41ms**

## v0.2.128
- clearTimeout, fileURLToPath, cross-module enum exports, worker_threads module

## v0.2.127
- UI widgets: Spacer, Divider, TextField, Toggle, Slider

## v0.2.126
- Eliminate js_is_truthy FFI in while-loop conditions for Compare expressions
  - **Mandelbrot: 48ms → 27ms (44% faster)**

## v0.2.124-v0.2.125
- Reactive text binding, disable while-loop unrolling, const value propagation

## v0.2.122-v0.2.123
- Fix button callbacks and VStack/HStack children in perry/ui

## Older (v0.2.37-v0.2.121)

### Performance (v0.2.115-v0.2.121)
- Integer function specialization, array pointer caching, i32 index arithmetic
- JSON.stringify optimization, self-recursive call fast path

### Native UI (v0.2.116-v0.2.121)
- Initial perry/ui: Text, Button, VStack/HStack, State, App

### Fastify (v0.2.79-v0.2.114)
- HTTP runtime, handle-based dispatch, NaN-boxing fixes

### Async & Promises (v0.2.39-v0.2.106)
- ClosurePtr callbacks, Promise.all, spawn_for_promise_deferred, async closures

### Cross-Module (v0.2.57-v0.2.110)
- Array exports, imported_func_param_counts, re-exports, topological init

### Cranelift Fixes (v0.2.83-v0.2.96)
- I32 conversions, is_pointer checks, try/catch restoration, constructor params

### Native Modules (v0.2.41-v0.2.98)
- mysql2, ioredis, ws, async_hooks, ethers.js, 8-arg closure calls

### Foundation (v0.2.37-v0.2.51)
- NaN-boxing, TAG_TRUE/FALSE, BigInt, inline array methods, function inlining

**Milestone: v0.2.49** — First production worker (MySQL, LLM APIs, string parsing, scoring)
