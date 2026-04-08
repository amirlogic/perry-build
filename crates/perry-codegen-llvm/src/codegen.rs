//! HIR → LLVM IR compilation entry point.
//!
//! Public contract:
//!
//! ```ignore
//! let opts = CompileOptions { target: None, is_entry_module: true };
//! let object_bytes: Vec<u8> = perry_codegen_llvm::compile_module(&hir, opts)?;
//! ```
//!
//! The returned bytes are a regular object file produced by `clang -c`.
//! Perry's existing linking stage in `crates/perry/src/commands/compile.rs`
//! picks them up identically to the Cranelift output.
//!
//! ## Phase A scope (in progress — primary-backend migration)
//!
//! Building toward feature parity with the Cranelift backend so LLVM can
//! become Perry's primary build platform. See
//! `/Users/amlug/.claude/plans/sorted-noodling-quilt.md` for the full
//! migration plan.
//!
//! Currently supported (Phases 1, 2, 2.1, A-strings):
//!
//! - User functions with typed `double` ABI
//! - Recursive and forward calls via `FuncRef`
//! - If/else, for loops, let, return
//! - Binary arithmetic (add/sub/mul/div/mod) and compare
//! - Update (++/--) and LocalSet
//! - `Date.now()` via `js_date_now`
//! - **String literals** via the hoisted `StringPool` (one allocation per
//!   literal at module init time, registered as a permanent GC root via
//!   `js_gc_register_global_root`; use sites are a single `load`)
//! - `console.log(<expr>)` — uses `js_console_log_number` for static number
//!   literals (optimized path) and `js_console_log_dynamic` for everything
//!   else (NaN-tag dispatch at runtime)
//!
//! Anything else (objects, arrays, classes, closures, async, imports, …)
//! errors with an actionable "Phase X not yet supported" message.

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use perry_hir::{Function, Module as HirModule};

use crate::expr::FnCtx;
use crate::module::LlModule;
use crate::runtime_decls;
use crate::stmt;
use crate::strings::StringPool;
use crate::types::{DOUBLE, I32, I64, LlvmType, PTR, VOID};

/// Options mirrored from the Cranelift backend's setter API.
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// Target triple override. `None` uses the host default.
    pub target: Option<String>,
    /// Whether this module is the program entry point. When true, codegen
    /// emits a `main` function that calls `js_gc_init` and then the module's
    /// top-level statements.
    pub is_entry_module: bool,
}

/// Compile a Perry HIR module to an object file via LLVM IR.
pub fn compile_module(hir: &HirModule, opts: CompileOptions) -> Result<Vec<u8>> {
    let triple = opts.target.clone().unwrap_or_else(default_target_triple);

    let mut llmod = LlModule::new(&triple);
    runtime_decls::declare_phase1(&mut llmod);

    // Phase A still only supports single-file entry modules — multi-module
    // imports land in Phase F.
    if !opts.is_entry_module {
        return Err(anyhow!(
            "perry-codegen-llvm Phase A only supports the entry module; \
             non-entry module '{}' is not yet supported",
            hir.name
        ));
    }
    if !hir.imports.is_empty() {
        return Err(anyhow!(
            "perry-codegen-llvm Phase A does not support imports; module '{}' has {} imports",
            hir.name,
            hir.imports.len()
        ));
    }
    // Phase C.1: classes are supported (data classes + simple
    // constructors). Inheritance lands in Phase C.3. Methods are
    // ALLOWED to exist on classes — Perry's HIR lowering inlines
    // many simple methods at use sites, so the codegen may never
    // see a method *call*. If a real method dispatch shows up, the
    // expression-level codegen errors at that specific call site.
    for c in &hir.classes {
        if c.extends.is_some() || c.extends_name.is_some() {
            return Err(anyhow!(
                "perry-codegen-llvm Phase C.1: class '{}' uses inheritance (Phase C.3)",
                c.name
            ));
        }
    }

    // Module-wide string literal pool. Owned by the codegen so that
    // `compile_function` and `compile_main` can take split borrows of
    // (&mut LlFunction, &mut StringPool) without confusing the borrow
    // checker — the pool lives outside LlModule.
    let mut strings = StringPool::new();

    // Class lookup table for `Expr::New`. Indexed by class name —
    // the HIR has unique names per module.
    let class_table: HashMap<String, &perry_hir::Class> = hir
        .classes
        .iter()
        .map(|c| (c.name.clone(), c))
        .collect();

    // Resolve user function names up-front so body lowering can emit
    // forward/recursive calls without worrying about emission order.
    let mut func_names: HashMap<u32, String> = HashMap::new();
    for f in &hir.functions {
        func_names.insert(f.id, llvm_fn_name(&f.name));
    }

    // Lower each user function into the module.
    for f in &hir.functions {
        compile_function(&mut llmod, f, &func_names, &mut strings, &class_table)
            .with_context(|| format!("lowering function '{}'", f.name))?;
    }

    // Emit `int main()` that bootstraps GC, runs the string-pool init,
    // then runs init statements.
    compile_main(&mut llmod, hir, &func_names, &mut strings, &class_table)
        .with_context(|| format!("lowering main of module '{}'", hir.name))?;

    // After all user code is lowered, the string pool's contents are final.
    // Emit the bytes globals, handle globals, and the `__perry_init_strings`
    // function that runs once at startup. We do this AFTER `compile_main`
    // so the string pool sees every literal — including those in init
    // statements and inside the main function body.
    emit_string_pool(&mut llmod, &strings);

    let ll_text = llmod.to_ir();
    log::debug!(
        "perry-codegen-llvm: emitted {} bytes of LLVM IR for '{}' ({} interned strings)",
        ll_text.len(),
        hir.name,
        strings.len()
    );
    crate::linker::compile_ll_to_object(&ll_text, opts.target.as_deref())
}

/// Compile a single user function into the module.
fn compile_function(
    llmod: &mut LlModule,
    f: &Function,
    func_names: &HashMap<u32, String>,
    strings: &mut StringPool,
    classes: &HashMap<String, &perry_hir::Class>,
) -> Result<()> {
    let llvm_name = func_names
        .get(&f.id)
        .cloned()
        .ok_or_else(|| anyhow!("function name not resolved for {}", f.name))?;

    // Phase A assumes all user-function params are `double`. Parameter
    // registers are named `%arg{LocalId}` so the body can store them into
    // alloca slots keyed by the same HIR LocalId.
    let params: Vec<(LlvmType, String)> = f
        .params
        .iter()
        .map(|p| (DOUBLE, format!("%arg{}", p.id)))
        .collect();

    let lf = llmod.define_function(&llvm_name, DOUBLE, params);
    let _ = lf.create_block("entry");

    // Store each param into an alloca slot, collecting LocalId → slot
    // mappings. We release the &mut LlBlock at scope end before handing
    // the function over to the FnCtx lowering pass.
    let locals: HashMap<u32, String> = {
        let blk = lf.block_mut(0).unwrap();
        let mut map = HashMap::new();
        for p in &f.params {
            let slot = blk.alloca(DOUBLE);
            blk.store(DOUBLE, &format!("%arg{}", p.id), &slot);
            map.insert(p.id, slot);
        }
        map
    };

    // Param types feed local_types so type-aware dispatch (e.g. string
    // concat detection on a `: string` parameter) works inside the body.
    let local_types: HashMap<u32, perry_types::Type> = f
        .params
        .iter()
        .map(|p| (p.id, p.ty.clone()))
        .collect();

    let mut ctx = FnCtx {
        func: lf,
        locals,
        local_types,
        current_block: 0,
        func_names,
        strings,
        loop_targets: Vec::new(),
        classes,
        this_stack: Vec::new(),
    };
    stmt::lower_stmts(&mut ctx, &f.body)
        .with_context(|| format!("lowering body of '{}'", f.name))?;

    // Defensive: a well-typed numeric function always returns via an
    // explicit `return`, but we emit `ret double 0.0` as a fallback so
    // the LLVM verifier doesn't reject a missing terminator.
    if !ctx.block().is_terminated() {
        ctx.block().ret(DOUBLE, "0.0");
    }
    Ok(())
}

/// Emit `int main() { js_gc_init(); __perry_init_strings(); <init stmts>; return 0; }`.
///
/// The `__perry_init_strings()` call is added unconditionally — if there
/// are no string literals in the program, `emit_string_pool` will skip
/// emitting the init function entirely and the call here would dangle.
/// To handle that, we defer the call insertion until AFTER the string pool
/// is finalized: see `emit_string_pool` for the patch logic.
fn compile_main(
    llmod: &mut LlModule,
    hir: &HirModule,
    func_names: &HashMap<u32, String>,
    strings: &mut StringPool,
    classes: &HashMap<String, &perry_hir::Class>,
) -> Result<()> {
    let main = llmod.define_function("main", I32, vec![]);
    let _ = main.create_block("entry");
    {
        let blk = main.block_mut(0).unwrap();
        blk.call_void("js_gc_init", &[]);
        // String-pool init call — see `emit_string_pool`. We always emit
        // the call; if the pool turns out to be empty, `emit_string_pool`
        // emits an empty `__perry_init_strings()` body which clang -O2
        // collapses to nothing.
        blk.call_void("__perry_init_strings", &[]);
    }

    let mut ctx = FnCtx {
        func: main,
        locals: HashMap::new(),
        local_types: HashMap::new(),
        current_block: 0,
        func_names,
        strings,
        loop_targets: Vec::new(),
        classes,
        this_stack: Vec::new(),
    };
    stmt::lower_stmts(&mut ctx, &hir.init)
        .with_context(|| format!("lowering init statements of module '{}'", hir.name))?;

    // `main` returns i32, but stmt lowering emits `ret double` for explicit
    // returns. Phase A doesn't allow explicit returns at top level, so we
    // just append `ret i32 0` if the block didn't terminate.
    if !ctx.block().is_terminated() {
        ctx.block().ret(I32, "0");
    }
    Ok(())
}

/// Emit the string pool into the module: byte-array constants, handle
/// globals, and the `__perry_init_strings()` function that allocates +
/// NaN-boxes + GC-roots each handle exactly once at startup.
///
/// Always emits `__perry_init_strings`, even when the pool is empty —
/// `compile_main` already injected the unconditional call, and removing
/// the call after the fact is awkward. An empty body (`ret void`) costs
/// nothing at runtime and clang -O2 inlines/dead-strips the empty call.
fn emit_string_pool(llmod: &mut LlModule, strings: &StringPool) {
    // Emit per-literal globals.
    for entry in strings.iter() {
        // .rodata bytes — `[N+1 x i8]` because we include the null terminator.
        llmod.add_named_string_constant(
            &entry.bytes_global,
            entry.byte_len + 1,
            &entry.escaped_ir,
        );
        // Mutable handle global initialized to 0.0; populated by
        // __perry_init_strings.
        llmod.add_internal_global(&entry.handle_global, DOUBLE, "0.0");
    }

    // Build __perry_init_strings function. One block, straight-line code:
    // for each entry, allocate via js_string_from_bytes, NaN-box, store
    // into the handle global, register as GC root.
    let init_fn = llmod.define_function("__perry_init_strings", VOID, vec![]);
    let _ = init_fn.create_block("entry");
    let blk = init_fn.block_mut(0).unwrap();

    for entry in strings.iter() {
        let bytes_ref = format!("@{}", entry.bytes_global);
        let handle_ref = format!("@{}", entry.handle_global);
        let len_str = entry.byte_len.to_string();

        // %h = call i64 @js_string_from_bytes(ptr @.str.N.bytes, i32 N)
        let handle = blk.call(
            I64,
            "js_string_from_bytes",
            &[(PTR, &bytes_ref), (I32, &len_str)],
        );
        // %b = call double @js_nanbox_string(i64 %h)
        let nanboxed = blk.call(DOUBLE, "js_nanbox_string", &[(I64, &handle)]);
        // store double %b, ptr @.str.N.handle
        blk.store(DOUBLE, &nanboxed, &handle_ref);
        // %addr = ptrtoint ptr @.str.N.handle to i64
        let addr_i64 = blk.ptrtoint(&handle_ref, I64);
        // call void @js_gc_register_global_root(i64 %addr)
        blk.call_void("js_gc_register_global_root", &[(I64, &addr_i64)]);
    }

    blk.ret_void();
}

/// Mangle a HIR function name into an LLVM symbol.
///
/// We prefix with `perry_fn_` to avoid colliding with runtime symbols like
/// `main`, `js_console_log_*`, or the C stdlib. Non-alphanumeric characters
/// are replaced with underscores because LLVM symbol names are restrictive.
fn llvm_fn_name(hir_name: &str) -> String {
    let sanitized: String = hir_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    format!("perry_fn_{}", sanitized)
}

/// Host default triple.
fn default_target_triple() -> String {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "arm64-apple-macosx15.0.0".to_string()
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-macosx15.0.0".to_string()
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu".to_string()
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu".to_string()
    } else {
        "arm64-apple-macosx15.0.0".to_string()
    }
}
