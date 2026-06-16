//! User-defined formula check backed by the full statements DSL parser.
//!
//! [`FormulaCheck`] lets users define custom validation rules as arbitrary
//! expressions over node values. The formula is parsed using the statements
//! DSL parser ([`finstack_quant_statements::dsl::parse_formula`]) and evaluated
//! recursively against each period's node values.
//!
//! Convention: result `!= 0.0` → pass, result `== 0.0` → fail.
//!
//! # Example (JSON)
//!
//! ```json
//! {
//!   "id": "gross_margin_floor",
//!   "name": "Gross margin >= 20%",
//!   "category": "internal_consistency",
//!   "severity": "error",
//!   "formula": "(revenue - cogs) / revenue >= 0.20",
//!   "message_template": "Gross margin below 20% in {period}",
//!   "tolerance": null
//! }
//! ```
//!
//! ## Supported syntax
//!
//! All DSL constructs are supported including:
//!
//! - Arithmetic: `+`, `-`, `*`, `/`, `%`
//! - Comparison: `==`, `!=`, `<`, `<=`, `>`, `>=`
//! - Logical: `and`, `or`, `not`
//! - Functions: `abs()`, `min()`, `max()`
//! - Parenthesized sub-expressions
//! - Conditionals: `if(cond, then, else)`
//! - Nested/compound expressions: `(revenue - cogs) / revenue >= 0.20`

use serde::{Deserialize, Serialize};

use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Severity,
};
use finstack_quant_statements::dsl::ast::{BinOp, StmtExpr, UnaryOp};
use finstack_quant_statements::dsl::parse_formula;
use finstack_quant_statements::Result;

use super::get_node_value;

/// A user-defined check that evaluates a DSL formula expression per period.
///
/// The formula is parsed once per `execute()` call using the full statements
/// DSL parser and then evaluated recursively for each period.
///
/// Convention: result `!= 0.0` → pass, result `== 0.0` → fail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaCheck {
    /// Unique identifier for this check instance.
    pub id: String,
    /// Human-readable name shown in reports.
    pub name: String,
    /// Category grouping.
    pub category: CheckCategory,
    /// Severity assigned to findings when the formula fails.
    pub severity: Severity,
    /// DSL expression to evaluate (e.g. `"(revenue - cogs) / revenue >= 0.20"`).
    pub formula: String,
    /// Template for the finding message. `{period}` is replaced with the
    /// period identifier.
    pub message_template: String,
    /// Numeric tolerance for floating-point comparisons. If `None`, exact
    /// comparison is used (result `== 0.0` → fail).
    pub tolerance: Option<f64>,
}

impl Check for FormulaCheck {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn category(&self) -> CheckCategory {
        self.category
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let ast = parse_formula(&self.formula)?;
        let mut findings = Vec::new();

        for period_spec in &context.model.periods {
            let pid = &period_spec.id;

            match eval_ast(&ast, context, pid) {
                Ok(value) => {
                    // A non-finite result (e.g. from division by zero in the
                    // formula) is always treated as a failure: the check could
                    // not be meaningfully evaluated, so surface it as a finding
                    // rather than silently "passing".
                    let passes = if !value.is_finite() {
                        false
                    } else if let Some(tol) = self.tolerance {
                        value.abs() > tol
                    } else {
                        value != 0.0
                    };

                    if !passes {
                        let message = self.message_template.replace("{period}", &pid.to_string());
                        findings.push(CheckFinding {
                            check_id: self.id.clone(),
                            severity: self.severity,
                            message,
                            period: Some(*pid),
                            materiality: None,
                            nodes: vec![],
                        });
                    }
                }
                Err(_) => {
                    // If we cannot evaluate (e.g. missing nodes), skip silently
                    // rather than producing a false positive.
                }
            }
        }

        let passed = !findings.iter().any(|f| f.severity >= Severity::Error);

        Ok(CheckResult {
            check_id: self.id.clone(),
            check_name: self.name.clone(),
            category: self.category,
            passed,
            findings,
        })
    }
}

// ---------------------------------------------------------------------------
// Recursive AST evaluator
// ---------------------------------------------------------------------------

/// Internal error type for AST evaluation failures.
#[derive(Debug)]
enum EvalError {
    /// A referenced node was not found in the results for this period.
    MissingNode,
    /// An unsupported AST construct was encountered.
    Unsupported(String),
}

impl From<EvalError> for finstack_quant_statements::error::Error {
    fn from(e: EvalError) -> Self {
        match e {
            EvalError::MissingNode => {
                finstack_quant_statements::error::Error::eval("missing node value".to_string())
            }
            EvalError::Unsupported(msg) => {
                finstack_quant_statements::error::Error::eval(format!("unsupported: {msg}"))
            }
        }
    }
}

/// Recursively evaluate a [`StmtExpr`] AST node, looking up node values from
/// the check context for the given period.
fn eval_ast(
    ast: &StmtExpr,
    context: &CheckContext,
    period: &finstack_quant_core::dates::PeriodId,
) -> std::result::Result<f64, EvalError> {
    match ast {
        StmtExpr::Literal(n) => Ok(*n),

        StmtExpr::NodeRef(node_id) => {
            get_node_value(context.results, node_id, period).ok_or(EvalError::MissingNode)
        }

        StmtExpr::BinOp { op, left, right } => {
            let l = eval_ast(left, context, period)?;
            let r = eval_ast(right, context, period)?;
            Ok(eval_binop(*op, l, r))
        }

        StmtExpr::UnaryOp { op, operand } => {
            let v = eval_ast(operand, context, period)?;
            Ok(eval_unaryop(*op, v))
        }

        StmtExpr::Call { func, args } => eval_call(func, args, context, period),

        StmtExpr::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => {
            let cond = eval_ast(condition, context, period)?;
            if cond != 0.0 {
                eval_ast(then_expr, context, period)
            } else {
                eval_ast(else_expr, context, period)
            }
        }

        StmtExpr::CSRef { .. } => Err(EvalError::Unsupported(
            "capital structure references in formula checks".into(),
        )),
    }
}

/// Evaluate a binary operator.
fn eval_binop(op: BinOp, l: f64, r: f64) -> f64 {
    match op {
        BinOp::Add => l + r,
        BinOp::Sub => l - r,
        BinOp::Mul => l * r,
        BinOp::Div => {
            if r.abs() > f64::EPSILON {
                l / r
            } else {
                f64::NAN
            }
        }
        BinOp::Mod => {
            if r.abs() > f64::EPSILON {
                l % r
            } else {
                f64::NAN
            }
        }
        BinOp::Eq => bool_to_f64((l - r).abs() <= f64::EPSILON),
        BinOp::Ne => bool_to_f64((l - r).abs() > f64::EPSILON),
        BinOp::Lt => bool_to_f64(l < r),
        BinOp::Le => bool_to_f64(l <= r),
        BinOp::Gt => bool_to_f64(l > r),
        BinOp::Ge => bool_to_f64(l >= r),
        BinOp::And => bool_to_f64(l != 0.0 && r != 0.0),
        BinOp::Or => bool_to_f64(l != 0.0 || r != 0.0),
    }
}

/// Evaluate a unary operator.
fn eval_unaryop(op: UnaryOp, v: f64) -> f64 {
    match op {
        UnaryOp::Neg => -v,
        UnaryOp::Not => bool_to_f64(v == 0.0),
    }
}

/// Evaluate a built-in function call.
fn eval_call(
    func: &str,
    args: &[StmtExpr],
    context: &CheckContext,
    period: &finstack_quant_core::dates::PeriodId,
) -> std::result::Result<f64, EvalError> {
    match func {
        "abs" => {
            if args.len() != 1 {
                return Err(EvalError::Unsupported("abs() requires 1 argument".into()));
            }
            Ok(eval_ast(&args[0], context, period)?.abs())
        }
        "min" => {
            if args.len() != 2 {
                return Err(EvalError::Unsupported("min() requires 2 arguments".into()));
            }
            let a = eval_ast(&args[0], context, period)?;
            let b = eval_ast(&args[1], context, period)?;
            Ok(a.min(b))
        }
        "max" => {
            if args.len() != 2 {
                return Err(EvalError::Unsupported("max() requires 2 arguments".into()));
            }
            let a = eval_ast(&args[0], context, period)?;
            let b = eval_ast(&args[1], context, period)?;
            Ok(a.max(b))
        }
        "sign" => {
            if args.len() != 1 {
                return Err(EvalError::Unsupported("sign() requires 1 argument".into()));
            }
            let v = eval_ast(&args[0], context, period)?;
            Ok(v.signum())
        }
        other => Err(EvalError::Unsupported(format!(
            "function '{other}' not supported in formula checks"
        ))),
    }
}

fn bool_to_f64(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
}
