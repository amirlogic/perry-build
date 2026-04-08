//! Expression codegen — Phase 2.
//!
//! Scope: numeric expressions (literals, LocalGet, Binary add/sub/mul/div,
//! Compare, direct FuncRef calls) plus the `console.log(<expr>)` sink. All
//! values are raw LLVM `double` — no NaN-boxing, no strings, no objects.
//!
//! Anything outside the supported shape returns an explicit "unsupported"
//! error so a user running `--backend llvm` on richer TypeScript gets a
//! one-line explanation instead of a silent broken binary.

use anyhow::{anyhow, bail, Result};
use perry_hir::{BinaryOp, CompareOp, Expr, UpdateOp};
use perry_types::Type as HirType;

use crate::block::LlBlock;
use crate::function::LlFunction;
use crate::nanbox::{double_literal, POINTER_MASK_I64};
use crate::strings::StringPool;
use crate::types::{DOUBLE, I32, I64};

/// Per-function codegen context. Held briefly during lowering, never stored.
pub(crate) struct FnCtx<'a> {
    /// Function being built (blocks, params, registers).
    pub func: &'a mut LlFunction,
    /// Map from HIR LocalId → LLVM alloca pointer (e.g. `%r3`).
    pub locals: std::collections::HashMap<u32, String>,
    /// Map from HIR LocalId → static HIR Type. Used by `is_string_expr` and
    /// future type-aware dispatch sites (Phase B's "native instance flag
    /// tracking" extension). Populated from function params and `Stmt::Let`
    /// declarations as they're lowered.
    pub local_types: std::collections::HashMap<u32, HirType>,
    /// Index into `func.blocks()` pointing at the block currently receiving
    /// instructions. Lowering fns update this when control flow splits.
    pub current_block: usize,
    /// HIR FuncId → LLVM function name. Resolved at the top of
    /// `compile_module` so `FuncRef(id)` calls know what to emit.
    pub func_names: &'a std::collections::HashMap<u32, String>,
    /// Module-wide string literal pool. Disjoint borrow from `func` because
    /// it lives in `codegen.rs` as a separate variable, not inside the
    /// LlModule that `func` was derived from. See `crate::strings` for the
    /// design rationale.
    pub strings: &'a mut StringPool,
}

impl<'a> FnCtx<'a> {
    pub fn block(&mut self) -> &mut LlBlock {
        self.func
            .block_mut(self.current_block)
            .expect("current_block index points at a valid block")
    }

    /// Create a new block and return its index, **without** switching the
    /// current_block pointer. The caller is responsible for deciding when
    /// to flip.
    pub fn new_block(&mut self, name: &str) -> usize {
        let _ = self.func.create_block(name);
        self.func.num_blocks() - 1
    }

    /// Label of a block by index — needed when emitting a branch.
    pub fn block_label(&self, idx: usize) -> String {
        self.func
            .blocks()
            .get(idx)
            .map(|b| b.label.clone())
            .expect("valid block index")
    }

}

/// Lower an expression to a raw LLVM `double` value. Returns the string form
/// of the value (either a `%rN` register or a literal like `42.0`).
pub(crate) fn lower_expr(ctx: &mut FnCtx<'_>, expr: &Expr) -> Result<String> {
    match expr {
        // -------- Literals --------
        Expr::Integer(i) => Ok(double_literal(*i as f64)),
        Expr::Number(f) => Ok(double_literal(*f)),

        // String literals are pre-allocated at module init via the
        // StringPool's hoisting strategy (see `crate::strings`). At the use
        // site we just load the cached NaN-boxed handle from the pool's
        // `.handle` global. ONE instruction, no per-use allocation.
        Expr::String(s) => {
            let idx = ctx.strings.intern(s);
            let entry = ctx.strings.entry(idx);
            // Clone the global name out so we don't keep `entry` borrowed
            // across the call to `ctx.block()` (which mutably borrows
            // `ctx.func`, distinct from `ctx.strings` but the borrow checker
            // sees `entry` as borrowing `ctx`).
            let handle_global = format!("@{}", entry.handle_global);
            Ok(ctx.block().load(DOUBLE, &handle_global))
        }

        // -------- Variables --------
        Expr::LocalGet(id) => {
            let slot = ctx
                .locals
                .get(id)
                .ok_or_else(|| anyhow!("LocalGet({}): local not in scope", id))?
                .clone();
            Ok(ctx.block().load(DOUBLE, &slot))
        }

        // `total = expr` — store the new value into the local's alloca slot
        // and return it (matches JS semantics: assignment is an expression
        // whose value is the assigned value).
        Expr::LocalSet(id, value) => {
            let v = lower_expr(ctx, value)?;
            let slot = ctx
                .locals
                .get(id)
                .ok_or_else(|| anyhow!("LocalSet({}): local not in scope", id))?
                .clone();
            ctx.block().store(DOUBLE, &v, &slot);
            Ok(v)
        }

        // `i++` / `++i` / `i--` / `--i`. Postfix returns the OLD value,
        // prefix returns the NEW value. Inside a for-loop update slot the
        // result is discarded, but we honor JS semantics in case it's used
        // somewhere like `let x = i++`.
        Expr::Update { id, op, prefix } => {
            let slot = ctx
                .locals
                .get(id)
                .ok_or_else(|| anyhow!("Update({}): local not in scope", id))?
                .clone();
            let blk = ctx.block();
            let old = blk.load(DOUBLE, &slot);
            let new = match op {
                UpdateOp::Increment => blk.fadd(&old, "1.0"),
                UpdateOp::Decrement => blk.fsub(&old, "1.0"),
            };
            blk.store(DOUBLE, &new, &slot);
            Ok(if *prefix { new } else { old })
        }

        // `Date.now()` — special HIR variant that lowers to a single FFI
        // call returning a `double` (milliseconds since UNIX epoch as
        // produced by `js_date_now` in `perry-runtime/src/date.rs`).
        Expr::DateNow => Ok(ctx.block().call(DOUBLE, "js_date_now", &[])),

        // -------- Arithmetic --------
        // String concatenation (Phase B): if Add receives two operands that
        // are statically known to be strings, route through `js_string_concat`
        // instead of fadd. Type detection consults `ctx.local_types` for
        // LocalGet operands and recurses through nested Adds.
        Expr::Binary { op, left, right } => {
            if matches!(op, BinaryOp::Add)
                && is_string_expr(ctx, left)
                && is_string_expr(ctx, right)
            {
                return lower_string_concat(ctx, left, right);
            }
            let l = lower_expr(ctx, left)?;
            let r = lower_expr(ctx, right)?;
            let blk = ctx.block();
            let v = match op {
                BinaryOp::Add => blk.fadd(&l, &r),
                BinaryOp::Sub => blk.fsub(&l, &r),
                BinaryOp::Mul => blk.fmul(&l, &r),
                BinaryOp::Div => blk.fdiv(&l, &r),
                BinaryOp::Mod => blk.frem(&l, &r),
                other => bail!(
                    "perry-codegen-llvm Phase A: BinaryOp::{:?} not yet supported",
                    other
                ),
            };
            Ok(v)
        }

        // -------- Comparison --------
        // LLVM `fcmp` returns `i1`. We zext to double so the value fits the
        // standard number ABI used by the rest of the codegen — JS "true"
        // round-trips through numeric contexts as 1.0 and "false" as 0.0,
        // which is what Perry's runtime expects from typed boolean returns.
        Expr::Compare { op, left, right } => {
            let l = lower_expr(ctx, left)?;
            let r = lower_expr(ctx, right)?;
            let pred = match op {
                CompareOp::Eq | CompareOp::LooseEq => "oeq",
                CompareOp::Ne | CompareOp::LooseNe => "one",
                CompareOp::Lt => "olt",
                CompareOp::Le => "ole",
                CompareOp::Gt => "ogt",
                CompareOp::Ge => "oge",
            };
            let blk = ctx.block();
            let bit = blk.fcmp(pred, &l, &r);
            // `bit` is `i1`; zext to `i64` then sitofp to `double` so that
            // downstream consumers see a canonical 0.0/1.0 double.
            let as_i64 = blk.zext(crate::types::I1, &bit, crate::types::I64);
            Ok(blk.sitofp(crate::types::I64, &as_i64, DOUBLE))
        }

        // -------- Arrays (Phase B.3) --------
        // `[a, b, c]` literal: allocate via js_array_alloc(N), then
        // sequentially push each element. js_array_push_f64 may return a
        // new pointer if it had to realloc, so we thread the pointer
        // through each push. Final pointer is NaN-boxed via js_nanbox_pointer
        // (POINTER_TAG, not STRING_TAG).
        Expr::Array(elements) => lower_array_literal(ctx, elements),

        // `arr[i]` index access. Currently number-typed only — assumes the
        // receiver is an array of numbers and the result is a raw f64.
        Expr::IndexGet { object, index } => {
            if !is_array_expr(ctx, object) {
                bail!(
                    "perry-codegen-llvm Phase B.3: IndexGet receiver must be a known array (got {})",
                    variant_name(object)
                );
            }
            let arr_box = lower_expr(ctx, object)?;
            let idx_double = lower_expr(ctx, index)?;
            let blk = ctx.block();
            // Unbox the array pointer: bitcast double → i64, mask off the
            // POINTER_TAG bits with POINTER_MASK_I64.
            let arr_bits = blk.bitcast_double_to_i64(&arr_box);
            let arr_handle = blk.and(I64, &arr_bits, POINTER_MASK_I64);
            // Index is a double in our value model; convert to i32 for the
            // runtime ABI.
            let idx_i32 = blk.fptosi(DOUBLE, &idx_double, I32);
            Ok(blk.call(
                DOUBLE,
                "js_array_get_f64",
                &[(I64, &arr_handle), (I32, &idx_i32)],
            ))
        }

        // `arr.length` — for arrays specifically. Strings, objects, and
        // other receivers fall through to the unsupported error.
        Expr::PropertyGet { object, property } if property == "length" && is_array_expr(ctx, object) => {
            let arr_box = lower_expr(ctx, object)?;
            let blk = ctx.block();
            let arr_bits = blk.bitcast_double_to_i64(&arr_box);
            let arr_handle = blk.and(I64, &arr_bits, POINTER_MASK_I64);
            let len_i32 = blk.call(I32, "js_array_length", &[(I64, &arr_handle)]);
            // Convert i32 → double for our number ABI.
            Ok(blk.sitofp(I32, &len_i32, DOUBLE))
        }

        // -------- Calls --------
        Expr::Call { callee, args, .. } => lower_call(ctx, callee, args),

        // -------- Unsupported (clear error) --------
        other => bail!(
            "perry-codegen-llvm Phase 2: expression {} not yet supported",
            variant_name(other)
        ),
    }
}

/// Lower a `Call` expression. Two shapes are supported:
/// 1. `FuncRef(id)(args...)` — direct call to a user function by HIR id.
/// 2. `console.log(expr)` where `expr` lowers to a double — emits a
///    `js_console_log_number` call and returns `0.0` as the statement value.
fn lower_call(ctx: &mut FnCtx<'_>, callee: &Expr, args: &[Expr]) -> Result<String> {
    // User function call via FuncRef.
    if let Expr::FuncRef(fid) = callee {
        let fname = ctx
            .func_names
            .get(fid)
            .ok_or_else(|| anyhow!("FuncRef({}): function name not resolved", fid))?
            .clone();

        // Lower all arguments first.
        let mut lowered: Vec<String> = Vec::with_capacity(args.len());
        for a in args {
            lowered.push(lower_expr(ctx, a)?);
        }
        let arg_slices: Vec<(crate::types::LlvmType, &str)> =
            lowered.iter().map(|s| (DOUBLE, s.as_str())).collect();

        return Ok(ctx.block().call(DOUBLE, &fname, &arg_slices));
    }

    // console.log(<numeric expr>) sink.
    if let Expr::PropertyGet { object, property } = callee {
        if matches!(object.as_ref(), Expr::GlobalGet(_)) && property == "log" {
            if args.len() != 1 {
                bail!(
                    "perry-codegen-llvm Phase A: console.log expects 1 arg, got {}",
                    args.len()
                );
            }
            // For statically-known number literals, take the optimized
            // `js_console_log_number` path which prints the f64 directly
            // without going through the NaN-tag dispatch. For everything
            // else (string literals, computed values whose runtime type
            // we don't track at codegen time, locals from union types),
            // route through `js_console_log_dynamic` which inspects the
            // NaN tag at runtime and dispatches to the right printer.
            //
            // js_console_log_dynamic falls through to the regular-number
            // printer when the value isn't NaN-tagged, so passing a raw
            // f64 (e.g. fibonacci(40)'s 102334155.0) still prints
            // correctly — verified in `crates/perry-runtime/src/builtins.rs:81`.
            let arg = &args[0];
            let is_number_literal = matches!(arg, Expr::Integer(_) | Expr::Number(_));
            let v = lower_expr(ctx, arg)?;
            let runtime_fn = if is_number_literal {
                "js_console_log_number"
            } else {
                "js_console_log_dynamic"
            };
            ctx.block().call_void(runtime_fn, &[(DOUBLE, &v)]);
            // console.log returns undefined. Phase A has no notion of
            // undefined; we return 0.0 as a sentinel — it's only valid
            // inside an Expr statement and the caller discards it.
            return Ok("0.0".to_string());
        }
    }

    bail!(
        "perry-codegen-llvm Phase 2: Call callee shape not supported ({})",
        variant_name(callee)
    )
}

/// Statically determine whether an expression is a string. Conservative —
/// returns `false` for anything that requires type information we don't
/// track (function-call returns, dynamic property access).
///
/// Recognizes:
/// - literal strings (`"foo"`)
/// - LocalGet of string-typed locals (params with `: string`, `let x = "a"`)
/// - recursive Add of strings (`"a" + "b" + s`)
fn is_string_expr(ctx: &FnCtx<'_>, e: &Expr) -> bool {
    match e {
        Expr::String(_) => true,
        Expr::LocalGet(id) => matches!(ctx.local_types.get(id), Some(HirType::String)),
        Expr::Binary { op: BinaryOp::Add, left, right } => {
            is_string_expr(ctx, left) && is_string_expr(ctx, right)
        }
        _ => false,
    }
}

/// Statically determine whether an expression is an array. Used for
/// dispatch on `arr.length` and `arr[i]`.
fn is_array_expr(ctx: &FnCtx<'_>, e: &Expr) -> bool {
    match e {
        Expr::Array(_) => true,
        Expr::LocalGet(id) => matches!(ctx.local_types.get(id), Some(HirType::Array(_))),
        _ => false,
    }
}

/// Lower an array literal `[a, b, c, …]`.
///
/// Pattern:
/// ```llvm
/// %arr0 = call i64 @js_array_alloc(i32 N)        ; pre-sized
/// %arr1 = call i64 @js_array_push_f64(i64 %arr0, double <a>)
/// %arr2 = call i64 @js_array_push_f64(i64 %arr1, double <b>)
/// %arr3 = call i64 @js_array_push_f64(i64 %arr2, double <c>)
/// %boxed = call double @js_nanbox_pointer(i64 %arr3)
/// ```
///
/// Each `push_f64` returns a possibly-realloc'd pointer that the next push
/// must use. Since we pre-size with `js_array_alloc(N)`, the pushes
/// shouldn't actually realloc, but we honor the ABI to stay correct if the
/// runtime grows the array for any reason.
///
/// All elements are lowered to raw `double` first; the array stores them
/// as f64 (the typed-Number array layout). Mixed-type arrays come in a
/// later Phase B slice.
fn lower_array_literal(ctx: &mut FnCtx<'_>, elements: &[Expr]) -> Result<String> {
    let n = elements.len() as u32;
    let cap_str = n.to_string();

    // Allocate. The result is a raw i64 array pointer (NOT NaN-boxed).
    let mut current_arr = ctx
        .block()
        .call(I64, "js_array_alloc", &[(I32, &cap_str)]);

    for value_expr in elements {
        let v = lower_expr(ctx, value_expr)?;
        current_arr = ctx.block().call(
            I64,
            "js_array_push_f64",
            &[(I64, &current_arr), (DOUBLE, &v)],
        );
    }

    // NaN-box the final pointer with POINTER_TAG via js_nanbox_pointer.
    Ok(ctx
        .block()
        .call(DOUBLE, "js_nanbox_pointer", &[(I64, &current_arr)]))
}

/// Lower a static `s1 + s2` string concatenation. Both operands must
/// already be statically string-typed (caller's responsibility — see
/// `is_string_expr`).
///
/// Pattern:
/// ```llvm
/// ; %l_box and %r_box are NaN-boxed strings (double values with STRING_TAG)
/// %l_bits = bitcast double %l_box to i64
/// %l_handle = and i64 %l_bits, 281474976710655   ; POINTER_MASK_I64
/// %r_bits = bitcast double %r_box to i64
/// %r_handle = and i64 %r_bits, 281474976710655
/// %result_handle = call i64 @js_string_concat(i64 %l_handle, i64 %r_handle)
/// %result_box = call double @js_nanbox_string(i64 %result_handle)
/// ```
///
/// The bitcast+and is the inline-fast unboxing pattern. We avoid calling
/// the slower `js_nanbox_get_pointer` (which does the same thing in Rust)
/// to keep concat hot-path overhead minimal.
fn lower_string_concat(ctx: &mut FnCtx<'_>, left: &Expr, right: &Expr) -> Result<String> {
    let l_box = lower_expr(ctx, left)?;
    let r_box = lower_expr(ctx, right)?;
    let blk = ctx.block();
    let l_bits = blk.bitcast_double_to_i64(&l_box);
    let l_handle = blk.and(I64, &l_bits, POINTER_MASK_I64);
    let r_bits = blk.bitcast_double_to_i64(&r_box);
    let r_handle = blk.and(I64, &r_bits, POINTER_MASK_I64);
    let result_handle = blk.call(
        I64,
        "js_string_concat",
        &[(I64, &l_handle), (I64, &r_handle)],
    );
    let result_box = blk.call(DOUBLE, "js_nanbox_string", &[(I64, &result_handle)]);
    Ok(result_box)
}

pub(crate) fn variant_name(e: &Expr) -> &'static str {
    match e {
        Expr::Undefined => "Undefined",
        Expr::Null => "Null",
        Expr::Bool(_) => "Bool",
        Expr::Number(_) => "Number",
        Expr::Integer(_) => "Integer",
        Expr::BigInt(_) => "BigInt",
        Expr::String(_) => "String",
        Expr::I18nString { .. } => "I18nString",
        Expr::LocalGet(_) => "LocalGet",
        Expr::LocalSet(_, _) => "LocalSet",
        Expr::GlobalGet(_) => "GlobalGet",
        Expr::GlobalSet(_, _) => "GlobalSet",
        Expr::Update { .. } => "Update",
        Expr::Binary { .. } => "Binary",
        Expr::Unary { .. } => "Unary",
        Expr::Compare { .. } => "Compare",
        Expr::Logical { .. } => "Logical",
        Expr::Call { .. } => "Call",
        Expr::CallSpread { .. } => "CallSpread",
        Expr::FuncRef(_) => "FuncRef",
        Expr::ExternFuncRef { .. } => "ExternFuncRef",
        Expr::NativeModuleRef(_) => "NativeModuleRef",
        Expr::NativeMethodCall { .. } => "NativeMethodCall",
        Expr::PropertyGet { .. } => "PropertyGet",
        Expr::PropertySet { .. } => "PropertySet",
        Expr::PropertyUpdate { .. } => "PropertyUpdate",
        _ => "<other>",
    }
}
