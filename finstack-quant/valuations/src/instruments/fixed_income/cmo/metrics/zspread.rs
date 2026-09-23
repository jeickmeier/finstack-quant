//! CMO tranche static Z-spread calculation.
//!
//! Solves for the constant spread over the discount curve that reprices the
//! tranche's waterfall-generated cashflows to the quoted market price. The
//! cashflows are projected under a *single* deterministic prepayment path, so
//! this is a static Z-spread — **not** an option-adjusted spread (a true
//! MC-OAS over stochastic rate/prepayment paths is deferred).

use crate::instruments::fixed_income::cmo::pricer::{resolve_collateral, tranche_cashflows_on};
use crate::instruments::fixed_income::cmo::AgencyCmo;
use crate::instruments::fixed_income::mbs_passthrough::pricer::quote_basis_pool;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::Result;

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
) -> Result<f64> {
    let tranche = cmo.reference_tranche().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "Tranche {} not found",
            cmo.reference_tranche_id
        ))
    })?;

    // The clean quote plus accrued buys the settlement-month accrual onward;
    // the collateral's prior-month in-flight P&I belongs to the seller.
    let collateral = quote_basis_pool(&resolve_collateral(cmo, as_of)?, as_of)?;

    // Tranche interest accrues on the collateral's day count.
    let month_start = Date::from_calendar_date(as_of.year(), as_of.month(), 1)
        .map_err(|err| finstack_quant_core::Error::Validation(err.to_string()))?;
    let accrual_start = month_start.max(cmo.issue_date);
    let accrued = tranche.current_face.amount()
        * tranche.coupon
        * collateral.day_count.year_fraction(
            accrual_start.min(as_of),
            as_of,
            DayCountContext::default(),
        )?;
    let market_price = market_price_pct / 100.0 * tranche.current_face.amount() + accrued;

    // Cache cashflows outside the solver loop — they don't depend on spread.
    let tranche_cfs = tranche_cashflows_on(cmo, &collateral, as_of, None)?;
    let discount_curve = market.get_discount(&cmo.discount_curve_id)?;
    let day_count = DayCount::Thirty360;

    // Price function with spread (uses cached cashflows).
    let price_at_spread = |spread: f64| -> Result<f64> {
        let mut pv = 0.0;
        for cf in &tranche_cfs {
            let years =
                day_count.year_fraction(as_of, cf.payment_date, DayCountContext::default())?;
            let base_df = discount_curve.df_between_dates(as_of, cf.payment_date)?;
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

    result.map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "CMO tranche Z-spread solver failed to converge within bounds [-10%, 20%]: {e}. \
             Check that market price {market_price_pct} pct is within the model's reachable PV range."
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::cmo::pricer::generate_tranche_cashflows;
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
            "an unbracketable Z-spread solve must return Err, got {result:?}"
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
        let zspread = calculate_tranche_zspread(&cmo, price_pct, &market, as_of).expect("zspread");

        assert!(zspread.abs() < 0.01);
    }
}

#[cfg(test)]
mod production_mortgage_audit {
    use super::*;
    use crate::instruments::fixed_income::cmo::pricer::price_cmo;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::macros::date;

    #[test]
    fn cmo_clean_quote_has_zero_spread_at_clean_model_value() {
        let cmo = AgencyCmo::example().expect("cmo");
        let as_of = date!(2024 - 01 - 15);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (40.0, 1.0)])
                .build()
                .expect("curve"),
        );
        let tranche = cmo.reference_tranche().expect("tranche");
        let dirty = price_cmo(&cmo, &market, as_of).expect("price").amount();
        let accrued = tranche.current_face.amount() * tranche.coupon * 14.0 / 360.0;
        let clean = (dirty - accrued) / tranche.current_face.amount() * 100.0;
        let spread = calculate_tranche_zspread(&cmo, clean, &market, as_of).expect("spread");
        assert!(spread.abs() < 1e-9, "spread {spread}");
    }

    /// Same pool, same clean quote, before (Feb 10) and after (Feb 27) the
    /// Feb 26 payment of the January collateral accrual: the Z-spread must
    /// not jump. Over the 17 days the dirty target grows by the tranche
    /// accrual `c × 17/360` and the projected PV by `(r + s) × 17/365`. The
    /// flat curve rate is set to `c × 365/360` so the two carries match when
    /// the spread is near zero; the residual mismatch is then far below the
    /// 0.1 bp tolerance on a ~3-year-duration tranche. Before the fix the
    /// Feb 10 solve fed the seller's in-flight January collateral flow into
    /// tranche A: 4.60 bp on Feb 10 versus −5.05 bp on Feb 27.
    #[test]
    fn cmo_zspread_is_stable_across_the_in_flight_payment_date() {
        let mut cmo = AgencyCmo::example().expect("cmo");
        cmo.issue_date = date!(2023 - 01 - 01);
        let flat = 0.035_f64 * 365.0 / 360.0;
        let quote = 99.6;
        let spread_at = |as_of: Date| {
            let market = MarketContext::new().insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (40.0, (-flat * 40.0).exp())])
                    .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
                    .build()
                    .expect("curve"),
            );
            calculate_tranche_zspread(&cmo, quote, &market, as_of).expect("spread")
        };
        let before = spread_at(date!(2024 - 02 - 10));
        let after = spread_at(date!(2024 - 02 - 27));
        assert!(
            (before - after).abs() < 1e-5,
            "Z-spread must not jump across the payment date: before={before} after={after}"
        );
        assert!(
            before < after + 5e-4,
            "Feb 10 Z-spread must no longer include the seller's in-flight collateral flow"
        );
    }
}
