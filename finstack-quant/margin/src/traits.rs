//! Traits for marginable instruments.
//!
//! Defines the common interface for instruments that support margin calculations,
//! enabling uniform margin metric calculation and portfolio aggregation.

use crate::types::OtcMarginSpec;
use crate::types::RepoMarginSpec;
use crate::types::{NettingSetId, SimmSensitivities};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Trait for instruments that support margin calculations.
///
/// This is a standalone trait so `finstack-quant-margin` has no dependency on
/// `finstack-quant-valuations`. Concrete implementations live in valuations as a
/// bridge layer.
///
/// # Conventions
///
/// - [`Self::mtm_for_vm`] returns the current mark-to-market in instrument or
///   reporting currency units, not basis points.
/// - [`Self::im_exposure_base`] returns an explicit regulatory notional,
///   CCP scan-risk proxy, or other methodology-specific exposure base for
///   percentage-of-exposure IM calculators. It must not fall back to current
///   MtM unless that is explicitly the regulatory exposure input for the
///   instrument.
/// - [`Self::simm_sensitivities`] must return sensitivities in the units
///   documented by [`crate::types::SimmSensitivities`], especially for DV01/CS01-style
///   inputs where decimal-vs-basis-point mistakes materially change IM.
pub trait Marginable: Send + Sync {
    /// Get the instrument's unique identifier.
    fn id(&self) -> &str;

    /// Get the OTC margin specification for this instrument.
    fn margin_spec(&self) -> Option<&OtcMarginSpec>;

    /// Get the repo margin specification for this instrument.
    ///
    /// Default implementation returns `None`. Override for repo instruments.
    fn repo_margin_spec(&self) -> Option<&RepoMarginSpec> {
        None
    }

    /// Get the netting set identifier for margin aggregation.
    fn netting_set_id(&self) -> Option<NettingSetId>;

    /// Calculate SIMM sensitivities for this instrument.
    ///
    /// # Arguments
    ///
    /// * `market` - Market data used to produce the sensitivities
    /// * `as_of` - Valuation date for the sensitivity snapshot
    ///
    /// # Returns
    ///
    /// A [`SimmSensitivities`] value whose buckets use the conventions in
    /// [`crate::types::SimmSensitivities`]. In particular, delta and vega values should
    /// already be expressed in currency amounts per the SIMM risk measure being
    /// populated rather than raw quote moves.
    ///
    /// This method always performs a full recompute from the supplied
    /// market state.
    ///
    /// # Errors
    ///
    /// Returns an error when the instrument cannot produce a consistent SIMM
    /// sensitivity set for the requested market state.
    ///
    /// # References
    ///
    /// - ISDA SIMM: `docs/REFERENCES.md#isda-simm`
    fn simm_sensitivities(&self, market: &MarketContext, as_of: Date) -> Result<SimmSensitivities>;

    /// Get the current mark-to-market value used for margin calculations.
    ///
    /// This value is the exposure base for variation margin only. Initial
    /// margin calculators that require regulatory notional or model exposure
    /// inputs use [`Self::im_exposure_base`] instead.
    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money>;

    /// Get the explicit exposure base for percentage-of-exposure IM methods.
    ///
    /// Schedule IM, CCP conservative-rate proxies, and internal-model fallback
    /// proxies must scale a regulatory notional, CCP scan-risk base, or another
    /// methodology-approved exposure amount. Returning `None` makes those paths
    /// fail closed rather than silently using current MtM as a pseudo-notional.
    ///
    /// # Default behavior
    ///
    /// Returns `Ok(None)` so existing instruments keep compiling but cannot
    /// accidentally receive non-SIMM percentage IM without supplying a proper
    /// exposure base or an external IM source.
    fn im_exposure_base(&self, _market: &MarketContext, _as_of: Date) -> Result<Option<Money>> {
        Ok(None)
    }

    /// Check if margin applies to this instrument.
    fn has_margin(&self) -> bool {
        self.margin_spec().is_some() || self.repo_margin_spec().is_some()
    }
}
