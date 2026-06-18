//! Metrics for structured credit instruments.
//!
//! This module organizes metrics by category:
//! - pricing: Valuation-focused metrics (prices, accrued, WAL)
//! - risk: Risk and sensitivity metrics (duration, spreads, YTM)
//! - pool: Collateral pool characteristics (WAM, CPR, CDR, WARF, WAS)
//! - deal_specific: Deal-type specific metrics (ABS, CLO, CMBS, RMBS)

pub(crate) mod deal_specific;
pub(crate) mod pool;
pub(crate) mod pricing;
pub(crate) mod risk;

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
    calculate_tranche_convexity, calculate_tranche_cs01, calculate_tranche_discount_margin,
    calculate_tranche_duration, calculate_tranche_z_spread, ConvexityCalculator, Cs01Calculator,
    MacaulayDurationCalculator, ModifiedDurationCalculator, SpreadDurationCalculator,
    YtmCalculator, ZSpreadCalculator,
};

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
