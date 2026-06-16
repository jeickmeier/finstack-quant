//! Clearing house IM calculator.
//!
//! Stub implementation for CCP-specific margin methodologies.
//! In production, this would interface with CCP margin APIs or
//! replicate their VaR/SPAN-based calculations.

use crate::calculators::traits::{ImCalculator, ImResult};
use crate::registry::{
    embedded_registry_or_panic, margin_registry_from_config, CcpParams, MarginRegistry,
};
use crate::traits::Marginable;
use crate::types::ImMethodology;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;
use std::sync::Arc;

/// CCP methodology type.
///
/// Represents the clearing-house rulebook used to source conservative fallback
/// parameters such as MPOR and the decimal conservative-rate proxy.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum CcpMethodology {
    /// LCH SwapClear (VaR-based for IRS)
    LchSwapClear,
    /// LCH CDSClear
    LchCdsClear,
    /// CME Clearing (SPAN-based)
    Cme,
    /// ICE Clear Credit (for CDS/CDX)
    IceClearCredit,
    /// ICE Clear US
    IceClearUs,
    /// JSCC (Japan)
    Jscc,
    /// Eurex
    Eurex,
    /// Generic VaR-based
    GenericVaR {
        /// Confidence level (e.g., 0.99 for 99%)
        confidence: f64,
        /// Lookback period in days
        lookback_days: u32,
    },
}

impl std::fmt::Display for CcpMethodology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CcpMethodology::LchSwapClear => write!(f, "LCH SwapClear"),
            CcpMethodology::LchCdsClear => write!(f, "LCH CDSClear"),
            CcpMethodology::Cme => write!(f, "CME"),
            CcpMethodology::IceClearCredit => write!(f, "ICE Clear Credit"),
            CcpMethodology::IceClearUs => write!(f, "ICE Clear US"),
            CcpMethodology::Jscc => write!(f, "JSCC"),
            CcpMethodology::Eurex => write!(f, "Eurex"),
            CcpMethodology::GenericVaR { confidence, .. } => {
                write!(f, "Generic VaR ({:.0}%)", confidence * 100.0)
            }
        }
    }
}

impl CcpMethodology {
    fn registry_key(&self) -> &'static str {
        match self {
            CcpMethodology::LchSwapClear => "lch_swapclear",
            CcpMethodology::LchCdsClear => "lch_cdsclear",
            CcpMethodology::Cme => "cme",
            CcpMethodology::IceClearCredit => "ice_clear_credit",
            CcpMethodology::IceClearUs => "ice_clear_us",
            CcpMethodology::Jscc => "jscc",
            CcpMethodology::Eurex => "eurex",
            CcpMethodology::GenericVaR { .. } => "generic_var",
        }
    }

    fn params_from_registry(&self, registry: &MarginRegistry) -> CcpParams {
        let key = self.registry_key();
        if let Some(params) = registry.ccp.get(key) {
            return params.clone();
        }
        registry.ccp_default_params.clone()
    }

    fn generic_var_from_registry(registry: &MarginRegistry) -> Self {
        let defaults = &registry.ccp_generic_var_defaults;
        CcpMethodology::GenericVaR {
            confidence: defaults.confidence,
            lookback_days: defaults.lookback_days,
        }
    }

    fn from_ccp_name_with_registry(ccp: &str, registry: &MarginRegistry) -> Self {
        Self::from_ccp_name_with_fallback(ccp, || Self::generic_var_from_registry(registry))
    }

    fn from_ccp_name_with_fallback<F>(ccp: &str, fallback: F) -> Self
    where
        F: FnOnce() -> Self,
    {
        let normalized = ccp.trim().to_ascii_lowercase();
        let is_credit = normalized.contains("credit") || normalized.contains("cds");

        if normalized.contains("lch") {
            if is_credit || normalized.contains("cds") {
                CcpMethodology::LchCdsClear
            } else {
                CcpMethodology::LchSwapClear
            }
        } else if normalized.contains("ice") {
            if is_credit || normalized.contains("credit") || normalized.contains("cds") {
                CcpMethodology::IceClearCredit
            } else {
                CcpMethodology::IceClearUs
            }
        } else if normalized.contains("cme") {
            CcpMethodology::Cme
        } else if normalized.contains("jscc") {
            CcpMethodology::Jscc
        } else if normalized.contains("eurex") {
            CcpMethodology::Eurex
        } else {
            fallback()
        }
    }

    /// Choose a CCP methodology from a CCP display name.
    ///
    /// The mapping is heuristic and string-based. Unknown names fall back to
    /// registry-backed [`CcpMethodology::GenericVaR`] defaults.
    ///
    /// # Arguments
    ///
    /// * `ccp` - Human-readable CCP name such as `"LCH"` or `"ICE Clear Credit"`
    ///
    /// # Returns
    ///
    /// The closest built-in CCP methodology.
    #[must_use]
    pub fn from_ccp_name(ccp: &str) -> Self {
        Self::from_ccp_name_with_registry(ccp, embedded_registry_or_panic())
    }
}

/// Clearing house IM calculator.
///
/// Provides CCP-specific initial-margin approximations for cleared derivatives.
/// When an [`ExternalImSource`](super::ExternalImSource) is attached, the calculator uses the
/// externally supplied IM amount and optional MPOR override. Otherwise it
/// requires [`Marginable::im_exposure_base`] and scales that explicit CCP
/// exposure base by a conservative rate. It refuses to use current MtM as a
/// pseudo-notional.
///
/// # Real-World Implementation
///
/// In production, this would:
/// 1. Interface with CCP margin APIs (e.g., LCH SMART, CME CORE)
/// 2. Replicate VaR/SPAN calculations with historical scenarios
/// 3. Apply portfolio margining and cross-product netting
///
/// # Conventions
///
/// - `conservative_rate` values are decimal fractions, so `0.02` means 2%.
/// - MPOR is expressed in calendar days.
/// - The proxy fallback uses [`Marginable::im_exposure_base`], not current MtM.
///
/// # Example
///
/// ```ignore
/// use finstack_quant_margin::{ClearingHouseImCalculator, ImCalculator, Marginable};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let calc = ClearingHouseImCalculator::lch_swapclear();
/// # let cleared_trade: &dyn Marginable = todo!("provide a cleared marginable instrument");
/// # let context = MarketContext::new();
/// # let as_of: Date = date!(2025-01-01);
/// let im = calc.calculate(cleared_trade, &context, as_of)?;
/// # let _ = im;
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - BCBS-IOSCO uncleared margin framework: `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`
/// - Quantitative Risk Management: `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`
#[derive(Clone)]
pub struct ClearingHouseImCalculator {
    /// CCP methodology
    pub methodology: CcpMethodology,
    /// Optional external CCP margin input source
    pub input_source: Option<Arc<dyn super::ExternalImSource>>,
    /// Optional resolved parameters (overrides or pre-fetched config)
    pub params_override: Option<CcpParams>,
}

impl std::fmt::Debug for ClearingHouseImCalculator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClearingHouseImCalculator")
            .field("methodology", &self.methodology)
            .field("params_override", &self.params_override)
            .field(
                "input_source",
                &self.input_source.as_ref().map(|_| "<dyn ExternalImSource>"),
            )
            .finish()
    }
}

impl ClearingHouseImCalculator {
    /// Create a new calculator for a specific CCP methodology.
    ///
    /// # Arguments
    ///
    /// * `methodology` - CCP rule set used to resolve conservative fallback parameters
    ///
    /// # Returns
    ///
    /// A calculator with no external input source and no parameter overrides.
    #[must_use]
    pub fn new(methodology: CcpMethodology) -> Self {
        Self {
            methodology,
            input_source: None,
            params_override: None,
        }
    }

    /// Create a calculator from a CCP display name.
    ///
    /// # Arguments
    ///
    /// * `ccp` - Human-readable CCP name passed to [`CcpMethodology::from_ccp_name`]
    ///
    /// # Returns
    ///
    /// A calculator using the inferred built-in methodology.
    #[must_use]
    pub fn for_ccp(ccp: &str) -> Self {
        Self::new(CcpMethodology::from_ccp_name(ccp))
    }

    /// Create a calculator with explicitly resolved parameters.
    ///
    /// # Arguments
    ///
    /// * `methodology` - CCP methodology identifier
    /// * `params` - Conservative fallback parameters with decimal rates and day counts
    ///
    /// # Returns
    ///
    /// A calculator using `params` instead of registry lookup.
    #[must_use]
    pub fn with_params(methodology: CcpMethodology, params: CcpParams) -> Self {
        Self {
            methodology,
            input_source: None,
            params_override: Some(params),
        }
    }

    /// Create a calculator using registry overrides from config.
    ///
    /// # Arguments
    ///
    /// * `ccp` - Human-readable CCP name used to infer the methodology
    /// * `cfg` - Config whose margin-registry extension may override CCP parameters
    ///
    /// # Errors
    ///
    /// Returns an error if the margin registry cannot be loaded from `cfg`.
    pub fn for_ccp_with_config(ccp: &str, cfg: &FinstackConfig) -> Result<Self> {
        let registry = margin_registry_from_config(cfg)?;
        let methodology = CcpMethodology::from_ccp_name_with_registry(ccp, &registry);
        let params = methodology.params_from_registry(&registry);
        Ok(Self::with_params(methodology, params))
    }

    /// Create calculator for LCH SwapClear (IRS).
    #[must_use]
    pub fn lch_swapclear() -> Self {
        Self::new(CcpMethodology::LchSwapClear)
    }

    /// Create calculator for ICE Clear Credit (CDS/CDX).
    #[must_use]
    pub fn ice_clear_credit() -> Self {
        Self::new(CcpMethodology::IceClearCredit)
    }

    /// Create calculator for CME.
    #[must_use]
    pub fn cme() -> Self {
        Self::new(CcpMethodology::Cme)
    }

    /// Create a generic VaR-based fallback calculator.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Confidence level in decimal form, such as `0.99`
    /// * `lookback_days` - Historical lookback window in calendar days
    ///
    /// # Returns
    ///
    /// A calculator with generic VaR metadata and default conservative-rate lookup.
    #[must_use]
    pub fn generic_var(confidence: f64, lookback_days: u32) -> Self {
        Self::new(CcpMethodology::GenericVaR {
            confidence,
            lookback_days,
        })
    }

    /// Create a generic VaR-based fallback calculator from registry defaults.
    #[must_use]
    pub fn generic_var_default() -> Self {
        Self::new(CcpMethodology::generic_var_from_registry(
            embedded_registry_or_panic(),
        ))
    }

    /// Attach a CCP input source that provides external VaR or SPAN outputs.
    ///
    /// # Arguments
    ///
    /// * `source` - Provider of CCP-supplied margin amounts and optional MPOR overrides
    ///
    /// # Returns
    ///
    /// The updated calculator.
    #[must_use]
    pub fn with_input_source(mut self, source: Arc<dyn super::ExternalImSource>) -> Self {
        self.input_source = Some(source);
        self
    }

    /// Calculate a conservative fallback IM amount from an exposure proxy.
    ///
    /// # Arguments
    ///
    /// * `exposure_base` - CCP-approved exposure base to scale, such as scan-risk
    ///   base, regulatory notional, or another house-defined proxy
    ///
    /// # Returns
    ///
    /// `|exposure_base| × conservative_rate`, where the rate is a decimal fraction.
    pub fn calculate_conservative(&self, exposure_base: Money) -> Money {
        super::conservative_im(exposure_base, self.params().conservative_rate)
    }
}

impl ImCalculator for ClearingHouseImCalculator {
    fn calculate(
        &self,
        instrument: &dyn Marginable,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<ImResult> {
        let mut mpor_days = self.params().mpor_days;

        // Try the external source first (and pick up any MPOR override).
        // If no source is registered or the source declines this trade,
        // fall back to `|exposure_base| × conservative_rate`. The
        // exposure-base lookup fails closed when neither is available.
        let im_amount = if let Some(source) = &self.input_source {
            if let Some(override_mpor) = source.external_mpor_days() {
                mpor_days = override_mpor;
            }
            source
                .external_initial_margin(instrument, context, as_of)
                .map_or_else(
                    || self.conservative_fallback(instrument, context, as_of),
                    Ok,
                )?
        } else {
            self.conservative_fallback(instrument, context, as_of)?
        };

        let mut breakdown = HashMap::default();
        breakdown.insert(self.methodology.to_string(), im_amount);

        Ok(ImResult::with_breakdown(
            im_amount,
            ImMethodology::ClearingHouse,
            as_of,
            mpor_days,
            breakdown,
        ))
    }

    fn methodology(&self) -> ImMethodology {
        ImMethodology::ClearingHouse
    }
}

impl ClearingHouseImCalculator {
    /// Conservative `|exposure_base| × rate` fallback used when no
    /// external CCP IM is available. Fails closed (via
    /// [`super::require_im_exposure_base`]) if the instrument cannot
    /// supply a regulatory exposure base.
    fn conservative_fallback(
        &self,
        instrument: &dyn Marginable,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        let exposure_base = super::require_im_exposure_base(
            "ClearingHouse",
            instrument,
            context,
            as_of,
            "an external IM source amount",
        )?;
        Ok(self.calculate_conservative(exposure_base))
    }

    fn params(&self) -> CcpParams {
        if let Some(p) = &self.params_override {
            return p.clone();
        }
        self.methodology
            .params_from_registry(embedded_registry_or_panic())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use std::sync::Arc;

    #[derive(Clone)]
    struct TestInstrument {
        id: String,
        value: Money,
    }

    impl TestInstrument {
        fn new(value: Money) -> Self {
            Self {
                id: "TEST-INST".to_string(),
                value,
            }
        }
    }

    impl Marginable for TestInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn margin_spec(&self) -> Option<&crate::OtcMarginSpec> {
            None
        }

        fn netting_set_id(&self) -> Option<crate::NettingSetId> {
            None
        }

        fn simm_sensitivities(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> Result<crate::SimmSensitivities> {
            Ok(crate::SimmSensitivities::new(self.value.currency()))
        }

        fn mtm_for_vm(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
            Ok(self.value)
        }
    }

    #[test]
    fn ccp_methodology_display() {
        assert_eq!(CcpMethodology::LchSwapClear.to_string(), "LCH SwapClear");
        assert_eq!(
            CcpMethodology::IceClearCredit.to_string(),
            "ICE Clear Credit"
        );
    }

    #[test]
    fn conservative_rates() {
        let lch = ClearingHouseImCalculator::lch_swapclear();
        let ice = ClearingHouseImCalculator::ice_clear_credit();
        assert_eq!(lch.params().conservative_rate, 0.02);
        assert_eq!(ice.params().conservative_rate, 0.10);
    }

    #[test]
    fn mpor_days() {
        let lch = ClearingHouseImCalculator::lch_swapclear();
        let cme = ClearingHouseImCalculator::cme();
        assert_eq!(lch.params().mpor_days, 5);
        assert_eq!(cme.params().mpor_days, 5);
    }

    #[test]
    fn conservative_calculation() {
        let calc = ClearingHouseImCalculator::lch_swapclear();
        let notional = Money::new(100_000_000.0, Currency::USD);
        let im = calc.calculate_conservative(notional);

        // LCH SwapClear ~2%
        assert_eq!(im.amount(), 2_000_000.0);
    }

    #[test]
    fn ice_clear_credit_calculation() {
        let calc = ClearingHouseImCalculator::ice_clear_credit();
        let notional = Money::new(50_000_000.0, Currency::USD);
        let im = calc.calculate_conservative(notional);

        // ICE Clear Credit ~10%
        assert_eq!(im.amount(), 5_000_000.0);
    }

    #[test]
    fn ccp_name_mapping() {
        assert_eq!(
            CcpMethodology::from_ccp_name("LCH"),
            CcpMethodology::LchSwapClear
        );
        assert_eq!(
            CcpMethodology::from_ccp_name("LCH CDSClear"),
            CcpMethodology::LchCdsClear
        );
        assert_eq!(
            CcpMethodology::from_ccp_name("ICE Clear Credit"),
            CcpMethodology::IceClearCredit
        );
    }

    #[derive(Debug)]
    struct TestInputSource {
        amount: Money,
        mpor_days: u32,
    }

    impl super::super::ExternalImSource for TestInputSource {
        fn external_initial_margin(
            &self,
            _instrument: &dyn Marginable,
            _context: &MarketContext,
            _as_of: Date,
        ) -> Option<Money> {
            Some(self.amount)
        }

        fn external_mpor_days(&self) -> Option<u32> {
            Some(self.mpor_days)
        }
    }

    #[test]
    fn uses_ccp_input_source_when_available() {
        let calc = ClearingHouseImCalculator::lch_swapclear().with_input_source(Arc::new(
            TestInputSource {
                amount: Money::new(3_000_000.0, Currency::USD),
                mpor_days: 7,
            },
        ));
        let notional = Money::new(100_000_000.0, Currency::USD);
        let fallback = calc.calculate_conservative(notional);

        let fake_inst = TestInstrument::new(notional);
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date");
        let im = calc.calculate(&fake_inst, &market, as_of).expect("im");

        assert_eq!(fallback.amount(), 2_000_000.0);
        assert_eq!(im.amount.amount(), 3_000_000.0);
        assert_eq!(im.mpor_days, 7);
    }

    #[test]
    fn fails_closed_without_external_source_or_exposure_base() {
        let calc = ClearingHouseImCalculator::lch_swapclear();
        let instrument = TestInstrument::new(Money::new(0.0, Currency::USD));
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date");

        let err = calc
            .calculate(&instrument, &market, as_of)
            .expect_err("CCP IM must not use zero MtM as a notional proxy");

        assert!(
            err.to_string().contains("external IM source")
                && err.to_string().contains("exposure base"),
            "expected missing source/exposure-base error, got {err}"
        );
    }
}
