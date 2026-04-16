# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**NOTE**: This file is kept intentionally concise (~300 lines) because it is loaded into every conversation. Detailed historical changelogs are in CHANGELOG.md. When adding new changes, keep entries to 1-2 lines max and move older entries to CHANGELOG.md periodically.

## Project Overview

Perry is a native TypeScript compiler written in Rust that compiles TypeScript source code directly to native executables. It uses SWC for TypeScript parsing and LLVM for code generation.

**Current Version:** 0.5.47

## TypeScript Parity Status

Tracked via the gap test suite (`test-files/test_gap_*.ts`, 22 tests). Each test exercises a feature cluster and is compared byte-for-byte against `node --experimental-strip-types`. Run via `/tmp/run_gap_tests.sh` after `cargo build --release -p perry-runtime -p perry-stdlib -p perry`.

**Last sweep (post-v0.4.87):** **8/22 passing**, **347 total diff lines**.

| Status | Test | Diffs |
|--------|------|-------|
| ✅ PASS | `date_methods` | 0 |
| ✅ PASS | `encoding_timers` | 0 |
| ✅ PASS | `error_extensions` | 0 |
| ✅ PASS | `fetch_response` | 0 |
| ✅ PASS | `json_advanced` | 0 |
| ✅ PASS | `node_path` | 0 |
| ✅ PASS | `node_process` | 0 |
| ✅ PASS | `weakref_finalization` | 0 |
| 🟡 close | `regexp_advanced` | 2 (lookbehind only) |
| 🟡 close | `generators` | 3 |
| 🟡 close | `number_math` | 4 |
| 🟡 close | `string_methods` | 2 (lone surrogates only) |
| 🟡 mid | `class_advanced` | 18 |
| 🟡 mid | `proxy_reflect` | 27 (segfault) |
| 🟡 mid | `object_methods` | 28 |
| 🟡 mid | `node_fs` | 30 |
| 🟡 mid | `global_apis` | 30 |
| 🔴 work | `symbols` | 31 (segfault) |
| 🔴 work | `async_advanced` | 35 (segfault) |
| 🔴 work | `console_methods` | 40 |
| 🔴 work | `array_methods` | 45 |
| 🔴 work | `node_crypto_buffer` | 46 |

**Known categorical gaps**: lookbehind regex (Rust `regex` crate limitation), `Proxy`/`Reflect` not implemented, `Symbol(...)` returns garbage, `Object.getPrototypeOf` returns wrong sentinel, `console.dir` formatting differs from Node, `console.group*` doesn't indent, `console.table` works for the standard shapes, lone surrogate handling (`isWellFormed`/`toWellFormed` — needs WTF-8 support).

**Next-impact targets** (biggest single-commit wins): `console.dir` formatting + `console.group` indent (~15 lines), `Promise.withResolvers` + segfault fix (~35 lines), `URL`/`Blob`/`AbortController` extensions (~15 lines), `Proxy` identity stub (~10 lines), `Symbol` sentinel stub (~10 lines).

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
TypeScript (.ts) → Parse (SWC) → AST → Lower → HIR → Transform → Codegen (LLVM) → .o → Link (cc) → Executable
```

| Crate | Purpose |
|-------|---------|
| **perry** | CLI driver (parallel module codegen via rayon) |
| **perry-parser** | SWC wrapper for TypeScript parsing |
| **perry-types** | Type system definitions |
| **perry-hir** | HIR data structures (`ir.rs`) and AST→HIR lowering (`lower.rs`) |
| **perry-transform** | IR passes (closure conversion, async lowering, inlining) |
| **perry-codegen** | LLVM-based native code generation |
| **perry-runtime** | Runtime: value.rs, object.rs, array.rs, string.rs, gc.rs, arena.rs, thread.rs |
| **perry-stdlib** | Node.js API support (mysql2, redis, fetch, fastify, ws, etc.) |
| **perry-ui** / **perry-ui-macos** / **perry-ui-ios** / **perry-ui-tvos** | Native UI (AppKit/UIKit) |
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

## Threading (`perry/thread`)

User code is single-threaded by default. `perry/thread` module provides three primitives with compile-time safety (no mutable captures allowed):

- **`parallelMap(array, fn)`** — data-parallel array processing across all CPU cores
- **`parallelFilter(array, fn)`** — data-parallel array filtering across all CPU cores
- **`spawn(fn)`** — background OS thread, returns Promise

Values cross threads via `SerializedValue` deep-copy (zero-cost for numbers, O(n) for strings/arrays/objects). Each thread has independent arena + GC. Arena `Drop` frees blocks when worker threads exit. Results from `spawn` flow back via `PENDING_THREAD_RESULTS` queue, drained during `js_promise_run_microtasks()`.

**Compiler pipeline** also parallelized via rayon: module codegen, transform passes, and nm symbol scanning.

## Native UI (`perry/ui`)

Declarative TypeScript compiles to AppKit/UIKit calls. 47 `perry_ui_*` FFI functions. Handle-based widget system (1-based i64 handles, NaN-boxed with POINTER_TAG). 5 reactive binding types dispatched from `state_set()`. `--target ios-simulator`/`--target ios`/`--target tvos-simulator`/`--target tvos` for cross-compilation.

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

**Dedup logic**: When `@noble/hashes` appears in multiple `node_modules/`, the first-resolved directory is cached in `compile_package_dirs`. Subsequent imports redirect to the same copy, preventing duplicate linker symbols.

## Known Limitations

- **No runtime type checking**: Types erased at compile time. `typeof` via NaN-boxing tags. `instanceof` via class ID chain.
- **No shared mutable state across threads**: Thread primitives enforce immutable captures at compile time. No `SharedArrayBuffer` or `Atomics`.

## Common Pitfalls & Patterns

### NaN-Boxing Mistakes
- **Double NaN-boxing**: If value is already F64, don't NaN-box again. Check `builder.func.dfg.value_type(val)`.
- **Wrong tag**: Strings=STRING_TAG, objects=POINTER_TAG, BigInt=BIGINT_TAG.
- **`as f64` vs `from_bits`**: `u64 as f64` is numeric conversion (WRONG). Use `f64::from_bits(u64)` to preserve bits.

### LLVM Type Mismatches
- Loop counter optimization produces i32 — always convert before passing to f64/i64 functions
- Check LLVM value types before conversion; handle f64↔i64, i32→f64, i32→i64
- Constructor parameters always f64 (NaN-boxed) at signature level

### Async / Threading
- Thread-local arenas: JSValues from tokio workers invalid on main thread
- Use `spawn_for_promise_deferred()` — return raw Rust data, convert to JSValue on main thread
- Async closures: Promise pointer (I64) must be NaN-boxed with POINTER_TAG before returning as F64

### Cross-Module Issues
- ExternFuncRef values are NaN-boxed — use `js_nanbox_get_pointer` to extract
- Module init order: topological sort by import dependencies
- Optional params need `imported_func_param_counts` propagation through re-exports

### Closure Captures
- `collect_local_refs_expr()` must handle all expression types — catch-all silently skips refs
- Captured string/pointer values must be NaN-boxed before storing, not raw bitcast
- Loop counter i32 values: `fcvt_from_sint` to f64 before capture storage

### Handle-Based Dispatch
- TWO systems: `HANDLE_METHOD_DISPATCH` (methods) and `HANDLE_PROPERTY_DISPATCH` (properties)
- Both must be registered. Small pointer detection: value < 0x100000 = handle.

### objc2 v0.6 API
- `define_class!` with `#[unsafe(super(NSObject))]`, `msg_send!` returns `Retained` directly
- All AppKit constructors require `MainThreadMarker`

## Recent Changes

Keep entries here to 1-2 lines max. Detailed write-ups live in CHANGELOG.md.

- **v0.5.47** — `Buffer.indexOf(byte)` / `Buffer.includes(byte)` with numeric argument now searches for the byte value instead of returning -1/false (closes #56). Added INT32_TAG and plain-double branches to `js_buffer_index_of`.
- **v0.5.46** — Fix PIC miss handler reading past inline allocation for objects with >8 dynamic fields (closes #55). `js_object_get_field_ic_miss` now checks `alloc_limit` before reading inline memory — fields in the overflow map fall through to the slow path. Also: Zero-copy JSON string parsing + incremental object build. `parse_string_bytes` returns `ParsedStr::Borrowed(&[u8])` for non-escaped strings (zero-copy slice from input buffer), falling back to `ParsedStr::Owned(Vec<u8>)` only for strings with `\` escapes. `parse_object` builds incrementally (no intermediate `Vec<(Vec<u8>, JSValue)>` — fields set via `js_object_set_field_by_name` as they're parsed). Fixed double-RefCell-borrow crash: `js_string_from_bytes` inside `PARSE_KEY_CACHE.borrow_mut()` could trigger GC → `scan_parse_roots` → re-borrow panic; split into check-then-alloc-then-insert. Real JSON pipeline (100-record fixture): **Perry 180ms vs Node 140ms (1.3× gap, was 547×)**.
- **v0.5.45** — JSON.parse key interning + transition-cache shape sharing (v0.5.45). `parse_object` now uses a thread-local `PARSE_KEY_CACHE` (HashMap<Vec<u8>, *const StringHeader>) to intern key strings — first record allocates N keys, subsequent records 0. Objects are built via `js_object_set_field_by_name` (transition cache) instead of manual keys_array construction, so all records from the same schema share their `keys_array` pointer. This enables the v0.5.44 PIC to hit on every PropertyGet after the first record. Combined: parse(500×20 records) dropped from unmeasurable (OOM/crash at 2000 iters) to 10ms, transform from 10s+ to 1ms. 20-record pipeline: Perry 12ms vs Node 4ms (3× gap, was 547×).
- **v0.5.44** — Monomorphic inline cache for PropertyGet (closes #51). Per-site `[2 x i64]` globals (`@perry_ic_N`) cache `(keys_array_ptr, slot_index)`. Fast path: load obj→keys_array (offset 16), compare cached → direct field load at obj+24+slot*8 (no call, no hash, no linear scan). Miss: `js_object_get_field_ic_miss` does full lookup + primes cache. Guards: skip cache for non-regular objects (Error, Closure, etc.) and when `ACCESSORS_IN_USE` is set (getters/setters need the full dispatch). JSON-parsed objects currently get per-object keys_arrays (no shape sharing from the parser), so the PIC hits only for class instances and object-literal shapes shared via the transition cache. Follow-up: shape-sharing in JSON.parse to unlock the PIC for parsed records.
- **v0.5.43** — Wire #49 int-analysis ↔ #50 flat-const bridge (v0.5.43). `collect_integer_let_ids` now accepts `let k = krow[j]` (flat-const IndexGet) as an integer init, and `can_lower_expr_as_i32` + `lower_expr_as_i32` accept `LocalGet(k)` for any local in `integer_locals` (falling through to `lower_expr→fptosi` for const locals without an i32 slot — LLVM's instcombine collapses the `fptosi(sitofp(load i32))` identity). This closes the loop between #49's i32 accumulator and #50's flat table: `rAcc += src[idx] * k` where `k` comes from `KERNEL[ky+2][kx+2]` now runs entirely in i32. image_conv 3840×2160 Gaussian blur: **1.95s → 0.66s (-66%)**, now within 2.7× of Zig (vs previous 8×).
- **v0.5.42** — `!invariant.load !0` metadata on Array/Buffer length loads (closes #52). `LlBlock::safe_load_i32_from_ptr` (used by every bounds check for `Uint8ArrayGet`/`Set`, `IndexGet`, `IndexSet`, `arr.length`) now tags the header `i32` load with `!invariant.load`. LLVM's GVN + LICM are then free to hoist the length reload out of read-only loops over the same buffer — the prior emission forced a reload on every bounds check even when the buffer was loop-invariant, blocking autovectorization of pixel/hash/codec loops. Module-level `!0 = !{}` declared alongside the attribute groups in `LlModule::to_ir`.
- **v0.5.41** — Flat `[N x i32]` constants for module-level `const` 2D int arrays (closes #50). Module compile scans `hir.init` for `const X = [[int, ...], ...]` with rectangular int-literal shape and no mutation anywhere in the module (LocalSet/Update/IndexSet/`push`/`pop`/`splice`/…). Qualifying locals get a private unnamed-addr `[rows*cols x i32]` constant emitted into `.rodata`. `IndexGet` lowering intercepts two patterns: inline `X[i][j]` and the aliased row pattern `const krow = X[i]; krow[j]` (via per-function `array_row_aliases` populated in `Stmt::Let`). Both emit a direct `getelementptr inbounds [N x i32], ... + sitofp` — no arena header read, no NaN-box unwrap, no bounds check. Combines with #49's int-arithmetic fast path: synthetic 100M-iter row+col table lookup dropped to 108ms (vs Node 185ms). image_conv's `KERNEL[ky+2][kx+2]` now folds; its remaining wall-time gap vs Zig is dominated by the per-pixel `clampIdx` call, not the kernel load.
- **v0.5.40** — Accumulator-pattern int-arithmetic fast path (closes #49). `collect_integer_locals` now recognizes `acc = acc + int_expr` (and `-`/`*`) as int-stable via fixed-point iteration — `LocalGet(id)` is int-producing when `id` is itself int-stable, `Uint8ArrayGet`/`BufferIndexGet` always are, and `Add`/`Sub`/`Mul` are when both operands are. The `LocalSet` lowering adds a fast path that, when the target has an i32 slot and every leaf of the rhs is int-sourced, emits the whole rhs as a short chain of `add/sub/mul i32` and stores directly to the i32 slot (one sitofp then maintains the parallel double slot for non-int readers). Skips the `sitofp → fadd → fptosi` round-trip on every iter of `acc += byte * k`. Tight sum-of-bytes benchmark (1M × 100 iters): 272ms → 63ms (-77%). image_conv unchanged pending #50 (`KERNEL[ky+2][kx+2]` still lowers as nested IndexGet, so the `k` operand isn't int-lowerable yet).
- **v0.5.39** — Int32-stable local specialization (closes #48). Two parts: (a) extended `collect_integer_locals` to accept LocalSets whose rhs is `(expr) | 0` / `>>> 0` / pure-bitwise, allocating a parallel i32 alloca for every qualifying mutable function-local — `LocalGet` then loads the i32 + sitofp instead of going through the double slot, so LLVM's mem2reg/DSE flatten the round-trip and `i++` becomes `add w, w, #1` on a real i32 register. (b) Fixed a long-standing bug in `boxed_vars`: `collect_closure_refs_and_writes_in_expr`'s `Expr::Update` arm was inserting unconditionally instead of only when reached via `Expr::Closure`'s body walk — every plain `for (let i = …; …; i++)` body's counter looked like a closure-captured-and-mutated var and got box-allocated even with no closure in the function. Tight `sum=(sum+i)|0; i++` loop is now `add w, w, #1` + `cmp w, w` instead of `bl js_box_get / js_box_set` per iter.
- **v0.5.38** — Inline `Buffer` / `Uint8Array` bracket-access for statically-typed receivers (closes #47). `Uint8ArrayGet` / `Uint8ArraySet` in codegen now emit `ldrb` / `strb` with an unsigned bounds compare instead of `bl js_buffer_get` / `bl js_buffer_set`; unblocks LLVM loop-invariant hoisting of the length load and autovectorization of pixel / hash / codec loops. image_conv 3840×2160 Gaussian blur: 2.19s → 1.98s (-10%); 10M-byte tight sum loop: 275ms → 243ms (-12%).
- **v0.5.37** — `JSON.parse` on large arrays no longer silently truncates (closes #46). Added a thread-local GC-root stack for in-progress `parse_array`/`parse_object` frames: the in-progress ArrayHeader, each fresh value before it lands in its parent, and `parse_object`'s `Vec<(Vec<u8>, JSValue)>` backing storage were all invisible to the conservative stack scan. Mid-parse `gc_malloc` (adaptive count trigger at ~1666 records) swept them. Also root the input `StringHeader` so `parser.input: &[u8]` — a pointer into the data region that the valid-ptr-set doesn't index — can't dangle.
- **v0.5.36** — Buffer param `src[i]` reads/writes bytes (closes #42). HIR lowering of computed-member access on locals now treats `Type::Named("Buffer")` as a synonym for `Uint8Array`, routing to `Uint8ArrayGet`/`Set` instead of the generic `IndexGet` that returned NaN-boxed pointer bits as a denormal f64.
- **v0.5.35** — `process.argv.slice(N)` returns a real array (closes #41). `Expr::ProcessArgv` added to the HIR `.slice()` array-receiver allow-list so it lowers to `ArraySlice` instead of falling through to String.slice semantics.
- **v0.5.34** — `Math.imul(a, b)` lowers in the LLVM backend (closes #40). `fptosi→trunc i32→mul→sitofp` inline sequence — matches Node for every 32-bit-wrap case. Unblocks FNV-1a-32 / MurmurHash3 / xxhash32 / CRC32 / PCG in user TS.
- **v0.5.33** — JSON.stringify/parse on large arrays (closes #43, #44). GC now transitively marks arena-block-persisting objects (fixes malloc children freed under `arr.push` when new object lives only in caller-saved regs); `trace_array` length cap raised 65k → 16M; `stringify_value` dispatches on GC `obj_type` tag instead of capacity heuristic that misread length-≥10k arrays as strings.
- **v0.5.32** — BigInt bitwise ops (`&`, `|`, `^`, `<<`, `>>`) dispatch through the runtime's bigint helpers (closes #39). Previously these fell through to the i32 ToInt32 path, which `fptosi`'d NaN-boxed bigint bits and returned garbage — XOR gave small signed ints, AND-masking collapsed to 0.
- **v0.5.31** — `new Uint8Array(n)` with non-literal `n` allocates correctly (closes #38). Runtime dispatch `js_uint8array_new(val)` inspects the NaN-box tag and routes numeric lengths to `js_uint8array_alloc` instead of misreading them as `ArrayHeader*`.
- **v0.5.30** — Dynamic property write at Node parity (closes #37). Shape-transition cache `(prev_keys, key_ptr) → (next_keys, slot_idx)` skips linear scan; `Vec<u64>` overflow replaces nested HashMap; last-accessed Vec cache skips outer HashMap lookup; inlined fast-path field write; single `ANY_DESCRIPTORS_IN_USE` gate. 10k×20 build: 43.3→6.4ms (-85%). cols≥20: parity/edge vs Node v25 (cols=80: 22.4 vs 22.6ms).
- **v0.5.29** — Row-object alloc perf (-14% on @perry/postgres 10k-row bulk decode): skip needless keys_array clones via `GC_FLAG_SHAPE_SHARED`, defer descriptor-lookup String alloc, i64 bigint fast path.
- **v0.5.28** — Register module-level `let`/`const` globals as GC roots (closes #36). Stops sweep of `const X = new Map(...)` when only the stack-less global holds the ref.
- **v0.5.27** — GC root scanners for `ws` / `http` / `events` / `fastify` listener closures (refs #35). Follow-up sweep after v0.5.26.
- **v0.5.26** — GC root scanner for `net.Socket` listener closures in `NET_LISTENERS` (closes #35). Unblocked after v0.5.25 made malloc-triggered GC common.
- **v0.5.25** — GC fires from `gc_malloc` + per-thread adaptive malloc-count threshold (closes #34). 2M bigint allocs: 8.45 GB → 36 MB peak RSS.
- **v0.5.24** — Bigint literals use `BIGINT_TAG`, `BigInt()` coercion, `Binary` ops dispatch to bigint runtime when statically typed (closes #33).
- **v0.5.23** — Module init follows topological order (not alphabetical); `import * as O` namespace property dispatch (closes #32).
- **v0.5.22** — Doc URL swaps; compile output gated behind `--verbose`; CI pins `MACOSX_DEPLOYMENT_TARGET=13.0`.
- **v0.5.21** — Fastify handler params tagged `FastifyRequest`/`FastifyReply` in HIR; `gc()` no-ops while tokio servers live (closes #30, #31).
- **v0.5.20** — `String.length` returns UTF-16 code units (`"café".length` → 4, `"😀".length` → 2) (closes #18 partially).
- **v0.5.19** — Restore native module dispatch (mysql/pg/redis/mongo/sqlite/fastify/ws) lost in v0.5.0 cutover; fix `gc()` symbol; drop `--warn-unresolved-symbols` (closes #28).
- **v0.5.18** — Native `axios` dispatch; fetch GET segfault fix; async pump wired into await loop; `.d.ts` stubs for `perry/ui|thread|i18n|system` (closes #24-#27).
- **v0.5.17** — Escape analysis + scalar replacement of non-escaping objects (zero heap allocs on hot paths). Perry beats Node on all 15 benchmarks.
- **v0.5.16** — watchOS device target uses `arm64_32` (ILP32) triple instead of `aarch64`.
- **v0.5.15** — perry/ui `State` constructor + `.value`/`.set()` dispatch; `is_perry_builtin()` guard in check-deps (closes #24, #25).
- **v0.5.14** — Windows build fix: `date.rs` split into `#[cfg(unix)]` (`localtime_r`) / `#[cfg(windows)]` (`localtime_s`) branches.
- **v0.5.13** — `Buffer.indexOf`/`includes` routed to buffer dispatch instead of string-method path.
- **v0.5.12** — perry/ui full widget dispatch (~40 methods, VStack/HStack/Button special cases); mango renders full UI.
- **v0.5.11** — Inline-allocator: post-init boundary for keys_array load; `js_register_class_parent` so `instanceof` walks inheriting classes. Parity 80% → 94%.
- **v0.5.10** — `perry/ui.App({...})` dispatch — mango actually launches (enters `NSApplication.run()`).
- **v0.5.9** — `let C = SomeClass; new C()` resolves the alias; `refine_type_from_init` follows through `local_class_aliases`.
- **v0.5.2** — Fast-math FMFs on `fadd`/`fsub`/...; integer-modulo fast path (`fptosi → srem → sitofp`). Beats Node on 8/11 numeric benchmarks.
- **v0.5.0** — Cranelift backend deleted; LLVM is the only codegen. Parity identical pre/post: 102 MATCH / 9 DIFF (91.8%).
