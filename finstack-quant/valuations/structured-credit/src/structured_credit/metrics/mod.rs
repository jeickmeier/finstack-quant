//! Metrics for structured credit instruments.
//!

//! This module organizes metrics by category:
//! - pricing: Valuation-focused metrics (prices, accrued, WAL)
//! - risk: Risk and sensitivity metrics (duration, spreads, YTM)
//! - pool: Collateral pool characteristics (WAM, CPR, CDR, WARF, WAS)
//! - deal_specific: Deal-type specific metrics (ABS, CLO, CMBS, RMBS)

use finstack_quant_core::market_data::traits::Discounting;

/// Time basis shared by every structured-credit risk metric.
///
/// Duration measured time with the DISCOUNT CURVE's day count while
/// convexity, z-spread, CS01, discount margin, OAS and WAL all hardcoded
/// Act/365F. On an Act/360 curve that is a 1.39% relative difference in `t`,
/// so the second-order price expansion
///
/// ```text
/// dP/P ~= -D*dy + 0.5*C*dy^2
/// ```
///
/// mixed two clocks: `D` and `C` were not measured against the same yield
/// unit, and combining them was internally inconsistent.
///
/// Act/365F is the right common basis here rather than the curve's own
/// convention:
///   * The bump metrics (z-spread, CS01, convexity) DEFINE their shocks in
///     this basis, and duration is compared against them in the expansion.
///   * A duration quoted "in years" conventionally means Act/365-style years.
///
/// Aligning duration to the majority also leaves nine of eleven call sites
/// untouched, so pinned golden values move only where they were inconsistent.
///
/// Use this constant rather than naming a convention inline, so the metrics
/// cannot drift apart again.
pub(crate) const METRIC_TIME_BASIS: finstack_quant_core::dates::DayCount =
    finstack_quant_core::dates::DayCount::Act365F;

/// The flows after `settlement` as `(t, df, amount)`: the year fraction in
/// [`METRIC_TIME_BASIS`], the discount factor from `settlement` and the
/// cashflow amount. Every spread, duration and convexity metric folds over
/// this list, so they measure time and discounting identically.
///
/// # Arguments
///
/// * `flows` - Dated cashflows; flows on or before `settlement` are dropped.
/// * `curve` - Discount curve supplying `df_between_dates(settlement, date)`.
/// * `settlement` - Date the flows are discounted to and timed from.
///
/// # Errors
///
/// Returns the first year-fraction or discount-factor error, in flow order.
pub(crate) fn discounted_flows(
    flows: &[(
        finstack_quant_core::dates::Date,
        finstack_quant_core::money::Money,
    )],
    curve: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    settlement: finstack_quant_core::dates::Date,
) -> finstack_quant_core::Result<Vec<(f64, f64, f64)>> {
    flows
        .iter()
        .filter(|(date, _)| *date > settlement)
        .map(|(date, amount)| {
            let t = METRIC_TIME_BASIS.year_fraction(
                settlement,
                *date,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;
            let df = curve.df_between_dates(settlement, *date)?;
            Ok((t, df, amount.amount()))
        })
        .collect()
}

pub(crate) mod deal_specific;
mod finite;
pub(crate) mod pool;
pub(crate) mod pricing;
pub(crate) mod quote;
pub(crate) mod risk;
pub(crate) mod scenario;
pub(crate) mod summary;

/// Reprice a (bumped) deal on the scope of the metric context: the named
/// tranche when the context carries one tranche's cashflows (set by
/// [`tranche_metrics`]'s registry pass), else the whole deal
/// through the context's pricing dispatch. Keeps the bump metrics
/// (Default01, Prepayment01, Recovery01, Severity01) per tranche.
///
/// # Arguments
///
/// * `context` - Metric context; its `detailed_tranche_cashflows` names the
///   tranche in scope.
/// * `deal` - The (bumped) deal to price.
/// * `as_of` - Valuation date of the reprice.
pub(crate) fn reprice_in_scope(
    context: &finstack_quant_valuations_core::metrics::MetricContext,
    deal: &crate::structured_credit::StructuredCredit,
    as_of: finstack_quant_core::dates::Date,
) -> finstack_quant_core::Result<f64> {
    match context.ext::<crate::structured_credit::TrancheCashflows>() {
        Some(flows) => Ok(deal
            .value_tranche(&flows.tranche_id, context.curves.as_ref(), as_of)?
            .amount()),
        None => context.reprice_instrument_raw(deal, context.curves.as_ref(), as_of),
    }
}

pub use deal_specific::{
    AbsChargeOffCalculator, AbsCreditEnhancementCalculator, AbsExcessSpreadCalculator,
    AbsPaymentRateCalculator, CmbsDscrCalculator,
};
pub use pool::{CdrCalculator, CloWarfCalculator, CprCalculator};
pub use pricing::calculate_tranche_wal;
pub use risk::{
    calculate_tranche_convexity, calculate_tranche_cs01, calculate_tranche_duration,
    calculate_tranche_z_spread, tranche_breakeven_cdr, tranche_discount_margin, tranche_oas,
    OasConfig, OasResult,
};
pub use scenario::{tranche_scenario_table, ScenarioCell, ScenarioGrid, ScenarioTable};
pub use summary::{calculate_equity_metrics, tranche_metrics, EquityMetrics, TrancheMetrics};

/// Register all structured credit metrics
pub(crate) fn register_structured_credit_metrics(
    registry: &mut finstack_quant_valuations_core::metrics::MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::StructuredCredit,
        metrics: [
            // Standard cashflow-based metrics
            (Accrued, pricing::AccruedCalculator),
            (DirtyPrice, pricing::DirtyPriceCalculator),
            (CleanPrice, pricing::CleanPriceCalculator),
            (WAL, pricing::WalCalculator),
            (DurationMac, risk::MacaulayDurationCalculator),
            (DurationMod, risk::ModifiedDurationCalculator),
            (Convexity, risk::ConvexityCalculator),
            (Ytm, risk::YtmCalculator),
            (ZSpread, risk::ZSpreadCalculator),
            (Cs01, risk::Cs01Calculator),
            // BucketedCs01: key-rate z-spread CS01 — the parallel z-spread shock
            // attributed to standard tenor buckets by cashflow year fraction.
            // (StructuredCredit has no credit curve; the z-spread is a scalar,
            // so "key-rate" here is a time-bucketing of that scalar's effect.)
            (BucketedCs01, risk::BucketedCs01Calculator),
            (SpreadDuration, risk::SpreadDurationCalculator),
            // AssetPool metrics
            (WAM, pool::WamCalculator),
            (CPR, pool::CprCalculator),
            (CDR, pool::CdrCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            // Model-specific risk metrics (custom metrics)
            (Recovery01, risk::recovery01::Recovery01Calculator),
            (Prepayment01, risk::prepayment01::Prepayment01Calculator),
            (Default01, risk::default01::Default01Calculator),
            (Severity01, risk::severity01::Severity01Calculator),
            (CloWarf, pool::CloWarfCalculator),
            (CmbsDscr, deal_specific::CmbsDscrCalculator::new()),
            (CloWas, pool::CloWasCalculator),
            (AbsChargeOff, deal_specific::AbsChargeOffCalculator),
            (AbsDelinquency, deal_specific::AbsDelinquencyCalculator),
            (AbsExcessSpread, deal_specific::AbsExcessSpreadCalculator),
            (AbsPaymentRate, deal_specific::AbsPaymentRateCalculator),
            (AbsCreditEnhancement, deal_specific::AbsCreditEnhancementCalculator),
        ]
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use finstack_quant_valuations_core::metrics::MetricId;
    use finstack_quant_valuations_core::pricer::InstrumentType;

    /// Pool and deal-level metrics that compute purely from the deal model are
    /// reachable through the registry, so `price_instrument` and
    /// `list_standard_metrics` surface them by id without extra wiring.
    #[test]
    fn data_complete_deal_metrics_are_registered_for_structured_credit() {
        let registry = crate::STRUCTURED_CREDIT.metrics();
        for id in [
            MetricId::CloWas,
            MetricId::AbsChargeOff,
            MetricId::AbsDelinquency,
            MetricId::AbsExcessSpread,
            MetricId::AbsPaymentRate,
            MetricId::AbsCreditEnhancement,
        ] {
            assert!(
                registry.is_applicable(&id, InstrumentType::StructuredCredit),
                "metric {id} should be registered for StructuredCredit"
            );
        }
    }
}
