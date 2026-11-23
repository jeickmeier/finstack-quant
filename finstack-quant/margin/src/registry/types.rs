//! Resolved margin registry data types and default-conversion helpers.

use finstack_quant_core::FxHashMap;
use finstack_quant_core::money::Money;
use serde::Deserialize;

use crate::SimmVersion;
use crate::calculators::im::schedule::{MaturityBucket, ScheduleAssetClass};
use crate::types::{
    CollateralAssetClass, EligibleCollateralSchedule, ImMethodology, ImParameters,
    MarginCallTiming, MarginTenor, SimmCreditSector, SimmRiskClass, VmParameters,
};

/// Fully resolved, ready-to-use margin registry.
///
/// This is the public, parsed view of the embedded margin registry after any
/// optional JSON overlay has been applied.
#[derive(Debug, Clone)]
pub(crate) struct MarginRegistry {
    /// Workspace-wide default VM, IM, timing, and settlement settings.
    pub(crate) defaults: MarginDefaults,
    /// Regulatory schedule IM grids keyed by registry id such as `"bcbs_iosco"`.
    pub(crate) schedule_im: FxHashMap<String, ScheduleImSchedule>,
    /// Default collateral haircuts keyed by collateral asset class.
    pub(crate) collateral_asset_class_defaults: FxHashMap<CollateralAssetClass, AssetClassDefault>,
    /// Eligible collateral schedules keyed by registry id.
    pub(crate) collateral_schedules: FxHashMap<String, EligibleCollateralSchedule>,
    /// CCP conservative fallback parameters keyed by CCP identifier.
    pub(crate) ccp: FxHashMap<String, CcpParams>,
    /// Resolved default CCP conservative fallback parameters.
    pub(crate) ccp_default_params: CcpParams,
    /// SIMM parameter sets keyed by registry id such as `"v2_6"`.
    pub(crate) simm: FxHashMap<String, SimmParams>,
}

/// Top-level default settings shared across margin methodologies.
#[derive(Debug, Clone)]
pub(crate) struct MarginDefaults {
    /// Variation-margin defaults.
    pub(crate) vm: VmDefaults,
    /// Initial-margin defaults split by methodology.
    pub(crate) im: ImDefaults,
    /// Margin call timing defaults for different agreement types.
    pub(crate) timing: TimingDefaults,
    /// Settlement defaults for cleared margin flows.
    pub(crate) cleared_settlement: ClearedSettlementDefaults,
}

/// Default variation-margin parameters stored in raw numeric form.
#[derive(Debug, Clone)]
pub(crate) struct VmDefaults {
    /// VM threshold amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub(crate) threshold: f64,
    /// Minimum transfer amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub(crate) mta: f64,
    /// Rounding amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub(crate) rounding: f64,
    /// Independent amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub(crate) independent_amount: f64,
    /// Margin-call frequency.
    pub(crate) frequency: MarginTenor,
    /// Settlement lag in business days.
    pub(crate) settlement_lag: u32,
}

/// Default initial-margin settings grouped by methodology.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImDefaults {
    /// Defaults for SIMM-based bilateral IM.
    pub(crate) simm: ImMethodDefaults,
    /// Defaults for schedule-based IM.
    pub(crate) schedule: ImMethodDefaults,
    /// Defaults for cleared-derivative CCP IM.
    pub(crate) cleared: ImMethodDefaults,
    /// Defaults for repo haircut IM.
    pub(crate) repo_haircut: ImMethodDefaults,
}

/// Raw default parameters for a single IM methodology.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImMethodDefaults {
    /// Margin period of risk in business days.
    pub(crate) mpor_days: u32,
    /// Threshold amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub(crate) threshold: f64,
    /// Minimum transfer amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub(crate) mta: f64,
    /// Whether the methodology assumes segregated collateral.
    pub(crate) segregated: bool,
}

/// Default timing conventions for margin calls and responses.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TimingDefaults {
    /// Standard bilateral timing defaults.
    // Retained in the registry record for schema examples.
    #[cfg_attr(not(feature = "json-schema"), allow(dead_code))]
    pub(crate) standard: MarginCallTiming,
    /// Regulatory VM timing defaults.
    pub(crate) regulatory_vm: MarginCallTiming,
    /// Cleared or CCP timing defaults.
    pub(crate) ccp: MarginCallTiming,
}

/// Default settlement handling for cleared collateral flows.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClearedSettlementDefaults {
    /// Settlement rounding amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub(crate) rounding: f64,
    /// Settlement lag in business days.
    pub(crate) settlement_lag: u32,
}

/// Parsed regulatory schedule IM entry from the registry.
#[derive(Debug, Clone)]
pub(crate) struct ScheduleImSchedule {
    /// Year-fraction boundaries separating short, medium, and long buckets.
    pub(crate) boundaries: ScheduleBucketBoundaries,
    /// Fallback decimal rate used when a bucket-specific entry is absent.
    pub(crate) default_rate: f64,
    /// Default asset class used by the schedule calculator's trait-based fallback path.
    pub(crate) default_asset_class: ScheduleAssetClass,
    /// Default maturity expressed as a year fraction.
    pub(crate) default_maturity_years: f64,
    /// Margin period of risk in business days.
    pub(crate) mpor_days: u32,
    /// Decimal rates keyed by `(asset_class, maturity_bucket)`.
    pub(crate) rates: FxHashMap<(ScheduleAssetClass, MaturityBucket), f64>,
}

/// Boundaries, in years, for the schedule IM maturity buckets.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScheduleBucketBoundaries {
    /// Cutoff between short and medium maturity buckets, in years.
    pub(crate) short_to_medium: f64,
    /// Cutoff between medium and long maturity buckets, in years.
    pub(crate) medium_to_long: f64,
}

/// Default collateral haircut settings for an asset class.
#[derive(Debug, Clone)]
pub(crate) struct AssetClassDefault {
    /// Standard collateral haircut as a decimal fraction.
    pub(crate) standard_haircut: f64,
    /// Additional FX haircut as a decimal fraction when collateral currency differs.
    pub(crate) fx_addon: f64,
    /// Optional concentration limit as a fraction of total collateral.
    pub(crate) concentration_limit: Option<f64>,
}

/// Conservative fallback CCP parameters.
#[derive(Debug, Clone)]
pub(crate) struct CcpParams {
    /// Margin period of risk in business days.
    pub(crate) mpor_days: u32,
    /// Conservative fallback margin rate as a decimal fraction.
    pub(crate) conservative_rate: f64,
}

pub(super) struct ParsedCcpRegistry {
    pub(super) ccp: FxHashMap<String, CcpParams>,
    pub(super) ccp_default_params: CcpParams,
}

/// Registry-backed SIMM parameter set.
#[derive(Debug, Clone)]
pub(crate) struct SimmParams {
    /// SIMM version identifier for this parameter set.
    pub version: SimmVersion,
    /// Margin period of risk in business days.
    pub mpor_days: u32,
    /// Interest-rate delta risk weights keyed by tenor label.
    pub ir_delta_weights: FxHashMap<String, f64>,
    /// Credit-non-qualifying delta risk weight.
    pub cnq_delta_weight: f64,
    /// Equity delta risk weight.
    pub equity_delta_weight: f64,
    /// FX delta risk weight.
    pub fx_delta_weight: f64,
    /// FX delta intra-bucket correlation between distinct currency risk factors.
    ///
    /// Historical v2.6 regular/regular FX factors against USD use 0.5.
    pub fx_intra_bucket_correlation: f64,
    /// Cross-risk-class correlations keyed by risk-class pair.
    pub risk_class_correlations: FxHashMap<(SimmRiskClass, SimmRiskClass), f64>,
    /// Commodity delta risk weights keyed by bucket label.
    pub commodity_bucket_weights: FxHashMap<String, f64>,
    /// Interest-rate tenor correlations keyed by ordered tenor pair.
    pub ir_tenor_correlations: FxHashMap<(String, String), f64>,
    /// Inter-currency correlation γ for IR delta aggregation across currencies.
    /// Historical v2.6 paragraph 37 specifies 0.32.
    pub ir_inter_currency_correlation: f64,
    /// Interest-rate vega risk weight.
    pub ir_vega_weight: f64,
    /// Credit-qualifying vega risk weight.
    pub cq_vega_weight: f64,
    /// Credit-non-qualifying vega risk weight.
    pub cnq_vega_weight: f64,
    /// Equity vega risk weight.
    pub equity_vega_weight: f64,
    /// FX vega risk weight.
    pub fx_vega_weight: f64,
    /// Commodity vega risk weight.
    pub commodity_vega_weight: f64,
    /// Correlation of distinct IR subcurves within a currency (v2.6: 99.3%).
    pub ir_subcurve_correlation: f64,
    /// Commodity within-bucket factor correlations by numeric bucket.
    pub commodity_intra_bucket_correlations: FxHashMap<String, f64>,
    /// Interest-rate HVR for curvature scaling; historical v2.6 paragraph 34.
    pub ir_historical_volatility_ratio: f64,
    /// Equity HVR applied to volatility-weighted vega; paragraph 57.
    pub equity_historical_volatility_ratio: f64,
    /// FX HVR applied to volatility-weighted vega; paragraph 70.
    pub fx_historical_volatility_ratio: f64,
    /// Commodity HVR applied to volatility-weighted vega; paragraph 62.
    pub commodity_historical_volatility_ratio: f64,
    /// High-volatility FX delta weight in percentage points against USD; paragraph 69.
    pub fx_high_delta_weight: f64,
    /// FX delta correlation between regular and high-volatility currencies against USD.
    pub fx_regular_high_correlation: f64,
    /// FX delta correlation between two distinct high-volatility currencies against USD.
    pub fx_high_high_correlation: f64,
    /// Credit qualifying correlation across tenors of the same issuer; paragraph 42.
    pub cq_same_issuer_correlation: f64,
    /// Credit residual-bucket correlation for distinct risk factors; paragraphs 42 and 48.
    pub credit_residual_correlation: f64,
    /// Interest-rate delta weights in basis points for JPY by tenor.
    pub ir_delta_weights_low: FxHashMap<String, f64>,
    /// Interest-rate delta weights in basis points for high-volatility currencies by tenor.
    pub ir_delta_weights_high: FxHashMap<String, f64>,
    /// Raw DV01 thresholds in USD per bp, keyed by currency risk group.
    pub ir_delta_concentration_thresholds: FxHashMap<String, f64>,
    /// Volatility-weighted IR vega thresholds in USD, keyed by currency risk group.
    pub ir_vega_concentration_thresholds: FxHashMap<String, f64>,
    /// Raw FX delta thresholds in USD per one-percent relative move, keyed by category 1, 2 or 3.
    pub fx_delta_concentration_thresholds: FxHashMap<String, f64>,
    /// FX vega thresholds in USD keyed by ordered category pair, such as 1_2.
    pub fx_vega_concentration_thresholds: FxHashMap<String, f64>,
    /// Raw commodity delta thresholds in USD per one-percent relative move by numeric bucket.
    pub commodity_delta_concentration_thresholds: FxHashMap<String, f64>,
    /// Commodity vega thresholds in USD by numeric bucket.
    pub commodity_vega_concentration_thresholds: FxHashMap<String, f64>,
    /// Volatility-weighted vega concentration thresholds in USD for unclassified risk factors.
    pub vega_concentration_thresholds: FxHashMap<SimmRiskClass, f64>,
    /// Concentration thresholds keyed by SIMM risk class.
    pub concentration_thresholds: FxHashMap<SimmRiskClass, f64>,
    /// Credit qualifying bucket risk weights keyed by [`SimmCreditSector`].
    ///
    /// Maps each ISDA SIMM credit qualifying sector bucket to its risk weight.
    /// When populated, enables bucket-level credit delta aggregation with
    /// intra/inter-bucket diversification per ISDA SIMM v2.6.
    pub cq_bucket_weights: FxHashMap<SimmCreditSector, f64>,
    /// Intra-bucket name correlation for credit qualifying delta.
    ///
    /// Per ISDA SIMM v2.6, the correlation between distinct names within the
    /// same credit qualifying sector bucket (v2.6: 0.46).
    pub cq_intra_bucket_correlation: f64,
    /// Inter-bucket correlations for credit qualifying delta.
    ///
    /// Correlations between different ISDA SIMM credit qualifying sector
    /// buckets, keyed by ordered sector pair.
    pub cq_inter_bucket_correlations: FxHashMap<(SimmCreditSector, SimmCreditSector), f64>,
    /// Per-bucket concentration thresholds for credit qualifying delta.
    ///
    /// Thresholds apply to raw CS01 netted by issuer across tenors, before risk weighting.
    pub cq_concentration_thresholds: FxHashMap<SimmCreditSector, f64>,
    /// Commodity inter-bucket correlation matrix (17×17, row-major).
    ///
    /// Per ISDA SIMM v2.6 Table 11. Populated at registry load from the
    /// default SIMM commodity correlation table and PSD-validated via
    /// `validate_simm_correlations_psd`. Bucket
    /// 16 ("Other" / residual) is zero-correlated with every other
    /// bucket per the specification.
    pub commodity_inter_bucket_correlations: Vec<f64>,
}

impl VmDefaults {
    /// Convert raw VM defaults into currency-tagged [`VmParameters`].
    ///
    /// # Arguments
    ///
    /// * `currency` - Currency used to wrap raw numeric defaults in [`finstack_quant_core::money::Money`]
    ///
    /// # Returns
    ///
    /// Concrete VM parameters in `currency`.
    pub(crate) fn to_vm_params(
        &self,
        currency: finstack_quant_core::currency::Currency,
    ) -> finstack_quant_core::Result<VmParameters> {
        Ok(VmParameters {
            threshold: Money::new(self.threshold, currency)?,
            mta: Money::new(self.mta, currency)?,
            rounding: Money::new(self.rounding, currency)?,
            independent_amount: Money::new(self.independent_amount, currency)?,
            frequency: self.frequency,
            settlement_lag: self.settlement_lag,
        })
    }
}

impl ImMethodDefaults {
    /// Convert raw IM defaults into currency-tagged [`ImParameters`].
    ///
    /// # Arguments
    ///
    /// * `methodology` - IM methodology label for the returned parameter set
    /// * `currency` - Currency used to wrap raw numeric defaults in [`finstack_quant_core::money::Money`]
    ///
    /// # Returns
    ///
    /// Concrete IM parameters in `currency`.
    pub(crate) fn to_im_params(
        &self,
        methodology: ImMethodology,
        currency: finstack_quant_core::currency::Currency,
    ) -> finstack_quant_core::Result<ImParameters> {
        Ok(ImParameters {
            methodology,
            mpor_days: self.mpor_days,
            threshold: Money::new(self.threshold, currency)?,
            mta: Money::new(self.mta, currency)?,
            segregated: self.segregated,
        })
    }
}
