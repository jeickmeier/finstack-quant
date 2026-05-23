//! Carry decomposition calculator.
//!
//! Computes carry as a decomposition into coupon income, pull-to-par, roll-down,
//! and optional funding cost.

use crate::metrics::sensitivities::theta::{
    calculate_theta_date, collect_cashflows_in_period_cached,
};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_core::dates::DayCountContext;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::math::Compounding;
use finstack_core::types::CurveId;
use finstack_core::Result;

/// Computes carry decomposition and stores all components in `context.computed`.
pub(crate) struct CarryDecompositionCalculator;

impl Default for CarryDecompositionCalculator {
    fn default() -> Self {
        Self
    }
}

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
        let base_ccy = context.base_value.currency();

        let start_date = context.as_of;
        let coupon_income =
            collect_cashflows_in_period_cached(context, start_date, rolled_date, base_ccy)?;

        let curved_pv = context
            .instrument_value_with_scenario(context.curves.as_ref(), rolled_date)?
            .amount();
        let total_pv_change = curved_pv - base_pv;

        // `degenerate` is set when the pull-to-par / roll-down split cannot be
        // computed because the `Ytm` metric is unavailable for this instrument.
        // In that case `pull_to_par` is reported as `0.0` and `roll_down`
        // absorbs the entire PV change — a silent collapse that the
        // `CarryDecompositionDegenerate` diagnostic surfaces so downstream
        // consumers do not mistake the absorbed amount for genuine roll-down.
        let (pull_to_par, degenerate) = if let Some(&ytm) = context.computed.get(&MetricId::Ytm) {
            let flat_market = build_flat_curve_market(context, ytm)?;
            let flat_pv = context.reprice_money(&flat_market, rolled_date)?.amount();
            (flat_pv - base_pv, false)
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

        Ok(carry_total)
    }

    fn dependencies(&self) -> &[MetricId] {
        static DEPS: &[MetricId] = &[MetricId::Ytm];
        DEPS
    }
}

fn build_flat_curve_market(context: &MetricContext, ytm: f64) -> Result<MarketContext> {
    let curve_id = discount_curve_id(context)?;
    let original_curve = context.curves.get_discount(curve_id.as_str())?;
    let knots: Vec<(f64, f64)> = (0..=120)
        .map(|i| {
            let t = i as f64 * 0.5;
            (t, (-ytm * t).exp())
        })
        .collect();

    let flat_curve = DiscountCurve::builder(curve_id.as_str())
        .base_date(original_curve.base_date())
        .day_count(original_curve.day_count())
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .build()?;

    Ok(context.curves.as_ref().clone().insert(flat_curve))
}

fn discount_curve_id(context: &MetricContext) -> Result<CurveId> {
    if let Some(id) = &context.discount_curve_id {
        return Ok(id.clone());
    }

    context
        .instrument
        .market_dependencies()?
        .curve_dependencies()
        .discount_curves
        .first()
        .cloned()
        .ok_or_else(|| finstack_core::InputError::NotFound {
            id: format!("discount_curve_for:{}", context.instrument.id()),
        })
        .map_err(Into::into)
}

fn compute_funding_cost(
    context: &MetricContext,
    rolled_date: finstack_core::dates::Date,
) -> Result<f64> {
    let Some(funding_curve_id) = context.instrument.funding_curve_id() else {
        return Ok(0.0);
    };

    let funding_curve = context.curves.get_discount(funding_curve_id.as_str())?;
    let annual_rate = funding_curve.zero_rate_on_date(rolled_date, Compounding::Continuous)?;
    let day_count = context.day_count.ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "funding cost for '{}' requires an explicit day-count convention",
            context.instrument.id()
        ))
    })?;
    let dcf = day_count.year_fraction(context.as_of, rolled_date, DayCountContext::default())?;

    Ok(context.base_value.amount() * annual_rate * dcf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::Bond;
    use finstack_core::config::FinstackConfig;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{DayCount, DayCountContext};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::money::Money;
    use finstack_core::types::CurveId;
    use std::sync::Arc;
    use time::macros::date;

    fn flat_discount_curve(
        id: &str,
        rate: f64,
        base_date: finstack_core::dates::Date,
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
            Money::new(100.0, Currency::USD),
            0.0,
            date!(2025 - 01 - 15),
            date!(2026 - 01 - 15),
            "USD-OIS",
        )
        .expect("zero coupon bond")
    }

    fn context_for(
        bond: Bond,
        market: MarketContext,
        as_of: finstack_core::dates::Date,
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

    #[test]
    fn test_zero_coupon_bond_flat_curve_has_positive_pull_to_par_and_no_roll_down() {
        let as_of = date!(2025 - 01 - 15);
        let bond = zero_coupon_bond();
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.05, as_of));
        let mut context = context_for(bond, market, as_of, "1M", Some(0.05));

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

        assert!(coupon_income.abs() < 1e-12);
        assert!(pull_to_par > 0.0);
        assert!(
            roll_down.abs() < 1e-8,
            "flat curve should have no roll-down"
        );
        assert!((total - pull_to_par).abs() < 1e-8);
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
        let expected = context.base_value.amount() * 0.02 * expected_dcf;
        let funding_cost = *context
            .computed
            .get(&MetricId::FundingCost)
            .expect("funding cost");

        assert!((funding_cost - expected).abs() < 1e-8);
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
