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

/// Set the i18n string table and locale codes for the current compilation thread.
/// Must be called before compiling any module that contains I18nString expressions.
pub fn set_i18n_table(translations: Vec<String>, key_count: usize, locale_count: usize, locale_codes: Vec<String>) {
    util::I18N_TABLE.with(|t| {
        *t.borrow_mut() = util::I18nCodegenTable {
            locale_count,
            key_count,
            translations,
        };
    });
    util::I18N_LOCALE_CODES.with(|c| {
        *c.borrow_mut() = locale_codes;
    });
}
