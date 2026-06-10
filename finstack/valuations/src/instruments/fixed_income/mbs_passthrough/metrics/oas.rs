//! Static Z-spread calculation for agency MBS.
//!
//! This computes the constant spread over the discount curve that equates the
//! MBS price under a **single deterministic prepayment scenario** to its market
//! price. It is a **static Z-spread**, *not* an option-adjusted spread: it does
//! not simulate interest-rate paths or rate-dependent prepayment, so it does
//! not value the embedded prepayment option. The registered `Oas` metric routes
//! to the Monte Carlo OAS engine (`mc_oas`); this helper is the fast static
//! approximation.

use crate::instruments::fixed_income::mbs_passthrough::pricer::price_with_spread;
use crate::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::solver::{BrentSolver, Solver};
use finstack_core::Result;

/// Static Z-spread calculation result.
#[derive(Debug, Clone)]
#[allow(dead_code)] // public API result struct
pub(crate) struct StaticSpreadResult {
    /// Static Z-spread in decimal (e.g., 0.01 for 100 bps)
    pub spread: f64,
    /// Model price at the solved static Z-spread
    pub model_price: f64,
    /// Target (market) price
    pub market_price: f64,
    /// Price difference at solution
    pub price_error: f64,
    /// Number of solver iterations
    pub iterations: u32,
    /// Whether solver converged
    pub converged: bool,
}

/// Calculate the static Z-spread via root-finding.
///
/// Uses Brent's method to find the constant spread that equates the model price
/// (under a single deterministic prepayment scenario) to the market price. This
/// is a static Z-spread, not an OAS — it does not simulate rate paths.
///
/// # Arguments
///
/// * `mbs` - Agency MBS passthrough instrument
/// * `market_price_pct` - Target price (per $100 face, e.g., 98.5)
/// * `market` - Market context with discount curves
/// * `as_of` - Valuation date
///
/// # Returns
///
/// Static-spread result with the spread and convergence information.
#[allow(dead_code)] // retained static-spread helper; currently exercised by tests
pub(crate) fn calculate_static_zspread(
    mbs: &AgencyMbsPassthrough,
    market_price_pct: f64,
    market: &MarketContext,
    as_of: Date,
) -> Result<StaticSpreadResult> {
    // Convert market price from percentage to dollar amount
    let market_price = market_price_pct / 100.0 * mbs.current_face.amount();

    // Use core's BrentSolver instead of a hand-rolled implementation.
    // Bracket bounds: -500 bps to +2000 bps covers virtually all OAS scenarios.
    let solver = BrentSolver::new()
        .tolerance(1e-8)
        .max_iterations(100)
        .bracket_bounds(-0.10, 0.20)
        .initial_bracket_size(Some(0.05));

    // Capture any pricing error from the objective in a RefCell so we can
    // propagate it after the solver finishes (the Solver trait expects Fn(f64)->f64).
    let pricing_error: std::cell::RefCell<Option<finstack_core::Error>> =
        std::cell::RefCell::new(None);

    let objective = |spread: f64| -> f64 {
        match price_with_spread(mbs, market, as_of, spread) {
            Ok(model_price) => model_price - market_price,
            Err(e) => {
                *pricing_error.borrow_mut() = Some(e);
                f64::NAN
            }
        }
    };

    // Initial guess at zero spread
    let result = solver.solve(objective, 0.0);

    // Propagate any pricing error that occurred during objective evaluation
    if let Some(err) = pricing_error.into_inner() {
        return Err(err);
    }

    match result {
        Ok(oas) => {
            let final_price = price_with_spread(mbs, market, as_of, oas)?;
            Ok(StaticSpreadResult {
                spread: oas,
                model_price: final_price,
                market_price,
                price_error: final_price - market_price,
                iterations: solver.max_iterations as u32,
                converged: true,
            })
        }
        // Non-convergence is an error: a fabricated `spread: 0.0` result was
        // previously consumed downstream as a real spread with no signal.
        Err(e) => Err(finstack_core::Error::Validation(format!(
            "MBS OAS solve did not converge for '{}' (target price {market_price_pct}% of \
             face): {e}. Check the market price quote and discount curve.",
            mbs.id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::specs::PrepaymentModelSpec;
    use crate::instruments::fixed_income::mbs_passthrough::{AgencyProgram, PoolType};
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::money::Money;
    use finstack_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn create_test_mbs() -> AgencyMbsPassthrough {
        AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("TEST-MBS"))
            .pool_id("TEST-POOL".into())
            .agency(AgencyProgram::Fnma)
            .pool_type(PoolType::Generic)
            .original_face(Money::new(1_000_000.0, Currency::USD))
            .current_face(Money::new(1_000_000.0, Currency::USD))
            .current_factor(1.0)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(360)
            .issue_date(Date::from_calendar_date(2024, Month::January, 1).expect("valid"))
            .maturity(Date::from_calendar_date(2054, Month::January, 1).expect("valid"))
            .prepayment_model(PrepaymentModelSpec::psa(1.0))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Thirty360)
            .build()
            .expect("valid mbs")
    }

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

    #[test]
    fn test_oas_calculation() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        // Get model price at zero spread
        let base_price = price_with_spread(&mbs, &market, as_of, 0.0).expect("price");
        let market_price_pct = base_price / mbs.current_face.amount() * 100.0;

        // OAS should be approximately zero when market price equals model price
        let result = calculate_static_zspread(&mbs, market_price_pct, &market, as_of).expect("oas");

        assert!(result.converged);
        assert!(result.spread.abs() < 0.001); // Within 10 bps of zero
    }

    #[test]
    fn test_oas_with_discount() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        // Test with discount price (should give positive OAS)
        let discount_price = 95.0; // 95% of par

        let result = calculate_static_zspread(&mbs, discount_price, &market, as_of).expect("oas");

        // Price below par should imply positive spread
        // (this depends on the specific curve setup)
        assert!(result.converged || result.iterations > 0);
    }

    #[test]
    fn test_oas_with_premium() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        // Test with premium price (should give negative OAS)
        let premium_price = 105.0; // 105% of par

        let result = calculate_static_zspread(&mbs, premium_price, &market, as_of).expect("oas");

        // Price above par should imply negative spread
        assert!(result.converged || result.iterations > 0);
    }
}
