# Changelog

Detailed changelog for Perry. See CLAUDE.md for concise summaries.

## v0.3.0 — Compile-Time Internationalization

Major release adding a complete compile-time i18n system to Perry.

### Core Mechanism
- `[i18n]` section in perry.toml: `locales`, `default_locale`, `dynamic`, `[i18n.currencies]`
- Embedded 2D string table: `translations[locale_idx * key_count + string_idx]` — all locales baked into binary
- UI widget string detection: string literals in `Button`, `Text`, `Label`, `TextField`, `TextArea`, `Tab`, `NavigationTitle`, `SectionHeader`, `SecureField`, `Alert` automatically treated as localizable keys
- `Expr::I18nString` HIR variant with transform pass (`perry-transform/src/i18n.rs`) and Cranelift codegen with locale branching
- Compile-time validation: warns on missing translations, unused keys, parameter mismatches
- Key registry: `.perry/i18n-keys.json` updated on every build

### Locale Detection (all 6 platforms)
- macOS/iOS: `CFLocaleCopyCurrent()` (CoreFoundation) — works for GUI apps launched from Finder/SpringBoard
- Windows: `GetUserDefaultLocaleName()` (Win32)
- Android: `__system_property_get("persist.sys.locale")` (bionic libc)
- Linux: `LANG` / `LC_ALL` / `LC_MESSAGES` env vars
- Platform-native APIs tried first, env vars as fallback
- Fuzzy matching: `de_DE.UTF-8` matches `de`, normalizes `_` to `-`

### Interpolation & Plurals
- Parameterized strings: `Text("Hello, {name}!", { name: user.name })` — runtime `perry_i18n_interpolate()` does `{param}` → value substitution
- CLDR plural rules for 30+ locales: `.one`/`.other`/`.few`/`.many`/`.zero`/`.two` suffixes, compile-time validation, runtime `perry_i18n_plural_category()` category selection
- `perry/i18n` native module: `import { t } from "perry/i18n"` for non-UI string localization

### Format Wrappers
- `Currency(value)`, `Percent(value)`, `ShortDate(timestamp)`, `LongDate(timestamp)`, `FormatNumber(value)`, `FormatTime(timestamp)`, `Raw(value)` — importable from `perry/i18n`
- Hand-rolled formatting rules for 25+ locales: number grouping, decimal/thousands separators, currency symbol placement, date ordering (MDY/DMY/YMD), 12h vs 24h time, percent spacing
- `[i18n.currencies]` config: locale → ISO 4217 code mapping

### CLI & Platform Output
- `perry i18n extract`: scans `.ts`/`.tsx` files, generates/updates `locales/*.json` scaffolds
- iOS: `{locale}.lproj/Localizable.strings` generated inside `.app` bundle
- Android: `res/values-{locale}/strings.xml` generated alongside `.so`

### New Files
- `crates/perry-transform/src/i18n.rs` — HIR transform pass
- `crates/perry-runtime/src/i18n.rs` — Runtime: locale detection, interpolation, plural rules, formatters
- `crates/perry/src/commands/i18n.rs` — CLI extract command
- `docs/src/i18n/` — 4 documentation pages (overview, interpolation, formatting, CLI)

## v0.2.202
- Fix `perry setup ios` not saving bundle_id to perry.toml — bundle ID was used for provisioning profile creation but never written to `[ios].bundle_id`; `perry publish` fell back to default `com.perry.<name>`, causing profile/bundle mismatch

## v0.2.201
- `perry setup` improvements: auto-detect signing identity from Keychain when reusing existing certificate; show both global and project config paths; bundle_id lookup checks `[ios]` → `[app]` → `[project]` priority; app name checks `[app]` → `[project]`

## v0.2.200
- Fix `perry setup` not saving to project perry.toml: all 3 platform wizards silently skipped writing when file didn't exist — now auto-creates it
- Audio capture API (`perry/system`): `audioStart`, `audioStop`, `audioGetLevel`, `audioGetPeak`, `audioGetWaveformSamples`, `getDeviceModel` — all 6 platforms; A-weighted IIR filter, EMA smoothing, lock-free ring buffer
- Camera API (`perry/ui`, iOS only): `CameraView`, `cameraStart`/`Stop`/`Freeze`/`Unfreeze`, `cameraSampleColor(x,y)` — AVCaptureSession + AVCaptureVideoPreviewLayer

## v0.2.199
- Fix `import * as X` namespace function calls: intercept in `Call { PropertyGet { ExternFuncRef } }` path; also handles exported closures via `js_closure_callN` fallback
- Fix ScrollView invisible inside ZStack: `widgets::add_child` now detects ZStack parents via handle tracking
- Fix SIGBUS during module init with JS runtime async calls: proper V8 stack limit from `pthread_get_stackaddr_np`; `js_run_stdlib_pump()` in UI pump timer
- Fix regex test assertions + fastify URL query stripping

## v0.2.198
- Widget: full iOS + Android + watchOS + Wear OS support: WidgetDecl extended with config_params, provider_func_name, placeholder, family_param_name, app_group, reload_after_seconds
- New WidgetNode variants: ForEach, Divider, Label, FamilySwitch, Gauge (watchOS)
- New crates: `perry-codegen-glance` (Android Glance widgets), `perry-codegen-wear-tiles` (Wear OS Tiles)
- 4 new compile targets: `--target watchos-widget`, `--target android-widget`, `--target wearos-tile`, `--target watchos-widget-simulator`

## v0.2.197
- Cross-platform `menuClear` + `menuAddStandardAction` FFI to all 6 platforms (were macOS-only)
- Fix `dispatch_menu_item` RefCell re-entrancy panic on Windows

## v0.2.196
- Fix `perry publish` showing wrong platform for Windows/Web: `target_display` match was missing cases

## v0.2.195
- Documentation: comprehensive perry.toml reference (`docs/src/cli/perry-toml.md`)
- Documentation: comprehensive geisterhand reference rewrite (`docs/src/testing/geisterhand.md`)

## v0.2.194
- CLI: platform as positional arg for `run` and `publish` (`perry run ios`, `perry publish macos`)

## v0.2.193
- Fix bundle ID not reading from perry.toml: `AppConfig` struct was missing `bundle_id` field

## v0.2.192
- Configurable geisterhand port: `--geisterhand-port <PORT>` CLI flag

## v0.2.191
- Geisterhand: in-process input fuzzer for Perry UI — `--enable-geisterhand` embeds HTTP server (port 7676)
- Screenshot capture all 5 native platforms
- Auto-build geisterhand libs when missing

## v0.2.189
- WASM target: Firefox NaN canonicalization fix — memory-based calling convention for all bridge functions

## v0.2.188
- WASM target: full perry/ui support — 170+ DOM-based UI functions via JS runtime bridge

## v0.2.187
- WASM target: class getters/setters, exception propagation, setTimeout/setInterval, Buffer methods, crypto.sha256

## v0.2.186
- WASM target: full class compilation, try/catch/finally, URL/Buffer bridges, async→JS bridge, 192+ runtime imports

## v0.2.185
- WASM target: closures, higher-order array methods, classes, JSON/Map/Set/Date/Error/RegExp, 139 bridge imports

## v0.2.184
- Documentation: WebAssembly platform page, perry-styling/theming page, `perry run` docs, `--minify` docs

## v0.2.183
- WebAssembly target (`--target wasm`): `perry-codegen-wasm` crate, WASM bytecode via `wasm-encoder`, self-contained HTML output

## v0.2.182
- Web target minification/obfuscation: Rust-native JS minifier, name mangling, `--minify` CLI flag

## v0.2.181
- iOS keyboard avoidance, `--console` flag for live stdout/stderr streaming
- Fix `RefCell already borrowed` panic in state callbacks (GH-4)
- Fix fetch linker error without stdlib imports (GH-5): `uses_fetch` flag

## v0.2.180
- `perry run` command: compile and launch in one step, platform-aware device detection
- Remote build fallback for iOS: auto-detect missing toolchain, build on Perry Hub

## v0.2.179
- Public beta notice for publish/verify: opt-in error reporting via Chirp telemetry

## v0.2.178
- Fix `--enable-js-runtime` linker error on Linux/WSL: `--allow-multiple-definition` for ELF linker
- Splash screen support for iOS and Android (parse `perry.splash` config, auto-generate LaunchScreen.storyboard / splash drawable)

## v0.2.177
- Project-specific provisioning profiles: save as `{bundle_id}.mobileprovision` instead of generic name

## v0.2.176
- Anonymous telemetry: opt-in usage statistics via Chirp API; opt out via `PERRY_NO_TELEMETRY=1`

## v0.2.175
- Documentation site: mdBook-based docs (`docs/`), 49 pages, GitHub Pages CI, `llms.txt`

## v0.2.174
- `perry/widget` module + `--target ios-widget`: compile TS widget declarations to SwiftUI WidgetKit extensions via `perry-codegen-swiftui` crate

## v0.2.173
- `perry publish` auto-export .p12: auto-detect signing identity from macOS Keychain

## v0.2.172
- Codebase refactor: split `codegen.rs` (40k→1.6k lines) into 12 modules, `lower.rs` (11k→5.4k lines) into 8 modules

## v0.2.171
- Auto-update checker: background version check, `perry update` self-update, `perry doctor` update status

## v0.2.170
- FFI safety: `catch_callback_panic` for all ObjC callbacks
- BigInt bitwise ops, button enhancements (SF Symbols), ScrollView pull-to-refresh, removeChild/reorderChild, openFolderDialog

## v0.2.169
- Type inference: `infer_type_from_expr()` eliminates `Type::Any` for common patterns
- `--type-check` flag: optional tsgo IPC integration

## v0.2.168
- Native application menu bars: 6 FFI functions across all 6 platforms

## v0.2.167
- `perry.compilePackages`: compile pure TS/JS npm packages natively, dedup across nested node_modules

## v0.2.166
- `packages/perry-styling`: design system bridge, token codegen CLI, compile-time `__platform__` constants

## v0.2.165
- Background process management, `fs.readFileBuffer`, `fs.rmRecursive`, `__platform__` compile-time constant

## v0.2.164
- `perry publish` auto-register free license; remove debug logging from runtime

## v0.2.163
- Table widget: NSTableView/DOM `<table>`, column headers/widths, row selection

## v0.2.162
- Web platform full feature parity: 60 new JS functions (100% coverage across all 6 platforms)

## v0.2.161
- Android full feature parity: 62 new JNI functions

## v0.2.160
- Windows full feature parity: 62 new Win32 functions

## v0.2.159
- GTK4 full feature parity: 62 new functions

## v0.2.158
- Cross-platform feature parity test suite: `perry-ui-test` crate, 127-entry feature matrix

## v0.2.157
- 12 new UI/system features: saveFileDialog, Alert, Sheet, Toolbar, LazyVStack, Window, Keychain, notifications

## v0.2.156
- `--target web`: `perry-codegen-js` crate emits JavaScript from HIR, self-contained HTML files

## v0.2.155
- 20+ new UI widgets (SecureField, ProgressView, Image, Picker, Form/Section, NavigationStack, ZStack)
- `perry/system` module: openURL, isDarkMode, preferencesSet/Get

## v0.2.153
- Automatic binary size reduction: link runtime-only when possible (0.3MB vs 48MB)

## v0.2.151
- Plugin system v2: hook priority, 3 modes (filter/action/waterfall), event bus, tool invocation, config system

## v0.2.150
- Native plugin system: `--output-type dylib`, PluginRegistry, dlopen/dlclose

## v0.2.149
- `string.match()` support, regex.test() verification, object destructuring, method chaining

## v0.2.148
- `Array.from()`, singleton pattern type inference, multi-module class ID management
- Array mutation on properties, Map/Set NaN-boxing fixes, native module overridability

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
