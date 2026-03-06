//! Cranelift Code Generation for Perry
//!
//! Translates HIR to Cranelift IR and generates native machine code.

pub(crate) mod types;
pub(crate) mod util;
pub mod stubs;
mod runtime_decls;
mod classes;
mod functions;
mod closures;
mod module_init;
mod stmt;
mod expr;
pub mod codegen;

pub use codegen::Compiler;
pub use stubs::generate_stub_object;
