/// A formula at exactly the term budget must compile without overflowing a
/// small stack.
///
/// This pins the budget to the property that motivates it. Formulas are
/// compiled on rayon workers during Monte Carlo, which get Rust's 2 MiB
/// default stack rather than the main thread's 8 MiB, and a stack overflow
/// aborts the process (SIGABRT) rather than unwinding — the bindings cannot
/// catch it. 512 KiB is deliberately 4x smaller than that default, so this
/// test fails long before a real deployment would.
///
/// If this test starts aborting, `MAX_FORMULA_TERMS` is too high for the
/// per-term stack cost of the AST walkers (`compile`, `infer_value_type`,
/// `Drop`) — lower the budget rather than raising the stack here.
#[test]
fn formula_at_term_budget_compiles_on_a_small_stack() {
    // One term per operand, so this sits exactly at the budget.
    let formula = std::iter::repeat_n("1", 256).collect::<Vec<_>>().join("+");
    let compiled = std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(move || crate::dsl::parse_and_compile(&formula).is_ok())
        .expect("probe thread should spawn")
        .join()
        .expect("compiling a budget-sized formula must not overflow a 512 KiB stack");
    assert!(compiled, "a budget-sized formula must compile successfully");
}

/// The deepest tree the preflight accepts must lower, render and drop
/// on the same small stack.
#[test]
fn tree_at_depth_limit_compiles_on_a_small_stack() {
    use crate::dsl::StmtExpr;
    use finstack_quant_core::expr::UnaryOp;

    let compiled = std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let tree = (1..crate::dsl::validate::MAX_AST_DEPTH)
                .fold(StmtExpr::node_ref("a"), |acc, _| {
                    StmtExpr::unary_op(UnaryOp::Neg, acc)
                });
            let _ = tree.to_string();
            crate::dsl::compile(&tree).is_ok()
        })
        .expect("probe thread should spawn")
        .join()
        .expect("a depth-limit tree must not overflow a 512 KiB stack");
    assert!(compiled, "a depth-limit tree must compile successfully");
}
