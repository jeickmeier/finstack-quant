//! Carry decomposition calculator.
//!
//! Computes carry as a decomposition into coupon income, pull-to-par, roll-down,
//! and optional funding cost.
//!
//! Pull-to-par uses the constant-yield accretion identity: holding the solved
//! yield fixed, the dirty PV accretes by `1/DF_y(tau)` over the horizon, and
//! coupons paid inside the horizon are carved out (they are reported
//! separately as coupon income). Roll-down is the residual
//! `total_pv_change - pull_to_par`, so on a flat curve self-consistent with
//! the yield it vanishes. The accretion inverts the same discounting
//! convention the instrument's `Ytm` metric was solved under (Street
//! compounding for bonds).

use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
    df_from_yield, YieldCompounding,
};
use crate::instruments::Bond;
use crate::metrics::sensitivities::theta::{
    calculate_theta_date, collect_cashflows_in_period_cached,
};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::math::Compounding;
use finstack_quant_core::Result;

/// Computes carry decomposition and stores all components in `context.computed`.
#[derive(Default)]
pub(crate) struct CarryDecompositionCalculator;

impl MetricCalculator for CarryDecompositionCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let period_str = context
            .get_metric_overrides()
            .and_then(|po| po.theta_period.as_deref())
            .unwrap_or("1D");

        let expiry_date = context.instrument.expiry();
        let rolled_date = calculate_theta_date(context.as_of, period_str, expiry_date)?;

        if rolled_date <= context.as_of {
            context.computed.insert(MetricId::CouponIncome, 0.0);
            context.computed.insert(MetricId::PullToPar, 0.0);
            context.computed.insert(MetricId::RollDown, 0.0);
            context.computed.insert(MetricId::FundingCost, 0.0);
            context
                .computed
                .insert(MetricId::CarryDecompositionDegenerate, 0.0);
            return Ok(0.0);
        }

        let base_pv = context.base_value.amount();
        let base_currency = context.base_value.currency();

        let start_date = context.as_of;
        let coupon_income =
            collect_cashflows_in_period_cached(context, start_date, rolled_date, base_currency)?;

        let curved_pv = context
            .reprice_money(context.curves.as_ref(), rolled_date)?
            .amount();
        let total_pv_change = curved_pv - base_pv;

        // `degenerate` is set when the pull-to-par / roll-down split cannot be
        // computed because the `Ytm` metric is unavailable for this instrument.
        // In that case `pull_to_par` is reported as `0.0` and `roll_down`
        // absorbs the entire PV change — a silent collapse that the
        // `CarryDecompositionDegenerate` diagnostic surfaces so downstream
        // consumers do not mistake the absorbed amount for genuine roll-down.
        let (pull_to_par, degenerate) = if let Some(&ytm) = context.computed.get(&MetricId::Ytm) {
            // Constant-yield accretion identity (Tuckman & Serrat, "Fixed
            // Income Securities", carry/roll-down decomposition): holding the
            // yield fixed, the dirty PV accretes by `1/DF_y(tau)` over the
            // horizon, and cash paid out inside the horizon is carved out so
            // it is not double counted against `coupon_income`. For a par
            // bond over a full coupon period this is ~0; for a zero-coupon
            // bond it is the full accretion.
            let accretion = ytm_accretion_factor(context, ytm, rolled_date)?;
            (base_pv * (accretion - 1.0) - coupon_income, false)
        } else {
            tracing::warn!(
                instrument_id = context.instrument.id(),
                "Carry decomposition missing YTM dependency; pull_to_par reported as 0.0, \
                     roll_down absorbs the remaining PV change, and \
                     carry_decomposition_degenerate is flagged 1.0"
            );
            (0.0, true)
        };

        let roll_down = total_pv_change - pull_to_par;
        let funding_cost = compute_funding_cost(context, rolled_date)?;
        let carry_total = coupon_income + pull_to_par + roll_down - funding_cost;

        context
            .computed
            .insert(MetricId::CouponIncome, coupon_income);
        context.computed.insert(MetricId::PullToPar, pull_to_par);
        context.computed.insert(MetricId::RollDown, roll_down);
        context.computed.insert(MetricId::FundingCost, funding_cost);
        context.computed.insert(
            MetricId::CarryDecompositionDegenerate,
            if degenerate { 1.0 } else { 0.0 },
        );
        // Stamp the realized horizon so consumers rescaling these period
        // totals (e.g. P&L attribution) can normalize instead of assuming 1D.
        context.computed.insert(
            MetricId::ThetaPeriodDays,
            (rolled_date - context.as_of).whole_days() as f64,
        );

        Ok(carry_total)
    }

    fn dependencies(&self) -> &[MetricId] {
        static DEPS: &[MetricId] = &[MetricId::Ytm];
        DEPS
    }
}

/// Accretion factor `1 / DF_y(tau)` over `[as_of, rolled_date]` under the same
/// discounting convention the instrument's `Ytm` metric was solved with.
///
/// Bond YTMs are Street-compounded at the bond's coupon frequency and day
/// count (see `YtmCalculator` / `solve_ytm`), so the accretion must invert
/// `(1 + y/f)^(-f*tau)` measured with those conventions. Non-bond instruments
/// that expose a `Ytm` metric (term loans via XIRR, structured credit by
/// default) solve annually compounded yields, so those fall back to
/// `(1 + y)^tau` using the context day count when one was stamped.
fn ytm_accretion_factor(context: &MetricContext, ytm: f64, rolled_date: Date) -> Result<f64> {
    let (compounding, frequency, day_count) =
        if let Some(bond) = context.instrument.as_any().downcast_ref::<Bond>() {
            (
                YieldCompounding::Street,
                bond.cashflow_spec.frequency(),
                bond.cashflow_spec.day_count(),
            )
        } else {
            (
                YieldCompounding::Annual,
                Tenor::annual(),
                context.day_count.unwrap_or(DayCount::Act365F),
            )
        };

    // ACT/ACT (ICMA) requires the coupon frequency in the day-count context
    // (mirrors `price_from_ytm_compounded_params`). The schedule-derived
    // reference coupon period lets ISMA resolve the mid-coupon carry horizon,
    // which is never a whole number of coupons.
    let dc_ctx = DayCountContext {
        frequency: Some(frequency),
        coupon_period: context.cashflows.as_deref().and_then(|flows| {
            crate::instruments::fixed_income::bond::pricing::quote_conversions::icma_reference_period(
                day_count,
                frequency,
                flows,
                context.as_of,
            )
        }),
        ..DayCountContext::default()
    };
    let tau = day_count.year_fraction(context.as_of, rolled_date, dc_ctx)?;
    let df = df_from_yield(ytm, tau, compounding, frequency)?;
    Ok(1.0 / df)
}

fn compute_funding_cost(context: &MetricContext, rolled_date: Date) -> Result<f64> {
    let Some(funding_curve_id) = context.instrument.funding_curve_id() else {
        return Ok(0.0);
    };

    let funding_curve = context.curves.get_discount(funding_curve_id.as_str())?;
    let annual_rate = funding_curve.zero_rate_on_date(rolled_date, Compounding::Continuous)?;
    let day_count = context.day_count.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "funding cost for '{}' requires an explicit day-count convention",
            context.instrument.id()
        ))
    })?;
    let dcf = day_count.year_fraction(context.as_of, rolled_date, DayCountContext::default())?;

    // `annual_rate` is a continuously compounded zero, so the funding accrual
    // over the horizon is PV * (exp(r*dcf) - 1), not simple-interest PV*r*dcf.
    Ok(context.base_value.amount() * ((annual_rate * dcf).exp() - 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
        df_from_yield, YieldCompounding,
    };
    use crate::instruments::fixed_income::bond::CashflowSpec;
    use crate::instruments::Bond;
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, DayCountContext, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use std::sync::Arc;
    use time::macros::date;

    fn flat_discount_curve(
        id: &str,
        rate: f64,
        base_date: finstack_quant_core::dates::Date,
    ) -> DiscountCurve {
        let knots: Vec<(f64, f64)> = (0..=20)
            .map(|i| {
                let t = i as f64 * 0.5;
                (t, (-rate * t).exp())
            })
            .collect();

        DiscountCurve::builder(id)
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots(knots)
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("flat discount curve")
    }

    fn zero_coupon_bond() -> Bond {
        Bond::fixed(
            "ZERO",
            Money::from((100_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.0).expect("valid rate fixture"),
            date!(2025 - 01 - 15),
            date!(2026 - 01 - 15),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("zero coupon bond")
    }

    /// Semiannual fixed bond accruing Act/365F so bond day-count time matches
    /// the (Act/365F) test discount curve time exactly.
    fn act365_semi_bond(
        id: &str,
        coupon_rate: f64,
        issue: finstack_quant_core::dates::Date,
        maturity: finstack_quant_core::dates::Date,
    ) -> Bond {
        Bond::builder()
            .id(id.into())
            .notional(Money::from((100_i64, Currency::USD)))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(coupon_rate, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .build()
            .expect("bond")
    }

    /// Street semi-annual yield equivalent to a continuously compounded flat
    /// rate `r`: `(1 + y/2)^2 = exp(r)`, so per-flow Street discount factors
    /// coincide with the flat curve's continuous discount factors and the
    /// market is self-consistent with the injected YTM.
    fn street_equivalent_yield(r: f64) -> f64 {
        2.0 * ((r / 2.0).exp() - 1.0)
    }

    fn context_for(
        bond: Bond,
        market: MarketContext,
        as_of: finstack_quant_core::dates::Date,
        theta_period: &str,
        ytm: Option<f64>,
    ) -> MetricContext {
        let instrument: Arc<dyn Instrument> = Arc::new(bond);
        let base_value = instrument.value(&market, as_of).expect("base pv");
        let mut context = MetricContext::new(
            Arc::clone(&instrument),
            Arc::new(market),
            as_of,
            base_value,
            Arc::new(FinstackConfig::default()),
        );
        let overrides = crate::instruments::MetricPricingOverrides::default()
            .with_theta_period(theta_period.to_string());
        context.set_metric_overrides(Some(overrides));
        if let Some(ytm) = ytm {
            context.computed.insert(MetricId::Ytm, ytm);
        }
        context
    }

    #[test]
    fn test_zero_horizon_sets_all_components_to_zero() {
        let as_of = date!(2025 - 01 - 15);
        let bond = zero_coupon_bond();
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.05, as_of));
        let mut context = context_for(bond, market, as_of, "0D", Some(0.05));

        let total = CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("carry decomposition should calculate");

        assert_eq!(total, 0.0);
        assert_eq!(context.computed.get(&MetricId::CouponIncome), Some(&0.0));
        assert_eq!(context.computed.get(&MetricId::PullToPar), Some(&0.0));
        assert_eq!(context.computed.get(&MetricId::RollDown), Some(&0.0));
        assert_eq!(context.computed.get(&MetricId::FundingCost), Some(&0.0));
    }

    /// Test B: for a zero-coupon bond, pull-to-par is exactly the Street-yield
    /// accretion `base_pv * ((1 + y/f)^(f*tau) - 1)` over the horizon, and on a
    /// self-consistent flat curve there is no roll-down.
    ///
    /// The injected YTM is the Street semi-annual equivalent of the flat
    /// continuous curve rate (previously this test injected the continuous
    /// rate 0.05 directly, which only looked self-consistent because the old
    /// implementation built its flat curve as `exp(-ytm*t)` — the compounding
    /// mismatch this fix removes).
    #[test]
    fn test_zero_coupon_bond_flat_curve_has_positive_pull_to_par_and_no_roll_down() {
        let as_of = date!(2025 - 01 - 15);
        let bond = act365_semi_bond("ZERO365", 0.0, as_of, date!(2026 - 01 - 15));
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.05, as_of));
        let ytm = street_equivalent_yield(0.05);
        let mut context = context_for(bond, market, as_of, "1M", Some(ytm));
        let base_pv = context.base_value.amount();

        let total = CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("carry decomposition should calculate");

        let coupon_income = *context
            .computed
            .get(&MetricId::CouponIncome)
            .expect("coupon income");
        let pull_to_par = *context
            .computed
            .get(&MetricId::PullToPar)
            .expect("pull to par");
        let roll_down = *context
            .computed
            .get(&MetricId::RollDown)
            .expect("roll down");

        // Expected accretion over [2025-01-15, 2025-02-15) under the same
        // Street discounting the YTM is quoted in.
        let rolled = date!(2025 - 02 - 15);
        let tau = DayCount::Act365F
            .year_fraction(as_of, rolled, DayCountContext::default())
            .expect("tau");
        let df = df_from_yield(ytm, tau, YieldCompounding::Street, Tenor::semi_annual())
            .expect("street df");
        let expected_pull = base_pv * (1.0 / df - 1.0);

        assert!(coupon_income.abs() < 1e-12);
        assert!(pull_to_par > 0.0);
        assert!(
            (pull_to_par - expected_pull).abs() < 1e-9,
            "pull_to_par {pull_to_par} should equal Street accretion {expected_pull}"
        );
        assert!(
            roll_down.abs() < 1e-8,
            "flat self-consistent curve should have no roll-down, got {roll_down}"
        );
        assert!((total - pull_to_par).abs() < 1e-8);
    }

    /// Test A: a semiannual coupon bond over a horizon that crosses a coupon
    /// date, on a flat market curve self-consistent with the injected Street
    /// YTM. Pull-to-par must be accretion-sized (yield income net of the
    /// coupon; a few cents per 100 for a near-par bond over a full coupon
    /// period), NOT approximately minus the whole coupon, and roll-down on a
    /// flat self-consistent curve must vanish (up to the tiny in-horizon
    /// coupon reinvestment residual).
    #[test]
    fn test_coupon_bond_flat_curve_pull_to_par_is_accretion_and_roll_down_vanishes() {
        let as_of = date!(2025 - 01 - 20);
        // Coupons on Jan-15 / Jul-15; the 6M horizon [2025-01-20, 2025-07-20)
        // contains the 2025-07-15 coupon (~2.5 per 100).
        let bond = act365_semi_bond("CPN5", 0.05, date!(2025 - 01 - 15), date!(2030 - 01 - 15));
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.05, as_of));
        let ytm = street_equivalent_yield(0.05);
        let mut context = context_for(bond, market, as_of, "6M", Some(ytm));

        let total = CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("carry decomposition should calculate");

        let coupon_income = *context
            .computed
            .get(&MetricId::CouponIncome)
            .expect("coupon income");
        let pull_to_par = *context
            .computed
            .get(&MetricId::PullToPar)
            .expect("pull to par");
        let roll_down = *context
            .computed
            .get(&MetricId::RollDown)
            .expect("roll down");

        // The in-horizon coupon actually landed in coupon_income.
        assert!(
            coupon_income > 2.0,
            "expected the 2025-07-15 coupon in the horizon, got {coupon_income}"
        );
        // Pull-to-par is accretion minus coupon: small for a near-par bond
        // over a full coupon period. The old flat-curve construction reported
        // ~-0.24 here (coupon contamination + compounding mismatch).
        assert!(
            pull_to_par.abs() < 0.2,
            "pull_to_par should be accretion-sized, got {pull_to_par}"
        );
        // On a self-consistent flat curve rolling down the curve is a no-op.
        assert!(
            roll_down.abs() < 0.02,
            "flat self-consistent curve should have ~no roll-down, got {roll_down}"
        );
        // Identity: total = coupon + pull + roll (no funding leg configured).
        assert!((total - (coupon_income + pull_to_par + roll_down)).abs() < 1e-9);
    }

    #[test]
    fn test_missing_ytm_keeps_pull_to_par_zero() {
        let as_of = date!(2025 - 01 - 15);
        let bond = zero_coupon_bond();
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.05, as_of));
        let mut context = context_for(bond, market, as_of, "1M", None);

        CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("carry decomposition should calculate without YTM");

        assert_eq!(context.computed.get(&MetricId::PullToPar), Some(&0.0));
    }

    /// Audit item #9: when `Ytm` is absent the pull-to-par / roll-down split is
    /// degenerate (pull_to_par forced to 0, roll_down absorbs everything). That
    /// degeneracy must be SURFACED via the `CarryDecompositionDegenerate`
    /// diagnostic rather than silently folded into roll_down.
    #[test]
    fn test_missing_ytm_flags_decomposition_as_degenerate() {
        let as_of = date!(2025 - 01 - 15);
        let bond = zero_coupon_bond();
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.05, as_of));
        let mut context = context_for(bond, market, as_of, "1M", None);

        CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("carry decomposition should calculate without YTM");

        // Degeneracy flag must be raised.
        assert_eq!(
            context
                .computed
                .get(&MetricId::CarryDecompositionDegenerate),
            Some(&1.0),
            "missing YTM must flag the carry decomposition as degenerate"
        );
        // roll_down silently absorbed the whole PV change — exactly the harm
        // the diagnostic warns about.
        let pull_to_par = *context.computed.get(&MetricId::PullToPar).expect("ptp");
        let roll_down = *context.computed.get(&MetricId::RollDown).expect("roll");
        assert_eq!(pull_to_par, 0.0);
        assert!(
            roll_down.abs() > 1e-8,
            "degenerate roll_down should hold the absorbed PV change"
        );
    }

    /// When `Ytm` IS available the split is well-defined and the diagnostic
    /// must report `0.0` (not degenerate).
    #[test]
    fn test_present_ytm_marks_decomposition_non_degenerate() {
        let as_of = date!(2025 - 01 - 15);
        let bond = zero_coupon_bond();
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.05, as_of));
        let mut context = context_for(bond, market, as_of, "1M", Some(0.05));

        CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("carry decomposition should calculate with YTM");

        assert_eq!(
            context
                .computed
                .get(&MetricId::CarryDecompositionDegenerate),
            Some(&0.0),
            "a well-defined split must not be flagged degenerate"
        );
    }

    #[test]
    fn test_funding_cost_uses_bond_day_count_fraction() {
        let as_of = date!(2025 - 01 - 15);
        let mut bond = zero_coupon_bond();
        bond.funding_curve_id = Some(CurveId::new("USD-REPO"));
        let market = MarketContext::new()
            .insert(flat_discount_curve("USD-OIS", 0.05, as_of))
            .insert(flat_discount_curve("USD-REPO", 0.02, as_of));
        let mut context = context_for(bond, market, as_of, "1M", Some(0.05));
        context.day_count = Some(DayCount::Thirty360);

        CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("carry decomposition should calculate");

        let rolled = crate::metrics::calculate_theta_date(as_of, "1M", Some(date!(2026 - 01 - 15)))
            .expect("rolled date");
        let expected_dcf = DayCount::Thirty360
            .year_fraction(as_of, rolled, DayCountContext::default())
            .expect("year fraction");
        // Test C: the funding curve rate is a CONTINUOUSLY compounded zero, so
        // the accrual over the horizon is PV * (exp(r*dcf) - 1), not the
        // simple-interest PV * r * dcf (which understates by ~PV*(r*dcf)^2/2).
        let expected = context.base_value.amount() * ((0.02f64 * expected_dcf).exp() - 1.0);
        let funding_cost = *context
            .computed
            .get(&MetricId::FundingCost)
            .expect("funding cost");

        assert!(
            (funding_cost - expected).abs() < 1e-9,
            "funding_cost {funding_cost} != continuous accrual {expected}"
        );
    }

    #[test]
    fn test_funding_cost_requires_explicit_day_count() {
        let as_of = date!(2025 - 01 - 15);
        let mut bond = zero_coupon_bond();
        bond.funding_curve_id = Some(CurveId::new("USD-REPO"));
        let market = MarketContext::new()
            .insert(flat_discount_curve("USD-OIS", 0.05, as_of))
            .insert(flat_discount_curve("USD-REPO", 0.02, as_of));
        let mut context = context_for(bond, market, as_of, "1M", Some(0.05));

        let err = CarryDecompositionCalculator
            .calculate(&mut context)
            .expect_err("funding cost should not silently default the day-count convention");

        assert!(
            err.to_string().contains("day-count"),
            "expected day-count validation error, got: {err}"
        );
    }

    #[test]
    fn test_standard_registry_exposes_all_carry_metrics() {
        let as_of = date!(2025 - 01 - 15);
        let bond = zero_coupon_bond();
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.05, as_of));

        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[
                    MetricId::CarryTotal,
                    MetricId::CouponIncome,
                    MetricId::PullToPar,
                    MetricId::RollDown,
                    MetricId::FundingCost,
                ],
                crate::instruments::PricingOptions::default(),
            )
            .expect("carry metrics should be registered in the standard registry");

        let carry_total = *result
            .measures
            .get(MetricId::CarryTotal.as_str())
            .expect("carry total");
        let coupon_income = *result
            .measures
            .get(MetricId::CouponIncome.as_str())
            .expect("coupon income");
        let pull_to_par = *result
            .measures
            .get(MetricId::PullToPar.as_str())
            .expect("pull to par");
        let roll_down = *result
            .measures
            .get(MetricId::RollDown.as_str())
            .expect("roll down");
        let funding_cost = *result
            .measures
            .get(MetricId::FundingCost.as_str())
            .expect("funding cost");

        assert!(
            (carry_total - (coupon_income + pull_to_par + roll_down - funding_cost)).abs() < 1e-8
        );
    }
}
