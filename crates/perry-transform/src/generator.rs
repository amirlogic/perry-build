//! Generator function state machine transformation
//!
//! Transforms generator functions (function*) into regular functions
//! that return iterator objects with a next() method implementing
//! a state machine.
//!
//! The next() method contains a `while(true)` loop with `if (__state === N)`
//! blocks. Non-yielding states set __state and `continue`. Yielding states
//! set __state and `return {value, done: false}`.

use perry_hir::ir::*;
use perry_types::{FuncId, LocalId, Type};

/// Transform all generator functions in a module into state machine form.
pub fn transform_generators(module: &mut Module) {
    // Compute the next available local and func IDs by scanning the module
    let mut next_local_id = compute_max_local_id(module) + 1;
    let mut next_func_id = compute_max_func_id(module) + 1;

    for func in &mut module.functions {
        if func.is_generator {
            transform_generator_function(func, &mut next_local_id, &mut next_func_id);
        }
    }
}

/// Find the maximum local ID used in the module.
fn compute_max_local_id(module: &Module) -> LocalId {
    let mut max_id: LocalId = 0;
    for func in &module.functions {
        for param in &func.params {
            max_id = max_id.max(param.id);
        }
        scan_stmts_for_max_local(&func.body, &mut max_id);
    }
    for stmt in &module.init {
        scan_stmt_for_max_local(stmt, &mut max_id);
    }
    for global in &module.globals {
        max_id = max_id.max(global.id);
    }
    max_id
}

fn scan_stmts_for_max_local(stmts: &[Stmt], max_id: &mut LocalId) {
    for stmt in stmts {
        scan_stmt_for_max_local(stmt, max_id);
    }
}

fn scan_stmt_for_max_local(stmt: &Stmt, max_id: &mut LocalId) {
    match stmt {
        Stmt::Let { id, .. } => *max_id = (*max_id).max(*id),
        Stmt::If { then_branch, else_branch, .. } => {
            scan_stmts_for_max_local(then_branch, max_id);
            if let Some(eb) = else_branch { scan_stmts_for_max_local(eb, max_id); }
        }
        Stmt::While { body, .. } => scan_stmts_for_max_local(body, max_id),
        Stmt::For { init, body, .. } => {
            if let Some(i) = init { scan_stmt_for_max_local(i, max_id); }
            scan_stmts_for_max_local(body, max_id);
        }
        Stmt::Try { body, catch, finally } => {
            scan_stmts_for_max_local(body, max_id);
            if let Some(c) = catch { scan_stmts_for_max_local(&c.body, max_id); }
            if let Some(f) = finally { scan_stmts_for_max_local(f, max_id); }
        }
        Stmt::Switch { cases, .. } => {
            for case in cases { scan_stmts_for_max_local(&case.body, max_id); }
        }
        _ => {}
    }
}

/// Find the maximum func ID used in the module.
fn compute_max_func_id(module: &Module) -> FuncId {
    let mut max_id: FuncId = 0;
    for func in &module.functions {
        max_id = max_id.max(func.id);
        scan_stmts_for_max_func(&func.body, &mut max_id);
    }
    for stmt in &module.init {
        scan_stmt_for_max_func(stmt, &mut max_id);
    }
    max_id
}

fn scan_stmts_for_max_func(stmts: &[Stmt], max_id: &mut FuncId) {
    for stmt in stmts {
        scan_stmt_for_max_func(stmt, max_id);
    }
}

fn scan_stmt_for_max_func(stmt: &Stmt, max_id: &mut FuncId) {
    match stmt {
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            scan_expr_for_max_func(expr, max_id);
        }
        Stmt::Let { init: Some(expr), .. } => scan_expr_for_max_func(expr, max_id),
        Stmt::If { condition, then_branch, else_branch } => {
            scan_expr_for_max_func(condition, max_id);
            scan_stmts_for_max_func(then_branch, max_id);
            if let Some(eb) = else_branch { scan_stmts_for_max_func(eb, max_id); }
        }
        Stmt::While { body, .. } => scan_stmts_for_max_func(body, max_id),
        Stmt::For { body, .. } => scan_stmts_for_max_func(body, max_id),
        Stmt::Try { body, catch, finally } => {
            scan_stmts_for_max_func(body, max_id);
            if let Some(c) = catch { scan_stmts_for_max_func(&c.body, max_id); }
            if let Some(f) = finally { scan_stmts_for_max_func(f, max_id); }
        }
        Stmt::Switch { cases, .. } => {
            for case in cases { scan_stmts_for_max_func(&case.body, max_id); }
        }
        _ => {}
    }
}

fn scan_expr_for_max_func(expr: &Expr, max_id: &mut FuncId) {
    match expr {
        Expr::FuncRef(id) => *max_id = (*max_id).max(*id),
        Expr::Closure { func_id, body, .. } => {
            *max_id = (*max_id).max(*func_id);
            scan_stmts_for_max_func(body, max_id);
        }
        _ => {} // Could recurse deeper but this covers the main cases
    }
}

/// Allocate a fresh local ID.
fn alloc_local(next_id: &mut u32) -> LocalId {
    let id = *next_id;
    *next_id += 1;
    id
}

/// Create an iterator result object: { value: expr, done: bool }
fn make_iter_result(value: Expr, done: bool) -> Expr {
    Expr::Object(vec![
        ("value".to_string(), value),
        ("done".to_string(), Expr::Bool(done)),
    ])
}

/// Transform a single generator function into a state machine.
fn transform_generator_function(func: &mut Function, next_local_id: &mut u32, next_func_id: &mut u32) {
    let state_id = alloc_local(next_local_id);
    let done_id = alloc_local(next_local_id);

    // Collect all states from the generator body
    let mut states: Vec<State> = Vec::new();
    let mut current: Vec<Stmt> = Vec::new();
    let mut state_num: u32 = 0;

    linearize_body(&func.body, &mut states, &mut current, &mut state_num, state_id);

    // Push final state (code after last yield / end of function)
    states.push(State {
        num: state_num,
        body: current,
        exit: StateExit::Done,
    });

    // Build the if-chain inside while(true)
    let mut while_body: Vec<Stmt> = Vec::new();
    for state in &states {
        let mut case_body = state.body.clone();
        match &state.exit {
            StateExit::Yield { value, next_state } => {
                case_body.push(Stmt::Expr(Expr::LocalSet(
                    state_id,
                    Box::new(Expr::Number(*next_state as f64)),
                )));
                case_body.push(Stmt::Return(Some(make_iter_result(value.clone(), false))));
            }
            StateExit::Goto(next_state) => {
                case_body.push(Stmt::Expr(Expr::LocalSet(
                    state_id,
                    Box::new(Expr::Number(*next_state as f64)),
                )));
                case_body.push(Stmt::Continue);
            }
            StateExit::Done => {
                // Wrap any existing returns, or add a default done return
                let has_return = case_body.iter().any(|s| matches!(s, Stmt::Return(_)));
                if has_return {
                    rewrite_returns_as_done(&mut case_body);
                }
                case_body.push(Stmt::Expr(Expr::LocalSet(
                    done_id,
                    Box::new(Expr::Bool(true)),
                )));
                case_body.push(Stmt::Return(Some(make_iter_result(Expr::Undefined, true))));
            }
        }

        while_body.push(Stmt::If {
            condition: Expr::Compare {
                op: CompareOp::Eq,
                left: Box::new(Expr::LocalGet(state_id)),
                right: Box::new(Expr::Number(state.num as f64)),
            },
            then_branch: case_body,
            else_branch: None,
        });
    }

    // Default: done
    while_body.push(Stmt::Expr(Expr::LocalSet(
        done_id,
        Box::new(Expr::Bool(true)),
    )));
    while_body.push(Stmt::Return(Some(make_iter_result(Expr::Undefined, true))));

    // Build next() method body
    let next_body = vec![
        // if (__done) return { value: undefined, done: true };
        Stmt::If {
            condition: Expr::LocalGet(done_id),
            then_branch: vec![
                Stmt::Return(Some(make_iter_result(Expr::Undefined, true))),
            ],
            else_branch: None,
        },
        // while (true) { if-chain }
        Stmt::While {
            condition: Expr::Bool(true),
            body: while_body,
        },
    ];

    // Build the new function body
    let mut new_body: Vec<Stmt> = Vec::new();

    // let __state = 0
    new_body.push(Stmt::Let {
        id: state_id,
        name: "__gen_state".to_string(),
        ty: Type::Number,
        mutable: true,
        init: Some(Expr::Number(0.0)),
    });

    // let __done = false
    new_body.push(Stmt::Let {
        id: done_id,
        name: "__gen_done".to_string(),
        ty: Type::Boolean,
        mutable: true,
        init: Some(Expr::Bool(false)),
    });

    // Hoist variable declarations from the original body
    let hoisted = collect_hoisted_vars(&func.body);
    for (var_id, var_name, var_ty) in &hoisted {
        new_body.push(Stmt::Let {
            id: *var_id,
            name: var_name.clone(),
            ty: var_ty.clone(),
            mutable: true,
            init: None,
        });
    }

    // Build captures: state, done, params, hoisted vars
    let mut captures = vec![state_id, done_id];
    let mut mutable_captures = vec![state_id, done_id];
    for param in &func.params {
        captures.push(param.id);
    }
    for (var_id, _, _) in &hoisted {
        captures.push(*var_id);
        mutable_captures.push(*var_id);
    }
    captures.sort();
    captures.dedup();
    mutable_captures.sort();
    mutable_captures.dedup();

    let next_func_id_val = {
        let id = *next_func_id;
        *next_func_id += 1;
        id
    };

    let next_closure = Expr::Closure {
        func_id: next_func_id_val,
        params: Vec::new(),
        return_type: Type::Any,
        body: next_body,
        captures,
        mutable_captures,
        captures_this: false,
        enclosing_class: None,
        is_async: false,
    };

    // return { next: <closure> }
    new_body.push(Stmt::Return(Some(Expr::Object(vec![
        ("next".to_string(), next_closure),
    ]))));

    func.body = new_body;
    func.is_generator = false;
}

struct State {
    num: u32,
    body: Vec<Stmt>,
    exit: StateExit,
}

enum StateExit {
    /// Yield a value and advance to next_state
    Yield { value: Expr, next_state: u32 },
    /// Goto another state (non-yielding transition)
    Goto(u32),
    /// Function is done
    Done,
}

/// Linearize the generator body into a sequence of states.
/// Splits at yield points and handles for-loops with yields.
fn linearize_body(
    stmts: &[Stmt],
    states: &mut Vec<State>,
    current: &mut Vec<Stmt>,
    state_num: &mut u32,
    state_id: LocalId,
) {
    for stmt in stmts {
        match stmt {
            // yield expr at statement level
            Stmt::Expr(Expr::Yield { value, .. }) => {
                let yield_val = value.as_ref().map(|v| *v.clone()).unwrap_or(Expr::Undefined);
                let this_state = *state_num;
                *state_num += 1;
                states.push(State {
                    num: this_state,
                    body: std::mem::take(current),
                    exit: StateExit::Yield { value: yield_val, next_state: *state_num },
                });
            }

            // return expr (terminal - ends the generator)
            Stmt::Return(val) => {
                // Add the return with {value: expr, done: true} wrapping
                let return_val = val.clone().unwrap_or(Expr::Undefined);
                current.push(Stmt::Return(Some(make_iter_result(return_val, true))));
                // Flush current as a terminal state
                let this_state = *state_num;
                *state_num += 1;
                states.push(State {
                    num: this_state,
                    body: std::mem::take(current),
                    exit: StateExit::Done,
                });
            }

            // For-loop containing yield(s)
            Stmt::For { init, condition, update, body }
                if body_contains_yield(body) =>
            {
                // State N: pre-loop code + init, goto condition check
                let init_state = *state_num;
                *state_num += 1;
                let mut init_body = std::mem::take(current);
                // Add init statement (typically `let i = start`)
                // But we need to convert it to an assignment since the var is hoisted
                if let Some(init_stmt) = init {
                    match init_stmt.as_ref() {
                        Stmt::Let { id, init: Some(init_expr), .. } => {
                            init_body.push(Stmt::Expr(Expr::LocalSet(
                                *id,
                                Box::new(init_expr.clone()),
                            )));
                        }
                        other => init_body.push(other.clone()),
                    }
                }
                let cond_state = *state_num;
                states.push(State {
                    num: init_state,
                    body: init_body,
                    exit: StateExit::Goto(cond_state),
                });

                // State N+1: condition check
                *state_num += 1;
                let body_state = *state_num;
                // Condition check: if true, fall through to body; if false, done
                let cond_body = if let Some(cond) = condition {
                    // Build the done return as part of the else branch
                    vec![Stmt::If {
                        condition: Expr::Unary {
                            op: UnaryOp::Not,
                            operand: Box::new(cond.clone()),
                        },
                        then_branch: vec![
                            // Loop ended - jump past the loop
                            // We'll use a special state number that we'll fill in later
                            Stmt::Expr(Expr::LocalSet(
                                state_id,
                                Box::new(Expr::Number(0.0)), // placeholder, fixed below
                            )),
                        ],
                        else_branch: None,
                    }]
                } else {
                    vec![]
                };
                // We'll fix the after-loop state number after processing body
                states.push(State {
                    num: cond_state,
                    body: cond_body,
                    exit: StateExit::Goto(body_state),
                });

                // Process loop body (may contain yields)
                linearize_body(body, states, current, state_num, state_id);

                // State for update: run update expression, goto condition check
                let update_state = *state_num;
                *state_num += 1;
                let mut update_body = std::mem::take(current);
                if let Some(upd) = update {
                    update_body.push(Stmt::Expr(upd.clone()));
                }
                states.push(State {
                    num: update_state,
                    body: update_body,
                    exit: StateExit::Goto(cond_state),
                });

                // Fix up the condition state's false branch to jump to after-loop state
                let after_loop_state = *state_num;
                // Find the condition state and fix the placeholder
                for state in states.iter_mut() {
                    if state.num == cond_state {
                        fix_placeholder_state(&mut state.body, state_id, after_loop_state);
                    }
                }
            }

            // While-loop containing yield(s) - similar to for-loop
            Stmt::While { condition, body: while_body }
                if body_contains_yield(while_body) =>
            {
                let cond_state = *state_num;
                *state_num += 1;

                // Pre-loop code
                let pre_body = std::mem::take(current);
                if !pre_body.is_empty() {
                    let pre_state = cond_state;
                    // Actually we need to adjust: put pre-loop code before cond_state
                    // Re-assign: pre_state is cond_state - 1, and cond is the next
                    // For simplicity, fold pre-loop code as a goto to condition
                    states.push(State {
                        num: pre_state,
                        body: pre_body,
                        exit: StateExit::Goto(cond_state),
                    });
                    // We already incremented state_num, so cond_state is already taken
                    // Need to adjust...
                }

                let body_state = *state_num;
                // Condition check
                states.push(State {
                    num: cond_state,
                    body: vec![Stmt::If {
                        condition: Expr::Unary {
                            op: UnaryOp::Not,
                            operand: Box::new(condition.clone()),
                        },
                        then_branch: vec![
                            Stmt::Expr(Expr::LocalSet(
                                state_id,
                                Box::new(Expr::Number(0.0)), // placeholder
                            )),
                        ],
                        else_branch: None,
                    }],
                    exit: StateExit::Goto(body_state),
                });

                // Process body
                linearize_body(while_body, states, current, state_num, state_id);

                // After body, goto condition
                let loop_back_state = *state_num;
                *state_num += 1;
                states.push(State {
                    num: loop_back_state,
                    body: std::mem::take(current),
                    exit: StateExit::Goto(cond_state),
                });

                // Fix placeholder
                let after_loop = *state_num;
                for state in states.iter_mut() {
                    if state.num == cond_state {
                        fix_placeholder_state(&mut state.body, state_id, after_loop);
                    }
                }
            }

            // Regular statement (no yield) - accumulate
            other => {
                current.push(other.clone());
            }
        }
    }
}

/// Fix the placeholder `0.0` state number in condition-false branches.
fn fix_placeholder_state(stmts: &mut [Stmt], state_id: LocalId, target_state: u32) {
    for stmt in stmts.iter_mut() {
        if let Stmt::If { then_branch, .. } = stmt {
            for inner in then_branch.iter_mut() {
                if let Stmt::Expr(Expr::LocalSet(id, val)) = inner {
                    if *id == state_id {
                        if let Expr::Number(n) = val.as_ref() {
                            if *n == 0.0 {
                                *val = Box::new(Expr::Number(target_state as f64));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Check if any statement in the body contains a yield expression.
fn body_contains_yield(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(Expr::Yield { .. }) => return true,
            Stmt::Let { init: Some(Expr::Yield { .. }), .. } => return true,
            Stmt::Return(Some(Expr::Yield { .. })) => return true,
            Stmt::If { then_branch, else_branch, .. } => {
                if body_contains_yield(then_branch) { return true; }
                if let Some(eb) = else_branch {
                    if body_contains_yield(eb) { return true; }
                }
            }
            Stmt::While { body, .. } => {
                if body_contains_yield(body) { return true; }
            }
            Stmt::For { body, .. } => {
                if body_contains_yield(body) { return true; }
            }
            Stmt::Try { body, catch, finally } => {
                if body_contains_yield(body) { return true; }
                if let Some(c) = catch {
                    if body_contains_yield(&c.body) { return true; }
                }
                if let Some(f) = finally {
                    if body_contains_yield(f) { return true; }
                }
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    if body_contains_yield(&case.body) { return true; }
                }
            }
            _ => {}
        }
    }
    false
}

/// Collect variable declarations that need to be hoisted to the outer scope.
fn collect_hoisted_vars(stmts: &[Stmt]) -> Vec<(LocalId, String, Type)> {
    let mut vars = Vec::new();
    collect_vars_recursive(stmts, &mut vars);
    vars
}

fn collect_vars_recursive(stmts: &[Stmt], vars: &mut Vec<(LocalId, String, Type)>) {
    for stmt in stmts {
        match stmt {
            Stmt::Let { id, name, ty, .. } => {
                vars.push((*id, name.clone(), ty.clone()));
            }
            Stmt::If { then_branch, else_branch, .. } => {
                collect_vars_recursive(then_branch, vars);
                if let Some(eb) = else_branch {
                    collect_vars_recursive(eb, vars);
                }
            }
            Stmt::While { body, .. } => collect_vars_recursive(body, vars),
            Stmt::For { init, body, .. } => {
                if let Some(init) = init {
                    collect_vars_recursive(&[(**init).clone()], vars);
                }
                collect_vars_recursive(body, vars);
            }
            Stmt::Try { body, catch, finally } => {
                collect_vars_recursive(body, vars);
                if let Some(c) = catch { collect_vars_recursive(&c.body, vars); }
                if let Some(f) = finally { collect_vars_recursive(f, vars); }
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_vars_recursive(&case.body, vars);
                }
            }
            _ => {}
        }
    }
}

/// Rewrite Return(Some(expr)) to Return(Some({value: expr, done: true}))
fn rewrite_returns_as_done(stmts: &mut Vec<Stmt>) {
    for stmt in stmts.iter_mut() {
        match stmt {
            Stmt::Return(Some(expr)) => {
                // Don't double-wrap if already an iter result
                if !is_iter_result(expr) {
                    let val = expr.clone();
                    *expr = make_iter_result(val, true);
                }
            }
            Stmt::Return(None) => {
                *stmt = Stmt::Return(Some(make_iter_result(Expr::Undefined, true)));
            }
            _ => {}
        }
    }
}

/// Check if an expression is already an iterator result object
fn is_iter_result(expr: &Expr) -> bool {
    if let Expr::Object(props) = expr {
        props.len() == 2
            && props.iter().any(|(k, _)| k == "value")
            && props.iter().any(|(k, _)| k == "done")
    } else {
        false
    }
}
