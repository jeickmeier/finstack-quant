//! CMO tranche static Z-spread calculation.
//!
//! Solves for the constant spread over the discount curve that reprices the
//! tranche's waterfall-generated cashflows to the quoted market price. The
//! cashflows are projected under a *single* deterministic prepayment path, so
//! this is a static Z-spread — **not** an option-adjusted spread (a true
//! MC-OAS over stochastic rate/prepayment paths is deferred).

use crate::instruments::fixed_income::cmo::pricer::generate_tranche_cashflows;
use crate::instruments::fixed_income::cmo::AgencyCmo;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::Result;

/// CMO tranche Z-spread result.
#[derive(Debug, Clone)]
#[allow(dead_code)] // public API result struct
pub(crate) struct CmoZSpreadResult {
    /// Static Z-spread (decimal)
    pub(crate) zspread: f64,
    /// Model price at the solved spread
    pub(crate) model_price: f64,
    /// Market price (target)
    pub(crate) market_price: f64,
    /// Iterations to converge
    pub(crate) iterations: u32,
}

/// Calculate the static Z-spread for a CMO tranche.
///
/// Uses Brent's method on waterfall-generated tranche cashflows.
///
/// # Errors
///
/// Returns `Error::Validation` when no spread in the bracket reprices the
/// tranche to the requested market price (non-convergence is propagated as
/// an error rather than silently reported as a zero spread).
pub(crate) fn calculate_tranche_zspread(
    cmo: &AgencyCmo,
    market_price_pct: f64,
    market: &MarketContext,
    as_of: Date,
) -> Result<CmoZSpreadResult> {
    let tranche = cmo.reference_tranche().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "Tranche {} not found",
            cmo.reference_tranche_id
        ))
    })?;

    let market_price = market_price_pct / 100.0 * tranche.current_face.amount();

    // Cache cashflows outside the solver loop — they don't depend on spread.
    let tranche_cfs = generate_tranche_cashflows(cmo, as_of, None)?;
    let discount_curve = market.get_discount(&cmo.discount_curve_id)?;
    let day_count = DayCount::Thirty360;

    // Price function with spread (uses cached cashflows).
    let price_at_spread = |spread: f64| -> Result<f64> {
        let mut pv = 0.0;
        for cf in &tranche_cfs {
            let years =
                day_count.year_fraction(as_of, cf.payment_date, DayCountContext::default())?;
            let base_df = discount_curve.df(years);
            let spread_adj = (-spread * years).exp();
            let df = base_df * spread_adj;
            pv += cf.total * df;
        }

        Ok(pv)
    };

    // `BrentSolver` returns `Err(SolverConvergenceFailed)` when no bracketing
    // interval exists; that is propagated as an error rather than reporting a
    // boundary (or a silent 0.0) as a converged spread.
    const MAX_ITERATIONS: usize = 100;
    let solver = BrentSolver::new()
        .tolerance(1e-8)
        .max_iterations(MAX_ITERATIONS)
        .bracket_bounds(-0.10, 0.20)
        .initial_bracket_size(Some(0.05));

    // Capture any pricing error from the objective so it can be propagated
    // after the solver finishes (the `Solver` trait expects `Fn(f64) -> f64`).
    let pricing_error: std::cell::RefCell<Option<finstack_quant_core::Error>> =
        std::cell::RefCell::new(None);
    let objective = |spread: f64| -> f64 {
        match price_at_spread(spread) {
            Ok(model_price) => model_price - market_price,
            Err(e) => {
                if pricing_error.borrow().is_none() {
                    *pricing_error.borrow_mut() = Some(e);
                }
                f64::NAN
            }
        }
    };

    let result = solver.solve(objective, 0.0);

    // A pricing error during objective evaluation takes precedence.
    if let Some(err) = pricing_error.into_inner() {
        return Err(err);
    }

    let zspread = result.map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "CMO tranche Z-spread solver failed to converge within bounds [-10%, 20%]: {e}. \
             Check that market price {market_price_pct} pct is within the model's reachable PV range."
        ))
    })?;

    let final_price = price_at_spread(zspread)?;
    Ok(CmoZSpreadResult {
        zspread,
        model_price: final_price,
        market_price,
        iterations: MAX_ITERATIONS as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use time::Month;

    fn create_test_market(as_of: Date) -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, 0.96),
                (5.0, 0.80),
                (10.0, 0.60),
                (30.0, 0.30),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .expect("valid curve");

        MarketContext::new().insert(disc)
    }

    /// Findings 13/14 regression: a spread solve that cannot bracket the
    /// target must surface an error — never a bracket boundary or a silent
    /// 0.0 disguised as a converged spread.
    ///
    /// Here we request an absurdly low price (1% of face) that no spread
    /// inside [-10%, +20%] can reach; the solve must fail loudly.
    #[test]
    fn unbracketable_target_errors() {
        let cmo = AgencyCmo::example().expect("AgencyCmo example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        // A price far below any value reachable within the spread bracket.
        let result = calculate_tranche_zspread(&cmo, 1.0, &market, as_of);

        assert!(
            result.is_err(),
            "an unbracketable Z-spread solve must return Err, got {:?}",
            result.map(|r| r.zspread)
        );
    }

    #[test]
    fn test_tranche_zspread() {
        let cmo = AgencyCmo::example().expect("AgencyCmo example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        // Get model price at zero spread to use as market price
        let tranche_cfs = generate_tranche_cashflows(&cmo, as_of, None).expect("cfs");
        let disc = market.get_discount(&cmo.discount_curve_id).expect("curve");
        let day_count = DayCount::Thirty360;

        let mut model_price = 0.0;
        for cf in &tranche_cfs {
            let years = day_count
                .year_fraction(as_of, cf.payment_date, DayCountContext::default())
                .expect("yf");
            model_price += cf.total * disc.df(years);
        }

        let tranche = cmo.reference_tranche().expect("tranche");
        let price_pct = model_price / tranche.current_face.amount() * 100.0;

        // Z-spread should be near zero at the model price.
        let result = calculate_tranche_zspread(&cmo, price_pct, &market, as_of).expect("zspread");

        assert!(result.zspread.abs() < 0.01);
    }
}
