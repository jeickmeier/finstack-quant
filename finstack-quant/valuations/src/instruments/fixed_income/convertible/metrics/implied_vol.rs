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

use crate::instruments::fixed_income::convertible::pricing::{
    calculate_accrued_interest, settlement_date,
};
use crate::instruments::fixed_income::convertible::ConvertibleBond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::{Error, Result};

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
                Error::from(finstack_quant_core::InputError::NotFound {
                    id: "pricing_overrides.market_quotes.quoted_clean_price".to_string(),
                })
            })?;
        let accrued = calculate_accrued_interest(bond, &context.curves, as_of)?;
        let target_dirty = quoted_clean * bond.notional.amount() / 100.0 + accrued;
        if !target_dirty.is_finite() || target_dirty <= 0.0 {
            return Err(Error::Validation(
                "convertible implied volatility requires a finite positive dirty-price target"
                    .into(),
            ));
        }
        let settle = settlement_date(bond, as_of)?;
        let settle_df = context
            .curves
            .get_discount(bond.discount_curve_id.as_str())?
            .df_between_dates(as_of, settle)?;

        // Validate the selected pricing path before probing its numerical domain.
        // Inversion changes only the effective equity-volatility quote; every
        // evaluation retains the caller's engine, curves and contractual fixings.
        context.reprice_money(&context.curves, as_of)?;
        let evaluate = |volatility: f64| -> Result<f64> {
            let mut trial = bond.clone();
            trial
                .instrument_pricing_overrides
                .market_quotes
                .implied_volatility = Some(volatility);
            let price = context.reprice_instrument_money(&trial, &context.curves, as_of)?;
            // Price error per unit notional keeps the stopping rule independent
            // of trade scale while preserving the settlement-date quote basis.
            let residual = (price.amount() / settle_df - target_dirty) / bond.notional.amount();
            if !residual.is_finite() {
                return Err(Error::Validation(
                    "non-finite convertible implied-volatility residual".into(),
                ));
            }
            Ok(residual)
        };

        // CRR and trinomial grids have a positive minimum admissible volatility
        // when drift is nonzero. Find that boundary before invoking Brent, so
        // its entire bracket consists of valid lattice evaluations.
        let upper = 3.0;
        let lower = admissible_lower_volatility(&evaluate, upper)?;
        let low_residual = evaluate(lower)?;
        let high_residual = evaluate(upper)?;
        if low_residual.abs() <= 1e-10 {
            return Ok(lower);
        }
        if high_residual.abs() <= 1e-10 {
            return Ok(upper);
        }
        if low_residual.signum() == high_residual.signum() {
            return Err(Error::Validation(format!(
                "convertible clean-price quote is not bracketed on the selected lattice's admissible volatility interval [{lower}, {upper}]; dirty-price residuals per unit notional are {low_residual} and {high_residual}"
            )));
        }
        let captured_err: Cell<Option<Error>> = Cell::new(None);
        let objective = |volatility| match evaluate(volatility) {
            Ok(residual) => residual,
            Err(error) => {
                let previous = captured_err.take();
                captured_err.set(previous.or(Some(error)));
                f64::NAN
            }
        };
        let solved = BrentSolver::new()
            .tolerance(1e-12)
            .max_iterations(100)
            .bracket_bounds(lower, upper)
            .solve(objective, 0.25_f64.clamp(lower, upper));
        if let Some(error) = captured_err.take() {
            return Err(error);
        }
        let volatility = solved?;
        let residual = evaluate(volatility)?;
        if residual.abs() > 1e-10 {
            return Err(Error::Validation(format!("convertible implied volatility failed repricing: dirty-price residual per unit notional {residual}")));
        }
        Ok(volatility)
    }
}

/// Locate the selected lattice's lower numerical boundary; the base pricing
/// path has already validated all non-volatility inputs. Invalid probes remain
/// outside the root solver's bracket and are never converted into prices.
fn admissible_lower_volatility(evaluate: &impl Fn(f64) -> Result<f64>, upper: f64) -> Result<f64> {
    let mut lower = 1e-8;
    let mut invalid = 0.0;
    loop {
        match evaluate(lower) {
            Ok(_) => break,
            Err(error) => {
                if lower >= upper {
                    return Err(error);
                }
                invalid = lower;
                lower = (lower * 2.0).min(upper);
            }
        }
    }
    if invalid > 0.0 {
        for _ in 0..32 {
            let middle = 0.5 * (invalid + lower);
            if evaluate(middle).is_ok() {
                lower = middle;
            } else {
                invalid = middle;
            }
        }
        // Stay strictly inside the admissible interval to avoid rounding at p=0/1.
        lower *= 1.0 + 1e-8;
        evaluate(lower)?;
    }
    Ok(lower)
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
                frequency: Tenor::semi_annual(),

                day_count: DayCount::Act365F,

                business_day_convention: BusinessDayConvention::Following,

                calendar_id: "weekends_only".to_string(),

                stub: StubKind::None,

                end_of_month: false,

                payment_lag_days: 0,

                adjust_accrual_dates: false,
                roll_rule: crate::cashflow::builder::specs::RollRule::None,
            },
        };

        let mut overrides = InstrumentPricingOverrides::default();
        overrides.market_quotes.quoted_clean_price = Some(quoted_clean_pct);

        ConvertibleBond {
            id: "TEST_CB_IVOL".to_string().into(),
            notional: Money::new(notional_usd, Currency::USD).expect("valid money fixture"),
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

    /// Regression: the quoted clean price is a settlement-date price, so the
    /// solver objective must forward-value the model PV to settlement (divide
    /// by DF(as_of → settle)) exactly like the OAS objective. Before the fix,
    /// bonds with `settlement_days` set solved against a target mismatched by
    /// the settlement discount factor. Verified by round-trip: repricing with
    /// the solved vol and forward-valuing must recover the dirty target.
    #[test]
    fn implied_vol_forward_values_to_settlement_date() {
        use crate::instruments::fixed_income::convertible::pricing::{
            calculate_accrued_interest, price_convertible_bond, settlement_date,
            ConvertibleTreeType,
        };

        let notional = 1_000.0;
        let quoted_clean_pct = 180.0;
        let mut bond = make_bond_with_quote(notional, quoted_clean_pct);
        bond.settlement_days = Some(2); // T+2, US corporate convention
        let as_of = Date::from_calendar_date(2025, Month::June, 2).expect("valid date");
        let market = make_market(as_of);

        let instrument: Arc<dyn Instrument> = Arc::new(bond.clone());
        let base_value = instrument.value(&market, as_of).expect("base value");
        let mut ctx = MetricContext::new(
            instrument,
            Arc::new(market.clone()),
            as_of,
            base_value,
            Arc::new(FinstackConfig::default()),
        );
        let ivol = super::ImpliedVolCalculator
            .calculate(&mut ctx)
            .expect("implied vol should converge with settlement lag");

        // Round-trip: reprice at the solved vol, forward-value to settlement,
        // and compare against the dirty target implied by the quote.
        let settle = settlement_date(&bond, as_of).expect("settlement date should resolve");
        assert!(settle > as_of, "T+2 settlement must roll forward");
        let settle_df = market
            .get_discount("USD-OIS")
            .expect("curve")
            .df_between_dates(as_of, settle)
            .expect("df");
        let accrued = calculate_accrued_interest(&bond, &market, as_of).expect("accrued");
        let target_dirty = quoted_clean_pct * notional / 100.0 + accrued;

        let repriced = market.insert_price("AAPL-VOL", MarketScalar::Unitless(ivol));
        let pv = price_convertible_bond(&bond, &repriced, ConvertibleTreeType::default(), as_of)
            .expect("reprice at solved vol");
        let recovered = pv.amount() / settle_df;
        assert!(
            (recovered - target_dirty).abs() < 1e-3,
            "round-trip at solved vol must recover the settlement-date dirty target: \
             got {recovered}, want {target_dirty}"
        );
    }
}
