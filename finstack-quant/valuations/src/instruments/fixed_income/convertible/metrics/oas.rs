//! Option-Adjusted Spread (OAS) for convertible bonds.
//!
//! OAS is the constant spread added to the credit/risky discount curve such that
//! the Tsiveriotis-Zhang tree-based model price equals the market-quoted clean
//! price. It isolates the residual credit component after removing the value of
//! embedded equity conversion, call, and put options.
//!
//! When a separate `credit_curve_id` is configured, OAS bumps that curve only
//! (affecting the cash/debt component while leaving equity drift unchanged).
//! When no credit curve is set, the risk-free discount curve is bumped as a
//! fallback, which also shifts the equity component's drift.
//!
//! # Dependencies
//!
//! Requires `quoted_clean_price` in `bond.instrument_pricing_overrides.market_quotes`.
//!
//! # Units
//!
//! Returned in **decimal** (e.g., 0.01 = 100bp), consistent with other spread
//! metrics in the library.

use std::cell::Cell;

use crate::instruments::fixed_income::convertible::pricing::{
    calculate_accrued_interest, price_convertible_bond, ConvertibleTreeType,
};
use crate::instruments::fixed_income::convertible::ConvertibleBond;
use crate::metrics::{bump_discount_curve_parallel, MetricCalculator, MetricContext};
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::Result;

pub(crate) struct OasCalculator;

impl MetricCalculator for OasCalculator {
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

        let accrued = calculate_accrued_interest(bond, &context.curves, as_of)?;
        // `quoted_clean` is percentage-of-par (e.g. 99.5 = 99.5% of face).
        // `accrued` and the model price are both notional-scaled currency amounts.
        // Scale the percentage quote to notional so the solver objective compares
        // commensurate values — mirroring the term-loan `target_price_from_quote_or_model`.
        let target_dirty = quoted_clean * bond.notional.amount() / 100.0 + accrued;

        // Use the same tree discretization as the registry pricer and every
        // other convertible metric so the solved OAS reprices to the quote
        // on the production tree (no discretization basis baked into the spread).
        let tree_type = ConvertibleTreeType::default();
        let base_market = context.curves.as_ref();

        // The quoted clean price is a *settlement-date* price, so the model PV
        // (computed at `as_of`) is forward-valued to settlement before comparison.
        // With the default `settlement_days = None`, settle == as_of and this
        // factor is 1.0 (no change); it only matters when a settlement lag is set.
        let settle =
            crate::instruments::fixed_income::convertible::pricing::settlement_date(bond, as_of)?;
        let settle_df = if settle > as_of {
            base_market
                .get_discount(bond.discount_curve_id.as_str())?
                .df_between_dates(as_of, settle)?
        } else {
            1.0
        };

        // Bump the credit curve when available (affects cash/debt component only
        // in TZ). Fall back to discount curve when no separate credit curve is set.
        let curve_to_bump = bond
            .credit_curve_id
            .as_ref()
            .unwrap_or(&bond.discount_curve_id);

        // Validate the unbumped pricing path before entering the solver. This
        // surfaces missing curves / vol surfaces / equity IDs with their real
        // error messages instead of letting the solver report opaque "did not
        // converge" failures driven by NaN objective values.
        let _ = price_convertible_bond(bond, base_market, tree_type, as_of)?;

        // Capture the first pricing error from inside the closure so that if the
        // solver bails we can report the underlying cause rather than a generic
        // bracket failure. We keep the *first* error: subsequent failures don't
        // overwrite it. `Cell::take` returns the existing value (clearing it),
        // and `Option::or` keeps the first `Some` while preferring it over a
        // later `Some(e)`. Naively using `if take().is_none() { set(Some(e)) }`
        // would lose the captured error on the second failure (take clears it,
        // is_none() is false, no re-set).
        let captured_err: Cell<Option<finstack_quant_core::Error>> = Cell::new(None);
        let record_err = |e: finstack_quant_core::Error| {
            let prev = captured_err.take();
            captured_err.set(prev.or(Some(e)));
        };
        let objective = |spread: f64| -> f64 {
            let spread_bp = spread * 10_000.0;
            let bumped = match bump_discount_curve_parallel(base_market, curve_to_bump, spread_bp) {
                Ok(m) => m,
                Err(e) => {
                    record_err(e);
                    return f64::NAN;
                }
            };
            match price_convertible_bond(bond, &bumped, tree_type, as_of) {
                Ok(pv) => pv.amount() / settle_df - target_dirty,
                Err(e) => {
                    record_err(e);
                    f64::NAN
                }
            }
        };

        let curve = base_market.get_discount(curve_to_bump.as_str())?;
        let mut forward_floor = curve.min_forward_rate().unwrap_or(f64::NEG_INFINITY);
        if !curve.allows_non_monotonic() {
            forward_floor = forward_floor.max(0.0);
        }
        let lower_spread = curve.knots().windows(2).zip(curve.dfs().windows(2)).fold(
            -0.10_f64,
            |lower, (times, dfs)| {
                let forward = -(dfs[1] / dfs[0]).ln() / (times[1] - times[0]);
                lower.max(forward_floor - forward)
            },
        );
        let lower_spread = (lower_spread + 1e-12).min(0.0);
        let solver = BrentSolver::new()
            .tolerance(1e-8)
            .max_iterations(100)
            .bracket_bounds(lower_spread, 0.50); // Curve-policy floor to +5000bp in decimal

        match solver.solve(objective, 0.0) {
            Ok(oas) => Ok(oas),
            Err(mut solver_err) => {
                if let Some(inner) = captured_err.take() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Convertible OAS solver failed because pricing failed inside the \
                         objective: {inner}"
                    )));
                }
                if let finstack_quant_core::Error::Input(
                    finstack_quant_core::InputError::SolverConvergenceFailed { reason, .. },
                ) = &mut solver_err
                {
                    *reason = format!(
                        "Convertible OAS on curve '{curve_to_bump}' failed within decimal spread bounds \
                         [{lower_spread:.6e}, {upper_spread:.6e}]; curve policy imposes an effective \
                         forward floor of {forward_floor:.6e} (decimal): {reason}",
                        upper_spread = solver.bracket_max,
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

    use crate::cashflow::builder::specs::{CouponType, FixedCouponSpec};
    use crate::instruments::common_impl::traits::Instrument;

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
            .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02))
    }

    /// Regression: quoted_clean_price is percentage-of-par, so it must be scaled
    /// by notional/100 before adding notional-scaled accrued interest to form the
    /// dirty price target.  Before the fix the solver compared ~$1e6 model price
    /// against ~102 target (pct + accrued), couldn't bracket, and returned an
    /// error or a pinned boundary value.
    ///
    /// With a $1,000,000 notional and a 102.0% clean quote the expected target
    /// dirty is ~$1,020,000 + small accrued — well within the solver's bracket.
    /// The resulting OAS should be a finite decimal spread in (-10%, +50%).
    #[test]
    fn oas_quoted_clean_price_scaled_to_notional() {
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
        let repriced = super::price_convertible_bond(
            bond,
            &bumped,
            super::ConvertibleTreeType::default(),
            as_of,
        )
        .unwrap();
        assert!((repriced.amount() - notional * quoted_clean_pct / 100.0).abs() < 1.0);
    }

    #[test]
    fn oas_search_respects_negative_rate_friendly_floor() {
        use finstack_quant_core::market_data::term_structures::ValidationMode;

        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, 0.95)])
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .validation(ValidationMode::NegativeRateFriendly {
                forward_floor: -0.005,
            })
            .build()
            .unwrap();
        let market = make_market(as_of).insert(curve);
        for expected_spread in [-0.01, 0.05] {
            let mut bond = make_bond_with_quote(1_000_000.0, 100.0);
            let bumped = super::bump_discount_curve_parallel(
                &market,
                &bond.discount_curve_id,
                expected_spread * 10_000.0,
            )
            .unwrap();
            let target = super::price_convertible_bond(
                &bond,
                &bumped,
                super::ConvertibleTreeType::default(),
                as_of,
            )
            .unwrap();
            bond.instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price = Some(target.amount() / bond.notional.amount() * 100.0);
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
        use finstack_quant_core::market_data::term_structures::ValidationMode;

        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        for (end_df, policy) in [
            (1.0, ValidationMode::MarketStandard),
            (
                1.05,
                ValidationMode::NegativeRateFriendly {
                    forward_floor: -1.05_f64.ln() / 10.0,
                },
            ),
        ] {
            for separate_credit in [false, true] {
                let curve_id = if separate_credit { "CREDIT" } else { "USD-OIS" };
                let curve_builder = || {
                    DiscountCurve::builder(curve_id)
                        .base_date(as_of)
                        .knots([(0.0, 1.0), (10.0, end_df)])
                        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
                };
                let unrestricted = curve_builder()
                    .validation(ValidationMode::Raw {
                        allow_non_monotonic: true,
                        forward_floor: None,
                    })
                    .build()
                    .unwrap();
                let market =
                    make_market(as_of).insert(curve_builder().validation(policy).build().unwrap());
                let unrestricted_market = market.clone().insert(unrestricted);
                for expected_spread in [-0.01, 0.0, 0.05] {
                    let mut bond = make_bond_with_quote(1_000_000.0, 100.0);
                    if separate_credit {
                        bond.credit_curve_id = Some(curve_id.into());
                        bond.recovery_rate = Some(0.4);
                    }
                    let bumped = super::bump_discount_curve_parallel(
                        &unrestricted_market,
                        &curve_id.into(),
                        expected_spread * 10_000.0,
                    )
                    .unwrap();
                    let target = super::price_convertible_bond(
                        &bond,
                        &bumped,
                        super::ConvertibleTreeType::default(),
                        as_of,
                    )
                    .unwrap();
                    bond.instrument_pricing_overrides
                        .market_quotes
                        .quoted_clean_price =
                        Some(target.amount() / bond.notional.amount() * 100.0);
                    let base_value = bond.value(&market, as_of).unwrap();
                    let mut ctx = MetricContext::new(
                        Arc::new(bond),
                        Arc::new(market.clone()),
                        as_of,
                        base_value,
                        Arc::new(FinstackConfig::default()),
                    );
                    let result = super::OasCalculator.calculate(&mut ctx);
                    if expected_spread < 0.0 {
                        let error = result.unwrap_err();
                        assert!(matches!(
                            error,
                            finstack_quant_core::Error::Input(
                                finstack_quant_core::InputError::SolverConvergenceFailed { .. }
                            )
                        ));
                        let message = error.to_string();
                        for detail in [
                            "Convertible OAS",
                            curve_id,
                            "decimal spread bounds",
                            "forward floor",
                            "no sign change",
                        ] {
                            assert!(message.contains(detail), "{message}");
                        }
                        let mut unrestricted_ctx = MetricContext::new(
                            Arc::clone(&ctx.instrument),
                            Arc::new(unrestricted_market.clone()),
                            as_of,
                            base_value,
                            Arc::new(FinstackConfig::default()),
                        );
                        let solved = super::OasCalculator
                            .calculate(&mut unrestricted_ctx)
                            .unwrap();
                        assert!((solved - expected_spread).abs() < 1e-8, "{solved}");
                    } else {
                        let solved = result.unwrap();
                        assert!((solved - expected_spread).abs() < 1e-8, "{solved}");
                    }
                }
            }
        }
    }

    /// Regression test for the Cell-based error capture pattern.
    ///
    /// An earlier version used `if captured_err.take().is_none() { set(Some(e)) }`
    /// which clears the captured value on the second call (take consumes the
    /// existing Some, is_none() is false on the just-cleared Cell, no re-set
    /// happens). The fix is `take().or(Some(e))` so the first error is always
    /// retained across N failures.
    #[test]
    fn record_err_keeps_first_error_across_multiple_failures() {
        use std::cell::Cell;

        let captured_err: Cell<Option<finstack_quant_core::Error>> = Cell::new(None);
        let record_err = |e: finstack_quant_core::Error| {
            let prev = captured_err.take();
            captured_err.set(prev.or(Some(e)));
        };

        // Simulate the solver objective firing three errors in sequence.
        record_err(finstack_quant_core::Error::Validation("first".into()));
        record_err(finstack_quant_core::Error::Validation("second".into()));
        record_err(finstack_quant_core::Error::Validation("third".into()));

        let captured = captured_err.take();
        assert!(captured.is_some(), "first error should be retained");
        let msg = captured.map(|e| e.to_string()).unwrap_or_default();
        assert!(
            msg.contains("first"),
            "expected first error to be preserved, got: {msg}"
        );
    }
}
