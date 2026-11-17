//! Canonical formula function signatures.
//!
//! One table names every DSL function, the core [`Function`] it lowers to and
//! the argument counts it accepts. The compiler, the caller-built AST
//! preflight and the evaluator's arity tests all read it, and the function
//! reference in [`crate::dsl`] is pinned to it by a test.

use finstack_quant_core::expr::Function;

/// Name, lowering target and accepted argument counts of one DSL function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FunctionSignature {
    /// Case-sensitive DSL name.
    pub(crate) name: &'static str,
    /// Core function the call lowers to.
    pub(crate) function: Function,
    /// Fewest accepted arguments.
    pub(crate) min_args: usize,
    /// Most accepted arguments, or `None` for a variadic function.
    pub(crate) max_args: Option<usize>,
}

const fn exactly(name: &'static str, function: Function, count: usize) -> FunctionSignature {
    FunctionSignature {
        name,
        function,
        min_args: count,
        max_args: Some(count),
    }
}

const fn between(
    name: &'static str,
    function: Function,
    min_args: usize,
    max_args: usize,
) -> FunctionSignature {
    FunctionSignature {
        name,
        function,
        min_args,
        max_args: Some(max_args),
    }
}

const fn at_least(name: &'static str, function: Function, min_args: usize) -> FunctionSignature {
    FunctionSignature {
        name,
        function,
        min_args,
        max_args: None,
    }
}

/// Every function the DSL accepts. `lead` is deliberately absent: a
/// forward-looking read is not a supported formula.
pub(crate) const FUNCTION_SIGNATURES: &[FunctionSignature] = &[
    at_least("sum", Function::Sum, 1),
    at_least("mean", Function::Mean, 1),
    at_least("min", Function::Min, 1),
    at_least("max", Function::Max, 1),
    at_least("coalesce", Function::Coalesce, 2),
    exactly("abs", Function::Abs, 1),
    exactly("sign", Function::Sign, 1),
    exactly("floor", Function::Floor, 1),
    exactly("ceil", Function::Ceil, 1),
    exactly("ln", Function::Ln, 1),
    exactly("exp", Function::Exp, 1),
    exactly("log10", Function::Log10, 1),
    exactly("sqrt", Function::Sqrt, 1),
    exactly("is_missing", Function::IsMissing, 1),
    exactly("ratio", Function::Ratio, 2),
    exactly("ttm", Function::Ttm, 1),
    exactly("ytd", Function::Ytd, 1),
    exactly("qtd", Function::Qtd, 1),
    exactly("cumsum", Function::CumSum, 1),
    exactly("cumprod", Function::CumProd, 1),
    exactly("cummin", Function::CumMin, 1),
    exactly("cummax", Function::CumMax, 1),
    exactly("std", Function::Std, 1),
    exactly("var", Function::Var, 1),
    exactly("median", Function::Median, 1),
    exactly("pow", Function::Pow, 2),
    exactly("lag", Function::Lag, 2),
    exactly("shift", Function::Shift, 2),
    exactly("ewm_mean", Function::EwmMean, 2),
    exactly("quantile", Function::Quantile, 2),
    exactly("fiscal_ytd", Function::FiscalYtd, 2),
    exactly("growth_rate", Function::GrowthRate, 2),
    exactly("annualize", Function::Annualize, 2),
    exactly("clamp", Function::Clamp, 3),
    exactly("annualize_rate", Function::AnnualizeRate, 3),
    between("round", Function::Round, 1, 2),
    between("diff", Function::Diff, 1, 2),
    between("pct_change", Function::PctChange, 1, 2),
    between("rank", Function::Rank, 1, 2),
    between("rolling_mean", Function::RollingMean, 2, 3),
    between("rolling_sum", Function::RollingSum, 2, 3),
    between("rolling_std", Function::RollingStd, 2, 3),
    between("rolling_var", Function::RollingVar, 2, 3),
    between("rolling_median", Function::RollingMedian, 2, 3),
    between("rolling_min", Function::RollingMin, 2, 3),
    between("rolling_max", Function::RollingMax, 2, 3),
    between("rolling_count", Function::RollingCount, 2, 3),
    between("ewm_std", Function::EwmStd, 2, 3),
    between("ewm_var", Function::EwmVar, 2, 3),
];

/// Capital-structure components a `cs.<component>.<target>` reference may name.
pub(crate) const CS_COMPONENTS: &[&str] = &[
    "interest_expense",
    "interest_expense_cash",
    "interest_expense_pik",
    "interest_income",
    "principal_payment",
    "debt_balance",
    "fees",
    "accrued_interest",
];

impl FunctionSignature {
    /// Look up a function by its case-sensitive DSL name.
    pub(crate) fn lookup(name: &str) -> Option<&'static Self> {
        FUNCTION_SIGNATURES
            .iter()
            .find(|signature| signature.name == name)
    }

    /// Whether `count` arguments satisfy this signature.
    pub(crate) fn accepts(&self, count: usize) -> bool {
        count >= self.min_args && self.max_args.is_none_or(|max| count <= max)
    }

    /// Human-readable accepted argument count, e.g. `exactly 1 argument`.
    pub(crate) fn arity_text(&self) -> String {
        let plural = |count: usize| if count == 1 { "" } else { "s" };
        match self.max_args {
            Some(max) if max == self.min_args => {
                format!("exactly {max} argument{}", plural(max))
            }
            Some(max) if max == self.min_args + 1 => {
                format!("{} or {max} arguments", self.min_args)
            }
            Some(max) => format!("{} to {max} arguments", self.min_args),
            None => format!(
                "at least {} argument{}",
                self.min_args,
                plural(self.min_args)
            ),
        }
    }
}

/// Whether `text` is a valid formula identifier.
///
/// An identifier is one or more nonempty segments separated by single dots.
/// The first segment starts with an alphabetic character or underscore; every
/// segment holds only alphanumeric characters and underscores.
pub(crate) fn is_valid_identifier(text: &str) -> bool {
    let starts_validly = text
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_');
    starts_validly
        && text.split('.').all(|segment| {
            !segment.is_empty() && segment.chars().all(|c| c.is_alphanumeric() || c == '_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_lookup_is_case_sensitive() {
        for (index, signature) in FUNCTION_SIGNATURES.iter().enumerate() {
            assert!(
                FUNCTION_SIGNATURES[..index]
                    .iter()
                    .all(|earlier| earlier.name != signature.name
                        && earlier.function != signature.function),
                "{} is listed twice",
                signature.name
            );
        }
        assert!(FunctionSignature::lookup("cumsum").is_some());
        assert!(FunctionSignature::lookup("CumSum").is_none());
        assert!(FunctionSignature::lookup("lead").is_none());
    }

    /// The function reference in the module docs lists every function with
    /// the arity the table enforces.
    #[test]
    fn module_reference_matches_the_table() {
        let reference = include_str!("mod.rs");
        for signature in FUNCTION_SIGNATURES {
            let call = format!("`{}(", signature.name);
            let row = reference
                .lines()
                .find(|line| line.starts_with("//! | ") && line.contains(&call))
                .unwrap_or_else(|| panic!("{} is missing from the reference", signature.name));
            let arity = match signature.max_args {
                Some(max) if max == signature.min_args => format!("| {max} |"),
                Some(max) => format!("| {}-{max} |", signature.min_args),
                None => format!("| {}+ |", signature.min_args),
            };
            assert!(
                row.contains(&arity),
                "reference row for {} must state arity {arity}",
                signature.name
            );
        }
    }

    /// The evaluator rejects the same argument counts for an expression built
    /// without the compiler.
    #[test]
    fn evaluator_enforces_the_same_arity() {
        use crate::evaluator::{EvaluationContext, formula::evaluate_formula};
        use finstack_quant_core::dates::{PeriodCalendar, PeriodId, PeriodPlan};
        use finstack_quant_core::expr::Expr;
        use indexmap::IndexMap;
        use std::sync::Arc;

        for signature in FUNCTION_SIGNATURES {
            let below = signature.min_args.checked_sub(1);
            let above = signature.max_args.map(|max| max + 1);
            for count in [below, above].into_iter().flatten() {
                let mut context = EvaluationContext::new(
                    PeriodId::quarter(2025, 1).expect("quarter"),
                    &PeriodPlan::try_new(PeriodCalendar::Gregorian, Vec::new())
                        .expect("empty plan"),
                    Arc::new(IndexMap::new()),
                    Arc::new(IndexMap::new()),
                );
                let call = Expr::call(signature.function, vec![Expr::literal(1.0); count]);
                let error = evaluate_formula(&call, &mut context, None)
                    .expect_err("the evaluator rejects a wrong argument count");
                assert!(
                    error.to_string().contains("argument"),
                    "{}() with {count} arguments: {error}",
                    signature.name
                );
            }
        }
    }

    #[test]
    fn identifiers_need_nonempty_segments() {
        for valid in [
            "revenue",
            "_x",
            "fin.gross_margin",
            "a.b.c",
            "rev.2024",
            "été",
        ] {
            assert!(is_valid_identifier(valid), "{valid} is valid");
        }
        for invalid in ["", "a..b", "a.", ".a", "1a", "a-b", "a b", "a.b-c"] {
            assert!(!is_valid_identifier(invalid), "{invalid:?} is invalid");
        }
    }
}
