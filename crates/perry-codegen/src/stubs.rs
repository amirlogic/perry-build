//! Stub object generation for unresolved imports.

use anyhow::{anyhow, Result};
use cranelift::prelude::*;
use cranelift_codegen::ir::AbiParam;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::{DataDescription, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::str::FromStr;

/// Generate a stub object file for missing symbols from unresolved imports.
/// `identity_func_symbols` are stubs that take an f64 arg and return it as-is (pass-through).
pub fn generate_stub_object(missing_data_symbols: &[String], missing_func_symbols: &[String], identity_func_symbols: &[String], target: Option<&str>) -> Result<Vec<u8>> {
    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    flag_builder.set("is_pic", "true").unwrap();
    let isa = match target {
        Some("ios-simulator") | Some("ios") => {
            let triple = target_lexicon::Triple::from_str("aarch64-apple-ios")
                .map_err(|e| anyhow!("Bad triple: {}", e))?;
            let isa_builder = cranelift::codegen::isa::lookup(triple)
                .map_err(|e| anyhow!("Failed to create iOS ISA: {}", e))?;
            isa_builder.finish(settings::Flags::new(flag_builder)).map_err(|e| anyhow!("{}", e))?
        }
        Some("android") => {
            let triple = target_lexicon::Triple::from_str("aarch64-unknown-linux-android")
                .map_err(|e| anyhow!("Bad triple: {}", e))?;
            let isa_builder = cranelift::codegen::isa::lookup(triple)
                .map_err(|e| anyhow!("Failed to create Android ISA: {}", e))?;
            isa_builder.finish(settings::Flags::new(flag_builder)).map_err(|e| anyhow!("{}", e))?
        }
        Some("windows") => {
            let triple = target_lexicon::Triple::from_str("x86_64-pc-windows-msvc")
                .map_err(|e| anyhow!("Bad triple: {}", e))?;
            let isa_builder = cranelift::codegen::isa::lookup(triple)
                .map_err(|e| anyhow!("Failed to create Windows ISA: {}", e))?;
            isa_builder.finish(settings::Flags::new(flag_builder)).map_err(|e| anyhow!("{}", e))?
        }
        _ => {
            let isa_builder = cranelift_native::builder().map_err(|e| anyhow!("{}", e))?;
            isa_builder.finish(settings::Flags::new(flag_builder)).map_err(|e| anyhow!("{}", e))?
        }
    };
    let builder = ObjectBuilder::new(isa, "perry_stubs", cranelift_module::default_libcall_names())?;
    let mut module = ObjectModule::new(builder);
    const TAG_UNDEF: u64 = 0x7FFC_0000_0000_0001;
    for name in missing_data_symbols {
        let data_id = module.declare_data(name, Linkage::Export, true, false)?;
        let mut dd = DataDescription::new();
        dd.define(TAG_UNDEF.to_le_bytes().to_vec().into_boxed_slice());
        module.define_data(data_id, &dd)?;
    }
    for name in missing_func_symbols {
        let mut sig = module.make_signature();
        sig.returns.push(AbiParam::new(types::F64));
        let func_id = module.declare_function(name, Linkage::Export, &sig)?;
        let mut ctx = module.make_context();
        ctx.func.signature = sig;
        let mut fc = cranelift_frontend::FunctionBuilderContext::new();
        {
            let mut fb = FunctionBuilder::new(&mut ctx.func, &mut fc);
            let block = fb.create_block();
            fb.append_block_params_for_function_params(block);
            fb.switch_to_block(block);
            fb.seal_block(block);
            let undef = fb.ins().f64const(f64::from_bits(TAG_UNDEF));
            fb.ins().return_(&[undef]);
            fb.finalize();
        }
        module.define_function(func_id, &mut ctx)?;
        module.clear_context(&mut ctx);
    }
    // Identity stubs: fn(f64) -> f64, returns the argument as-is.
    // Used for functions like js_await_any_promise that should pass through
    // values in standalone mode (no V8 runtime).
    for name in identity_func_symbols {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::F64));
        sig.returns.push(AbiParam::new(types::F64));
        let func_id = module.declare_function(name, Linkage::Export, &sig)?;
        let mut ctx = module.make_context();
        ctx.func.signature = sig;
        let mut fc = cranelift_frontend::FunctionBuilderContext::new();
        {
            let mut fb = FunctionBuilder::new(&mut ctx.func, &mut fc);
            let block = fb.create_block();
            fb.append_block_params_for_function_params(block);
            fb.switch_to_block(block);
            fb.seal_block(block);
            let arg = fb.block_params(block)[0];
            fb.ins().return_(&[arg]);
            fb.finalize();
        }
        module.define_function(func_id, &mut ctx)?;
        module.clear_context(&mut ctx);
    }
    let product = module.finish();
    Ok(product.emit().map_err(|e| anyhow!("Failed to emit stub object: {}", e))?)
}
