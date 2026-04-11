//! LLVM IR function builder.
//!
//! Port of `anvil/src/llvm/function.ts`. A function owns a `RegCounter` shared
//! by all its blocks (see `block.rs`), an ordered list of blocks, and emits
//! itself as an LLVM `define` when serialized.

use std::rc::Rc;

use crate::block::{LlBlock, RegCounter};
use crate::types::LlvmType;

pub struct LlFunction {
    pub name: String,
    pub return_type: LlvmType,
    pub params: Vec<(LlvmType, String)>,
    /// Optional LLVM linkage string, e.g. `"internal"` or `"private"`. Empty
    /// string means external (default) linkage.
    pub linkage: String,
    /// When true, the function body contains a `try` statement (setjmp/longjmp).
    /// We must emit `#1` (noinline optnone) on the definition so LLVM doesn't
    /// promote allocas to SSA registers across the setjmp call — otherwise
    /// mutations performed in the try body are invisible in the catch block
    /// after longjmp returns. `returns_twice` alone on the setjmp call is not
    /// sufficient at -O2 on aarch64.
    pub has_try: bool,
    blocks: Vec<LlBlock>,
    block_counter: u32,
    reg_counter: Rc<RegCounter>,
    /// Allocas hoisted to the function entry block. These are emitted at
    /// the very top of block 0 at IR-serialization time, so they dominate
    /// every use everywhere in the function.
    ///
    /// LLVM convention is that all `alloca` instructions live in the
    /// function entry block — that way the slot pointer is in scope from
    /// every reachable basic block. Putting an alloca inside an `if` arm
    /// works only when its uses are also in that arm; the moment a closure
    /// captures the slot from a sibling branch (or any code reached after
    /// the if-merge), we get "Instruction does not dominate all uses" from
    /// the LLVM verifier.
    ///
    /// Use `LlFunction::alloca_entry(ty)` to allocate; the helper bumps
    /// the shared register counter so the returned `%r<N>` name is unique
    /// function-wide, then appends `"  %r<N> = alloca <ty>"` to this list.
    /// `to_ir()` prepends the list to entry-block instructions in order.
    entry_allocas: Vec<String>,
}

impl LlFunction {
    pub fn new(name: impl Into<String>, return_type: LlvmType, params: Vec<(LlvmType, String)>) -> Self {
        Self {
            name: name.into(),
            return_type,
            params,
            linkage: String::new(),
            has_try: false,
            blocks: Vec::new(),
            block_counter: 0,
            reg_counter: Rc::new(RegCounter::new()),
            entry_allocas: Vec::new(),
        }
    }

    /// Allocate a fresh stack slot in the function entry block. Returns
    /// the SSA pointer name (e.g. `%r42`). The instruction is emitted at
    /// the top of block 0, ahead of any existing entry-block code, so
    /// the slot dominates every reachable use — even from inside nested
    /// if/else branches that would otherwise produce a "does not dominate
    /// all uses" verifier error.
    pub fn alloca_entry(&mut self, ty: LlvmType) -> String {
        let r = format!("%r{}", self.reg_counter.next());
        self.entry_allocas.push(format!("  {} = alloca {}", r, ty));
        r
    }

    /// Emit a one-time function-entry init sequence: allocate a `ptr`
    /// slot, call `func_name()` (no args), store the result in the
    /// slot, return the slot pointer name. Used by the inline bump
    /// allocator to cache the per-thread `InlineArenaState` pointer
    /// once per JS function (instead of paying a TLS access on every
    /// `new ClassName()`).
    ///
    /// Lives in `entry_allocas` so the call + store run before any
    /// user code in the entry block, dominating every reachable use.
    /// The slot pointer is returned for the caller to load from at
    /// each subsequent allocation site.
    pub fn entry_init_call_ptr(&mut self, func_name: &str) -> String {
        let slot = self.alloca_entry(crate::types::PTR);
        let result_reg = format!("%r{}", self.reg_counter.next());
        self.entry_allocas
            .push(format!("  {} = call ptr @{}()", result_reg, func_name));
        self.entry_allocas
            .push(format!("  store ptr {}, ptr {}", result_reg, slot));
        slot
    }

    /// Emit a one-time function-entry load of a module global into a
    /// stack slot, returning the slot pointer. Used by the inline
    /// bump allocator to cache class-static values like the per-class
    /// `keys_array` global once per function instead of reloading it
    /// inside the hot allocation loop.
    ///
    /// LLVM's LICM should hoist a loop-invariant global load on its
    /// own, but doesn't when the loop body contains a call to an
    /// external function (like `js_inline_arena_slow_alloc`) that
    /// LLVM can't prove won't modify the global. Hoisting manually
    /// at the codegen layer sidesteps the alias-analysis question.
    pub fn entry_init_load_global(&mut self, global_name: &str, ty: crate::types::LlvmType) -> String {
        let slot = self.alloca_entry(ty);
        let result_reg = format!("%r{}", self.reg_counter.next());
        self.entry_allocas
            .push(format!("  {} = load {}, ptr @{}", result_reg, ty, global_name));
        self.entry_allocas
            .push(format!("  store {} {}, ptr {}", ty, result_reg, slot));
        slot
    }

    /// Create a new basic block with the given semantic name (e.g. "entry",
    /// "if.then"). A numeric suffix is appended to make the label unique
    /// across the function.
    pub fn create_block(&mut self, name: &str) -> &mut LlBlock {
        let label = format!("{}.{}", name, self.block_counter);
        self.block_counter += 1;
        let block = LlBlock::new(label, self.reg_counter.clone());
        self.blocks.push(block);
        // Safe unwrap: we just pushed.
        self.blocks.last_mut().unwrap()
    }

    /// Accessor for an earlier block by index — needed when codegen has to
    /// come back and append to a predecessor (e.g. patching an unreachable
    /// fallthrough).
    pub fn block_mut(&mut self, idx: usize) -> Option<&mut LlBlock> {
        self.blocks.get_mut(idx)
    }

    pub fn blocks(&self) -> &[LlBlock] {
        &self.blocks
    }

    pub fn num_blocks(&self) -> usize {
        self.blocks.len()
    }

    /// Label of the last-created block — convenience for expression codegen
    /// that needs to feed a phi node the predecessor label after compiling a
    /// sub-expression whose control flow may have split.
    pub fn last_block_label(&self) -> Option<&str> {
        self.blocks.last().map(|b| b.label.as_str())
    }

    pub fn to_ir(&self) -> String {
        let param_str = self
            .params
            .iter()
            .map(|(t, n)| format!("{} {}", t, n))
            .collect::<Vec<_>>()
            .join(", ");

        let linkage = if self.linkage.is_empty() {
            String::new()
        } else {
            format!("{} ", self.linkage)
        };

        let attrs = if self.has_try { " #1" } else { "" };
        let mut ir = format!(
            "define {}{} @{}({}){} {{\n",
            linkage, self.return_type, self.name, param_str, attrs
        );

        for (i, blk) in self.blocks.iter().enumerate() {
            if i > 0 {
                ir.push('\n');
            }
            // Hoisted allocas live at the very top of the entry block so
            // they dominate every reachable use in the function. We splice
            // them in by emitting the block's label line first, then the
            // alloca instructions, then the block's regular body.
            if i == 0 && !self.entry_allocas.is_empty() {
                let body = blk.to_ir();
                // body looks like "label.0:\n  inst1\n  inst2\n"
                if let Some(nl) = body.find('\n') {
                    let (label_line, rest) = body.split_at(nl + 1);
                    ir.push_str(label_line);
                    for alloca in &self.entry_allocas {
                        ir.push_str(alloca);
                        ir.push('\n');
                    }
                    ir.push_str(rest);
                } else {
                    ir.push_str(&body);
                }
            } else {
                ir.push_str(&blk.to_ir());
            }
            ir.push('\n');
        }

        ir.push_str("}\n");
        ir
    }
}
