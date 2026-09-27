//! Shared rate-index compounding lookup for overnight vs term legs.
//!
//! IRS, revolvers, basis swaps, XCCY, and TRS use this module so an overnight
//! RFR index cannot silently price as a term fixing.

use crate::instruments::rates::irs::FloatingLegCompounding;
use crate::market::conventions::{ConventionRegistry, RateIndexConventions, RateIndexKind};
use finstack_quant_core::types::IndexId;
use finstack_quant_core::Result;

/// Look up rate-index conventions by index or forward-curve identifier.
///
/// # Arguments
///
/// * `index_id` - Registry key such as `USD-SOFR-OIS` or `USD-SOFR-3M`. The
///   same string is used for TRS/basis `forward_curve_id` values that match a
///   registered index.
///
/// # Returns
///
/// `Ok(None)` when the global registry is initialized but does not contain the
/// id. Callers must then keep the instrument's explicit compounding field.
///
/// # Errors
///
/// Returns a validation error when the global `ConventionRegistry` is not
/// initialized.
pub fn rate_index_conventions(index_id: &str) -> Result<Option<RateIndexConventions>> {
    let registry = ConventionRegistry::try_global()?;
    let idx = IndexId::new(index_id);
    Ok(registry.require_rate_index(&idx).ok().cloned())
}

/// Map registered index conventions to a floating-leg compounding method.
///
/// # Arguments
///
/// * `rate_conv` - Conventions already resolved from the registry.
///
/// # Errors
///
/// Returns a validation error when an overnight RFR record is missing
/// `ois_compounding` or records Simple compounding for an overnight index.
pub fn compounding_from_conventions(
    rate_conv: &RateIndexConventions,
) -> Result<FloatingLegCompounding> {
    match rate_conv.kind {
        RateIndexKind::Term => Ok(FloatingLegCompounding::Simple),
        RateIndexKind::OvernightRfr => {
            let compounding = rate_conv.ois_compounding.ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "Overnight RFR index conventions must specify `ois_compounding`".to_string(),
                )
            })?;
            reject_simple_overnight_kind(rate_conv.kind, &compounding)?;
            Ok(compounding)
        }
    }
}

/// Resolve compounding from a registered index id.
///
/// # Arguments
///
/// * `index_id` - Registry key. Unknown ids return `Ok(None)` so the caller
///   keeps its explicit field instead of guessing SOFR.
///
/// # Errors
///
/// Propagates registry initialization errors and overnight-convention
/// validation failures.
pub(crate) fn compounding_from_index_id(index_id: &str) -> Result<Option<FloatingLegCompounding>> {
    match rate_index_conventions(index_id)? {
        Some(conv) => Ok(Some(compounding_from_conventions(&conv)?)),
        None => Ok(None),
    }
}

/// Reject Simple compounding when `index_id` is a registered overnight RFR.
///
/// Unknown ids are left unchanged. Term indices may use Simple.
///
/// # Arguments
///
/// * `index_id` - Index or forward-curve identifier to classify.
/// * `compounding` - Compounding currently set on the instrument leg.
///
/// # Errors
///
/// Returns a validation error when a registered overnight RFR is paired with
/// `FloatingLegCompounding::Simple`.
pub(crate) fn reject_simple_overnight(
    index_id: &str,
    compounding: &FloatingLegCompounding,
) -> Result<()> {
    let Some(conv) = rate_index_conventions(index_id)? else {
        return Ok(());
    };
    reject_simple_overnight_kind(conv.kind, compounding)
}

fn reject_simple_overnight_kind(
    kind: RateIndexKind,
    compounding: &FloatingLegCompounding,
) -> Result<()> {
    if matches!(kind, RateIndexKind::OvernightRfr)
        && matches!(compounding, FloatingLegCompounding::Simple)
    {
        return Err(finstack_quant_core::Error::Validation(
            "Overnight RFR index requires compounded-in-arrears floating compounding; \
             Simple is valid only for term indices"
                .to_string(),
        ));
    }
    Ok(())
}

/// Map leg compounding onto the cashflow builder's optional coupon method.
///
/// `Simple` projects one term forward, which the builder expresses as `None`;
/// every overnight variant passes through unchanged.
///
/// # Arguments
///
/// * `compounding` - Canonical floating-leg compounding of the leg.
pub(crate) fn builder_overnight_method(
    compounding: FloatingLegCompounding,
) -> Option<FloatingLegCompounding> {
    compounding.is_overnight().then_some(compounding)
}

/// Resolve overnight compounding from an explicit spec field or the registry.
///
/// An explicit `compounding` wins (`simple` keeps term projection). Otherwise
/// a registered overnight RFR index supplies its OIS convention. Term and
/// unknown ids return `None` so the caller keeps term-style projection.
///
/// # Arguments
///
/// * `index_id` - Index or forward-curve identifier.
/// * `explicit` - Optional `FloatingRateSpec.compounding` from the instrument.
///
/// # Errors
///
/// Returns a validation error for `simple_average` (the shared overnight
/// projector compounds; it does not arithmetic-average) and propagates
/// registry errors.
pub(crate) fn resolved_overnight_compounding(
    index_id: &str,
    explicit: Option<&FloatingLegCompounding>,
) -> Result<Option<FloatingLegCompounding>> {
    if let Some(method) = explicit {
        return match method {
            FloatingLegCompounding::SimpleAverage => Err(finstack_quant_core::Error::Validation(
                "SimpleAverage overnight coupons are not supported by the shared \
                 compounded-in-arrears projector"
                    .to_string(),
            )),
            FloatingLegCompounding::Simple => Ok(None),
            other => Ok(Some(*other)),
        };
    }
    match compounding_from_index_id(index_id)? {
        Some(FloatingLegCompounding::Simple) | None => Ok(None),
        Some(compounding) => Ok(Some(compounding)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sofr_ois_resolves_to_compounded_in_arrears() {
        let compounding = compounding_from_index_id("USD-SOFR-OIS")
            .expect("registry")
            .expect("registered overnight index");
        assert!(
            !matches!(compounding, FloatingLegCompounding::Simple),
            "USD-SOFR-OIS must not resolve to Simple"
        );
    }

    #[test]
    fn sofr_term_resolves_to_simple() {
        let compounding = compounding_from_index_id("USD-SOFR-3M")
            .expect("registry")
            .expect("registered term index");
        assert_eq!(compounding, FloatingLegCompounding::Simple);
    }

    #[test]
    fn euribor_resolves_to_simple() {
        let compounding = compounding_from_index_id("EUR-EURIBOR-6M")
            .expect("registry")
            .expect("registered term index");
        assert_eq!(compounding, FloatingLegCompounding::Simple);
    }

    #[test]
    fn unknown_index_does_not_force_overnight() {
        let compounding = compounding_from_index_id("NOT-A-REGISTERED-INDEX")
            .expect("unknown id is not an error");
        assert!(
            compounding.is_none(),
            "unknown ids must leave the caller's explicit compounding unchanged"
        );
    }

    #[test]
    fn reject_simple_on_overnight_index() {
        let err = reject_simple_overnight("USD-SOFR-OIS", &FloatingLegCompounding::Simple)
            .expect_err("Simple + overnight RFR must fail");
        assert!(
            err.to_string().contains("compounded-in-arrears"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn allow_simple_on_term_index() {
        reject_simple_overnight("USD-SOFR-3M", &FloatingLegCompounding::Simple)
            .expect("term Simple is valid");
    }

    #[test]
    fn allow_simple_on_unknown_index() {
        reject_simple_overnight("CUSTOM-FWD", &FloatingLegCompounding::Simple)
            .expect("unknown ids must not guess overnight");
    }

    #[test]
    fn explicit_lookback_wins_over_term_index() {
        let compounding = resolved_overnight_compounding(
            "USD-SOFR-3M",
            Some(&FloatingLegCompounding::CompoundedInArrears { lookback_days: 5 }),
        )
        .expect("explicit method")
        .expect("overnight");
        assert_eq!(
            compounding,
            FloatingLegCompounding::CompoundedInArrears { lookback_days: 5 }
        );
    }

    #[test]
    fn sofr_ois_without_explicit_method_is_overnight() {
        let compounding = resolved_overnight_compounding("USD-SOFR-OIS", None)
            .expect("registry")
            .expect("overnight");
        assert!(!matches!(compounding, FloatingLegCompounding::Simple));
    }

    #[test]
    fn rate_cutoff_maps_to_overnight_lockout() {
        let method = builder_overnight_method(FloatingLegCompounding::CompoundedWithRateCutoff {
            cutoff_days: 1,
        });
        assert_eq!(
            method,
            Some(FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 })
        );
    }
}
