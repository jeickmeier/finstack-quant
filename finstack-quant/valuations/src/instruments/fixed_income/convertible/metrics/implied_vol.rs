//! Implied volatility calculator for convertible bonds.
//!
//! Solves for the equity volatility that makes the tree-based model price
//! equal the market-quoted clean price. This is the convertible bond analog
//! of implied volatility for equity options.
//!
//! # Dependencies
//!
//! Requires `quoted_clean_price` in `bond.instrument_pricing_overrides.market_quotes`.
//!
//! # Units
//!
//! Returned as a decimal fraction (e.g., 0.25 = 25% volatility).

use std::cell::Cell;

use crate::instruments::fixed_income::convertible::market_inputs::volatility_candidate_ids;
use crate::instruments::fixed_income::convertible::pricer::{
    calculate_accrued_interest, price_convertible_bond, ConvertibleTreeType,
};
use crate::instruments::fixed_income::convertible::ConvertibleBond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::Result;

pub(crate) struct ImpliedVolCalculator;

impl MetricCalculator for ImpliedVolCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        let as_of = context.as_of;

        if as_of >= bond.maturity {
            return Ok(0.0);
        }

        let quoted_clean = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "pricing_overrides.market_quotes.quoted_clean_price".to_string(),
                })
            })?;

        let accrued = calculate_accrued_interest(bond, as_of)?;
        // `quoted_clean` is percentage-of-par (e.g. 99.5 = 99.5% of face).
        // `accrued` and the model price are both notional-scaled currency amounts.
        // Scale the percentage quote to notional so the solver objective compares
        // commensurate values — mirroring the term-loan `target_price_from_quote_or_model`.
        let target_dirty = quoted_clean * bond.notional.amount() / 100.0 + accrued;

        let tree_type = ConvertibleTreeType::Binomial(100);

        let underlying_id = bond.underlying_equity_id.as_deref().ok_or_else(|| {
            finstack_quant_core::Error::internal(
                "convertible implied vol requires underlying_equity_id",
            )
        })?;

        let vol_candidates = volatility_candidate_ids(bond)?;

        let vol_id = vol_candidates
            .iter()
            .find(|id| {
                context.curves.get_price(id.as_str()).is_ok()
                    || context.curves.get_surface(id).is_ok()
            })
            .cloned()
            .unwrap_or_else(|| format!("{}-VOL", underlying_id));

        let base_market = context.curves.as_ref();

        // Validate the unbumped pricing path before entering the solver so that
        // missing equity / vol / curve inputs surface their real error messages
        // rather than appearing as opaque solver convergence failures.
        let _ = price_convertible_bond(bond, base_market, tree_type, as_of)?;

        // Capture the first pricing error so a downstream solver failure can
        // report the underlying cause. See the OAS solver for why
        // `take().or(Some(e))` is required instead of `if take().is_none()`.
        let captured_err: Cell<Option<finstack_quant_core::Error>> = Cell::new(None);
        let record_err = |e: finstack_quant_core::Error| {
            let prev = captured_err.take();
            captured_err.set(prev.or(Some(e)));
        };
        let objective = |vol: f64| -> f64 {
            let bumped = base_market
                .clone()
                .insert_price(&vol_id, MarketScalar::Unitless(vol));
            match price_convertible_bond(bond, &bumped, tree_type, as_of) {
                Ok(pv) => pv.amount() - target_dirty,
                Err(e) => {
                    record_err(e);
                    f64::NAN
                }
            }
        };

        let solver = BrentSolver::new()
            .tolerance(1e-6)
            .max_iterations(100)
            .bracket_bounds(0.001, 3.0); // 0.1% to 300% vol

        match solver.solve(objective, 0.25) {
            Ok(implied_vol) => Ok(implied_vol),
            Err(solver_err) => {
                if let Some(inner) = captured_err.take() {
                    Err(finstack_quant_core::Error::Validation(format!(
                        "Convertible implied vol solver failed because pricing failed inside \
                         the objective: {inner}"
                    )))
                } else {
                    Err(solver_err)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::cashflow::builder::specs::{CouponType, FixedCouponSpec};
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::fixed_income::convertible::{
        AntiDilutionPolicy, ConversionPolicy, ConversionSpec, ConvertibleBond, DividendAdjustment,
    };
    use crate::instruments::InstrumentPricingOverrides;
    use crate::metrics::{MetricCalculator, MetricContext};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::prelude::FinstackConfig;
    use time::Month;

    fn make_bond_with_quote(notional_usd: f64, quoted_clean_pct: f64) -> ConvertibleBond {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

        let fixed_coupon = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            schedule: finstack_quant_cashflows::builder::ScheduleParams {
                freq: Tenor::semi_annual(),

                dc: DayCount::Act365F,

                bdc: BusinessDayConvention::Following,

                calendar_id: "weekends_only".to_string(),

                stub: StubKind::None,

                end_of_month: false,

                payment_lag_days: 0,

                adjust_accrual_dates: false,
            },
        };

        let mut overrides = InstrumentPricingOverrides::default();
        overrides.market_quotes.quoted_clean_price = Some(quoted_clean_pct);

        ConvertibleBond {
            id: "TEST_CB_IVOL".to_string().into(),
            notional: Money::new(notional_usd, Currency::USD),
            issue_date: issue,
            maturity,
            discount_curve_id: "USD-OIS".into(),
            credit_curve_id: None,
            settlement_days: None,
            recovery_rate: None,
            conversion: ConversionSpec {
                ratio: Some(10.0),
                price: None,
                policy: ConversionPolicy::Voluntary,
                anti_dilution: AntiDilutionPolicy::None,
                dividend_adjustment: DividendAdjustment::None,
                dilution_events: Vec::new(),
            },
            underlying_equity_id: Some("AAPL".to_string()),
            call_put: None,
            soft_call_trigger: None,
            fixed_coupon: Some(fixed_coupon),
            floating_coupon: None,
            instrument_pricing_overrides: overrides,
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        }
    }

    fn make_market(base_date: Date) -> finstack_quant_core::market_data::MarketContext {
        // Identical market to the standard convertible pricer tests (notional=1000,
        // spot=150, ratio=10 → conversion value = 1500 > notional, deep ITM).
        // r ≈ 1% from knots [(0,1),(10,0.90)].  Setting div yield = r keeps
        // CRR risk-neutral probability p ≈ 0.5 at any vol (stable at vol=0.001).
        let r_approx = (1.0_f64 / 0.90_f64).ln() / 10.0; // ≈ 0.01054
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (10.0, 0.90)])
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .build()
            .expect("should succeed");

        finstack_quant_core::market_data::MarketContext::new()
            .insert(discount_curve)
            .insert_price("AAPL", MarketScalar::Unitless(150.0))
            .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
            .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(r_approx))
    }

    /// Regression: quoted_clean_price is percentage-of-par, so it must be scaled
    /// by notional/100 before adding notional-scaled accrued interest to form the
    /// dirty price target.  Before the fix the solver compared a ~1500 model price
    /// against ~150 target (pct only, unscaled), couldn't bracket, and returned an
    /// error.
    ///
    /// Setup: notional=1000, 5% coupon, 5yr, spot=150, ratio=10 (conversion value=1500,
    /// deep ITM). With div yield ≈ r the CRR probability p ≈ 0.5 keeping the lower
    /// bracket (vol=0.001) numerically stable. The model price varies with vol
    /// (optionality near ATM after scaling) and a 155% quoted price (target=1550)
    /// lies within the bracketed range, so the solver converges to a finite implied
    /// vol after the fix.
    #[test]
    fn implied_vol_quoted_clean_price_scaled_to_notional() {
        // notional=1000 matches the standard test bond; 155% → target = 1550 USD,
        // which is between straight-bond and ITM-parity prices.
        let notional = 1_000.0;
        let quoted_clean_pct = 180.0; // 180% of par = $1800 target dirty (≈ notional-scale)
        let bond = make_bond_with_quote(notional, quoted_clean_pct);
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let market = make_market(as_of);
        let instrument: Arc<dyn Instrument> = Arc::new(bond);
        let base_value = instrument.value(&market, as_of).expect("base value");
        let mut ctx = MetricContext::new(
            instrument,
            Arc::new(market),
            as_of,
            base_value,
            Arc::new(FinstackConfig::default()),
        );
        let result = super::ImpliedVolCalculator.calculate(&mut ctx);
        assert!(
            result.is_ok(),
            "Implied vol solver should converge with correctly scaled target; got: {:?}",
            result.err()
        );
        let ivol = result.unwrap();
        assert!(
            ivol > 0.01 && ivol < 3.0,
            "Implied vol should be in (1%, 300%) range; got {ivol}"
        );
        assert!(ivol.is_finite(), "Implied vol must be finite; got {ivol}");
    }
}
