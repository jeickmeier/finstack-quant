//! Typed formula values and the checked scalar operations over them.
//!
//! A formula evaluates to a [`Value`]: a finite scalar, an exact amount of
//! money, a Boolean, or an unavailable result with its reason. Invalid
//! arithmetic (an undefined domain, division or remainder by zero, overflow,
//! mixed units) is an error naming the node and period. An unavailable operand
//! makes the result unavailable; it is never read as zero or as false.
//!
//! [`Value::Number`] is the one untyped state: a finite number whose units
//! static analysis fixed but the value does not carry. History kernels and
//! capital-structure references produce it, and a column read from a
//! float-only context does. It adopts the currency of a monetary peer in
//! addition, subtraction and comparison, and the units of the node at the
//! cell boundary.

use crate::error::{Error, Result};
use crate::evaluator::cell::{Cell, UnavailableReason};
use crate::evaluator::formula::eval_error;
use finstack_quant_core::config::RoundingMode;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::expr::BinOp;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::money::Money;
use std::cmp::Ordering;

/// The result of evaluating one formula expression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Value {
    /// A finite unitless number.
    Scalar(f64),
    /// An exact amount of money.
    Money(Money),
    /// A finite number whose units the value does not carry.
    Number(f64),
    /// The result of a comparison, a logical operator or `is_missing`.
    Bool(bool),
    /// No value, for the stated reason.
    Unavailable(UnavailableReason),
}

/// Where a formula is being evaluated, for error messages.
#[derive(Debug, Clone, Copy)]
pub(crate) struct At<'a> {
    /// Node that owns the formula, when there is one.
    pub(crate) node_id: Option<&'a str>,
    /// Period being evaluated.
    pub(crate) period: PeriodId,
}

impl At<'_> {
    pub(crate) fn error(self, message: impl std::fmt::Display) -> Error {
        eval_error(self.node_id, format!("{message} in period {}", self.period))
    }

    fn core<T>(self, result: finstack_quant_core::Result<T>, what: &str) -> Result<T> {
        result.map_err(|cause| self.error(format!("{what} failed ({cause})")))
    }

    /// A finite result, or an error for an overflow or undefined operation.
    pub(crate) fn finite(self, value: f64, what: &str) -> Result<f64> {
        if value.is_finite() {
            Ok(value)
        } else {
            Err(self.error(format!("{what} is undefined or overflows")))
        }
    }
}

impl Value {
    /// The value a cell supplies to a formula. A missing or masked cell is an
    /// unavailable input.
    pub(crate) fn from_cell(cell: Cell) -> Self {
        match cell {
            Cell::Scalar(value) => Self::Scalar(value),
            Cell::Money(money) => Self::Money(money),
            Cell::Missing | Cell::Masked => Self::Unavailable(UnavailableReason::MissingInput),
            Cell::Unavailable(reason) => Self::Unavailable(reason),
        }
    }

    /// The value of a float from a history kernel or a float-only context,
    /// where a non-finite float means "no value".
    pub(crate) fn from_float(value: f64) -> Self {
        if value.is_finite() {
            Self::Number(value)
        } else {
            Self::Unavailable(UnavailableReason::MissingInput)
        }
    }

    /// The float view read by history kernels: `NaN` for an unavailable value,
    /// `1.0` / `0.0` for a Boolean, and the nearest float for money.
    pub(crate) fn float(self) -> f64 {
        match self {
            Self::Scalar(value) | Self::Number(value) => value,
            Self::Money(money) => money.amount(),
            Self::Bool(flag) => f64::from(u8::from(flag)),
            Self::Unavailable(_) => f64::NAN,
        }
    }

    pub(crate) fn is_unavailable(self) -> bool {
        matches!(self, Self::Unavailable(_))
    }

    fn describe(self) -> String {
        match self {
            Self::Scalar(_) | Self::Number(_) => "a number".to_string(),
            Self::Money(money) => format!("{} money", money.currency()),
            Self::Bool(_) => "a Boolean".to_string(),
            Self::Unavailable(_) => "an unavailable value".to_string(),
        }
    }
}

/// The first unavailable operand, which an operation propagates unchanged.
pub(crate) fn first_unavailable(values: &[Value]) -> Option<Value> {
    values.iter().copied().find(|value| value.is_unavailable())
}

/// A numeric operand: Booleans and unavailable values are excluded.
#[derive(Clone, Copy)]
enum Num {
    Scalar(f64),
    Number(f64),
    Money(Money),
}

impl Num {
    fn float(self) -> f64 {
        match self {
            Self::Scalar(value) | Self::Number(value) => value,
            Self::Money(money) => money.amount(),
        }
    }
}

fn numeric(value: Value, what: &str, at: At<'_>) -> Result<Num> {
    match value {
        Value::Scalar(value) => Ok(Num::Scalar(value)),
        Value::Number(value) => Ok(Num::Number(value)),
        Value::Money(money) => Ok(Num::Money(money)),
        Value::Bool(_) | Value::Unavailable(_) => Err(at.error(format!(
            "{what} needs numeric operands, got {}",
            value.describe()
        ))),
    }
}

/// The number as money in `currency`, at the floating-point boundary.
fn adopt(value: f64, peer: Money, at: At<'_>) -> Result<Money> {
    at.core(
        Money::from_model_value(value, peer.currency()),
        "converting a number to money",
    )
}

/// Float result of two operands when neither is typed money on both sides:
/// a scalar when both are scalars, otherwise an untyped number.
fn float_result(left: Num, right: Num, value: f64) -> Value {
    if matches!((left, right), (Num::Scalar(_), Num::Scalar(_))) {
        Value::Scalar(value)
    } else {
        Value::Number(value)
    }
}

/// Checked `+`, `-`, `*`, `/` and `%`.
///
/// # Errors
///
/// Returns an evaluation error for a Boolean operand, mixed scalar and
/// monetary units, mixed currencies, division or remainder by zero, and a
/// result that overflows.
pub(crate) fn arithmetic(op: BinOp, left: Value, right: Value, at: At<'_>) -> Result<Value> {
    if let Some(unavailable) = first_unavailable(&[left, right]) {
        return Ok(unavailable);
    }
    let what = match op {
        BinOp::Add => "addition",
        BinOp::Sub => "subtraction",
        BinOp::Mul => "multiplication",
        BinOp::Div => "division",
        _ => "remainder",
    };
    let (l, r) = (numeric(left, what, at)?, numeric(right, what, at)?);
    let mixed = || {
        at.error(format!(
            "{what} cannot combine {} and {}",
            left.describe(),
            right.describe()
        ))
    };
    match op {
        BinOp::Add | BinOp::Sub => {
            let money = |a: Money, b: Money| {
                at.core(
                    if op == BinOp::Add {
                        a.checked_add(b)
                    } else {
                        a.checked_sub(b)
                    },
                    what,
                )
                .map(Value::Money)
            };
            match (l, r) {
                (Num::Money(a), Num::Money(b)) => money(a, b),
                (Num::Money(a), Num::Number(b)) => money(a, adopt(b, a, at)?),
                (Num::Number(a), Num::Money(b)) => money(adopt(a, b, at)?, b),
                (Num::Money(_), Num::Scalar(_)) | (Num::Scalar(_), Num::Money(_)) => Err(mixed()),
                _ => {
                    let (a, b) = (l.float(), r.float());
                    let value = if op == BinOp::Add { a + b } else { a - b };
                    Ok(float_result(l, r, at.finite(value, what)?))
                }
            }
        }
        BinOp::Mul => match (l, r) {
            (Num::Money(_), Num::Money(_)) => Err(mixed()),
            (Num::Money(money), factor) | (factor, Num::Money(money)) => at
                .core(money.checked_mul_f64(factor.float()), what)
                .map(Value::Money),
            _ => Ok(float_result(l, r, at.finite(l.float() * r.float(), what)?)),
        },
        BinOp::Div => {
            if r.float() == 0.0 {
                return Err(at.error("division by zero"));
            }
            match (l, r) {
                (Num::Scalar(_), Num::Money(_)) => Err(mixed()),
                (Num::Money(a), Num::Money(b)) if a.currency() != b.currency() => Err(mixed()),
                (Num::Money(a), Num::Money(b)) => {
                    Ok(Value::Scalar(at.finite(a.amount() / b.amount(), what)?))
                }
                (Num::Money(money), Num::Scalar(divisor)) => at
                    .core(money.checked_div_f64(divisor), what)
                    .map(Value::Money),
                // Money over an untyped number is money or a ratio; static
                // analysis decides, so the quotient stays untyped.
                _ => Ok(float_result(l, r, at.finite(l.float() / r.float(), what)?)),
            }
        }
        _ => {
            if matches!(l, Num::Money(_)) || matches!(r, Num::Money(_)) {
                return Err(mixed());
            }
            if r.float() == 0.0 {
                return Err(at.error("remainder by zero"));
            }
            Ok(float_result(l, r, at.finite(l.float() % r.float(), what)?))
        }
    }
}

/// Exact ordering of two available numeric values in the same units.
pub(crate) fn order(left: Value, right: Value, what: &str, at: At<'_>) -> Result<Ordering> {
    let (l, r) = (numeric(left, what, at)?, numeric(right, what, at)?);
    let money = |a: Money, b: Money| at.core(a.checked_cmp(b), what);
    match (l, r) {
        (Num::Money(a), Num::Money(b)) => money(a, b),
        (Num::Money(a), Num::Number(b)) => money(a, adopt(b, a, at)?),
        (Num::Number(a), Num::Money(b)) => money(adopt(a, b, at)?, b),
        (Num::Money(_), Num::Scalar(_)) | (Num::Scalar(_), Num::Money(_)) => {
            Err(at.error(format!(
                "{what} cannot combine {} and {}",
                left.describe(),
                right.describe()
            )))
        }
        _ => l
            .float()
            .partial_cmp(&r.float())
            .ok_or_else(|| at.error(format!("{what} of an undefined number"))),
    }
}

/// `==`, `!=`, `<`, `<=`, `>` and `>=` at the represented values: scalars as
/// stored, same-currency money as exact decimals. No tolerance is applied.
///
/// # Errors
///
/// Returns an evaluation error for mixed units or currencies, and for a
/// Boolean compared with anything but a Boolean under `==` or `!=`.
pub(crate) fn compare(op: BinOp, left: Value, right: Value, at: At<'_>) -> Result<Value> {
    if let Some(unavailable) = first_unavailable(&[left, right]) {
        return Ok(unavailable);
    }
    if let (Value::Bool(a), Value::Bool(b)) = (left, right) {
        return match op {
            BinOp::Eq => Ok(Value::Bool(a == b)),
            BinOp::Ne => Ok(Value::Bool(a != b)),
            _ => Err(at.error("Booleans compare only with == and !=")),
        };
    }
    let ordering = order(left, right, "comparison", at)?;
    Ok(Value::Bool(match op {
        BinOp::Eq => ordering == Ordering::Equal,
        BinOp::Ne => ordering != Ordering::Equal,
        BinOp::Lt => ordering == Ordering::Less,
        BinOp::Le => ordering != Ordering::Greater,
        BinOp::Gt => ordering == Ordering::Greater,
        _ => ordering != Ordering::Less,
    }))
}

/// A Boolean control: `Some(flag)`, or `None` when the control is unavailable.
///
/// # Errors
///
/// Returns an evaluation error when the control is a number or money.
pub(crate) fn control(value: Value, what: &str, at: At<'_>) -> Result<Option<bool>> {
    match value {
        Value::Bool(flag) => Ok(Some(flag)),
        Value::Unavailable(_) => Ok(None),
        other => Err(at.error(format!(
            "{what} must be a Boolean (a comparison, a logical operator or is_missing), got {}",
            other.describe()
        ))),
    }
}

/// Unary minus, exact for money.
pub(crate) fn negate(value: Value, at: At<'_>) -> Result<Value> {
    match value {
        Value::Unavailable(_) => Ok(value),
        other => Ok(match numeric(other, "negation", at)? {
            Num::Scalar(value) => Value::Scalar(-value),
            Num::Number(value) => Value::Number(-value),
            Num::Money(money) => Value::Money(money.checked_neg()),
        }),
    }
}

/// `abs(value)`, exact for money.
pub(crate) fn abs(value: Value, at: At<'_>) -> Result<Value> {
    match value {
        Value::Unavailable(_) => Ok(value),
        other => Ok(match numeric(other, "abs()", at)? {
            Num::Scalar(value) => Value::Scalar(value.abs()),
            Num::Number(value) => Value::Number(value.abs()),
            Num::Money(money) => Value::Money(money.abs()),
        }),
    }
}

/// `sign(value)`: `-1`, `0` or `1` as a scalar.
pub(crate) fn sign(value: Value, at: At<'_>) -> Result<Value> {
    match value {
        Value::Unavailable(_) => Ok(value),
        other => {
            let float = numeric(other, "sign()", at)?.float();
            Ok(Value::Scalar(if float > 0.0 {
                1.0
            } else if float < 0.0 {
                -1.0
            } else {
                0.0
            }))
        }
    }
}

/// A unitless argument of a scalar-only function: `None` when unavailable.
///
/// # Errors
///
/// Returns an evaluation error for money or a Boolean.
pub(crate) fn scalar_argument(value: Value, what: &str, at: At<'_>) -> Result<Option<f64>> {
    match value {
        Value::Scalar(value) | Value::Number(value) => Ok(Some(value)),
        Value::Unavailable(_) => Ok(None),
        other => Err(at.error(format!(
            "{what} needs a unitless number, got {}",
            other.describe()
        ))),
    }
}

/// The scalar result of a float computation, keeping an untyped input untyped.
pub(crate) fn like(input: Value, value: f64) -> Value {
    if matches!(input, Value::Number(_)) {
        Value::Number(value)
    } else {
        Value::Scalar(value)
    }
}

/// Which n-ary aggregate to compute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aggregate {
    /// Sum of all operands.
    Sum,
    /// Arithmetic mean of all operands.
    Mean,
    /// Smallest operand.
    Min,
    /// Largest operand.
    Max,
}

/// `sum`, `mean`, `min` and `max` over operands that must all be available
/// and in the same units. Scalars use Neumaier compensated summation; money is
/// exact.
///
/// # Errors
///
/// Returns an evaluation error for a Boolean operand, mixed units or
/// currencies, and a result that overflows.
pub(crate) fn aggregate(kind: Aggregate, values: &[Value], at: At<'_>) -> Result<Value> {
    if let Some(unavailable) = first_unavailable(values) {
        return Ok(unavailable);
    }
    let what = match kind {
        Aggregate::Sum => "sum()",
        Aggregate::Mean => "mean()",
        Aggregate::Min => "min()",
        Aggregate::Max => "max()",
    };
    let Some((&first, rest)) = values.split_first() else {
        return Err(at.error(format!("{what} needs at least one operand")));
    };
    match kind {
        Aggregate::Min | Aggregate::Max => rest.iter().try_fold(first, |best, &next| {
            let ordering = order(next, best, what, at)?;
            let replace = if kind == Aggregate::Min {
                ordering == Ordering::Less
            } else {
                ordering == Ordering::Greater
            };
            Ok(if replace { next } else { best })
        }),
        Aggregate::Sum | Aggregate::Mean => {
            let nums = values
                .iter()
                .map(|&value| numeric(value, what, at))
                .collect::<Result<Vec<_>>>()?;
            let peer = nums.iter().find_map(|num| match num {
                Num::Money(money) => Some(*money),
                _ => None,
            });
            let Some(peer) = peer else {
                let total = at.finite(
                    at.core(neumaier_sum(nums.iter().map(|num| num.float())), what)?,
                    what,
                )?;
                let value = if kind == Aggregate::Mean {
                    total / nums.len() as f64
                } else {
                    total
                };
                let all_scalar = nums.iter().all(|num| matches!(num, Num::Scalar(_)));
                return Ok(if all_scalar {
                    Value::Scalar(value)
                } else {
                    Value::Number(value)
                });
            };
            let mut total = Money::zero(peer.currency());
            for (num, value) in nums.iter().zip(values) {
                let amount = match num {
                    Num::Money(money) => *money,
                    Num::Number(number) => adopt(*number, peer, at)?,
                    Num::Scalar(_) => {
                        return Err(at.error(format!(
                            "{what} cannot combine {} and {}",
                            value.describe(),
                            Value::Money(peer).describe()
                        )));
                    }
                };
                total = at.core(total.checked_add(amount), what)?;
            }
            if kind == Aggregate::Mean {
                total = at.core(
                    total.checked_div(rust_decimal::Decimal::from(nums.len())),
                    what,
                )?;
            }
            Ok(Value::Money(total))
        }
    }
}

/// `round(value, digits)`: midpoint ties away from zero. A scalar defaults to
/// zero decimal places. Money needs its scale stated.
///
/// # Errors
///
/// Returns an evaluation error for a Boolean, for money without an explicit
/// non-negative scale, and for a result that overflows.
pub(crate) fn round(value: Value, digits: Option<i32>, at: At<'_>) -> Result<Value> {
    match value {
        Value::Unavailable(_) => Ok(value),
        other => match numeric(other, "round()", at)? {
            Num::Money(money) => {
                let scale = digits
                    .and_then(|digits| u32::try_from(digits).ok())
                    .ok_or_else(|| {
                        at.error(
                            "round() of money needs an explicit non-negative number of decimal \
                             places",
                        )
                    })?;
                at.core(
                    money.round(scale, RoundingMode::MidpointAwayFromZero),
                    "round()",
                )
                .map(Value::Money)
            }
            num => {
                let rounded =
                    finstack_quant_core::math::round_half_away(num.float(), digits.unwrap_or(0));
                Ok(like(other, at.finite(rounded, "round()")?))
            }
        },
    }
}

/// `clamp(value, lo, hi)` with both endpoints included.
///
/// # Errors
///
/// Returns an evaluation error for mixed units and for `lo > hi`.
pub(crate) fn clamp(value: Value, lo: Value, hi: Value, at: At<'_>) -> Result<Value> {
    if let Some(unavailable) = first_unavailable(&[value, lo, hi]) {
        return Ok(unavailable);
    }
    if order(lo, hi, "clamp()", at)? == Ordering::Greater {
        return Err(at.error("clamp() lower bound is above its upper bound"));
    }
    if order(value, lo, "clamp()", at)? == Ordering::Less {
        Ok(lo)
    } else if order(value, hi, "clamp()", at)? == Ordering::Greater {
        Ok(hi)
    } else {
        Ok(value)
    }
}

/// The published cell of a formula result for a node of `value_type`.
///
/// A Boolean is published as the scalar `1` or `0`. An untyped number takes
/// the node's units, converting to exact money at the floating-point boundary.
///
/// # Errors
///
/// Returns an evaluation error when the result's units contradict the node's,
/// or a monetary result cannot be represented as `Money`.
pub(crate) fn into_cell(
    value: Value,
    value_type: Option<crate::types::NodeValueType>,
    at: At<'_>,
) -> Result<Cell> {
    use crate::types::NodeValueType;
    let mismatch = || {
        at.error(format!(
            "formula result is {}, which contradicts the node's declared units",
            value.describe()
        ))
    };
    match (value, value_type) {
        (Value::Unavailable(reason), _) => Ok(Cell::Unavailable(reason)),
        (Value::Money(money), Some(NodeValueType::Monetary { currency }))
            if money.currency() == currency =>
        {
            Ok(Cell::Money(money))
        }
        (Value::Money(money), None) => Ok(Cell::Money(money)),
        (Value::Money(_), _) => Err(mismatch()),
        (Value::Number(number), Some(NodeValueType::Monetary { currency })) => at
            .core(
                Money::from_model_value(number, currency),
                "representing the result as money",
            )
            .map(Cell::Money),
        (Value::Scalar(_) | Value::Bool(_), Some(NodeValueType::Monetary { .. })) => {
            Err(mismatch())
        }
        (Value::Scalar(number) | Value::Number(number), _) => Ok(Cell::Scalar(number)),
        (Value::Bool(flag), _) => Ok(Cell::Scalar(f64::from(u8::from(flag)))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    static AT: std::sync::LazyLock<At<'static>> = std::sync::LazyLock::new(|| At {
        node_id: Some("node"),
        period: PeriodId::quarter(2025, 1).expect("valid period"),
    });
    const GONE: Value = Value::Unavailable(UnavailableReason::MissingInput);

    fn usd(amount: &str) -> Value {
        Value::Money(Money::from_decimal_str(amount, Currency::USD).expect("amount"))
    }

    fn eur(amount: &str) -> Value {
        Value::Money(Money::from_decimal_str(amount, Currency::EUR).expect("amount"))
    }

    #[test]
    fn money_arithmetic_is_exact_and_scalar_arithmetic_is_checked() {
        assert_eq!(
            arithmetic(BinOp::Add, usd("0.10"), usd("0.20"), *AT).expect("add"),
            usd("0.30")
        );
        assert_eq!(
            arithmetic(BinOp::Mul, usd("100.10"), Value::Scalar(3.0), *AT).expect("mul"),
            usd("300.30")
        );
        assert_eq!(
            arithmetic(BinOp::Div, usd("30"), usd("120"), *AT).expect("ratio"),
            Value::Scalar(0.25)
        );
        assert_eq!(
            arithmetic(BinOp::Sub, usd("5"), Value::Number(1.5), *AT).expect("adopt"),
            usd("3.5")
        );
        for (left, right) in [
            (usd("1"), eur("1")),
            (usd("1"), Value::Scalar(1.0)),
            (Value::Bool(true), Value::Scalar(1.0)),
        ] {
            assert!(arithmetic(BinOp::Add, left, right, *AT).is_err());
        }
        assert!(arithmetic(BinOp::Mul, usd("2"), usd("2"), *AT).is_err());
        let overflow = arithmetic(BinOp::Mul, Value::Scalar(1e308), Value::Scalar(10.0), *AT)
            .expect_err("overflow");
        assert!(overflow.to_string().contains("2025Q1"), "{overflow}");
    }

    #[test]
    fn division_and_remainder_by_zero_are_errors_and_unavailable_propagates() {
        for op in [BinOp::Div, BinOp::Mod] {
            let error =
                arithmetic(op, Value::Scalar(1.0), Value::Scalar(0.0), *AT).expect_err("by zero");
            assert!(error.to_string().contains("by zero"), "{error}");
            // The unavailable operand wins over the zero divisor: nothing is
            // divided.
            assert_eq!(
                arithmetic(op, GONE, Value::Scalar(0.0), *AT).expect("gone"),
                GONE
            );
        }
        assert!(arithmetic(BinOp::Div, usd("1"), usd("0"), *AT).is_err());
        assert_eq!(
            arithmetic(BinOp::Add, Value::Scalar(1.0), GONE, *AT).expect("gone"),
            GONE
        );
    }

    #[test]
    fn comparisons_are_exact_with_no_tolerance() {
        let tiny = Value::Scalar(1e-12);
        assert_eq!(
            compare(BinOp::Eq, tiny, Value::Scalar(0.0), *AT).expect("eq"),
            Value::Bool(false)
        );
        assert_eq!(
            compare(BinOp::Eq, usd("1.10"), usd("1.1"), *AT).expect("eq"),
            Value::Bool(true)
        );
        assert_eq!(
            compare(BinOp::Lt, usd("1.10"), usd("1.11"), *AT).expect("lt"),
            Value::Bool(true)
        );
        assert_eq!(compare(BinOp::Gt, GONE, tiny, *AT).expect("gone"), GONE);
        assert!(compare(BinOp::Eq, usd("1"), eur("1"), *AT).is_err());
        assert!(compare(BinOp::Lt, Value::Bool(true), Value::Bool(false), *AT).is_err());
        assert_eq!(
            compare(BinOp::Ne, Value::Bool(true), Value::Bool(false), *AT).expect("ne"),
            Value::Bool(true)
        );
    }

    #[test]
    fn controls_must_be_boolean() {
        assert_eq!(
            control(Value::Bool(true), "if", *AT).expect("flag"),
            Some(true)
        );
        assert_eq!(control(GONE, "if", *AT).expect("gone"), None);
        for value in [Value::Scalar(1.0), Value::Number(0.0), usd("1")] {
            assert!(control(value, "if", *AT).is_err());
        }
    }

    #[test]
    fn aggregates_need_every_operand_and_keep_money_exact() {
        let scalars = [Value::Scalar(1.0), Value::Scalar(2.0), Value::Scalar(6.0)];
        assert_eq!(
            aggregate(Aggregate::Sum, &scalars, *AT).expect("sum"),
            Value::Scalar(9.0)
        );
        assert_eq!(
            aggregate(Aggregate::Mean, &scalars, *AT).expect("mean"),
            Value::Scalar(3.0)
        );
        assert_eq!(
            aggregate(Aggregate::Min, &scalars, *AT).expect("min"),
            Value::Scalar(1.0)
        );
        assert_eq!(
            aggregate(Aggregate::Max, &scalars, *AT).expect("max"),
            Value::Scalar(6.0)
        );
        let money = [usd("0.10"), usd("0.20"), usd("0.30")];
        assert_eq!(
            aggregate(Aggregate::Sum, &money, *AT).expect("sum"),
            usd("0.60")
        );
        assert_eq!(
            aggregate(Aggregate::Mean, &money, *AT).expect("mean"),
            usd("0.20")
        );
        assert_eq!(
            aggregate(Aggregate::Max, &money, *AT).expect("max"),
            usd("0.30")
        );
        for kind in [
            Aggregate::Sum,
            Aggregate::Mean,
            Aggregate::Min,
            Aggregate::Max,
        ] {
            assert_eq!(
                aggregate(kind, &[Value::Scalar(1.0), GONE], *AT).expect("gone"),
                GONE
            );
            assert!(aggregate(kind, &[usd("1"), Value::Scalar(1.0)], *AT).is_err());
            assert!(aggregate(kind, &[usd("1"), eur("1")], *AT).is_err());
        }
    }

    #[test]
    fn rounding_and_clamp_follow_the_stated_rules() {
        assert_eq!(
            round(Value::Scalar(2.5), None, *AT).expect("round"),
            Value::Scalar(3.0)
        );
        assert_eq!(
            round(Value::Scalar(-2.5), None, *AT).expect("round"),
            Value::Scalar(-3.0)
        );
        assert_eq!(
            round(Value::Scalar(1.005), Some(1), *AT).expect("round"),
            Value::Scalar(1.0)
        );
        assert_eq!(
            round(usd("2.345"), Some(2), *AT).expect("round"),
            usd("2.35")
        );
        assert_eq!(
            round(usd("-2.345"), Some(2), *AT).expect("round"),
            usd("-2.35")
        );
        assert!(round(usd("2.345"), None, *AT).is_err());
        assert!(round(usd("2.345"), Some(-1), *AT).is_err());

        let (lo, hi) = (Value::Scalar(0.0), Value::Scalar(1.0));
        assert_eq!(
            clamp(Value::Scalar(1.0), lo, hi, *AT).expect("clamp"),
            Value::Scalar(1.0)
        );
        assert_eq!(clamp(Value::Scalar(7.0), lo, hi, *AT).expect("clamp"), hi);
        assert_eq!(clamp(Value::Scalar(-7.0), lo, hi, *AT).expect("clamp"), lo);
        assert!(clamp(Value::Scalar(0.5), hi, lo, *AT).is_err());
        assert!(clamp(usd("1"), lo, hi, *AT).is_err());
        assert_eq!(clamp(GONE, lo, hi, *AT).expect("gone"), GONE);
    }

    #[test]
    fn cell_boundary_publishes_booleans_and_types_untyped_numbers() {
        use crate::types::NodeValueType;
        let usd_node = Some(NodeValueType::Monetary {
            currency: Currency::USD,
        });
        assert_eq!(
            into_cell(Value::Bool(true), Some(NodeValueType::Scalar), *AT).expect("flag"),
            Cell::Scalar(1.0)
        );
        assert_eq!(
            into_cell(Value::Number(1.5), usd_node, *AT).expect("money"),
            Cell::Money(Money::from_decimal_str("1.5", Currency::USD).expect("amount"))
        );
        assert_eq!(
            into_cell(GONE, usd_node, *AT).expect("gone"),
            Cell::Unavailable(UnavailableReason::MissingInput)
        );
        assert!(into_cell(eur("1"), usd_node, *AT).is_err());
        assert!(into_cell(Value::Scalar(1.0), usd_node, *AT).is_err());
        assert!(into_cell(usd("1"), Some(NodeValueType::Scalar), *AT).is_err());
    }
}
