//! Compiler from Statements DSL AST to core Expr.

use crate::dsl::ast::StmtExpr;
use crate::dsl::signatures::FunctionSignature;
use crate::error::Result;
use crate::types::{NodeId, NodeValueType};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::expr::{BinOp, Expr, UnaryOp};
use indexmap::IndexMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dimension {
    Unknown,
    Scalar,
    Monetary(finstack_quant_core::currency::Currency),
    /// The result of a comparison, a logical operator or `is_missing`. A
    /// node whose formula is Boolean stores the scalar `1` or `0`.
    Boolean,
}

/// Compile a [`StmtExpr`] into a core [`Expr`].
///
/// Converts the statements DSL syntax into the shared expression engine
/// representation used by the evaluator.
/// The whole tree is validated before it is lowered, so a tree built with the
/// public [`StmtExpr`] constructors meets the same rules as parsed text.
/// Capital-structure references are encoded as the core expression engine's
/// dedicated `cs` reference form.
///
/// # Arguments
///
/// * `ast` - Statements DSL expression to lower, parsed from text or built
///   with the [`StmtExpr`] constructors. At most 256 terms (literals,
///   references, calls and conditionals) and 320 levels deep.
///
/// # Errors
///
/// Returns a formula-parse error for a non-finite literal, an identifier
/// without nonempty dot-separated segments, or a tree over the size limits.
/// Returns an evaluation error for an unknown capital-structure component, an
/// unknown function, a wrong argument count, or a negative literal offset to
/// `lag`, `shift`, `diff`, `pct_change` or `growth_rate`. Every branch is
/// checked, including one evaluation would skip.
pub fn compile(ast: &StmtExpr) -> Result<Expr> {
    crate::dsl::validate::validate_ast(ast)?;
    lower(ast)
}

/// Lower a validated tree. Recursion depth is bounded by the preflight.
fn lower(ast: &StmtExpr) -> Result<Expr> {
    match ast {
        StmtExpr::Literal(val) => Ok(Expr::literal(*val)),
        StmtExpr::NodeRef(name) => Ok(Expr::column(name.as_str().to_string())),
        StmtExpr::CsRef {
            component,
            instrument_or_total,
        } => Ok(Expr::cs_ref(component.clone(), instrument_or_total.clone())),
        StmtExpr::BinOp { op, left, right } => Ok(Expr::bin_op(*op, lower(left)?, lower(right)?)),
        StmtExpr::UnaryOp { op, operand } => Ok(Expr::unary_op(*op, lower(operand)?)),
        StmtExpr::Call { func, args } => {
            let signature = FunctionSignature::lookup(func).ok_or_else(|| {
                crate::error::Error::eval(format!("Function '{func}' is not supported"))
            })?;
            let args = args.iter().map(lower).collect::<Result<Vec<_>>>()?;
            Ok(Expr::call(signature.function, args))
        }
        StmtExpr::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => Ok(Expr::if_then_else(
            lower(condition)?,
            lower(then_expr)?,
            lower(else_expr)?,
        )),
    }
}

/// Inputs to unit inference for one model.
#[derive(Clone, Copy)]
pub(crate) struct UnitContext<'a> {
    /// Resolved units of the model's nodes.
    pub(crate) node_types: &'a IndexMap<NodeId, NodeValueType>,
    /// Reporting currency of capital-structure totals, when resolved.
    pub(crate) capital_structure_currency: Option<Currency>,
    /// Native currency of each named instrument.
    pub(crate) instrument_currencies: &'a IndexMap<String, Currency>,
    /// Let an unresolved operand take its partner's units in addition,
    /// subtraction, comparison and selection. Used only while the model's
    /// node types are still being resolved, so a corkscrew such as
    /// `lag(balance, 1) + flow` resolves from `flow`; the final pass checks
    /// every formula with this off.
    pub(crate) provisional: bool,
}

/// Infer a formula's units: `Scalar`, `Monetary(currency)`, or `None` when an
/// operand is unresolved.
///
/// Totals use the reporting currency; named instruments keep their native
/// currencies. No operation converts currency.
///
/// # Errors
///
/// Returns a build error for an operation the unit rules reject, such as
/// adding two currencies, multiplying money by money, or the variance of a
/// monetary series.
pub(crate) fn infer_value_type(
    ast: &StmtExpr,
    context: UnitContext<'_>,
) -> Result<Option<NodeValueType>> {
    Ok(match context.infer(ast)? {
        Dimension::Unknown => None,
        Dimension::Scalar | Dimension::Boolean => Some(NodeValueType::Scalar),
        Dimension::Monetary(currency) => Some(NodeValueType::Monetary { currency }),
    })
}

/// Whether `ast` is a Boolean expression: a comparison, a logical operator
/// or `is_missing`, possibly selected by `if` or `coalesce`.
///
/// # Errors
///
/// Returns a build error for an operation the unit rules reject.
pub(crate) fn is_boolean(ast: &StmtExpr, context: UnitContext<'_>) -> Result<bool> {
    Ok(context.infer(ast)? == Dimension::Boolean)
}

/// Whether `ast` reads a capital-structure reference whose currency is not
/// resolved: a total without a reporting currency, or an unknown instrument.
pub(crate) fn has_unresolved_cs_reference(ast: &StmtExpr, context: UnitContext<'_>) -> bool {
    match ast {
        StmtExpr::CsRef { .. } => matches!(context.infer(ast), Ok(Dimension::Unknown)),
        StmtExpr::Literal(_) | StmtExpr::NodeRef(_) => false,
        StmtExpr::UnaryOp { operand, .. } => has_unresolved_cs_reference(operand, context),
        StmtExpr::BinOp { left, right, .. } => {
            has_unresolved_cs_reference(left, context)
                || has_unresolved_cs_reference(right, context)
        }
        StmtExpr::Call { args, .. } => args
            .iter()
            .any(|arg| has_unresolved_cs_reference(arg, context)),
        StmtExpr::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => [condition, then_expr, else_expr]
            .into_iter()
            .any(|branch| has_unresolved_cs_reference(branch, context)),
    }
}

impl UnitContext<'_> {
    fn infer(self, ast: &StmtExpr) -> Result<Dimension> {
        match ast {
            StmtExpr::Literal(_) => Ok(Dimension::Scalar),
            StmtExpr::NodeRef(name) => Ok(self
                .node_types
                .get(name)
                .map(node_value_type_to_dimension)
                .unwrap_or(Dimension::Unknown)),
            StmtExpr::CsRef {
                instrument_or_total,
                ..
            } => {
                let currency = if instrument_or_total == "total" {
                    self.capital_structure_currency
                } else {
                    self.instrument_currencies.get(instrument_or_total).copied()
                };
                Ok(currency
                    .map(Dimension::Monetary)
                    .unwrap_or(Dimension::Unknown))
            }
            StmtExpr::UnaryOp { op, operand } => {
                let dim = self.infer(operand)?;
                match op {
                    UnaryOp::Neg => {
                        reject_boolean("negation", &[dim])?;
                        Ok(dim)
                    }
                    UnaryOp::Not => require_boolean("logical negation", &[dim]),
                }
            }
            StmtExpr::BinOp { op, left, right } => {
                let left = self.infer(left)?;
                let right = self.infer(right)?;
                self.bin_op(*op, left, right)
            }
            StmtExpr::Call { func, args } => self.call(func, args),
            StmtExpr::IfThenElse {
                condition,
                then_expr,
                else_expr,
            } => {
                let condition = self.infer(condition)?;
                require_boolean("if condition", &[condition])?;
                let then_dim = self.infer(then_expr)?;
                let else_dim = self.infer(else_expr)?;
                self.compatible("if branches", then_dim, else_dim)
            }
        }
    }

    fn bin_op(self, op: BinOp, left: Dimension, right: Dimension) -> Result<Dimension> {
        match op {
            BinOp::Add | BinOp::Sub => {
                reject_boolean("arithmetic operands", &[left, right])?;
                self.compatible("arithmetic operands", left, right)
            }
            BinOp::Mul => {
                reject_boolean("multiplication", &[left, right])?;
                multiply_dimensions(left, right)
            }
            BinOp::Div => {
                reject_boolean("division", &[left, right])?;
                divide_dimensions(left, right)
            }
            BinOp::Mod => require_scalar_arguments("modulo", &[left, right]),
            BinOp::Eq | BinOp::Ne => {
                self.compatible("comparison operands", left, right)?;
                Ok(Dimension::Boolean)
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                reject_boolean("ordered comparison", &[left, right])?;
                self.compatible("comparison operands", left, right)?;
                Ok(Dimension::Boolean)
            }
            BinOp::And | BinOp::Or => require_boolean("logical operands", &[left, right]),
        }
    }

    /// Units of operands that must match: addition, subtraction, comparison,
    /// selection and same-unit aggregates.
    fn compatible(self, context: &str, left: Dimension, right: Dimension) -> Result<Dimension> {
        match (left, right) {
            (Dimension::Unknown, known) | (known, Dimension::Unknown) => Ok(if self.provisional {
                known
            } else {
                Dimension::Unknown
            }),
            (Dimension::Scalar, Dimension::Scalar) => Ok(Dimension::Scalar),
            (Dimension::Boolean, Dimension::Boolean) => Ok(Dimension::Boolean),
            (Dimension::Boolean, _) | (_, Dimension::Boolean) => Err(crate::error::Error::build(
                format!("Dimensional mismatch in {context}: cannot combine a Boolean and a number"),
            )),
            (Dimension::Monetary(lhs), Dimension::Monetary(rhs)) if lhs == rhs => {
                Ok(Dimension::Monetary(lhs))
            }
            (Dimension::Monetary(lhs), Dimension::Monetary(rhs)) => {
                Err(crate::error::Error::build(format!(
                    "Dimensional mismatch in {context}: cannot combine {lhs} and {rhs}"
                )))
            }
            (Dimension::Scalar, Dimension::Monetary(ccy))
            | (Dimension::Monetary(ccy), Dimension::Scalar) => Err(crate::error::Error::build(
                format!("Dimensional mismatch in {context}: cannot combine scalar and {ccy}"),
            )),
        }
    }

    fn call(self, func: &str, args: &[StmtExpr]) -> Result<Dimension> {
        let arg_dims: Vec<_> = args
            .iter()
            .map(|arg| self.infer(arg))
            .collect::<Result<Vec<_>>>()?;
        let auxiliary_dims = arg_dims.get(1..).unwrap_or_default();
        // `coalesce` may select between Boolean alternatives and `is_missing`
        // may test one. Every other function takes numbers.
        if !matches!(func, "coalesce" | "is_missing") {
            reject_boolean(func, &arg_dims)?;
        }
        match func {
            // Value-variadic: every argument is a value carried in the same
            // units, so a mismatch between them is a modelling error.
            // `mean`/`sum`/`min`/`max` aggregate across all of their
            // arguments, `coalesce` picks between interchangeable
            // alternatives, and `clamp`'s bounds are compared against its
            // value.
            "mean" | "sum" | "min" | "max" | "coalesce" | "clamp" => {
                let mut dims = arg_dims.into_iter();
                let Some(first) = dims.next() else {
                    return Ok(Dimension::Unknown);
                };
                dims.try_fold(first, |acc, dim| self.compatible(func, acc, dim))
            }
            "quantile" => {
                require_scalar_arguments("quantile level", auxiliary_dims)?;
                Ok(first_arg_dimension(&arg_dims))
            }
            "rank" => {
                require_scalar_arguments("rank ascending flag", auxiliary_dims)?;
                Ok(Dimension::Scalar)
            }
            // Level transforms and same-unit aggregates carry the units of
            // their first (series or value) argument, including a monetary
            // standard deviation. An argument that is a count, window size,
            // period offset, smoothing factor, digit count or month number
            // must be dimensionless and never unifies with the first.
            "abs" | "lag" | "shift" | "cumsum" | "cummin" | "cummax" | "diff" | "rolling_mean"
            | "rolling_sum" | "rolling_min" | "rolling_max" | "rolling_median" | "rolling_std"
            | "ewm_mean" | "ewm_std" | "median" | "std" | "ttm" | "ytd" | "qtd" | "fiscal_ytd"
            | "annualize" | "round" | "floor" | "ceil" => {
                require_scalar_arguments(func, auxiliary_dims)?;
                let first = first_arg_dimension(&arg_dims);
                // Rounding money needs its decimal places stated, which only
                // two-argument `round` can do.
                if matches!(first, Dimension::Monetary(_))
                    && (matches!(func, "floor" | "ceil") || (func == "round" && args.len() == 1))
                {
                    return Err(crate::error::Error::build(format!(
                        "Unsupported units in {func}: rounding money needs an explicit number \
                         of decimal places; use round(value, digits)"
                    )));
                }
                Ok(first)
            }
            // Ratios, counts, signs and predicates remove the leading
            // argument's units. Their offsets, windows and flags are scalar.
            "sign" | "pct_change" | "rolling_count" | "growth_rate" => {
                require_scalar_arguments(func, auxiliary_dims)?;
                Ok(Dimension::Scalar)
            }
            "is_missing" => Ok(Dimension::Boolean),
            // Products, rate compounding and nonlinear math cannot turn money
            // into a dimensionless output by dropping its currency tag.
            "cumprod" | "annualize_rate" | "pow" | "ln" | "exp" | "log10" | "sqrt" => {
                require_scalar_arguments(func, &arg_dims)
            }
            // Variance has squared units. Squared currency is not a supported
            // dimension, so the variance of money is rejected, not erased.
            "var" | "rolling_var" | "ewm_var" => {
                require_scalar_arguments(func, auxiliary_dims)?;
                match first_arg_dimension(&arg_dims) {
                    Dimension::Monetary(currency) => Err(crate::error::Error::build(format!(
                        "Unsupported units in {func}: the variance of a {currency} series has \
                         squared-currency units; use the standard deviation, or divide by a \
                         same-currency base first"
                    ))),
                    resolved => Ok(resolved),
                }
            }
            _ => Err(crate::error::Error::build(format!(
                "Function '{func}' has no unit rule"
            ))),
        }
    }
}

fn node_value_type_to_dimension(value_type: &NodeValueType) -> Dimension {
    match value_type {
        NodeValueType::Monetary { currency } => Dimension::Monetary(*currency),
        NodeValueType::Scalar => Dimension::Scalar,
    }
}

fn multiply_dimensions(left: Dimension, right: Dimension) -> Result<Dimension> {
    match (left, right) {
        (Dimension::Monetary(lhs), Dimension::Monetary(rhs)) => Err(crate::error::Error::build(
            format!("Dimensional mismatch in multiplication: cannot multiply {lhs} by {rhs}"),
        )),
        (Dimension::Unknown, _) | (_, Dimension::Unknown) => Ok(Dimension::Unknown),
        (Dimension::Scalar, Dimension::Monetary(ccy))
        | (Dimension::Monetary(ccy), Dimension::Scalar) => Ok(Dimension::Monetary(ccy)),
        // Booleans are rejected before this rule runs.
        _ => Ok(Dimension::Scalar),
    }
}

fn divide_dimensions(left: Dimension, right: Dimension) -> Result<Dimension> {
    match (left, right) {
        (Dimension::Scalar, Dimension::Monetary(ccy)) => Err(crate::error::Error::build(format!(
            "Dimensional mismatch in division: cannot divide scalar by {ccy}"
        ))),
        (Dimension::Monetary(lhs), Dimension::Monetary(rhs)) if lhs != rhs => {
            Err(crate::error::Error::build(format!(
                "Dimensional mismatch in division: cannot divide {lhs} by {rhs}"
            )))
        }
        (Dimension::Unknown, _) | (_, Dimension::Unknown) => Ok(Dimension::Unknown),
        (Dimension::Monetary(ccy), Dimension::Scalar) => Ok(Dimension::Monetary(ccy)),
        // Booleans are rejected before this rule runs.
        _ => Ok(Dimension::Scalar),
    }
}

/// Reject a Boolean where a number is required.
fn reject_boolean(context: &str, dimensions: &[Dimension]) -> Result<()> {
    if dimensions.contains(&Dimension::Boolean) {
        return Err(crate::error::Error::build(format!(
            "Type mismatch in {context}: a Boolean is not a number; select a number with if()"
        )));
    }
    Ok(())
}

/// Operands that must be Boolean: the condition of `if` and the operands of
/// `and`, `or` and `not`.
fn require_boolean(context: &str, dimensions: &[Dimension]) -> Result<Dimension> {
    if dimensions
        .iter()
        .any(|dimension| matches!(dimension, Dimension::Scalar | Dimension::Monetary(_)))
    {
        return Err(crate::error::Error::build(format!(
            "Type mismatch in {context}: operands must be Boolean (a comparison, a logical \
             operator or is_missing)"
        )));
    }
    Ok(if dimensions.contains(&Dimension::Unknown) {
        Dimension::Unknown
    } else {
        Dimension::Boolean
    })
}

fn require_scalar_arguments(context: &str, dimensions: &[Dimension]) -> Result<Dimension> {
    reject_boolean(context, dimensions)?;
    // An unresolved argument cannot excuse a known monetary argument, nor can
    // it prove that the resulting expression is dimensionless.
    if dimensions
        .iter()
        .any(|dimension| matches!(dimension, Dimension::Monetary(_)))
    {
        return Err(crate::error::Error::build(format!(
            "Dimensional mismatch in {context}: operands must be scalar"
        )));
    }
    Ok(if dimensions.contains(&Dimension::Unknown) {
        Dimension::Unknown
    } else {
        Dimension::Scalar
    })
}

/// Dimension of a call whose units come from its leading argument alone.
fn first_arg_dimension(arg_dims: &[Dimension]) -> Dimension {
    arg_dims.first().copied().unwrap_or(Dimension::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::parse_formula;

    fn infer_dimension(
        ast: &StmtExpr,
        node_types: &IndexMap<NodeId, NodeValueType>,
        capital_structure_currency: Option<Currency>,
        instrument_currencies: &IndexMap<String, Currency>,
    ) -> Result<Dimension> {
        UnitContext {
            node_types,
            capital_structure_currency,
            instrument_currencies,
            provisional: false,
        }
        .infer(ast)
    }

    fn value_type(
        ast: &StmtExpr,
        node_types: &IndexMap<NodeId, NodeValueType>,
        capital_structure_currency: Option<Currency>,
        instrument_currencies: &IndexMap<String, Currency>,
    ) -> Result<Option<NodeValueType>> {
        infer_value_type(
            ast,
            UnitContext {
                node_types,
                capital_structure_currency,
                instrument_currencies,
                provisional: false,
            },
        )
    }

    fn validate_dimensions(
        ast: &StmtExpr,
        node_types: &IndexMap<NodeId, NodeValueType>,
    ) -> Result<()> {
        infer_dimension(ast, node_types, None, &IndexMap::new()).map(|_| ())
    }
    use finstack_quant_core::expr::{ExprNode, Function};

    #[test]
    fn test_compile_literal() {
        let ast = StmtExpr::literal(42.0);
        let expr = compile(&ast).expect("should compile successfully");

        match expr.node {
            ExprNode::Literal(v) => assert_eq!(v, 42.0),
            _ => panic!("Expected Literal"),
        }
    }

    #[test]
    fn test_compile_node_ref() {
        let ast = StmtExpr::node_ref("revenue");
        let expr = compile(&ast).expect("should compile successfully");

        match expr.node {
            ExprNode::Column(ref name) => assert_eq!(name, "revenue"),
            _ => panic!("Expected Column"),
        }
    }

    #[test]
    fn test_compile_addition() {
        let ast = StmtExpr::bin_op(BinOp::Add, StmtExpr::literal(1.0), StmtExpr::literal(2.0));

        let expr = compile(&ast).expect("should compile successfully");

        // Should compile to a BinOp expression
        match expr.node {
            ExprNode::BinOp { .. } => {}
            _ => panic!("Expected BinOp for arithmetic"),
        }
    }

    #[test]
    fn test_compile_function_lag() {
        let ast = StmtExpr::call(
            "lag",
            vec![StmtExpr::node_ref("revenue"), StmtExpr::literal(1.0)],
        );

        let expr = compile(&ast).expect("should compile successfully");

        match expr.node {
            ExprNode::Call(Function::Lag, args) => {
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected Lag function call"),
        }
    }

    #[test]
    fn test_compile_from_parse() {
        let ast = parse_formula("revenue - cogs").expect("should parse successfully");
        let expr = compile(&ast).expect("should compile successfully");

        // Should compile successfully to a BinOp
        match expr.node {
            ExprNode::BinOp { .. } => {}
            _ => panic!("Expected BinOp for subtraction"),
        }
    }

    #[test]
    fn test_compile_complex_expression() {
        let ast = parse_formula("(revenue - cogs) / revenue").expect("should parse successfully");
        let expr = compile(&ast);

        assert!(expr.is_ok());
    }

    /// Rolling functions accept an optional third `min_periods` argument
    /// (pandas parity); one or four-plus arguments stay rejected.
    #[test]
    fn rolling_functions_accept_optional_min_periods() {
        for formula in ["rolling_mean(x, 4)", "rolling_mean(x, 4, 2)"] {
            let ast = parse_formula(formula).expect("should parse");
            assert!(compile(&ast).is_ok(), "should compile: {formula}");
        }
        for formula in ["rolling_mean(x)", "rolling_mean(x, 4, 2, 1)"] {
            let ast = parse_formula(formula).expect("should parse");
            assert!(compile(&ast).is_err(), "should reject: {formula}");
        }
    }

    /// Element-wise math helpers compile with the expected arities, and the
    /// dimension checker treats `round`'s `digits` argument as a unit-free
    /// count while still folding `clamp`'s bounds with its value.
    #[test]
    fn elementwise_math_arities_and_dimensions() {
        for formula in [
            "pow(1 + growth, 4)",
            "round(revenue, 2)",
            "round(revenue)",
            "floor(revenue)",
            "ceil(revenue)",
            "ln(revenue)",
            "exp(growth)",
            "log10(revenue)",
            "sqrt(revenue)",
            "clamp(margin, 0, 1)",
            "is_missing(revenue)",
        ] {
            let ast = parse_formula(formula).expect("should parse");
            assert!(compile(&ast).is_ok(), "should compile: {formula}");
        }

        let usd = NodeValueType::Monetary {
            currency: finstack_quant_core::currency::Currency::USD,
        };
        let eur = NodeValueType::Monetary {
            currency: finstack_quant_core::currency::Currency::EUR,
        };
        let mut node_types = IndexMap::new();
        node_types.insert(NodeId::new("usd_amount"), usd);
        node_types.insert(NodeId::new("eur_amount"), eur);

        // `round(monetary, digits)` must not be rejected as a monetary/scalar
        // mix: the digits argument is a dimensionless count.
        let ast = parse_formula("round(usd_amount, 2) + usd_amount").expect("should parse");
        assert!(validate_dimensions(&ast, &node_types).is_ok());

        // `clamp` folds value and bounds: mixing currencies is an error.
        let ast = parse_formula("clamp(usd_amount, eur_amount, usd_amount)").expect("should parse");
        assert!(validate_dimensions(&ast, &node_types).is_err());

        // ...while same-currency bounds are fine and preserve the unit.
        let ast = parse_formula("clamp(usd_amount, 0 * usd_amount, usd_amount) + usd_amount")
            .expect("should parse");
        assert!(validate_dimensions(&ast, &node_types).is_ok());
    }

    /// Windowed formulas take a dimensionless count, window or offset as their
    /// trailing argument, so the result must simply carry the series
    /// argument's dimension. Folding the count in rejected every windowed
    /// formula over a monetary node ("cannot combine scalar and USD"), which
    /// made a debt corkscrew (`lag(debt_balance, 1)`) impossible to express.
    #[test]
    fn windowed_functions_preserve_series_dimension() {
        let mut node_types = IndexMap::new();
        node_types.insert(
            NodeId::new("usd_amount"),
            NodeValueType::Monetary {
                currency: finstack_quant_core::currency::Currency::USD,
            },
        );
        node_types.insert(NodeId::new("ratio"), NodeValueType::Scalar);

        for formula in [
            "lag(usd_amount, 1)",
            "shift(usd_amount, 1)",
            "rolling_sum(usd_amount, 4)",
            "rolling_mean(usd_amount, 4, 2)",
            "ewm_mean(usd_amount, 0.5)",
            "diff(usd_amount, 1)",
            "fiscal_ytd(usd_amount, 4)",
            "annualize(usd_amount, 4)",
        ] {
            let ast = parse_formula(formula).expect("should parse");
            assert_eq!(
                infer_dimension(&ast, &node_types, None, &IndexMap::new()).expect("should infer"),
                Dimension::Monetary(finstack_quant_core::currency::Currency::USD),
                "{formula} must carry its series argument's currency"
            );
        }

        for formula in ["lag(ratio, 1)", "shift(ratio, 1)", "rolling_sum(ratio, 4)"] {
            let ast = parse_formula(formula).expect("should parse");
            assert_eq!(
                infer_dimension(&ast, &node_types, None, &IndexMap::new()).expect("should infer"),
                Dimension::Scalar,
                "{formula} must stay scalar"
            );
        }

        // The preserved dimension must still combine downstream: adding a
        // lagged USD balance to a USD flow is fine, mixing currencies is not.
        node_types.insert(
            NodeId::new("eur_amount"),
            NodeValueType::Monetary {
                currency: finstack_quant_core::currency::Currency::EUR,
            },
        );
        let ast = parse_formula("lag(usd_amount, 1) + usd_amount").expect("should parse");
        assert!(validate_dimensions(&ast, &node_types).is_ok());
        let ast = parse_formula("lag(usd_amount, 1) + eur_amount").expect("should parse");
        assert!(validate_dimensions(&ast, &node_types).is_err());
    }

    /// The first-argument rule must not widen into a hole: `clamp` and
    /// `coalesce` take genuine quantities in every position, so a currency
    /// mismatch between them stays an error.
    #[test]
    fn value_variadic_functions_still_reject_currency_mismatch() {
        let mut node_types = IndexMap::new();
        node_types.insert(
            NodeId::new("usd_amount"),
            NodeValueType::Monetary {
                currency: finstack_quant_core::currency::Currency::USD,
            },
        );
        node_types.insert(
            NodeId::new("eur_amount"),
            NodeValueType::Monetary {
                currency: finstack_quant_core::currency::Currency::EUR,
            },
        );

        for formula in [
            "clamp(usd_amount, eur_amount, usd_amount)",
            "coalesce(usd_amount, eur_amount)",
            "min(usd_amount, eur_amount)",
            "sum(usd_amount, eur_amount)",
        ] {
            let ast = parse_formula(formula).expect("should parse");
            assert!(
                validate_dimensions(&ast, &node_types).is_err(),
                "{formula} mixes currencies and must be rejected"
            );
        }

        // Same-currency arguments still resolve to that currency.
        let ast = parse_formula("coalesce(lag(usd_amount, 1), usd_amount)").expect("should parse");
        assert_eq!(
            infer_dimension(&ast, &node_types, None, &IndexMap::new()).expect("should infer"),
            Dimension::Monetary(finstack_quant_core::currency::Currency::USD),
        );
    }

    #[test]
    fn capital_structure_dimensions_distinguish_totals_from_native_instruments() {
        let node_types = IndexMap::from([
            (
                NodeId::new("usd"),
                NodeValueType::Monetary {
                    currency: Currency::USD,
                },
            ),
            (
                NodeId::new("eur"),
                NodeValueType::Monetary {
                    currency: Currency::EUR,
                },
            ),
        ]);
        let instrument_currencies = IndexMap::from([("euro".to_owned(), Currency::EUR)]);
        for component in [
            "interest_expense",
            "interest_expense_cash",
            "interest_expense_pik",
            "interest_income",
            "principal_payment",
            "debt_balance",
            "fees",
            "accrued_interest",
        ] {
            for (reference, expected) in [("total", Currency::USD), ("euro", Currency::EUR)] {
                let ast = parse_formula(&format!("lag(cs.{component}.{reference}, 1)"))
                    .expect("valid capital-structure reference");
                assert_eq!(
                    value_type(
                        &ast,
                        &node_types,
                        Some(Currency::USD),
                        &instrument_currencies,
                    )
                    .expect("reference currency"),
                    Some(NodeValueType::Monetary { currency: expected }),
                );
            }
        }
        for formula in [
            "cs.interest_expense.euro == usd",
            "cs.interest_expense.total == eur",
            "cs.debt_balance.euro + cs.debt_balance.total",
        ] {
            let ast = parse_formula(formula).expect("valid formula");
            assert!(
                value_type(
                    &ast,
                    &node_types,
                    Some(Currency::USD),
                    &instrument_currencies,
                )
                .is_err(),
                "{formula} must reject EUR/USD operations"
            );
        }
        let ast = parse_formula("cs.debt_balance.unknown").expect("valid reference");
        assert_eq!(
            value_type(
                &ast,
                &node_types,
                Some(Currency::USD),
                &instrument_currencies,
            )
            .expect("unresolved reference"),
            None,
            "an unknown instrument must never inherit the reporting currency"
        );
    }

    // Regression (C8): `min`/`max` now compile to a single n-ary
    // `Function::Min`/`Max` node rather than a nested if-then-else tree whose
    // size doubled per argument (an O(2^n) memory-exhaustion DoS reachable from
    // any inbound formula). A flat `min` over many arguments must compile to one
    // small call node, not an exponential tree.
    #[test]
    fn test_minmax_compiles_to_single_narity_node_not_exponential_tree() {
        let args = (0..40).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let formula = format!("min({args})");
        let ast = parse_formula(&formula).expect("should parse");
        let expr = compile(&ast).expect("should compile");
        match &expr.node {
            ExprNode::Call(Function::Min, call_args) => {
                assert_eq!(call_args.len(), 40, "all 40 args preserved on one node");
            }
            other => panic!("expected a single Function::Min call node, got {other:?}"),
        }
    }

    /// Every function in the signature table has a unit rule, so no call can
    /// leave a formula's units unresolved.
    #[test]
    fn every_function_has_a_unit_rule() {
        use crate::dsl::signatures::FUNCTION_SIGNATURES;

        let node_types = IndexMap::from([(NodeId::new("x"), NodeValueType::Scalar)]);
        for signature in FUNCTION_SIGNATURES {
            let args = vec![StmtExpr::node_ref("x"); signature.min_args];
            let dimension = infer_dimension(
                &StmtExpr::call(signature.name, args),
                &node_types,
                None,
                &IndexMap::new(),
            )
            .unwrap_or_else(|error| panic!("{}: {error}", signature.name));
            let expected = if signature.name == "is_missing" {
                Dimension::Boolean
            } else {
                Dimension::Scalar
            };
            assert_eq!(dimension, expected, "{}", signature.name);
        }
    }
}
