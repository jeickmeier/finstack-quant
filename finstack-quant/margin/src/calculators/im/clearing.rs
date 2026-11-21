//! Clearing house IM calculator — **conservative proxy, not a CCP replication**.
//!
//! This calculator does NOT replicate any CCP's SPAN or VaR methodology. It
//! scales an explicit exposure base (`Marginable::im_exposure_base`) by a
//! conservative rate sourced from the margin registry. Results carry
//! `ImResult::approximation == true` and emit a `tracing::warn!`.

use crate::Marginable;
use crate::calculators::traits::{ImCalculator, ImResult};
use crate::registry::{CcpParams, MarginRegistry, embedded_registry_or_panic};
use crate::types::ImMethodology;
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// CCP methodology type.
///
/// Represents the clearing-house rulebook used to source conservative fallback
/// parameters such as MPOR and the decimal conservative-rate proxy.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
    /// Conservative exposure-times-rate fallback for an unknown CCP.
    GenericProxy,
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
            CcpMethodology::GenericProxy => write!(f, "Generic proxy"),
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
            CcpMethodology::GenericProxy => "generic_proxy",
        }
    }

    fn params_from_registry(&self, registry: &MarginRegistry) -> CcpParams {
        let key = self.registry_key();
        if let Some(params) = registry.ccp.get(key) {
            return params.clone();
        }
        registry.ccp_default_params.clone()
    }

    /// Choose a CCP methodology from a CCP display name.
    ///
    /// The mapping is heuristic and string-based. Unknown names fall back to
    /// [`CcpMethodology::GenericProxy`].
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
        let normalized = ccp.trim().to_ascii_lowercase();
        let is_credit = normalized.contains("credit") || normalized.contains("cds");

        if normalized.contains("lch") {
            if is_credit {
                CcpMethodology::LchCdsClear
            } else {
                CcpMethodology::LchSwapClear
            }
        } else if normalized.contains("ice") {
            if is_credit {
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
            CcpMethodology::GenericProxy
        }
    }
}

/// Clearing house IM calculator — a conservative proxy, not a CCP replication.
///
/// Provides CCP-specific initial-margin approximations for cleared derivatives.
/// It requires [`Marginable::im_exposure_base`] and scales that explicit CCP
/// exposure base by a conservative rate. It refuses to use current MtM as a
/// pseudo-notional.
///
/// # Conventions
///
/// - `conservative_rate` values are decimal fractions, so `0.02` means 2%.
/// - MPOR is expressed in calendar days.
/// - Calculations use [`Marginable::im_exposure_base`], not current MtM.
///
/// # Example
///
/// ```no_run
/// use finstack_quant_margin::Marginable;
/// use finstack_quant_margin::calculators::im::ClearingHouseImCalculator;
/// use finstack_quant_margin::calculators::ImCalculator;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let calc = ClearingHouseImCalculator::new(finstack_quant_margin::calculators::im::CcpMethodology::LchSwapClear);
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
#[derive(Debug, Clone)]
pub struct ClearingHouseImCalculator {
    /// CCP methodology
    pub methodology: CcpMethodology,
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
    /// A calculator using registry-backed conservative proxy parameters.
    #[must_use]
    pub fn new(methodology: CcpMethodology) -> Self {
        Self { methodology }
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
    ///
    /// # Errors
    ///
    /// Returns an error when the conservative rate is non-finite or the
    /// scaled amount overflows the Decimal amount range.
    pub fn calculate_conservative(&self, exposure_base: Money) -> Result<Money> {
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
        let mpor_days = self.params().mpor_days;
        let exposure_base = super::require_im_exposure_base(
            "ClearingHouse",
            instrument,
            context,
            as_of,
            "an explicit CCP exposure base",
        )?;
        let im_amount = self.calculate_conservative(exposure_base)?;

        tracing::warn!(
            methodology = %self.methodology,
            instrument = instrument.id(),
            "CCP IM computed via conservative proxy (|exposure_base| x rate); \
             this is NOT a SPAN/VaR replication of the CCP's methodology"
        );

        let mut result = ImResult::with_single_breakdown(
            im_amount,
            ImMethodology::ClearingHouse,
            as_of,
            mpor_days,
            self.methodology.to_string(),
        );
        result.approximation = true;
        Ok(result)
    }
}

impl ClearingHouseImCalculator {
    fn params(&self) -> CcpParams {
        self.methodology
            .params_from_registry(embedded_registry_or_panic())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;

    #[derive(Clone)]
    struct TestInstrument {
        id: String,
        value: Money,
        exposure_base: Option<Money>,
    }

    impl TestInstrument {
        fn new(value: Money) -> Self {
            Self {
                id: "TEST-INST".to_string(),
                value,
                exposure_base: None,
            }
        }
    }

    impl Marginable for TestInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn margin_spec(&self) -> Option<&crate::types::OtcMarginSpec> {
            None
        }

        fn netting_set_id(&self) -> Option<crate::types::NettingSetId> {
            None
        }

        fn simm_sensitivities(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> Result<crate::types::SimmSensitivities> {
            Ok(crate::types::SimmSensitivities::new(self.value.currency()))
        }

        fn mtm_for_vm(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
            Ok(self.value)
        }

        fn im_exposure_base(&self, _market: &MarketContext, _as_of: Date) -> Result<Option<Money>> {
            Ok(self.exposure_base)
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
        let lch =
            ClearingHouseImCalculator::new(crate::calculators::im::CcpMethodology::LchSwapClear);
        let ice =
            ClearingHouseImCalculator::new(crate::calculators::im::CcpMethodology::IceClearCredit);
        assert_eq!(lch.params().conservative_rate, 0.02);
        assert_eq!(ice.params().conservative_rate, 0.10);
    }

    #[test]
    fn mpor_days() {
        let lch =
            ClearingHouseImCalculator::new(crate::calculators::im::CcpMethodology::LchSwapClear);
        let cme = ClearingHouseImCalculator::new(crate::calculators::im::CcpMethodology::Cme);
        assert_eq!(lch.params().mpor_days, 5);
        assert_eq!(cme.params().mpor_days, 5);
    }

    #[test]
    fn conservative_calculation() {
        let calc =
            ClearingHouseImCalculator::new(crate::calculators::im::CcpMethodology::LchSwapClear);
        let notional = Money::from((100_000_000_i64, Currency::USD));
        let im = calc
            .calculate_conservative(notional)
            .expect("conservative im");

        // LCH SwapClear ~2%
        assert_eq!(im.amount(), 2_000_000.0);
    }

    #[test]
    fn ice_clear_credit_calculation() {
        let calc =
            ClearingHouseImCalculator::new(crate::calculators::im::CcpMethodology::IceClearCredit);
        let notional = Money::from((50_000_000_i64, Currency::USD));
        let im = calc
            .calculate_conservative(notional)
            .expect("conservative im");

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

    #[test]
    fn fails_closed_without_exposure_base() {
        let calc =
            ClearingHouseImCalculator::new(crate::calculators::im::CcpMethodology::LchSwapClear);
        let instrument = TestInstrument::new(Money::from((0_i64, Currency::USD)));
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date");

        let err = calc
            .calculate(&instrument, &market, as_of)
            .expect_err("CCP IM must not use zero MtM as a notional proxy");

        assert!(
            err.to_string().contains("exposure base"),
            "expected missing exposure-base error, got {err}"
        );
    }

    #[test]
    fn conservative_fallback_is_marked_as_approximation() {
        // The calculator scales the exposure base by the conservative rate.
        // That number is a proxy, not SPAN/VaR — the result must say so.
        let calc =
            ClearingHouseImCalculator::new(crate::calculators::im::CcpMethodology::LchSwapClear);
        let notional = Money::from((100_000_000_i64, Currency::USD));
        let mut inst = TestInstrument::new(notional);
        inst.exposure_base = Some(notional);
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date");

        let im = calc.calculate(&inst, &market, as_of).expect("im");
        assert!(
            im.approximation,
            "conservative proxy must set approximation = true"
        );
        assert_eq!(im.amount.amount(), 2_000_000.0); // 2% of 100mm
    }
}
