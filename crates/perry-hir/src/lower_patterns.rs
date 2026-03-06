//! Pattern and literal lowering utilities.
//!
//! Contains functions for lowering literals, assignment targets, binding names,
//! parameter destructuring, and other pattern-related utilities.

use anyhow::{anyhow, Result};
use perry_types::{LocalId, Type};
use swc_ecma_ast as ast;
use crate::ir::*;
use crate::lower::{LoweringContext, lower_expr};
use crate::lower_types::*;

pub(crate) fn unescape_template(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('$') => result.push('$'),
                Some('`') => result.push('`'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

pub(crate) fn lower_lit(lit: &ast::Lit) -> Result<Expr> {
    match lit {
        ast::Lit::Num(n) => {
            let value = n.value;
            // Check if this is an integer that fits in i64
            if value.fract() == 0.0
                && value >= i64::MIN as f64
                && value <= i64::MAX as f64
            {
                Ok(Expr::Integer(value as i64))
            } else {
                Ok(Expr::Number(value))
            }
        }
        ast::Lit::Str(s) => Ok(Expr::String(s.value.as_str().unwrap_or("").to_string())),
        ast::Lit::Bool(b) => Ok(Expr::Bool(b.value)),
        ast::Lit::Null(_) => Ok(Expr::Null),
        ast::Lit::BigInt(bi) => Ok(Expr::BigInt(bi.value.to_string())),
        ast::Lit::Regex(re) => Ok(Expr::RegExp {
            pattern: re.exp.to_string(),
            flags: re.flags.to_string(),
        }),
        _ => Err(anyhow!("Unsupported literal type")),
    }
}

/// Convert an assignment target to an expression for reading its current value
/// Used for compound assignment operators like += to read the current value before modifying
pub(crate) fn lower_assign_target_to_expr(ctx: &mut LoweringContext, target: &ast::AssignTarget) -> Result<Expr> {
    match target {
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => {
            let name = ident.id.sym.to_string();
            if let Some(id) = ctx.lookup_local(&name) {
                Ok(Expr::LocalGet(id))
            } else {
                Err(anyhow!("Undefined variable in compound assignment: {}", name))
            }
        }
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) => {
            // Check if this is a static field access
            if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                let obj_name = obj_ident.sym.to_string();
                if ctx.lookup_class(&obj_name).is_some() {
                    if let ast::MemberProp::Ident(prop_ident) = &member.prop {
                        let field_name = prop_ident.sym.to_string();
                        if ctx.has_static_field(&obj_name, &field_name) {
                            return Ok(Expr::StaticFieldGet {
                                class_name: obj_name,
                                field_name,
                            });
                        }
                    }
                }
            }

            let object = Box::new(lower_expr(ctx, &member.obj)?);
            match &member.prop {
                ast::MemberProp::Ident(ident) => {
                    let property = ident.sym.to_string();
                    Ok(Expr::PropertyGet { object, property })
                }
                ast::MemberProp::Computed(computed) => {
                    let index = Box::new(lower_expr(ctx, &computed.expr)?);
                    Ok(Expr::IndexGet { object, index })
                }
                ast::MemberProp::PrivateName(private) => {
                    let property = format!("#{}", private.name.to_string());
                    Ok(Expr::PropertyGet { object, property })
                }
            }
        }
        _ => Err(anyhow!("Unsupported target in compound assignment")),
    }
}

pub(crate) fn get_binding_name(pat: &ast::Pat) -> Result<String> {
    match pat {
        ast::Pat::Ident(ident) => Ok(ident.id.sym.to_string()),
        _ => Err(anyhow!("Unsupported binding pattern")),
    }
}

/// Static counter for generating unique synthetic names for destructuring patterns
static DESTRUCT_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) fn get_pat_name(pat: &ast::Pat) -> Result<String> {
    match pat {
        ast::Pat::Ident(ident) => Ok(ident.id.sym.to_string()),
        ast::Pat::Assign(assign) => get_pat_name(&assign.left),
        ast::Pat::Rest(rest) => get_pat_name(&rest.arg),
        // For complex destructuring patterns, generate synthetic names
        // The actual destructuring will be handled at the call site or as a separate pass
        ast::Pat::Array(_) => {
            let id = DESTRUCT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(format!("__arr_destruct_{}", id))
        }
        ast::Pat::Object(_) => {
            let id = DESTRUCT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(format!("__obj_destruct_{}", id))
        }
        _ => Err(anyhow!("Unsupported pattern")),
    }
}

/// Extract the type annotation from a Pat (for arrow function parameters)
pub(crate) fn get_pat_type(pat: &ast::Pat, ctx: &LoweringContext) -> Type {
    match pat {
        ast::Pat::Ident(ident) => {
            ident.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, Some(ctx)))
                .unwrap_or(Type::Any)
        }
        ast::Pat::Assign(assign) => get_pat_type(&assign.left, ctx),
        ast::Pat::Rest(rest) => {
            rest.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, Some(ctx)))
                .unwrap_or(Type::Any)
        }
        ast::Pat::Array(arr) => {
            arr.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, Some(ctx)))
                .unwrap_or(Type::Any)
        }
        ast::Pat::Object(obj) => {
            obj.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, Some(ctx)))
                .unwrap_or(Type::Any)
        }
        _ => Type::Any,
    }
}

/// Generate Let statements to extract destructured variables from a synthetic parameter.
/// For array patterns like `[a, b]`, generates:
///   let a = param[0];
///   let b = param[1];
/// For object patterns like `{a, b}`, generates:
///   let a = param.a;
///   let b = param.b;
/// Returns the statements and defines the variables in the context.
pub(crate) fn generate_param_destructuring_stmts(
    ctx: &mut LoweringContext,
    pat: &ast::Pat,
    param_id: LocalId,
) -> Result<Vec<Stmt>> {
    let mut stmts = Vec::new();

    match pat {
        ast::Pat::Array(arr_pat) => {
            for (idx, elem) in arr_pat.elems.iter().enumerate() {
                if let Some(elem_pat) = elem {
                    match elem_pat {
                        ast::Pat::Ident(ident) => {
                            let name = ident.id.sym.to_string();
                            let id = ctx.define_local(name.clone(), Type::Any);
                            let index_expr = Expr::IndexGet {
                                object: Box::new(Expr::LocalGet(param_id)),
                                index: Box::new(Expr::Number(idx as f64)),
                            };
                            stmts.push(Stmt::Let {
                                id,
                                name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(index_expr),
                            });
                        }
                        ast::Pat::Array(nested_arr) => {
                            // Nested array destructuring: [[a, b], c]
                            // First extract the nested array element
                            let nested_id = ctx.fresh_local();
                            let nested_name = format!("__nested_{}", nested_id);
                            ctx.locals.push((nested_name.clone(), nested_id, Type::Any));
                            let index_expr = Expr::IndexGet {
                                object: Box::new(Expr::LocalGet(param_id)),
                                index: Box::new(Expr::Number(idx as f64)),
                            };
                            stmts.push(Stmt::Let {
                                id: nested_id,
                                name: nested_name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(index_expr),
                            });
                            // Recursively generate destructuring for nested pattern
                            let nested_stmts = generate_param_destructuring_stmts(ctx, &ast::Pat::Array(nested_arr.clone()), nested_id)?;
                            stmts.extend(nested_stmts);
                        }
                        ast::Pat::Object(nested_obj) => {
                            // Nested object destructuring: [{a, b}, c]
                            let nested_id = ctx.fresh_local();
                            let nested_name = format!("__nested_{}", nested_id);
                            ctx.locals.push((nested_name.clone(), nested_id, Type::Any));
                            let index_expr = Expr::IndexGet {
                                object: Box::new(Expr::LocalGet(param_id)),
                                index: Box::new(Expr::Number(idx as f64)),
                            };
                            stmts.push(Stmt::Let {
                                id: nested_id,
                                name: nested_name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(index_expr),
                            });
                            let nested_stmts = generate_param_destructuring_stmts(ctx, &ast::Pat::Object(nested_obj.clone()), nested_id)?;
                            stmts.extend(nested_stmts);
                        }
                        ast::Pat::Rest(rest_pat) => {
                            // Rest pattern: [a, ...rest]
                            // For now, skip (would need slice operation)
                            if let ast::Pat::Ident(ident) = rest_pat.arg.as_ref() {
                                let name = ident.id.sym.to_string();
                                let id = ctx.define_local(name.clone(), Type::Array(Box::new(Type::Any)));
                                // Create a slice from idx to end
                                let slice_expr = Expr::ArraySlice {
                                    array: Box::new(Expr::LocalGet(param_id)),
                                    start: Box::new(Expr::Number(idx as f64)),
                                    end: None,
                                };
                                stmts.push(Stmt::Let {
                                    id,
                                    name,
                                    ty: Type::Array(Box::new(Type::Any)),
                                    mutable: false,
                                    init: Some(slice_expr),
                                });
                            }
                        }
                        ast::Pat::Assign(assign_pat) => {
                            // Default value: [a = default, b]
                            if let ast::Pat::Ident(ident) = assign_pat.left.as_ref() {
                                let name = ident.id.sym.to_string();
                                let id = ctx.define_local(name.clone(), Type::Any);
                                let index_expr = Expr::IndexGet {
                                    object: Box::new(Expr::LocalGet(param_id)),
                                    index: Box::new(Expr::Number(idx as f64)),
                                };
                                // TODO: handle default value with nullish coalescing
                                stmts.push(Stmt::Let {
                                    id,
                                    name,
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(index_expr),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        ast::Pat::Object(obj_pat) => {
            for prop in &obj_pat.props {
                match prop {
                    ast::ObjectPatProp::KeyValue(kv) => {
                        let key = match &kv.key {
                            ast::PropName::Ident(ast::IdentName { sym, .. }) => sym.to_string(),
                            ast::PropName::Str(s) => String::from_utf8_lossy(s.value.as_bytes()).to_string(),
                            _ => continue,
                        };
                        if let ast::Pat::Ident(ident) = kv.value.as_ref() {
                            let name = ident.id.sym.to_string();
                            let id = ctx.define_local(name.clone(), Type::Any);
                            let prop_expr = Expr::PropertyGet {
                                object: Box::new(Expr::LocalGet(param_id)),
                                property: key,
                            };
                            stmts.push(Stmt::Let {
                                id,
                                name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(prop_expr),
                            });
                        }
                    }
                    ast::ObjectPatProp::Assign(assign) => {
                        let name = assign.key.sym.to_string();
                        let id = ctx.define_local(name.clone(), Type::Any);
                        let init_value = if let Some(default_expr) = &assign.value {
                            // { key = default } - use default if property is undefined
                            let prop_access = Expr::PropertyGet {
                                object: Box::new(Expr::LocalGet(param_id)),
                                property: name.clone(),
                            };
                            let default_val = lower_expr(ctx, default_expr)?;
                            let condition = Expr::Compare {
                                op: CompareOp::Ne,
                                left: Box::new(prop_access.clone()),
                                right: Box::new(Expr::Undefined),
                            };
                            Expr::Conditional {
                                condition: Box::new(condition),
                                then_expr: Box::new(prop_access),
                                else_expr: Box::new(default_val),
                            }
                        } else {
                            Expr::PropertyGet {
                                object: Box::new(Expr::LocalGet(param_id)),
                                property: name.clone(),
                            }
                        };
                        stmts.push(Stmt::Let {
                            id,
                            name,
                            ty: Type::Any,
                            mutable: false,
                            init: Some(init_value),
                        });
                    }
                    ast::ObjectPatProp::Rest(_) => {
                        // Rest pattern: {...rest} - skip for now
                    }
                }
            }
        }
        _ => {}
    }

    Ok(stmts)
}

/// Check if a pattern is a destructuring pattern (array or object)
pub(crate) fn is_destructuring_pattern(pat: &ast::Pat) -> bool {
    matches!(pat, ast::Pat::Array(_) | ast::Pat::Object(_))
}

/// Detect if an expression represents a native handle instance (Big, Decimal, etc.)
/// Returns the module name if it does.
pub(crate) fn detect_native_instance_expr(expr: &ast::Expr) -> Option<&'static str> {
    match expr {
        // new Big(...) / new Decimal(...) / new BigNumber(...)
        ast::Expr::New(new_expr) => {
            if let ast::Expr::Ident(ident) = new_expr.callee.as_ref() {
                match ident.sym.as_ref() {
                    "Big" => Some("big.js"),
                    "Decimal" => Some("decimal.js"),
                    "BigNumber" => Some("bignumber.js"),
                    "LRUCache" => Some("lru-cache"),
                    "Command" => Some("commander"),
                    _ => None,
                }
            } else {
                None
            }
        }
        // Chained method calls: new Big(...).plus(...).div(...)
        ast::Expr::Call(call_expr) => {
            if let ast::Callee::Expr(callee_expr) = &call_expr.callee {
                if let ast::Expr::Member(member) = callee_expr.as_ref() {
                    // Recursively check the object
                    detect_native_instance_expr(&member.obj)
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Check if a parameter pattern is a rest parameter (...args)
pub(crate) fn is_rest_param(pat: &ast::Pat) -> bool {
    matches!(pat, ast::Pat::Rest(_))
}

/// Extract default value from a parameter pattern (if any)
/// For optional parameters (x?: Type), we provide Expr::Undefined as the default
pub(crate) fn get_param_default(ctx: &mut LoweringContext, pat: &ast::Pat) -> Result<Option<Expr>> {
    match pat {
        ast::Pat::Ident(ident) => {
            // Check if this is an optional parameter (x?: Type)
            if ident.optional {
                Ok(Some(Expr::Undefined))
            } else {
                Ok(None)
            }
        }
        ast::Pat::Assign(assign) => {
            let default_expr = lower_expr(ctx, &assign.right)?;
            Ok(Some(default_expr))
        }
        _ => Ok(None),
    }
}

/// Built-in Node.js modules that are handled specially by the compiler
const BUILTIN_MODULES: &[&str] = &["fs", "path", "crypto"];

/// Check if an expression is a require() call for a built-in module.
/// Returns the module name if it is, None otherwise.
pub(crate) fn is_require_builtin_module(expr: &ast::Expr) -> Option<String> {
    if let ast::Expr::Call(call) = expr {
        if let ast::Callee::Expr(callee_expr) = &call.callee {
            if let ast::Expr::Ident(ident) = callee_expr.as_ref() {
                if ident.sym.as_ref() == "require" {
                    // Check if the first argument is a string literal
                    if let Some(arg) = call.args.first() {
                        if let ast::Expr::Lit(ast::Lit::Str(s)) = &*arg.expr {
                            let module_name = s.value.as_str().unwrap_or("").to_string();
                            if BUILTIN_MODULES.contains(&module_name.as_str()) {
                                return Some(module_name);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

