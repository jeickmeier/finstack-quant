//! Option-Adjusted Spread (OAS) for convertible bonds.
//!
//! OAS solves a parallel decimal intensity adjustment when the instrument has
//! an issuer hazard curve. The full defaultable-equity model is repriced,
//! including survival, recovery and survival-conditioned equity drift, against
//! the settlement-date dirty value implied by the clean quote. This is an
//! intensity-space quotation, not a CDS quote-recalibration sensitivity.
//!
//! Without a hazard curve the solver adjusts the risk-free discount curve,
//! including equity carry. Negative intensities are prohibited, so the lower
//! intensity bound is minus the minimum base hazard rate.
//!
//! # Dependencies
//!
//! Requires `quoted_clean_price_pct` in `bond.instrument_pricing_overrides.market_quotes`.
//!
//! # Units
//!
//! Returned in **decimal** (e.g., 0.01 = 100bp), consistent with other spread
//! metrics in the library.

use crate::convertible::ConvertibleBond;
use crate::convertible::pricing::{price_convertible_bond, settlement_dirty_price};
use finstack_quant_core::Result;
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_valuations_core::metrics::{
    MetricCalculator, MetricContext, bump_discount_curve_parallel,
};

pub(crate) struct OasCalculator;

impl MetricCalculator for OasCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        let as_of = context.as_of;
        let Some(target_dirty) = super::quoted_dirty_target(bond, &context.curves, as_of)? else {
            return Ok(0.0);
        };

        // Use the same tree discretization as the registry pricer and every
        // other convertible metric so the solved OAS reprices to the quote
        // on the production tree (no discretization basis baked into the spread).
        let base_market = context.curves.as_ref();

        // Validate the unbumped pricing path before entering the solver. This
        // surfaces missing curves / vol surfaces / equity IDs with their real
        // error messages instead of letting the solver report opaque "did not
        // converge" failures driven by NaN objective values.
        let _ = price_convertible_bond(bond, base_market, as_of)?;

        // Shift the issuer hazard curve when available (affects the cash/debt
        // component, default recovery and survival-conditioned equity drift).
        // Adjust the discount curve when no credit
        // curve is set.
        let hazard = bond
            .credit_curve_id
            .as_ref()
            .map(|id| base_market.get_hazard(id.as_str()))
            .transpose()?;
        let curve_to_bump = bond
            .credit_curve_id
            .as_ref()
            .unwrap_or(&bond.discount_curve_id);
        let shifted_market = |spread: f64| -> finstack_quant_core::Result<
            finstack_quant_core::market_data::context::MarketContext,
        > {
            let spread_bp = spread * 10_000.0;
            match &hazard {
                Some(curve) => base_market
                    .clone()
                    .try_insert(curve.with_parallel_hazard_rate_bump_bp(spread_bp)?),
                None => bump_discount_curve_parallel(base_market, curve_to_bump, spread_bp),
            }
        };
        // A pricing failure inside the objective aborts the solve; it is
        // reported as the cause rather than as a generic bracket failure.
        let objective = |spread: f64| -> finstack_quant_core::Result<f64> {
            shifted_market(spread)
                .and_then(|bumped| {
                    let pv = price_convertible_bond(bond, &bumped, as_of)?;
                    settlement_dirty_price(bond, &bumped, as_of, pv.amount())
                })
                .map(|dirty| dirty - target_dirty)
                .map_err(|inner| {
                    finstack_quant_core::Error::Validation(format!(
                        "Convertible OAS solver failed because pricing failed inside the \
                         objective: {inner}"
                    ))
                })
        };

        // Lower bound: the most negative spread the shifted curve admits.
        let (lower_spread, intensity_floor) = match &hazard {
            Some(curve) => {
                let min_hazard = curve
                    .knot_points()
                    .map(|(_, lambda)| lambda)
                    .fold(f64::INFINITY, f64::min);
                ((-min_hazard).max(-0.10), Some(0.0))
            }
            None => (-0.10_f64, None),
        };
        let lower_spread = (lower_spread + 1e-12).min(0.0);
        let upper_spread = 0.50;
        let solver = BrentSolver::new(1e-8, 100)?;

        let price_tolerance = 1e-10 * target_dirty.abs().max(1.0);
        if objective(0.0)?.abs() <= price_tolerance {
            return Ok(0.0);
        }
        match solver.solve(objective, 0.0, lower_spread, upper_spread, 0.05, 20) {
            Ok(oas) => Ok(oas),
            Err(mut solver_err) => {
                if let finstack_quant_core::Error::Input(
                    finstack_quant_core::InputError::SolverConvergenceFailed { reason, .. },
                ) = &mut solver_err
                {
                    let floor_note = intensity_floor.map_or_else(String::new, |floor| {
                        format!("; curve imposes an intensity floor of {floor:.6e} (decimal)")
                    });
                    *reason = format!(
                        "Convertible OAS on curve '{curve_to_bump}' failed within decimal spread bounds \
                         [{lower_spread:.6e}, {upper_spread:.6e}]{floor_note}: {reason}",
                    );
                }
                Err(solver_err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::convertible::ConvertibleBond;
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
    use finstack_quant_valuations_core::instruments::{
        AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
    };
    use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
    use time::Month;

    use finstack_quant_cashflows::builder::{CouponType, FixedCouponSpec};
    use finstack_quant_valuations_core::instruments::Instrument;

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

                calendar_id: "weekends_only".into(),

                stub: StubKind::None,

                end_of_month: false,

                payment_lag_days: 0,

                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            },
        };

        let mut overrides = InstrumentPricingOverrides::default();
        overrides.market_quotes.quoted_clean_price_pct = Some(quoted_clean_pct);

        ConvertibleBond {
            id: "TEST_CB_OAS".to_string().into(),
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
            spot_id: "AAPL".into(),
            vol_surface_id: "AAPL-VOL".into(),
            div_yield_id: Some("AAPL-DIVYIELD".into()),
            call_put: None,
            soft_call_trigger: None,
            resolved_settlement: None,
            cashflow_spec: crate::bond::CashflowSpec::Fixed(fixed_coupon),
            instrument_pricing_overrides: overrides,
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        }
    }

    fn make_market(base_date: Date) -> finstack_quant_core::market_data::context::MarketContext {
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (10.0, 0.90)])
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("should succeed");

        finstack_quant_core::market_data::context::MarketContext::new()
            .try_insert(discount_curve)
            .expect("insert market curve")
            .insert_price("AAPL", MarketScalar::unitless(150.0).expect("valid scalar"))
            .insert_price(
                "AAPL-VOL",
                MarketScalar::unitless(0.25).expect("valid scalar"),
            )
            .insert_price(
                "AAPL-DIVYIELD",
                MarketScalar::unitless(0.02).expect("valid scalar"),
            )
    }

    /// Regression: quoted_clean_price_pct is percentage-of-par, so it must be scaled
    /// by notional/100 before adding notional-scaled accrued interest to form the
    /// dirty price target.  Before the fix the solver compared ~$1e6 model price
    /// against ~102 target (pct + accrued), couldn't bracket, and returned an
    /// error or a pinned boundary value.
    ///
    /// With a $1,000,000 notional and a 102.0% clean quote the expected target
    /// dirty is ~$1,020,000 + small accrued — well within the solver's bracket.
    /// The resulting OAS should be a finite decimal spread in (-10%, +50%).
    #[test]
    fn oas_quoted_clean_price_pct_scaled_to_notional() {
        let notional = 1_000_000.0;
        let quoted_clean_pct = 102.0; // 102% of par
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
        let result = super::OasCalculator.calculate(&mut ctx);
        assert!(
            result.is_ok(),
            "OAS solver should converge with correctly scaled target; got: {:?}",
            result.err()
        );
        let oas = result.unwrap();
        assert!(
            oas > -0.10 && oas < 0.50,
            "OAS should be in (-10%, +50%) range; got {oas}"
        );
        assert!(oas.is_finite(), "OAS must be finite; got {oas}");
        let bond: &ConvertibleBond = ctx.instrument_as().unwrap();
        let bumped = super::bump_discount_curve_parallel(
            ctx.curves.as_ref(),
            &bond.discount_curve_id,
            oas * 10_000.0,
        )
        .unwrap();
        let repriced = super::price_convertible_bond(bond, &bumped, as_of).unwrap();
        assert!((repriced.amount() - notional * quoted_clean_pct / 100.0).abs() < 1.0);
    }

    #[test]
    fn oas_search_solves_negative_discount_spread() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, 0.95)])
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap();
        let market = make_market(as_of)
            .try_insert(curve)
            .expect("insert market curve");
        for expected_spread in [-0.01, 0.05] {
            let mut bond = make_bond_with_quote(1_000_000.0, 100.0);
            let bumped = super::bump_discount_curve_parallel(
                &market,
                &bond.discount_curve_id,
                expected_spread * 10_000.0,
            )
            .unwrap();
            let target = super::price_convertible_bond(&bond, &bumped, as_of).unwrap();
            bond.instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price_pct = Some(target.amount() / bond.notional.amount() * 100.0);
            let base_value = bond.value(&market, as_of).unwrap();
            let mut ctx = MetricContext::new(
                Arc::new(bond),
                Arc::new(market.clone()),
                as_of,
                base_value,
                Arc::new(FinstackConfig::default()),
            );
            let solved = super::OasCalculator.calculate(&mut ctx).unwrap();
            assert!((solved - expected_spread).abs() < 1e-8, "{solved}");
        }
    }

    #[test]
    fn oas_floor_curve_reports_search_constraints_and_accepts_boundary_root() {
        use finstack_quant_core::market_data::term_structures::HazardCurve;

        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let hazard = HazardCurve::flat("CREDIT", as_of, 0.005, 0.4, DayCount::Act365F).unwrap();
        let market = make_market(as_of)
            .try_insert(hazard)
            .expect("insert market curve");
        let mut bond = make_bond_with_quote(1_000_000.0, 100.0);
        bond.credit_curve_id = Some("CREDIT".into());
        bond.recovery_rate = Some(0.4);
        // A quote richer than zero-intensity credit spread lies below the
        // admissible non-negative hazard bound (-0.005).
        bond.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct = Some(125.0);
        let base_value = bond.value(&market, as_of).unwrap();
        let mut ctx = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            base_value,
            Arc::new(FinstackConfig::default()),
        );
        let error = super::OasCalculator.calculate(&mut ctx).unwrap_err();
        assert!(matches!(
            error,
            finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::SolverConvergenceFailed { .. }
            )
        ));
        let message = error.to_string();
        for detail in [
            "Convertible OAS",
            "CREDIT",
            "decimal spread bounds",
            "intensity floor",
            "no sign change",
        ] {
            assert!(message.contains(detail), "{message}");
        }
    }

    #[test]
    fn oas_shifts_the_issuer_hazard_curve() {
        use finstack_quant_core::market_data::term_structures::HazardCurve;

        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let hazard = HazardCurve::flat("CREDIT", as_of, 0.02, 0.4, DayCount::Act365F).unwrap();
        let market = make_market(as_of)
            .try_insert(hazard.clone())
            .expect("insert market curve");
        // Spreads down to -λ are admissible; each target is priced on the
        // hazard curve shifted by exactly that spread.
        for expected_spread in [-0.01, 0.0, 0.05] {
            let mut bond = make_bond_with_quote(1_000_000.0, 100.0);
            bond.credit_curve_id = Some("CREDIT".into());
            bond.recovery_rate = Some(0.4);
            let shifted = market
                .clone()
                .try_insert(
                    hazard
                        .with_parallel_hazard_rate_bump_bp(expected_spread * 10_000.0)
                        .unwrap(),
                )
                .expect("insert market curve");
            let target = super::price_convertible_bond(&bond, &shifted, as_of).unwrap();
            bond.instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price_pct = Some(target.amount() / bond.notional.amount() * 100.0);
            let base_value = bond.value(&market, as_of).unwrap();
            let mut ctx = MetricContext::new(
                Arc::new(bond),
                Arc::new(market.clone()),
                as_of,
                base_value,
                Arc::new(FinstackConfig::default()),
            );
            let solved = super::OasCalculator.calculate(&mut ctx).unwrap();
            assert!((solved - expected_spread).abs() < 1e-8, "{solved}");
        }
    }
}
