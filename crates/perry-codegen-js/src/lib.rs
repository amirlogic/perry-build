//! JavaScript code generation backend for Perry
//!
//! Compiles HIR modules to JavaScript for `--target web`.
//! Produces a self-contained HTML file with embedded JS runtime.

pub mod emit;

use anyhow::Result;
use perry_hir::ir::Module;
use std::collections::BTreeSet;

/// Embedded web runtime JavaScript
const WEB_RUNTIME_JS: &str = include_str!("web_runtime.js");

/// Compile a single HIR module to JavaScript source code.
/// Returns (js_source, exported_names).
pub fn compile_module_to_js(module: &Module) -> (String, BTreeSet<String>) {
    let emitter = emit::JsEmitter::new(&module.name);

    // Collect exported names before emitting
    let mut exported_names = BTreeSet::new();
    for export in &module.exports {
        match export {
            perry_hir::ir::Export::Named { exported, .. } => {
                exported_names.insert(exported.clone());
            }
            _ => {}
        }
    }

    let js = emitter.emit_module(module);
    (js, exported_names)
}

/// Compile multiple HIR modules into a self-contained HTML file.
///
/// Modules are emitted in topological order (dependency order).
/// The entry module is the last one in the list.
pub fn compile_modules_to_html(
    modules: &[(String, Module)],  // (module_name, hir_module)
    title: &str,
) -> Result<String> {
    let mut all_js = String::with_capacity(32768);

    // Emit non-entry modules as IIFE-wrapped sections that export their values
    let entry_idx = modules.len().saturating_sub(1);

    for (i, (mod_name, module)) in modules.iter().enumerate() {
        let is_entry = i == entry_idx;

        let (js, exported_names) = compile_module_to_js(module);

        if is_entry {
            // Entry module: emit directly (no IIFE wrapper)
            all_js.push_str("// --- Entry module ---\n");
            all_js.push_str(&js);
        } else if !exported_names.is_empty() {
            // Non-entry module with exports: wrap in IIFE
            let safe_name = sanitize_module_name(mod_name);
            let _ = std::fmt::Write::write_fmt(&mut all_js,
                format_args!("const __mod_{} = (() => {{\n", safe_name));
            all_js.push_str(&js);
            all_js.push_str("  return {");
            for (j, name) in exported_names.iter().enumerate() {
                if j > 0 { all_js.push_str(", "); }
                all_js.push_str(name);
            }
            all_js.push_str("};\n})();\n");

            // Destructure exports into local scope
            all_js.push_str("const {");
            for (j, name) in exported_names.iter().enumerate() {
                if j > 0 { all_js.push_str(", "); }
                all_js.push_str(name);
            }
            let _ = std::fmt::Write::write_fmt(&mut all_js,
                format_args!("}} = __mod_{};\n", safe_name));
        } else {
            // Non-entry module without exports: still wrap in IIFE for scope isolation
            all_js.push_str("(() => {\n");
            all_js.push_str(&js);
            all_js.push_str("})();\n");
        }

        all_js.push('\n');
    }

    // Build HTML
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title}</title>
</head>
<body>
  <div id="perry-root"></div>
  <script>
{WEB_RUNTIME_JS}
  </script>
  <script>
{all_js}
  </script>
</body>
</html>"#,
        title = html_escape(title),
        WEB_RUNTIME_JS = WEB_RUNTIME_JS,
        all_js = all_js,
    );

    Ok(html)
}

/// Sanitize a module name for use as a JavaScript identifier
fn sanitize_module_name(name: &str) -> String {
    name.chars().map(|c| {
        if c.is_alphanumeric() || c == '_' { c } else { '_' }
    }).collect()
}

/// Basic HTML escaping for title
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}
