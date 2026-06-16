//! Marginable trait implementations for financial instruments.
//!
//! This module provides implementations of the [`Marginable`] trait for
//! instruments that support margin calculations.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::instruments::credit_derivatives::cds_index::CDSIndex;
use crate::instruments::equity::equity_trs::EquityTotalReturnSwap;
use crate::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap;
use crate::instruments::rates::irs::InterestRateSwap;
use crate::instruments::rates::repo::Repo;
use crate::instruments::TrsSide;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_margin::constants::{
    self, CALENDAR_DAYS_PER_YEAR, DEFAULT_BOND_INDEX_DURATION, DURATION_APPROXIMATION_FACTOR,
    INVESTMENT_GRADE_SPREAD_THRESHOLD_BP, ONE_BP, STANDARD_CDS_MATURITY_YEARS,
};
use finstack_quant_margin::{
    ClearingStatus, Marginable, NettingSetId, OtcMarginSpec, RepoMarginSpec, SimmSensitivities,
};
use rust_decimal::prelude::ToPrimitive;

// ============================================================================
// Helper Functions
// ============================================================================

/// Reprice an instrument and return a single scalar metric from its measures.
///
/// Used by the SIMM sensitivity impls to obtain a **repriced** DV01 / CS01
/// (which respects the discount curve, coupon schedule and curve shape) instead
/// of a flat `duration ≈ maturity` proxy. Returns `None` if pricing fails or
/// the metric is absent, so callers can fall back to the proxy.
fn repriced_metric(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
    metric: crate::metrics::MetricId,
) -> Option<f64> {
    let result = instrument
        .price_with_metrics(
            market,
            as_of,
            std::slice::from_ref(&metric),
            crate::instruments::PricingOptions::default(),
        )
        .ok()?;
    result.measures.get(metric.as_str()).copied()
}

/// Reprice an instrument and return the per-tenor `BucketedDv01` series for the
/// given curve as `(tenor_years, dv01)` pairs.
///
/// The `BucketedDv01` calculator flattens its per-tenor series into `measures`
/// under composite keys `bucketed_dv01::{curve}::{tenor_label}`. Returns an
/// empty vec when no per-tenor series is available.
fn repriced_bucketed_dv01(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
    curve_id: &str,
) -> Vec<(f64, f64)> {
    // Standard DV01 bucket grid and labels (mirrors the sensitivities config).
    const TENORS: [f64; 11] = [0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0, 30.0];
    const LABELS: [&str; 11] = [
        "3m", "6m", "1y", "2y", "3y", "5y", "7y", "10y", "15y", "20y", "30y",
    ];
    let Ok(result) = instrument.price_with_metrics(
        market,
        as_of,
        &[crate::metrics::MetricId::BucketedDv01],
        crate::instruments::PricingOptions::default(),
    ) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (&tenor, label) in TENORS.iter().zip(LABELS.iter()) {
        let key = format!("bucketed_dv01::{curve_id}::{label}");
        if let Some(&dv01) = result.measures.get(key.as_str()) {
            out.push((tenor, dv01));
        }
    }
    out
}

/// Assign a years-to-maturity value to the appropriate SIMM IR tenor bucket.
///
/// Standalone variant of [`assign_ir_tenor_bucket`] used when mapping the
/// crate's DV01 bucket grid onto SIMM's IR tenor buckets.
#[must_use]
fn ir_bucket_for_tenor(years: f64) -> &'static str {
    assign_ir_tenor_bucket(years)
}

/// Assign a years-to-maturity value to the appropriate SIMM credit tenor bucket.
#[must_use]
fn assign_credit_tenor_bucket(years_to_maturity: f64) -> &'static str {
    use constants::tenor_buckets::*;
    match years_to_maturity {
        y if y <= BUCKET_1Y => "1Y",
        y if y <= BUCKET_2Y => "2Y",
        y if y <= BUCKET_3Y => "3Y",
        y if y <= BUCKET_5Y => "5Y",
        y if y <= BUCKET_10Y => "10Y",
        _ => "15Y",
    }
}

/// Assign a years-to-maturity value to the appropriate SIMM IR tenor bucket.
#[must_use]
fn assign_ir_tenor_bucket(years_to_maturity: f64) -> &'static str {
    use constants::tenor_buckets::*;
    match years_to_maturity {
        y if y <= BUCKET_6M => "6M",
        y if y <= BUCKET_1Y => "1Y",
        y if y <= BUCKET_2Y => "2Y",
        y if y <= BUCKET_3Y => "3Y",
        y if y <= BUCKET_5Y => "5Y",
        y if y <= BUCKET_10Y => "10Y",
        y if y <= BUCKET_15Y => "15Y",
        y if y <= BUCKET_20Y => "20Y",
        _ => "30Y",
    }
}

/// Extract reference entity from a credit curve ID.
///
/// Expects format `<ENTITY>-<TYPE>[-<TENOR>]` where ENTITY may contain dashes
/// (e.g. `GOLDMAN-SACHS-CDS-5Y` → `GOLDMAN-SACHS`). Strips a recognized tenor
/// suffix (e.g. `-5Y`, `-10Y`) and curve-type suffix (`-CDS`, `-BOND`,
/// `-CURVE`, `-OIS`) from the right; whatever remains is the entity name.
/// Inputs without a recognized suffix return the whole string with a warning.
fn extract_reference_entity(credit_curve_id: &str) -> Result<&str> {
    if credit_curve_id.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "credit curve id is empty".to_string(),
        ));
    }

    // Recognized tenor pattern: an integer followed by Y/M/W/D
    let is_tenor = |s: &str| {
        let last = s.chars().next_back().unwrap_or(' ');
        matches!(last, 'Y' | 'M' | 'W' | 'D')
            && s[..s.len() - 1].chars().all(|c| c.is_ascii_digit())
            && s.len() >= 2
    };
    // Recognized curve types
    const CURVE_TYPES: &[&str] = &[
        "CDS", "BOND", "CURVE", "OIS", "SOFR", "ESTR", "EURIBOR", "LIBOR", "HAZARD", "SPREAD",
    ];
    let is_curve_type = |s: &str| CURVE_TYPES.contains(&s);

    // Strip up to two suffixes from the right: tenor first, then curve type.
    let mut remaining = credit_curve_id;
    for _ in 0..2 {
        if let Some(pos) = remaining.rfind('-') {
            let suffix = &remaining[pos + 1..];
            if is_tenor(suffix) || is_curve_type(suffix) {
                remaining = &remaining[..pos];
                continue;
            }
        }
        break;
    }

    if remaining.is_empty() {
        // Pathological input like "-CDS-5Y" — fall back to the whole string.
        tracing::warn!(
            credit_curve_id,
            "credit curve id has no recognizable entity prefix; using full string"
        );
        return Ok(credit_curve_id);
    }
    Ok(remaining)
}

/// Determine if a credit entity is qualifying (investment grade) for SIMM bucketing.
///
/// Uses a combination of heuristics based on:
/// 1. Well-known index names (CDX.NA.IG, iTraxx Main = qualifying)
/// 2. Spread level as fallback (< 200bp threshold)
///
/// In production, this should be replaced with a lookup against a ratings database
/// or ISDA SIMM bucket mapping table.
fn is_credit_qualifying(name: &str, spread_bp: f64) -> bool {
    let upper = name.to_ascii_uppercase();
    if upper.contains("CDX.NA.IG") || (upper.contains("ITRAXX") && !upper.contains("XOVER")) {
        return true;
    }
    if upper.contains("CDX.NA.HY") || upper.contains("XOVER") || upper.contains("CDX.EM") {
        return false;
    }
    spread_bp < INVESTMENT_GRADE_SPREAD_THRESHOLD_BP
}

/// Derive a netting set ID from an OTC margin specification.
///
/// Maps clearing status to the appropriate netting set identifier.
#[must_use]
fn netting_set_id_from_spec(spec: &OtcMarginSpec) -> NettingSetId {
    match &spec.clearing_status {
        ClearingStatus::Cleared { ccp } => NettingSetId::cleared(ccp),
        _ => NettingSetId::bilateral(&spec.csa.id, &spec.csa.id),
    }
}

// ============================================================================
// InterestRateSwap Implementation
// ============================================================================

impl Marginable for InterestRateSwap {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        self.margin_spec.as_ref()
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        self.margin_spec.as_ref().map(netting_set_id_from_spec)
    }

    fn simm_sensitivities(&self, market: &MarketContext, as_of: Date) -> Result<SimmSensitivities> {
        let currency = self.notional.currency();
        let mut sens = SimmSensitivities::new(currency);

        let days_to_maturity = (self.float.end - as_of).whole_days().max(0) as f64;
        let years_to_maturity = days_to_maturity / CALENDAR_DAYS_PER_YEAR;

        if years_to_maturity <= 0.0 {
            return Ok(sens);
        }

        // Preferred path: repriced per-tenor (key-rate) DV01 across every rate
        // curve the swap depends on. This respects the actual discount curve,
        // coupon schedule and curve shape rather than a flat
        // `duration ≈ maturity` proxy. The repriced DV01 sign already reflects
        // the swap's `side` (the pricer prices the actual swap).
        let rate_curves: Vec<finstack_quant_core::types::CurveId> = self
            .market_dependencies()
            .map(|deps| {
                let cd = deps.curve_dependencies();
                cd.discount_curves
                    .iter()
                    .chain(cd.forward_curves.iter())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        let mut bucket_totals: std::collections::BTreeMap<&'static str, f64> =
            std::collections::BTreeMap::new();
        let mut any_repriced = false;
        for curve_id in &rate_curves {
            for (tenor, dv01) in repriced_bucketed_dv01(self, market, as_of, curve_id.as_str()) {
                any_repriced = true;
                *bucket_totals
                    .entry(ir_bucket_for_tenor(tenor))
                    .or_insert(0.0) += dv01;
            }
        }

        if any_repriced {
            for (bucket, dv01) in bucket_totals {
                sens.add_ir_delta(currency, bucket, dv01);
            }
            return Ok(sens);
        }

        // Fallback (no market data / pricer unavailable): the flat
        // duration-proxy DV01 distributed by maturity-overlap weights.
        tracing::warn!(
            instrument_id = self.id.as_str(),
            "SIMM IR delta falling back to duration ≈ maturity proxy: repriced \
             bucketed DV01 unavailable (missing rate curves?)"
        );
        let total_dv01 = self.notional.amount().abs()
            * years_to_maturity
            * DURATION_APPROXIMATION_FACTOR
            * ONE_BP;

        let sign = match self.side {
            crate::instruments::rates::irs::PayReceive::Pay => -1.0,
            crate::instruments::rates::irs::PayReceive::Receive => 1.0,
        };

        let buckets: &[(&str, f64, f64)] = &[
            ("6M", 0.0, 0.5),
            ("1Y", 0.5, 1.0),
            ("2Y", 1.0, 2.0),
            ("3Y", 2.0, 3.0),
            ("5Y", 3.0, 5.0),
            ("10Y", 5.0, 10.0),
            ("15Y", 10.0, 15.0),
            ("20Y", 15.0, 20.0),
            ("30Y", 20.0, 50.0),
        ];

        let mut total_weight = 0.0f64;
        let mut bucket_weights: Vec<(&str, f64)> = Vec::new();
        for &(name, lo, hi) in buckets {
            if years_to_maturity <= lo {
                break;
            }
            let effective_hi = hi.min(years_to_maturity);
            let weight = effective_hi - lo;
            if weight > 0.0 {
                bucket_weights.push((name, weight));
                total_weight += weight;
            }
        }

        if total_weight > 0.0 {
            for (name, weight) in bucket_weights {
                let fraction = weight / total_weight;
                let bucket_dv01 = sign * total_dv01 * fraction;
                sens.add_ir_delta(currency, name, bucket_dv01);
            }
        }

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        // Calculate NPV using the IRS pricer
        use crate::instruments::rates::irs::pricer::compute_pv;
        compute_pv(self, market, as_of)
    }
}

// ============================================================================
// CreditDefaultSwap Implementation
// ============================================================================

impl Marginable for CreditDefaultSwap {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        self.margin_spec.as_ref()
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        self.margin_spec.as_ref().map(netting_set_id_from_spec)
    }

    fn simm_sensitivities(&self, market: &MarketContext, as_of: Date) -> Result<SimmSensitivities> {
        let currency = self.notional.currency();
        let mut sens = SimmSensitivities::new(currency);

        let days_to_maturity = (self.premium.end - as_of).whole_days().max(0) as f64;
        let years_to_maturity = days_to_maturity / CALENDAR_DAYS_PER_YEAR;
        let years_to_maturity = if years_to_maturity <= 0.0 {
            STANDARD_CDS_MATURITY_YEARS
        } else {
            years_to_maturity
        };

        let ref_entity = extract_reference_entity(self.protection.credit_curve_id.as_str())?;
        let spread_bp_f64 = self.premium.spread_bp.to_f64().unwrap_or(f64::MAX);
        let qualifying = is_credit_qualifying(ref_entity, spread_bp_f64);
        let tenor = assign_credit_tenor_bucket(years_to_maturity);

        // Preferred path: repriced CS01 (respects the survival curve, discount
        // curve, recovery and premium schedule). The metric is computed on the
        // actual CDS, so its sign already reflects the protection side.
        if let Some(cs01) = repriced_metric(self, market, as_of, crate::metrics::MetricId::Cs01) {
            sens.add_credit_delta(ref_entity, qualifying, tenor, cs01);
            return Ok(sens);
        }

        // Fallback: flat risky-duration proxy when the survival curve / pricer
        // is unavailable.
        tracing::warn!(
            instrument_id = self.id.as_str(),
            "SIMM credit delta falling back to risky-duration proxy: repriced \
             CS01 unavailable (missing survival curve?)"
        );
        let risky_duration = years_to_maturity
            * (1.0 - self.protection.recovery_rate)
            * DURATION_APPROXIMATION_FACTOR;
        let cs01 = self.notional.amount().abs() * risky_duration * ONE_BP;
        let signed_cs01 = match self.side {
            crate::instruments::common_impl::parameters::legs::PayReceive::Pay => cs01,
            crate::instruments::common_impl::parameters::legs::PayReceive::Receive => -cs01,
        };
        sens.add_credit_delta(ref_entity, qualifying, tenor, signed_cs01);

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        use crate::instruments::credit_derivatives::cds::pricer::CDSPricer;

        // Get discount and survival curves from market context
        let disc = market.get_discount(self.premium.discount_curve_id.as_str())?;
        let surv = market.get_hazard(self.protection.credit_curve_id.as_str())?;

        let pricer = CDSPricer::new();
        let pv_prot = pricer.pv_protection_leg(self, disc.as_ref(), surv.as_ref(), as_of)?;
        let pv_prem = pricer.pv_premium_leg(self, disc.as_ref(), surv.as_ref(), as_of)?;

        // NPV from protection buyer perspective (Pay)
        let npv = match self.side {
            crate::instruments::common_impl::parameters::legs::PayReceive::Pay => {
                pv_prot.checked_sub(pv_prem)?
            }
            crate::instruments::common_impl::parameters::legs::PayReceive::Receive => {
                pv_prem.checked_sub(pv_prot)?
            }
        };

        Ok(npv)
    }
}

// ============================================================================
// CDSIndex Implementation
// ============================================================================

impl Marginable for CDSIndex {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        self.margin_spec.as_ref()
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        self.margin_spec.as_ref().map(netting_set_id_from_spec)
    }

    fn simm_sensitivities(&self, market: &MarketContext, as_of: Date) -> Result<SimmSensitivities> {
        let currency = self.notional.currency();
        let mut sens = SimmSensitivities::new(currency);

        let days_to_maturity = (self.premium.end - as_of).whole_days().max(0) as f64;
        let years_to_maturity = days_to_maturity / CALENDAR_DAYS_PER_YEAR;
        let years_to_maturity = if years_to_maturity <= 0.0 {
            STANDARD_CDS_MATURITY_YEARS
        } else {
            years_to_maturity
        };

        let qualifying = is_credit_qualifying(&self.index_name, 0.0);
        let tenor = assign_credit_tenor_bucket(years_to_maturity);

        // Preferred path: repriced CS01 (respects the index survival/discount
        // curves, recovery and schedule). Sign reflects the actual index side.
        if let Some(cs01) = repriced_metric(self, market, as_of, crate::metrics::MetricId::Cs01) {
            sens.add_credit_delta(&self.index_name, qualifying, tenor, cs01);
            return Ok(sens);
        }

        // Fallback: flat risky-duration proxy.
        tracing::warn!(
            instrument_id = self.id.as_str(),
            "SIMM credit delta falling back to risky-duration proxy: repriced \
             CS01 unavailable (missing survival curve?)"
        );
        let recovery_rate = self.protection.recovery_rate;
        let risky_duration =
            years_to_maturity * (1.0 - recovery_rate) * DURATION_APPROXIMATION_FACTOR;
        let cs01 = self.notional.amount().abs() * risky_duration * ONE_BP;
        let signed_cs01 = match self.side {
            crate::instruments::common_impl::parameters::legs::PayReceive::Pay => cs01,
            crate::instruments::common_impl::parameters::legs::PayReceive::Receive => -cs01,
        };
        sens.add_credit_delta(&self.index_name, qualifying, tenor, signed_cs01);

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        self.value(market, as_of)
    }
}

// ============================================================================
// EquityTotalReturnSwap Implementation
// ============================================================================

impl Marginable for EquityTotalReturnSwap {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        self.margin_spec.as_ref()
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        self.margin_spec.as_ref().map(netting_set_id_from_spec)
    }

    fn simm_sensitivities(
        &self,
        _market: &MarketContext,
        _as_of: Date,
    ) -> Result<SimmSensitivities> {
        let currency = self.notional.currency();
        let mut sens = SimmSensitivities::new(currency);

        // For Equity TRS, main sensitivity is equity delta
        // Delta = Notional (100% exposure to underlying)
        let delta = match self.side {
            TrsSide::ReceiveTotalReturn => self.notional.amount(),
            TrsSide::PayTotalReturn => -self.notional.amount(),
        };

        // Use the underlier as the equity identifier
        let underlier = &self.underlying.ticker;
        sens.add_equity_delta(underlier, delta);

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        use crate::instruments::common_impl::traits::Instrument;
        self.value(market, as_of)
    }
}

// ============================================================================
// FIIndexTotalReturnSwap Implementation
// ============================================================================

impl Marginable for FIIndexTotalReturnSwap {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        self.margin_spec.as_ref()
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        self.margin_spec.as_ref().map(netting_set_id_from_spec)
    }

    fn simm_sensitivities(
        &self,
        market: &MarketContext,
        _as_of: Date,
    ) -> Result<SimmSensitivities> {
        let currency = self.notional.currency();
        let mut sens = SimmSensitivities::new(currency);

        // Use duration from market data when available, otherwise fall back to default.
        // This mirrors the logic in DurationDv01Calculator for consistency.
        let duration = self
            .underlying
            .duration_id
            .as_ref()
            .map(|id| match market.get_price(id.as_str())? {
                finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => Ok(*v),
                finstack_quant_core::market_data::scalars::MarketScalar::Price(_) => {
                    Err(finstack_quant_core::Error::Validation(format!(
                        "duration_id '{}' must resolve to a unitless scalar",
                        id
                    )))
                }
            })
            .transpose()?
            .unwrap_or(DEFAULT_BOND_INDEX_DURATION);

        let dv01 = self.notional.amount().abs() * duration * ONE_BP;

        let signed_dv01 = match self.side {
            TrsSide::ReceiveTotalReturn => -dv01, // Long bond = short rates
            TrsSide::PayTotalReturn => dv01,      // Short bond = long rates
        };

        // Map duration to appropriate tenor bucket
        let tenor = assign_ir_tenor_bucket(duration);

        sens.add_ir_delta(currency, tenor, signed_dv01);

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        self.value(market, as_of)
    }
}

// ============================================================================
// Repo Implementation
// ============================================================================

impl Marginable for Repo {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        // Repos don't use OtcMarginSpec - they use RepoMarginSpec
        None
    }

    fn repo_margin_spec(&self) -> Option<&RepoMarginSpec> {
        self.margin_spec.as_ref()
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        // Repos typically have their own netting arrangements
        // Use the repo ID as a simple netting set identifier
        Some(NettingSetId::bilateral(self.id.as_str(), "REPO_NETTING"))
    }

    fn simm_sensitivities(
        &self,
        _market: &MarketContext,
        as_of: Date,
    ) -> Result<SimmSensitivities> {
        let currency = self.cash_amount.currency();
        let mut sens = SimmSensitivities::new(currency);

        // Repos have limited rate sensitivity - mainly to the repo rate
        // Short-term IR sensitivity
        let days_to_maturity = (self.maturity - as_of).whole_days().max(1) as f64;
        let years_to_maturity = days_to_maturity / CALENDAR_DAYS_PER_YEAR;

        // DV01 approximation for short-term lending
        let dv01 = self.cash_amount.amount() * years_to_maturity * ONE_BP;

        // Assign to shortest tenor bucket (3M for very short, otherwise 6M)
        let tenor = if years_to_maturity <= constants::tenor_buckets::BUCKET_3M {
            "3M"
        } else {
            "6M"
        };

        sens.add_ir_delta(currency, tenor, dv01);

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        self.pv(market, as_of)
    }
}

#[cfg(test)]
mod tests {
    #[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/test_utils.rs"
        ));
    }

    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, Month::June, 15).expect("valid date")
    }

    #[test]
    fn test_irs_marginable() {
        let start = test_date();
        let end = Date::from_calendar_date(2029, Month::June, 15).expect("valid date");

        let swap = test_utils::usd_irs_swap(
            "TEST_IRS",
            Money::new(100_000_000.0, Currency::USD),
            0.035,
            start,
            end,
            crate::instruments::rates::irs::PayReceive::Pay,
        )
        .expect("swap creation");

        // Without margin spec
        assert!(swap.margin_spec().is_none());
        assert!(!swap.has_margin());

        // Calculate sensitivities
        let market = MarketContext::new();
        let sens = swap
            .simm_sensitivities(&market, start)
            .expect("sensitivities");

        // Should have IR delta
        assert!(!sens.ir_delta.is_empty());
        assert!(sens.total_ir_delta().abs() > 0.0);
    }

    #[test]
    fn test_irs_multi_tenor_decomposition() {
        let start = test_date();
        let end = Date::from_calendar_date(2034, Month::June, 15).expect("valid date");

        let swap = test_utils::usd_irs_swap(
            "TEST_IRS_10Y",
            Money::new(100_000_000.0, Currency::USD),
            0.035,
            start,
            end,
            crate::instruments::rates::irs::PayReceive::Pay,
        )
        .expect("swap creation");

        let market = MarketContext::new();
        let sens = swap
            .simm_sensitivities(&market, start)
            .expect("sensitivities");

        assert!(
            sens.ir_delta.len() > 1,
            "Expected multi-tenor decomposition"
        );
        assert!(
            sens.total_ir_delta() < 0.0,
            "Pay fixed should be short rates"
        );
    }

    /// Audit item #10: with a real market the SIMM IR delta must come from a
    /// repriced (curve-aware) bucketed DV01, not the flat `duration ≈ maturity`
    /// proxy. We assert the repriced path is taken and that it disagrees with
    /// the proxy (the proxy ignores discounting / coupon / curve shape).
    #[test]
    fn test_irs_simm_uses_repriced_dv01_with_market() {
        let start = test_date();
        let end = Date::from_calendar_date(2034, Month::June, 15).expect("valid date");
        let notional = Money::new(100_000_000.0, Currency::USD);

        let swap = test_utils::usd_irs_swap(
            "TEST_IRS_REPRICED",
            notional,
            0.035,
            start,
            end,
            crate::instruments::rates::irs::PayReceive::Pay,
        )
        .expect("swap creation");

        // Real market: USD-OIS discount + USD-SOFR-3M forward, both flat at 3%.
        let market = MarketContext::new()
            .insert(test_utils::flat_discount_with_tenor(
                "USD-OIS", start, 0.03, 30.0,
            ))
            .insert(test_utils::flat_forward_with_tenor(
                "USD-SOFR-3M",
                start,
                0.03,
                30.0,
            ));

        let sens = swap
            .simm_sensitivities(&market, start)
            .expect("repriced SIMM sensitivities");

        assert!(!sens.ir_delta.is_empty(), "must produce IR delta buckets");
        let repriced_total = sens.total_ir_delta();
        assert!(
            repriced_total.is_finite() && repriced_total.abs() > 0.0,
            "repriced IR delta must be finite and non-zero, got {repriced_total}"
        );

        // The flat duration ≈ maturity proxy total (the OLD behavior) for a
        // ~10y swap: notional × 10 × 0.0001 (DURATION_APPROXIMATION_FACTOR) ×
        // ONE_BP, signed negative for Pay-fixed. The repriced curve-aware DV01
        // genuinely differs from this crude proxy.
        let proxy_total = -notional.amount().abs() * 10.0 * DURATION_APPROXIMATION_FACTOR * ONE_BP;
        assert!(
            (repriced_total - proxy_total).abs() > 1e-6,
            "repriced DV01 ({repriced_total}) must differ from the duration proxy ({proxy_total})"
        );
    }

    #[test]
    fn test_repo_marginable() {
        let repo = Repo::example();

        // Repo uses repo_margin_spec, not margin_spec
        assert!(repo.margin_spec().is_none());

        // Should have netting set
        let netting_set = repo.netting_set_id();
        assert!(netting_set.is_some());
    }

    #[test]
    fn test_netting_set_from_cleared_spec() {
        use finstack_quant_margin::types::{CsaSpec, ImMethodology, MarginTenor};

        let start = test_date();
        let end = Date::from_calendar_date(2029, Month::June, 15).expect("valid date");

        let mut swap = test_utils::usd_irs_swap(
            "TEST_IRS",
            Money::new(100_000_000.0, Currency::USD),
            0.035,
            start,
            end,
            crate::instruments::rates::irs::PayReceive::Pay,
        )
        .expect("swap creation");

        // Add cleared margin spec
        swap.margin_spec = Some(OtcMarginSpec {
            csa: CsaSpec::usd_regulatory().expect("registry should load"),
            clearing_status: ClearingStatus::Cleared {
                ccp: "LCH".to_string(),
            },
            im_methodology: ImMethodology::ClearingHouse,
            vm_frequency: MarginTenor::Daily,
            settlement_lag: 0,
        });

        let netting_set = swap.netting_set_id().expect("netting set");
        assert!(netting_set.is_cleared());
        assert_eq!(netting_set.ccp_id(), Some("LCH"));
    }
}
