# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**NOTE**: This file is kept intentionally concise (~300 lines) because it is loaded into every conversation. Detailed historical changelogs are in CHANGELOG.md. When adding new changes, keep entries to 1-2 lines max and move older entries to CHANGELOG.md periodically.

## Project Overview

Perry is a native TypeScript compiler written in Rust that compiles TypeScript source code directly to native executables. It uses SWC for TypeScript parsing and Cranelift for code generation.

**Current Version:** 0.3.0

## Workflow Requirements

**IMPORTANT:** Follow these practices for every code change:

1. **Update CLAUDE.md**: Add 1-2 line entry in "Recent Changes" for new features/fixes
2. **Increment Version**: Bump patch version (e.g., 0.2.147 → 0.2.148)
3. **Commit Changes**: Include code changes and CLAUDE.md updates together

## Build Commands

```bash
cargo build --release                          # Build all crates
cargo build --release -p perry-runtime -p perry-stdlib  # Rebuild runtime (MUST rebuild stdlib too!)
cargo test --workspace --exclude perry-ui-ios  # Run tests (exclude iOS on macOS host)
cargo run --release -- file.ts -o output && ./output    # Compile and run TypeScript
cargo run --release -- file.ts --print-hir              # Debug: print HIR
```

## Architecture

```
TypeScript (.ts) → Parse (SWC) → AST → Lower → HIR → Transform → Codegen (Cranelift) → .o → Link (cc) → Executable
```

| Crate | Purpose |
|-------|---------|
| **perry** | CLI driver |
| **perry-parser** | SWC wrapper for TypeScript parsing |
| **perry-types** | Type system definitions |
| **perry-hir** | HIR data structures (`ir.rs`) and AST→HIR lowering (`lower.rs`) |
| **perry-transform** | IR passes (closure conversion, async lowering, inlining) |
| **perry-codegen** | Cranelift-based native code generation |
| **perry-runtime** | Runtime: value.rs, object.rs, array.rs, string.rs, gc.rs, arena.rs, etc. |
| **perry-stdlib** | Node.js API support (mysql2, redis, fetch, fastify, ws, etc.) |
| **perry-ui** / **perry-ui-macos** / **perry-ui-ios** | Native UI (AppKit/UIKit) |
| **perry-jsruntime** | JavaScript interop via QuickJS |

## NaN-Boxing

Perry uses NaN-boxing to represent JavaScript values in 64 bits (`perry-runtime/src/value.rs`):

```
TAG_UNDEFINED = 0x7FFC_0000_0000_0001    BIGINT_TAG  = 0x7FFA (lower 48 = ptr)
TAG_NULL      = 0x7FFC_0000_0000_0002    POINTER_TAG = 0x7FFD (lower 48 = ptr)
TAG_FALSE     = 0x7FFC_0000_0000_0003    INT32_TAG   = 0x7FFE (lower 32 = int)
TAG_TRUE      = 0x7FFC_0000_0000_0004    STRING_TAG  = 0x7FFF (lower 48 = ptr)
```

Key functions: `js_nanbox_string/pointer/bigint`, `js_nanbox_get_pointer`, `js_get_string_pointer_unified`, `js_jsvalue_to_string`, `js_is_truthy`

**Module-level variables**: Strings stored as F64 (NaN-boxed), Arrays/Objects as I64 (raw pointers). Access via `module_var_data_ids`.

## Garbage Collection

Mark-sweep GC in `crates/perry-runtime/src/gc.rs` with conservative stack scanning. Arena objects (arrays, objects) discovered by linear block walking (zero per-alloc tracking). Malloc objects (strings, closures, promises, bigints, errors) tracked in thread-local Vec. Triggers on new arena block allocation (~8MB) or explicit `gc()` call. 8-byte GcHeader per allocation.

## Native UI (`perry/ui`)

Declarative TypeScript compiles to AppKit/UIKit calls. 47 `perry_ui_*` FFI functions. Handle-based widget system (1-based i64 handles, NaN-boxed with POINTER_TAG). 5 reactive binding types dispatched from `state_set()`. `--target ios-simulator`/`--target ios` for cross-compilation.

**To add a new widget** — change 4 places:
1. Runtime: `crates/perry-ui-macos/src/widgets/` — create widget, `register_widget(view)`
2. FFI: `crates/perry-ui-macos/src/lib.rs` — `#[no_mangle] pub extern "C" fn perry_ui_<widget>_create`
3. Codegen: `crates/perry-codegen/src/codegen.rs` — declare extern + NativeMethodCall dispatch
4. HIR: `crates/perry-hir/src/lower.rs` — only if widget has instance methods

## Compiling npm Packages Natively (`perry.compilePackages`)

Projects can list npm packages to compile natively instead of routing to V8. Configured in `package.json`:

```json
{ "perry": { "compilePackages": ["@noble/curves", "@noble/hashes"] } }
```

**Implementation** (`crates/perry/src/commands/compile.rs`):
- `CompilationContext.compile_packages: HashSet<String>` — packages to compile natively
- `CompilationContext.compile_package_dirs: HashMap<String, PathBuf>` — dedup cache (first-found dir per package)
- `resolve_package_source_entry()` — prefers `src/index.ts` over `lib/index.js`
- `is_in_compile_package()` — checks if a file path is inside a listed package
- `resolve_import()` — redirects compile packages to first-found dir for dedup, marks as `NativeCompiled`
- `collect_modules()` — `.js` files inside compile packages bypass JS runtime routing

**Dedup logic**: When `@noble/hashes` appears in both `@noble/curves/node_modules/` and `@solana/web3.js/node_modules/`, the first-resolved directory is cached in `compile_package_dirs`. Subsequent imports redirect to the same copy, preventing duplicate linker symbols.

## Known Limitations

- **No runtime type checking**: Types erased at compile time. `typeof` via NaN-boxing tags. `instanceof` via class ID chain.
- **Single-threaded**: User code on one thread. Async I/O on tokio worker pool. Use `spawn_for_promise_deferred()` for safe cross-thread data transfer.

## Common Pitfalls & Patterns

### NaN-Boxing Mistakes
- **Double NaN-boxing**: If value is already F64, don't NaN-box again. Check `builder.func.dfg.value_type(val)`.
- **Wrong tag**: Strings=STRING_TAG, objects=POINTER_TAG, BigInt=BIGINT_TAG.
- **`as f64` vs `from_bits`**: `u64 as f64` is numeric conversion (WRONG). Use `f64::from_bits(u64)` to preserve bits.
- **Handle extraction**: Handle-based objects are small integers NaN-boxed with POINTER_TAG. Use `js_nanbox_get_pointer`, not bitcast.

### Cranelift Type Mismatches
- Loop counter optimization produces I32 — always convert before passing to F64/I64 functions
- Check `builder.func.dfg.value_type(val)` before conversion; handle F64↔I64, I32→F64, I32→I64
- `is_pointer && !is_union` for variable type determination
- Constructor parameters always F64 (NaN-boxed) at signature level

### Function Inlining (inline.rs)
- `try_inline_call` returns body with `Stmt::Return` — in `Stmt::Expr` context, convert `Return(Some(e))` → `Expr(e)`
- `substitute_locals` must handle ALL expression types

### Async / Threading
- Thread-local arenas: JSValues from tokio workers invalid on main thread
- Use `spawn_for_promise_deferred()` — return raw Rust data, convert to JSValue on main thread
- Async closures: Promise pointer (I64) must be NaN-boxed with POINTER_TAG before returning as F64

### Cross-Module Issues
- ExternFuncRef values are NaN-boxed — use `js_nanbox_get_pointer` to extract
- Module init order: topological sort by import dependencies
- Optional params need `imported_func_param_counts` propagation through re-exports
- Wrapper functions: truncate `call_args` to match declared signature

### Closure Captures
- `collect_local_refs_expr()` must handle all expression types — catch-all silently skips refs
- Captured string/pointer values must be NaN-boxed before storing, not raw bitcast
- Loop counter i32 values: `fcvt_from_sint` to f64 before capture storage

### Loop Optimization
- Pattern 3 accumulator (8 f64 accumulators) — skip for string variables
- LICM: `hoisted_element_loads` + `hoisted_i32_products` cache invariant loads before inner loops
- `try_compile_index_as_i32` keeps i32 ops contained to array indexing only

### Handle-Based Dispatch
- TWO systems: `HANDLE_METHOD_DISPATCH` (methods) and `HANDLE_PROPERTY_DISPATCH` (properties)
- Both must be registered. Small pointer detection: value < 0x100000 = handle.

### UI Codegen
- NativeMethodCall has TWO arg paths: has-object and no-object — don't mix them up
- `js_object_get_field_by_name_f64` (NOT `js_object_get_field_by_name`) for runtime field extraction

### objc2 v0.6 API
- `define_class!` with `#[unsafe(super(NSObject))]`, `msg_send!` returns `Retained` directly
- All AppKit constructors require `MainThreadMarker`
- `CGPoint`/`CGSize`/`CGRect` in `objc2_core_foundation`

## Recent Changes

### v0.3.0
- Compile-time i18n system (`perry/i18n` module): zero-ceremony localization for UI strings, `[i18n]` config in perry.toml, embedded 2D string table, native locale detection (all 6 platforms via OS APIs), `{param}` interpolation, CLDR plural rules (30+ locales), format wrappers (`Currency`, `Percent`, `ShortDate`, `LongDate`, `FormatNumber`, `FormatTime`, `Raw`), `perry i18n extract` CLI, iOS `.lproj` + Android `values-xx/` generation, compile-time validation

### v0.2.203
- `perry setup` summary: show what's stored in global config vs project perry.toml for all platforms

### v0.2.202
- Fix `perry setup ios` not saving bundle_id to perry.toml

### v0.2.201
- `perry setup` improvements: auto-detect signing identity, show config paths, bundle_id priority lookup

### v0.2.200
- Fix `perry setup` not saving to project perry.toml when file didn't exist
- Audio capture API (`perry/system`): 6 platforms, A-weighted dB(A) metering
- Camera API (`perry/ui`, iOS only): CameraView, freeze/unfreeze, pixel sampling

### v0.2.199
- Fix `import * as X` namespace function calls, ScrollView in ZStack, SIGBUS during module init

### v0.2.198
- WidgetKit: full iOS + Android + watchOS + Wear OS support, 4 new compile targets

### Older (v0.2.37-v0.2.197)
See CHANGELOG.md for detailed history. Key milestones:
- v0.2.191: Geisterhand UI testing framework
- v0.2.183-189: WebAssembly target (`--target wasm`)
- v0.2.180: `perry run` command with remote build fallback
- v0.2.174: `perry/widget` module + `--target ios-widget`
- v0.2.172: Codebase refactor (codegen.rs split into 12 modules, lower.rs into 8)
- v0.2.156-162: Cross-platform UI parity (all 6 platforms at 100%)
- v0.2.150-151: Native plugin system
- v0.2.147: Mark-sweep garbage collection
- v0.2.116: Native UI module (perry/ui)
- v0.2.115: Integer function specialization (fibonacci 2x faster than Node)
- v0.2.79: Fastify-compatible HTTP runtime
- v0.2.49: First production worker (MySQL, LLM APIs, scoring)
- v0.2.37: NaN-boxing foundation
