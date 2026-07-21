//! Metrics for structured credit instruments.
//!

//! This module organizes metrics by category:
//! - pricing: Valuation-focused metrics (prices, accrued, WAL)
//! - risk: Risk and sensitivity metrics (duration, spreads, YTM)
//! - pool: Collateral pool characteristics (WAM, CPR, CDR, WARF, WAS)
//! - deal_specific: Deal-type specific metrics (ABS, CLO, CMBS, RMBS)

/// Time basis shared by every structured-credit risk metric.
///
/// SC-m02: duration measured time with the DISCOUNT CURVE's day count while
/// convexity, z-spread, CS01, discount margin, OAS and WAL all hardcoded
/// Act/365F. On an Act/360 curve that is a 1.39% relative difference in `t`,
/// so the second-order price expansion
///
///     dP/P ~= -D*dy + 0.5*C*dy^2
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

pub(crate) mod deal_specific;
pub(crate) mod pool;
pub(crate) mod pricing;
pub(crate) mod risk;
pub(crate) mod scenario;
pub(crate) mod summary;

// Re-export all calculators for convenience
pub use deal_specific::{
    AbsChargeOffCalculator, AbsCreditEnhancementCalculator, AbsDelinquencyCalculator,
    AbsExcessSpreadCalculator, AbsSpeedCalculator, CloWalCalculator, CmbsDscrCalculator,
    CmbsLtvCalculator, RmbsFicoCalculator, RmbsLtvCalculator, RmbsWalCalculator,
};
pub use pool::{CdrCalculator, CloWarfCalculator, CloWasCalculator, CprCalculator, WamCalculator};
pub use pricing::{
    calculate_tranche_wal, AccruedCalculator, CleanPriceCalculator, DirtyPriceCalculator,
    WalCalculator,
};
pub use risk::{
    calculate_tranche_breakeven_cdr, calculate_tranche_convexity, calculate_tranche_cs01,
    calculate_tranche_discount_margin, calculate_tranche_duration, calculate_tranche_oas,
    calculate_tranche_z_spread, ConvexityCalculator, Cs01Calculator, MacaulayDurationCalculator,
    ModifiedDurationCalculator, OasConfig, OasResult, SpreadDurationCalculator, YtmCalculator,
    ZSpreadCalculator,
};
pub use scenario::{scenario_table, ScenarioCell, ScenarioGrid, ScenarioTable};
pub use summary::{calculate_tranche_metrics, TrancheMetrics};

// Standalone tranche metric functions are included in the explicit lists above.

/// Register all structured credit metrics
pub(crate) fn register_structured_credit_metrics(registry: &mut crate::metrics::MetricRegistry) {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // Model-specific risk metrics (custom metrics)
    registry.register_metric(
        MetricId::Recovery01,
        Arc::new(risk::recovery01::Recovery01Calculator),
        &[InstrumentType::StructuredCredit],
    );
    registry.register_metric(
        MetricId::Prepayment01,
        Arc::new(risk::prepayment01::Prepayment01Calculator),
        &[InstrumentType::StructuredCredit],
    );
    registry.register_metric(
        MetricId::Default01,
        Arc::new(risk::default01::Default01Calculator),
        &[InstrumentType::StructuredCredit],
    );
    registry.register_metric(
        MetricId::Severity01,
        Arc::new(risk::severity01::Severity01Calculator),
        &[InstrumentType::StructuredCredit],
    );
    registry.register_metric(
        MetricId::CloWarf,
        Arc::new(pool::CloWarfCalculator),
        &[InstrumentType::StructuredCredit],
    );
    registry.register_metric(
        MetricId::CmbsDscr,
        Arc::new(deal_specific::CmbsDscrCalculator::new()),
        &[InstrumentType::StructuredCredit],
    );

    crate::register_metrics! {
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
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::fixed_income::structured_credit::StructuredCredit,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::fixed_income::structured_credit::StructuredCredit,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            // Theta is now registered universally in metrics::standard_registry()
        ]
    }

    // Other deal-specific metrics (WAS, ABS speed, delinquency, excess spread, LTV, FICO)
    // are still used directly via their calculator structs when needed.
}
