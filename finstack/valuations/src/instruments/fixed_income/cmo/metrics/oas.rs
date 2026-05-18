//! CMO tranche OAS calculation.
//!
//! OAS for CMO tranches requires running the waterfall at multiple
//! spread levels to find the spread that equates model price to market.

use crate::instruments::fixed_income::cmo::pricer::generate_tranche_cashflows;
use crate::instruments::fixed_income::cmo::AgencyCmo;
use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::solver::{BrentSolver, Solver};
use finstack_core::Result;

/// CMO tranche OAS result.
#[derive(Debug, Clone)]
#[allow(dead_code)] // public API result struct
pub(crate) struct CmoOasResult {
    /// Option-adjusted spread (decimal)
    pub(crate) oas: f64,
    /// Model price at OAS
    pub(crate) model_price: f64,
    /// Market price (target)
    pub(crate) market_price: f64,
    /// Iterations to converge
    pub(crate) iterations: u32,
    /// Whether converged
    pub(crate) converged: bool,
}

/// Calculate OAS for a CMO tranche.
///
/// Uses the same Brent's method approach as MBS OAS but with
/// waterfall-generated tranche cashflows.
pub(crate) fn calculate_tranche_oas(
    cmo: &AgencyCmo,
    market_price_pct: f64,
    market: &MarketContext,
    as_of: Date,
) -> Result<CmoOasResult> {
    let tranche = cmo.reference_tranche().ok_or_else(|| {
        finstack_core::Error::Validation(format!("Tranche {} not found", cmo.reference_tranche_id))
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

    // Use core's `BrentSolver` instead of a hand-rolled Brent loop. The
    // previous implementation only widened its bounds once and never
    // re-verified that the wider interval actually bracketed the root, so an
    // unbracketed solve could fall through the `(b - a) < tolerance` exit and
    // report a *boundary* as a converged OAS. `BrentSolver` returns
    // `Err(SolverConvergenceFailed)` when no bracketing interval exists, which
    // we surface here as an honest `converged: false` result rather than a
    // false positive.
    const MAX_ITERATIONS: usize = 100;
    let solver = BrentSolver::new()
        .tolerance(1e-8)
        .max_iterations(MAX_ITERATIONS)
        .bracket_bounds(-0.10, 0.20)
        .initial_bracket_size(Some(0.05));

    // Capture any pricing error from the objective so it can be propagated
    // after the solver finishes (the `Solver` trait expects `Fn(f64) -> f64`).
    let pricing_error: std::cell::RefCell<Option<finstack_core::Error>> =
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

    match result {
        Ok(oas) => {
            let final_price = price_at_spread(oas)?;
            Ok(CmoOasResult {
                oas,
                model_price: final_price,
                market_price,
                iterations: MAX_ITERATIONS as u32,
                converged: true,
            })
        }
        Err(_) => {
            // No bracketing interval / no convergence: report this honestly.
            // OAS is reported as 0.0 (best-effort) with `converged = false` —
            // we do NOT pass off a bracket boundary as a converged OAS.
            let model_price_zero = price_at_spread(0.0)?;
            Ok(CmoOasResult {
                oas: 0.0,
                model_price: model_price_zero,
                market_price,
                iterations: MAX_ITERATIONS as u32,
                converged: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
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

    /// Item 14 regression: an OAS solve that cannot bracket the target must
    /// report `converged = false` — never a bracket boundary disguised as a
    /// converged OAS.
    ///
    /// The pre-fix hand-rolled Brent widened its bounds once without
    /// re-checking the bracket, so an out-of-range market price could exit via
    /// the interval-width test and return a *boundary* as the "OAS". Here we
    /// request an absurdly low price (1% of face) that no spread inside
    /// [-10%, +20%] can reach; the result must be flagged non-converged.
    #[test]
    fn unbracketable_target_reports_non_convergence() {
        let cmo = AgencyCmo::example().expect("AgencyCmo example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        // A price far below any value reachable within the spread bracket.
        let result = calculate_tranche_oas(&cmo, 1.0, &market, as_of).expect("oas call");

        assert!(
            !result.converged,
            "an unbracketable OAS solve must report converged = false, got oas={}",
            result.oas
        );
        // The reported OAS for a non-converged solve must be the explicit
        // best-effort 0.0, not a leaked bracket boundary (-0.10 or 0.20).
        assert!(
            result.oas.abs() < 1e-12,
            "non-converged OAS should be reported as 0.0, not a boundary; got {}",
            result.oas
        );
    }

    #[test]
    fn test_tranche_oas() {
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

        // OAS should be near zero
        let result = calculate_tranche_oas(&cmo, price_pct, &market, as_of).expect("oas");

        assert!(result.oas.abs() < 0.01);
    }
}
