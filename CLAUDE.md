# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**NOTE**: This file is kept intentionally concise (~300 lines) because it is loaded into every conversation. Detailed historical changelogs are in CHANGELOG.md. When adding new changes, keep entries to 1-2 lines max and move older entries to CHANGELOG.md periodically.

## Project Overview

Perry is a native TypeScript compiler written in Rust that compiles TypeScript source code directly to native executables. It uses SWC for TypeScript parsing and LLVM for code generation.

**Current Version:** 0.5.6

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
| 🟡 close | `string_methods` | 8 (UTF-16 length) |
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

**Known categorical gaps**: lookbehind regex (Rust `regex` crate limitation), `String.length` returns byte count instead of UTF-16 code units, `Proxy`/`Reflect` not implemented, `Symbol(...)` returns garbage, `Object.getPrototypeOf` returns wrong sentinel, `console.dir` formatting differs from Node, `console.group*` doesn't indent, `console.table` works for the standard shapes.

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

For older versions (v0.4.144 and earlier), see CHANGELOG.md.

### v0.5.6 (llvm-backend) — perry-stdlib auto-optimize `hex` crate fix
- **fix**: `crates/perry-stdlib/src/sqlite.rs:54` was using `hex::encode(b)` to format SQLite `Blob` columns as hex strings, but the `hex` crate dep in `perry-stdlib`'s `Cargo.toml` is gated behind the `crypto` Cargo feature. Auto-optimize rebuilds that enabled only `database-sqlite` (e.g. mango: `better-sqlite3` + `mongodb` + fetch, no crypto) failed with `error[E0433]: failed to resolve: use of unresolved module or unlinked crate hex` and fell back to the prebuilt full stdlib, leaving every user binary 100KB+ larger than necessary. Replaced with a hand-rolled nibble loop (`const HEX: &[u8; 16] = b"0123456789abcdef"; for &byte in b { out.push(HEX[(byte >> 4) as usize]); out.push(HEX[(byte & 0x0f) as usize]); }`) so sqlite no longer depends on hex. Surgical fix — no Cargo.toml or auto-optimize logic changes. Mango now goes through the auto-optimize rebuild path: prebuilt-fallback 5.18 MB → optimized 5.01 MB (~168 KB / 3.4% savings, mostly from features the user doesn't import being stripped). Original fix done as a worktree-isolated subagent task; the agent's commit was based on a stale `llvm-backend` HEAD so the sqlite.rs change was applied manually here on top of v0.5.5.

### v0.5.5 (llvm-backend) — `alloca_entry` sweep
- **fix**: 7 cross-block alloca sites in `expr.rs` / `lower_call.rs` / `stmt.rs` migrated to `LlFunction.alloca_entry()` to close the latent SSA dominance hazards flagged in v0.5.2's followup list. Migrated: catch-clause exception binding (capturable by nested closures in the catch body), `super()`-inlined parent ctor params (capturable by closures inside the parent ctor body), `forEach` loop counter (spans cond/body/exit successor blocks), `Await` result slot (spans check/wait/settled/done/merge blocks; can be lowered inside a nested if-arm), `NewClass` `this_slot` (pushed on `this_stack` for the entire inlined ctor body with nested closures capturing `this`), and the inlined-ctor param slots in two places. Left alone with comment: `js_array_splice out_slot` (single-block scratch, dominance-safe by construction). Mango compiles + links cleanly. Original sweep done as a worktree-isolated subagent task because main was being concurrently edited; cherry-picked back here.

### v0.5.4 (llvm-backend) — `Expr::ExternFuncRef`-as-value via static `ClosureHeader` thunks
- **fix**: imported functions can now be passed as callbacks, stored in variables, and called indirectly. Previously `Expr::ExternFuncRef` lowered as a value returned a `TAG_TRUE` sentinel that worked for `if (importedFn)` truthiness checks but crashed at runtime the moment anything tried to dispatch through `js_closure_callN`. The fix mirrors the existing `__perry_wrap_<name>` machinery for local funcs (`crates/perry-codegen/src/codegen.rs:870-904`): for every entry in `opts.import_function_prefixes`, `compile_module` now emits a thin `__perry_wrap_extern_<src>__<name>` wrapper (`internal` linkage so per-module copies don't collide at link time) plus a static `ClosureHeader` constant `__perry_extern_closure_<src>__<name>` whose `func_ptr` points at the wrapper and `type_tag = CLOSURE_MAGIC`. The expr.rs lowering returns `ptrtoint @<global> to i64` NaN-boxed as POINTER. New `LlModule.add_internal_constant()` helper. Verified end-to-end with a TS test that uses `arr.map(double)`, `if (double)`, `f === g`, and `fn(3, 4)` indirect call — all four cases produce correct output (was `[undef, undef, ...]` and `undefined` before). Mango unaffected (entry path uses truthiness only).

### v0.5.3 (llvm-backend) — driver hard-fails on entry-module codegen errors
- **fix**: `crates/perry/src/commands/compile.rs` now refuses to link when the entry module is in `failed_modules`. The original 0.5.0 mango bug was a misdiagnosis chain: 13 modules (including `mango/src/app.ts`) failed codegen, the driver silently replaced each with an empty `_perry_init_*` stub, and the link step exploded with `Undefined symbols for architecture arm64: "_main"` — a downstream symptom that took manual digging to trace back to the real codegen errors hidden in cargo build noise. The driver now (a) prints a loud box-drawn failure summary right after the parallel compile loop, *before* `build_optimized_libs` floods stdout, (b) marks the entry module with `(entry)` in the failure list, and (c) returns `Err` immediately if the entry module is in the list, with a message explaining why. Non-entry failures keep the previous "stub the init, continue linking" behavior but get the same loud summary so the codegen errors aren't drowned in the cargo noise. `use_color` (was `_use_color`) is now wired through to ANSI red on the headers.

### v0.5.2 (llvm-backend) — crushing the numeric benchmarks
- **perf**: `fadd/fsub/fmul/fdiv/frem/fneg` IR builder now emits `reassoc contract` fast-math flags. Clang's `-ffast-math` does NOT retroactively apply to ops in a `.ll` input — the FMFs must be on each instruction. Adding `reassoc contract` lets LLVM break serial accumulator chains into parallel accumulators + 8x-unroll + NEON 2-wide vectorize. **`loop_overhead` 99ms → 13ms (4.1x faster than Node 54ms); `math_intensive` 50ms → 14ms (3.3x faster than Node)**.
- **perf**: Integer-modulo fast path in `BinaryOp::Mod` when both operands are provably integer-valued. New `crate::collectors::collect_integer_locals` walker tracks locals that start from an `Integer` literal and are only ever mutated via `Update` (++/--, no `LocalSet`). Mod-by-integer on such values emits `fptosi → srem → sitofp` instead of `frem double`, which lowers to a libm `fmod()` call on ARM (no hardware instruction). LLVM's SCEV then replaces the div with a reciprocal-multiplication `msub` and hoists the conversions. **`factorial` (sum += i % 1000) 1553ms → 24ms — 64x faster, 25x faster than Node 603ms**.
- Perry now beats Node on 8/11 numeric benchmarks (loop_overhead, math_intensive, factorial, closure, mandelbrot, matrix_multiply, array_read, nested_loops); ties on 2; loses on object_create/binary_trees only (blocked on inline bump-allocator, a pending refactor).

### v0.5.1 (llvm-backend) — mango compile sweep
- feat: 13 LLVM-backend gap fixes that let `mango` compile end-to-end with 0.5.0 (was hitting 13 module-level codegen errors that the driver silently turned into empty `_perry_init_*` stubs, leaving the link with no `_main`). Fixed: `Array.slice()` 0-arg, variadic `arr.push(a,b,c,…)`, `Expr::ArraySome`/`ArrayEvery`/`NewDynamic`/`FetchWithOptions`/`I18nString`/`ExternFuncRef`-as-value, `js_closure_call6..16` (was capped at 5). Killed the buggy cross-module pre-walker (`collect_extern_func_refs_in_*`) and replaced it with **lazy declares** via `FnCtx.pending_declares`, drained after each compile pass — fixes `use of undefined value @perry_fn_*` from cross-module calls inside closures, try/switch, and array callbacks. Closure pre-walker now also walks getters/setters/static_methods (was only methods+ctor) and recurses through ArraySome/Every/NewDynamic/FetchWithOptions/I18nString/Yield. New `LlFunction.alloca_entry()` hoists `Stmt::Let` slots to the entry block — fixes pre-existing SSA dominance verifier failure when a `let` declared inside an `if` arm is captured by a closure in a sibling branch. Mango binary: 4.9MB, links clean.

### v0.5.0 — Phase K hard cutover (LLVM-only)
- **Cranelift backend deleted.** `crates/perry-codegen-llvm/` renamed to `crates/perry-codegen/` as the only codegen path. `--backend` CLI flag removed; all `cranelift*` workspace deps dropped. Parity sweep identical pre/post: **102 MATCH / 9 DIFF / 0 CRASH / 91.8%**. Remaining DIFFs are 8 nondeterministic (timing/RNG/UUID) + async-generator baseline + long-tail features (lookbehind regex, UTF-8/UTF-16 length gap, lone surrogates).

### v0.4.146-followup-2 (llvm-backend)
- feat: `test_gap_array_methods` DIFF (3) → **MATCH**. Four coordinated fixes: 16-pass microtask drain in `main()` so top-level `.then(cb)` fires; `is_promise_expr` recognizes async-FuncRef calls via new `local_async_funcs` HashSet; nested `async function*` declarations hoist to top-level so generator transform sees them; `scan_expr_for_max_local`/`_max_func` in `perry-transform/generator.rs` now walk all array fast-path variants (ArrayMap/Filter/etc.) to prevent LocalId/FuncId collisions.

### v0.4.146-followup (llvm-backend)
- feat: **`Object.groupBy`**, **`Array.fromAsync`**, optional-chain array fast path (`obj?.map(...)` folds through array dispatch), `typeof Object.<method>` → `"function"` constant fold. `test_gap_array_methods` DIFF (7) → DIFF (3).

### v0.4.148 (llvm-backend)
- feat: `test_gap_node_crypto_buffer` DIFF (54) → **MATCH**. Full Node-style Buffer/crypto surface: new `dispatch_buffer_method` in `object.rs` routes `js_native_call_method` for any registered buffer (read/write numeric family, `swap*`, `indexOf`/`includes`, `slice`/`fill`/`compare`/`toString(enc)`); `crypto.getRandomValues`, `Buffer.compare/from/alloc/concat` wired; `Buffer.from([arr])` path decodes via `js_buffer_from_value`; type inference refines `Buffer.from`/`crypto.randomBytes` to `Named("Uint8Array")`; crypto `createHash(...).update(...).digest(enc)` chain detected as string; `bigint_value_to_i64` accepts POINTER_TAG-boxed BigInt pointers.

### v0.4.147 (llvm-backend)
- feat: `test_gap_symbols` DIFF (4) → **MATCH**. `Symbol.hasInstance` and `Symbol.toStringTag` via HIR class lowering of well-known keys (lifts to `__perry_wk_hasinstance_*`/`__perry_wk_tostringtag_*`), new `CLASS_HAS_INSTANCE_REGISTRY`/`CLASS_TO_STRING_TAG_REGISTRY` in runtime, and `Object.prototype.toString.call(x)` → `js_object_to_string` dispatch in HIR.

### v0.4.146 (llvm-backend)
- feat: `Symbol.toPrimitive` semantic support — `+currency` / `` `${currency}` `` / `currency + 0` all consult `obj[Symbol.toPrimitive]` via new `js_to_primitive(v, hint)` hook threaded through `js_number_coerce` and `js_jsvalue_to_string`. Well-known symbol cache in `symbol.rs`; computed-key method lowering via new `PostInit::SetMethodWithThis` variant. `test_gap_symbols` DIFF (10) → DIFF (4).

### v0.4.145 (llvm-backend)
- feat: real **TypedArray** support (Int8/Int16/Int32, Uint16/Uint32, Float32/Float64). New `typedarray.rs` with `TYPED_ARRAY_REGISTRY`; generic array helpers (`js_array_at`, `js_array_to_sorted`, `js_array_with`, `js_array_find_last`, etc.) detect typed-array pointers and dispatch per-kind, preserving `Int32Array(N) [ ... ]` Node format on round-trip. Reserved class IDs `0xFFFF0030..0037` for `instanceof`. `test_gap_array_methods` DIFF (35) → DIFF (7).

