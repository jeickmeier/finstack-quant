//! Simplified TBA pool allocation (cheapest-to-deliver).
//!
//! This module provides simplified CTD allocation using on-the-run
//! pool characteristics rather than full SIFMA good delivery rules.

use std::sync::OnceLock;

use super::AgencyTba;
use crate::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough;
use finstack_quant_core::{Error, Result};
use serde::Deserialize;

const TBA_ASSUMPTIONS: &str = include_str!("../../../../data/assumptions/tba_assumptions.v1.json");

static TBA_DEFAULTS: OnceLock<Result<TbaAssumptions>> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TbaAssumptions {
    schema: Option<String>,
    version: Option<u32>,
    pool_characteristics: PoolCharacteristicAssumptions,
    assumed_pool: AssumedPoolAssumptions,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolCharacteristicAssumptions {
    base_psa_multiplier: f64,
    seasoned_wala_months: u32,
    seasoned_wala_multiplier: f64,
    low_pool_factor_threshold: f64,
    low_pool_factor_multiplier: f64,
    small_loan_threshold: f64,
    small_loan_multiplier: f64,
    large_loan_threshold: f64,
    large_loan_multiplier: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssumedPoolAssumptions {
    pub(crate) default_pool_factor: f64,
    pub(crate) servicing_fee_rate: f64,
    pub(crate) agency_guarantee_fee_rate: f64,
    pub(crate) gnma_guarantee_fee_rate: f64,
    pub(crate) psa_multiplier: f64,
}

pub(crate) fn assumed_pool_assumptions() -> Result<AssumedPoolAssumptions> {
    Ok(embedded_tba_assumptions()?.assumed_pool)
}

/// Pool allocation result.
#[derive(Debug, Clone)]
pub struct AllocationResult {
    /// Allocated pool
    pub pool: AgencyMbsPassthrough,
    /// Value advantage/disadvantage vs. generic pool
    pub value_adjustment: f64,
    /// Whether this is a specified pool or generic
    pub is_specified: bool,
}

/// Simplified CTD allocation using on-the-run pool characteristics.
///
/// This function creates an assumed pool for TBA valuation using
/// standard generic pool assumptions rather than evaluating actual
/// deliverable pools.
///
/// # Simplifying Assumptions
///
/// - Pool factor: 1.0 (newly issued)
/// - WAC: TBA coupon + standard fee strip (50 bps)
/// - WAM: Full term (180/240/360 months)
/// - Prepayment: 100% PSA
///
/// For full CTD analysis, users should provide explicit `assumed_pool`
/// on the TBA instrument.
pub fn allocate_generic_pool(tba: &AgencyTba) -> Result<AllocationResult> {
    use crate::instruments::fixed_income::tba::pricer::create_assumed_pool;
    use finstack_quant_core::dates::Date;

    // Use a reference date for pool creation
    let reference_date = Date::from_calendar_date(
        tba.settlement_year,
        time::Month::try_from(tba.settlement_month)
            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?,
        1,
    )
    .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;

    let pool = create_assumed_pool(tba, reference_date)?;

    Ok(AllocationResult {
        pool,
        value_adjustment: 0.0, // Generic pool has no adjustment
        is_specified: false,
    })
}

/// Pool characteristics that affect deliverability and value.
#[derive(Debug, Clone)]
pub struct PoolCharacteristics {
    /// Weighted average coupon
    pub wac: f64,
    /// Weighted average maturity (months)
    pub wam: u32,
    /// Weighted average loan age (months)
    pub wala: u32,
    /// Pool factor (remaining balance / original)
    pub factor: f64,
    /// Average loan size
    pub avg_loan_size: Option<f64>,
    /// Geographic concentration
    pub geographic_concentration: Option<f64>,
}

impl PoolCharacteristics {
    /// Estimate prepayment speed adjustment based on characteristics.
    ///
    /// Returns a PSA multiplier (1.0 = baseline).
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded TBA assumptions fail to load.
    pub fn estimated_psa_multiplier(&self) -> Result<f64> {
        let assumptions = embedded_tba_assumptions()?;
        let pool = &assumptions.pool_characteristics;
        let mut multiplier = pool.base_psa_multiplier;

        // Higher WAC relative to market tends to prepay faster
        // (This would need market rate context for proper calculation)

        // Seasoned pools (high WALA) may have different prepayment patterns
        if self.wala > pool.seasoned_wala_months {
            // Post-ramp, use burnout adjustment
            multiplier *= pool.seasoned_wala_multiplier;
        }

        // Lower factors indicate pool has already experienced prepayments
        if self.factor < pool.low_pool_factor_threshold {
            multiplier *= pool.low_pool_factor_multiplier;
        }

        // Smaller loans tend to prepay faster (can't refi as easily)
        if let Some(avg_size) = self.avg_loan_size {
            if avg_size < pool.small_loan_threshold {
                multiplier *= pool.small_loan_multiplier;
            } else if avg_size > pool.large_loan_threshold {
                multiplier *= pool.large_loan_multiplier;
            }
        }

        Ok(multiplier)
    }

    /// Check if pool meets TBA good delivery standards.
    ///
    /// Validates WAC spread, factor threshold, and optionally the SIFMA
    /// face amount variance rule (±0.01%).
    pub fn meets_good_delivery(&self, tba_coupon: f64) -> bool {
        self.meets_good_delivery_with_face(tba_coupon, None, None)
    }

    /// Check good delivery with optional face amount variance validation.
    ///
    /// # Arguments
    /// * `tba_coupon` - TBA pass-through coupon rate
    /// * `allocated_face` - Optional allocated pool face amount
    /// * `trade_notional` - Optional trade notional for variance check
    pub fn meets_good_delivery_with_face(
        &self,
        tba_coupon: f64,
        allocated_face: Option<f64>,
        trade_notional: Option<f64>,
    ) -> bool {
        // WAC should be within reasonable range of TBA coupon
        let wac_spread = self.wac - tba_coupon;
        if !(0.0025..=0.01).contains(&wac_spread) {
            return false; // Typical servicing spread: 25-100 bps
        }

        // Factor shouldn't be too low (seasoned pools may not deliver)
        if self.factor < 0.50 {
            return false;
        }

        // SIFMA face amount variance check (±0.01%) if amounts provided
        if let (Some(face), Some(notional)) = (allocated_face, trade_notional) {
            if !validate_sifma_variance(face, notional) {
                return false;
            }
        }

        true
    }
}

fn embedded_tba_assumptions() -> Result<&'static TbaAssumptions> {
    match TBA_DEFAULTS.get_or_init(parse_tba_assumptions) {
        Ok(defaults) => Ok(defaults),
        Err(err) => Err(err.clone()),
    }
}

fn parse_tba_assumptions() -> Result<TbaAssumptions> {
    let defaults: TbaAssumptions = serde_json::from_str(TBA_ASSUMPTIONS)
        .map_err(|err| Error::Validation(format!("failed to parse TBA assumptions: {err}")))?;
    let _schema = &defaults.schema;
    let _version = defaults.version;
    validate_tba_assumptions(&defaults)?;
    Ok(defaults)
}

fn validate_tba_assumptions(defaults: &TbaAssumptions) -> Result<()> {
    let pool = &defaults.pool_characteristics;
    validate_positive("tba.base_psa_multiplier", pool.base_psa_multiplier)?;
    validate_positive(
        "tba.seasoned_wala_multiplier",
        pool.seasoned_wala_multiplier,
    )?;
    validate_positive(
        "tba.low_pool_factor_threshold",
        pool.low_pool_factor_threshold,
    )?;
    validate_positive(
        "tba.low_pool_factor_multiplier",
        pool.low_pool_factor_multiplier,
    )?;
    validate_positive("tba.small_loan_threshold", pool.small_loan_threshold)?;
    validate_positive("tba.small_loan_multiplier", pool.small_loan_multiplier)?;
    validate_positive("tba.large_loan_threshold", pool.large_loan_threshold)?;
    validate_positive("tba.large_loan_multiplier", pool.large_loan_multiplier)?;
    let assumed = defaults.assumed_pool;
    validate_positive(
        "tba.assumed_pool.default_pool_factor",
        assumed.default_pool_factor,
    )?;
    validate_positive(
        "tba.assumed_pool.servicing_fee_rate",
        assumed.servicing_fee_rate,
    )?;
    validate_positive(
        "tba.assumed_pool.agency_guarantee_fee_rate",
        assumed.agency_guarantee_fee_rate,
    )?;
    validate_positive(
        "tba.assumed_pool.gnma_guarantee_fee_rate",
        assumed.gnma_guarantee_fee_rate,
    )?;
    validate_positive("tba.assumed_pool.psa_multiplier", assumed.psa_multiplier)
}

fn validate_positive(label: &str, value: f64) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(Error::Validation(format!("{label} must be positive")))
    }
}

/// Validate SIFMA good delivery variance.
///
/// SIFMA allows ±0.01% variance on face amount for TBA allocation.
///
/// # Reference
/// SIFMA Good Delivery Guidelines Section 3.2
pub fn validate_sifma_variance(allocated_face: f64, trade_notional: f64) -> bool {
    if trade_notional <= 0.0 {
        return false;
    }
    let variance = (allocated_face - trade_notional).abs() / trade_notional;
    variance <= 0.0001 // ±0.01% tolerance
}

/// Calculate value adjustment for a specified pool vs. generic.
///
/// Positive adjustment means the specified pool is worth more than generic.
pub fn calculate_pay_up(
    _characteristics: &PoolCharacteristics,
    tba: &AgencyTba,
) -> finstack_quant_core::Result<f64> {
    Err(finstack_quant_core::Error::Validation(format!(
        "AgencyTba '{}' specified-pool pay-up model is not implemented; refusing to return a zero adjustment",
        tba.id
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_generic_pool() {
        let tba = AgencyTba::example().expect("AgencyTba example is valid");
        let result = allocate_generic_pool(&tba).expect("should allocate");

        assert!(!result.is_specified);
        assert!((result.value_adjustment).abs() < 1e-10);
        assert!((result.pool.current_factor - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_pool_characteristics_psa() {
        let chars = PoolCharacteristics {
            wac: 0.045,
            wam: 348,
            wala: 12,
            factor: 0.98,
            avg_loan_size: Some(300_000.0),
            geographic_concentration: None,
        };

        let psa = chars.estimated_psa_multiplier().expect("assumptions load");
        // Should be around 1.0 for standard characteristics
        assert!(psa > 0.8 && psa < 1.2);
    }

    #[test]
    fn test_pool_characteristics_burnout() {
        let chars = PoolCharacteristics {
            wac: 0.045,
            wam: 300,
            wala: 60,     // 5 years seasoned
            factor: 0.70, // Significant paydown
            avg_loan_size: Some(300_000.0),
            geographic_concentration: None,
        };

        let psa = chars.estimated_psa_multiplier().expect("assumptions load");
        // Should be lower due to burnout
        assert!(psa < 1.0);
    }

    #[test]
    fn test_good_delivery_check() {
        let chars = PoolCharacteristics {
            wac: 0.045, // 4.5% WAC
            wam: 350,
            wala: 10,
            factor: 0.95,
            avg_loan_size: None,
            geographic_concentration: None,
        };

        // With 4.0% TBA coupon, 50 bps spread should pass
        assert!(chars.meets_good_delivery(0.04));

        // With 4.2% TBA coupon, 30 bps spread is within range (25-100 bps)
        assert!(chars.meets_good_delivery(0.042));
    }

    #[test]
    fn test_good_delivery_fails_low_factor() {
        let chars = PoolCharacteristics {
            wac: 0.045,
            wam: 200,
            wala: 160,
            factor: 0.40, // Too low
            avg_loan_size: None,
            geographic_concentration: None,
        };

        assert!(!chars.meets_good_delivery(0.04));
    }

    #[test]
    fn test_validate_sifma_variance_within_tolerance() {
        // Exact match
        assert!(validate_sifma_variance(10_000_000.0, 10_000_000.0));
        // Within ±0.01% (= 1000 on 10M)
        assert!(validate_sifma_variance(10_000_500.0, 10_000_000.0));
        assert!(validate_sifma_variance(9_999_500.0, 10_000_000.0));
    }

    #[test]
    fn test_validate_sifma_variance_exceeds_tolerance() {
        // Over ±0.01% (> 1000 on 10M)
        assert!(!validate_sifma_variance(10_002_000.0, 10_000_000.0));
        assert!(!validate_sifma_variance(9_998_000.0, 10_000_000.0));
    }

    #[test]
    fn test_validate_sifma_variance_zero_notional() {
        assert!(!validate_sifma_variance(100.0, 0.0));
        assert!(!validate_sifma_variance(100.0, -1.0));
    }

    #[test]
    fn test_good_delivery_with_face_variance() {
        let chars = PoolCharacteristics {
            wac: 0.045,
            wam: 350,
            wala: 10,
            factor: 0.95,
            avg_loan_size: None,
            geographic_concentration: None,
        };

        // Passes without face amounts
        assert!(chars.meets_good_delivery(0.04));

        // Passes with matching face amounts
        assert!(chars.meets_good_delivery_with_face(0.04, Some(10_000_000.0), Some(10_000_000.0)));

        // Fails with excessive face variance
        assert!(!chars.meets_good_delivery_with_face(0.04, Some(10_100_000.0), Some(10_000_000.0)));
    }
}
