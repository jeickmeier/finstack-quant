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
use crate::instruments::fixed_income::bond::pricing::time_basis::{
    act365l_year_fraction, bond_coupon_periods,
};
use crate::instruments::Bond;
use crate::metrics::sensitivities::theta::{
    calculate_theta_date, collect_period_cash_cached, theta_termination_date,
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
        let expiry_date = theta_termination_date(context)?;
        let period = crate::metrics::theta_period(context);

        let rolled_date = calculate_theta_date(context.as_of, period, expiry_date)?;

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
        let cash = collect_period_cash_cached(context, start_date, rolled_date, base_currency)?;
        let coupon_income = cash.income.amount();
        let principal_cash = cash.total.checked_sub(cash.income)?.amount();

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

        // Principal settlement reduces the live instrument PV without being
        // coupon income or curve roll-down. Neutralize that receipt against
        // the price drop before partitioning the remaining carry.
        let roll_down = total_pv_change + principal_cash - pull_to_par;
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
    let bond = context.instrument.as_any().downcast_ref::<Bond>();
    let (compounding, frequency, day_count) = if let Some(bond) = bond {
        (
            YieldCompounding::Street,
            bond.cashflow_spec.frequency(),
            bond.cashflow_spec.day_count(),
        )
    } else {
        (
            YieldCompounding::Rate(Compounding::Annual),
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
                flows.iter().map(|(d, _)| *d),
                context.as_of,
            )
        }),
        ..DayCountContext::default()
    };
    // ACT/365L must retain each enclosing coupon's denominator when the
    // carry horizon crosses a coupon boundary, just as the bond yield clock does.
    let tau = if let Some(bond) = bond.filter(|_| day_count == DayCount::Act365L) {
        act365l_year_fraction(
            frequency,
            &bond_coupon_periods(bond)?,
            context.as_of,
            rolled_date,
        )?
    } else {
        day_count.year_fraction(context.as_of, rolled_date, dc_ctx)?
    };
    let df = df_from_yield(ytm, tau, compounding, frequency)?;
    Ok(1.0 / df)
}

fn compute_funding_cost(context: &MetricContext, rolled_date: Date) -> Result<f64> {
    let Some(repo_curve_id) = context.instrument.repo_curve_id() else {
        return Ok(0.0);
    };

    let funding_curve = context.curves.get_discount(repo_curve_id.as_str())?;
    let horizon_df = funding_curve.df_between_dates(context.as_of, rolled_date)?;
    Ok(context.base_value.amount() * (1.0 / horizon_df - 1.0))
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
        theta_period: Tenor,
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
        let overrides =
            crate::instruments::MetricPricingOverrides::default().with_theta_period(theta_period);
        context.set_metric_pricing_overrides(Some(overrides));
        if let Some(ytm) = ytm {
            context.computed.insert(MetricId::Ytm, ytm);
        }
        context
    }

    #[test]
    fn test_zero_horizon_sets_all_components_to_zero() {
        // Valued on its maturity date, the horizon is capped at expiry and
        // no time elapses.
        let as_of = date!(2026 - 01 - 15);
        let bond = zero_coupon_bond();
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.05, as_of));
        let mut context = context_for(bond, market, as_of, Tenor::daily(), Some(0.05));

        let total = CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("carry decomposition should calculate");

        assert_eq!(total, 0.0);
        assert_eq!(context.computed.get(&MetricId::CouponIncome), Some(&0.0));
        assert_eq!(context.computed.get(&MetricId::PullToPar), Some(&0.0));
        assert_eq!(context.computed.get(&MetricId::RollDown), Some(&0.0));
        assert_eq!(context.computed.get(&MetricId::FundingCost), Some(&0.0));
    }

    #[test]
    fn act365l_carry_and_theta_cross_coupon_denominators() {
        let as_of = date!(2024 - 09 - 15);
        let rolled = date!(2024 - 12 - 15);
        let mut bond = act365_semi_bond(
            "ACT365L-CARRY",
            0.05,
            date!(2023 - 10 - 15),
            date!(2025 - 10 - 15),
        );
        bond.cashflow_spec = CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365L)
            .expect("finite coupon");
        if let CashflowSpec::Fixed(spec) = &mut bond.cashflow_spec {
            spec.schedule.business_day_convention =
                finstack_quant_core::dates::BusinessDayConvention::Unadjusted;
        }
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.05, as_of));
        let ytm: f64 = 0.047;
        let mut context = context_for(bond, market, as_of, Tenor::quarterly(), Some(ytm));
        let base_pv = context.base_value.amount();

        // The Apr-Oct coupon ends in leap year 2024; the Oct-Apr coupon
        // ends in 2025. A single denominator for this horizon is incorrect.
        let tau = 30.0 / 366.0 + 61.0 / 365.0;
        let accretion = (1.0 + ytm / 2.0).powf(2.0 * tau);
        assert!(
            (ytm_accretion_factor(&context, ytm, rolled).expect("accretion") - accretion).abs()
                < 1e-12
        );
        let total = CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("ACT/365L carry");
        let coupon_income = 100.0 * 0.05 * 183.0 / 366.0;
        assert!((context.computed[&MetricId::CouponIncome] - coupon_income).abs() < 1e-12);
        assert!(
            (context.computed[&MetricId::PullToPar]
                - (base_pv * (accretion - 1.0) - coupon_income))
                .abs()
                < 1e-10
        );
        assert_eq!(
            context.computed[&MetricId::CarryDecompositionDegenerate],
            0.0
        );
        let theta = crate::metrics::sensitivities::theta::GenericThetaAny
            .calculate(&mut context)
            .expect("ACT/365L theta");
        assert!((total - theta).abs() < 1e-10);
        assert_eq!(context.computed[&MetricId::ThetaPeriodDays], 91.0);
    }

    #[test]
    fn zero_coupon_redemption_is_not_coupon_income_or_roll_down() {
        let as_of = date!(2026 - 01 - 14);
        let bond = act365_semi_bond(
            "ZERO-REDEEM",
            0.0,
            date!(2025 - 01 - 15),
            date!(2026 - 01 - 15),
        );
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.05, as_of));
        let mut context = context_for(
            bond,
            market,
            as_of,
            Tenor::daily(),
            Some(street_equivalent_yield(0.05)),
        );
        let total = CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("carry");
        assert_eq!(context.computed[&MetricId::CouponIncome], 0.0);
        assert!(context.computed[&MetricId::PullToPar] > 0.0);
        assert!(context.computed[&MetricId::RollDown].abs() < 1e-8);
        assert!(total > 0.0 && total < 0.1);
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
        let mut context = context_for(bond, market, as_of, Tenor::monthly(), Some(ytm));
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
        let mut context = context_for(bond, market, as_of, Tenor::semi_annual(), Some(ytm));

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
        let mut context = context_for(bond, market, as_of, Tenor::monthly(), None);

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
        let mut context = context_for(bond, market, as_of, Tenor::monthly(), None);

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
        let mut context = context_for(bond, market, as_of, Tenor::monthly(), Some(0.05));

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
    fn test_funding_cost_uses_funding_curve_day_count() {
        let as_of = date!(2025 - 01 - 15);
        let mut bond = zero_coupon_bond();
        bond.repo_curve_id = Some(CurveId::new("USD-REPO"));
        let market = MarketContext::new()
            .insert(flat_discount_curve("USD-OIS", 0.05, as_of))
            .insert(flat_discount_curve("USD-REPO", 0.02, as_of));
        let mut context = context_for(bond, market, as_of, Tenor::monthly(), Some(0.05));
        context.day_count = Some(DayCount::Thirty360);

        CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("carry decomposition should calculate");

        let rolled = crate::metrics::calculate_theta_date(
            as_of,
            Tenor::monthly(),
            Some(date!(2026 - 01 - 15)),
        )
        .expect("rolled date");
        let expected_dcf = DayCount::Act365F
            .year_fraction(as_of, rolled, DayCountContext::default())
            .expect("year fraction");
        // The repo curve uses ACT/365F, independent of the bond's 30/360 basis.
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
    fn test_funding_cost_uses_forward_df_from_curve_base_before_as_of() {
        let as_of = date!(2025 - 01 - 15);
        let rolled = date!(2025 - 02 - 15);
        let mut bond = zero_coupon_bond();
        bond.repo_curve_id = Some(CurveId::new("USD-REPO"));
        let repo = DiscountCurve::builder("USD-REPO")
            .base_date(date!(2024 - 01 - 15))
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::LogLinear)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.01_f64).exp()),
                (2.0, (-0.06_f64).exp()),
                (3.0, (-0.15_f64).exp()),
            ])
            .build()
            .expect("repo curve");
        let t_start = DayCount::Act365F
            .year_fraction(date!(2024 - 01 - 15), as_of, DayCountContext::default())
            .expect("start year fraction");
        let t_end = DayCount::Act365F
            .year_fraction(date!(2024 - 01 - 15), rolled, DayCountContext::default())
            .expect("end year fraction");
        // Both horizon endpoints lie in the second log-linear segment, whose
        // continuously compounded forward rate is 5%, rather than its zero rate.
        let expected_factor = (0.05 * (t_end - t_start)).exp() - 1.0;
        let market = MarketContext::new()
            .insert(flat_discount_curve("USD-OIS", 0.05, as_of))
            .insert(repo);
        let mut context = context_for(bond, market, as_of, Tenor::monthly(), Some(0.05));
        context.day_count = None;
        let funding_cost = compute_funding_cost(&context, rolled)
            .expect("repo accrual does not require the instrument day count");
        assert!((funding_cost - context.base_value.amount() * expected_factor).abs() < 1e-8);

        // A full carry calculation still uses the bond's day count for its
        // separate constant-yield accretion component.
        context.day_count = Some(DayCount::Thirty360);
        CarryDecompositionCalculator
            .calculate(&mut context)
            .expect("carry decomposition");
        assert!((context.computed[&MetricId::FundingCost] - funding_cost).abs() < 1e-8);
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
