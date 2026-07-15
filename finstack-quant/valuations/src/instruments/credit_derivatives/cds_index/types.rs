//! CDS Index types and implementations.
//!
//! This module defines the `CDSIndex` instrument along with its pricing
//! configuration and constituents. The index can be priced in two modes:
//! - `SingleCurve`: delegate to a synthetic single-name CDS priced off a
//!   single index hazard curve.
//! - `Constituents`: expand into per-name CDS positions with weights and
//!   aggregate results across names.

use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::common_impl::parameters::CreditParams;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::PricingOverrides;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_margin::types::OtcMarginSpec;
use rust_decimal::Decimal;
use time::macros::date;

// Reuse CDS components for conventions and legs
use crate::instruments::credit_derivatives::cds::{
    CDSConvention, CreditDefaultSwap, PayReceive, PremiumLegSpec, ProtectionLegSpec,
};

use super::parameters::CDSIndexParams;
use super::pricer::CDSIndexPricer;
use crate::impl_instrument_base;

/// Pricing mode for CDS indices.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum IndexPricing {
    /// Price the index against a single index hazard curve (synthetic CDS)
    SingleCurve,
    /// Price each issuer separately and aggregate by weight
    Constituents,
}

/// Par spread denominator method for indices in constituents mode.
/// Method for computing par spread of a CDS index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParSpreadMethod {
    /// Par spread computed using risky annuity (RPV01) method
    RiskyAnnuity,
    /// Par spread with full premium and accrual-on-default
    FullPremiumAoD,
}

/// Constituent in a CDS index with weight and credit parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CDSIndexConstituent {
    /// Credit configuration for the issuer (includes hazard curve id and recovery)
    pub credit: CreditParams,
    /// Weight of the issuer in the index notional (e.g., 1/125.0 for CDX IG)
    pub weight: f64,
    /// Whether the constituent has defaulted. Defaulted names are excluded from the
    /// premium leg but their settled protection payment is already reflected in `index_factor`.
    /// Per O'Kane (2008) Ch. 7: "On default, the protection payment is settled and the
    /// name is removed from the index. The index factor adjusts to reflect the reduced notional."
    #[serde(default)]
    pub defaulted: bool,
}

impl CDSIndexConstituent {
    /// Construct an active (non-defaulted) constituent.
    ///
    /// Common case for new trades: `defaulted` defaults to `false`. Mark
    /// names as defaulted explicitly via the struct literal when modeling
    /// post-default state.
    pub fn active(credit: CreditParams, weight: f64) -> Self {
        Self {
            credit,
            weight,
            defaulted: false,
        }
    }
}

/// Per-constituent result entry for index-level analytics.
#[derive(Debug, Clone)]
pub struct ConstituentResult<T> {
    /// Hazard curve identifier for the constituent.
    pub credit_curve_id: CurveId,
    /// Recovery rate used for the constituent.
    pub recovery_rate: f64,
    /// Raw weight supplied on the index definition.
    pub weight_raw: f64,
    /// Effective weight used after optional normalization.
    pub weight_effective: f64,
    /// Computed value for the constituent.
    pub value: T,
}

/// Aggregate result for index-level analytics.
///
/// In `SingleCurve` mode, `constituents` is empty.
#[derive(Debug, Clone)]
pub struct IndexResult<T> {
    /// Total aggregated value.
    pub total: T,
    /// Optional per-constituent breakdown.
    pub constituents: Vec<ConstituentResult<T>>,
}

impl<T> IndexResult<T> {
    /// Construct a single-curve result with no breakdown.
    pub fn single_curve(total: T) -> Self {
        Self {
            total,
            constituents: Vec::new(),
        }
    }
}

/// Detailed par spread result for CDS indices.
///
/// Note: constituent par spreads are informational and are not additive.
#[derive(Debug, Clone)]
pub struct IndexParSpreadResult {
    /// Total par spread in basis points.
    pub total_spread_bp: f64,
    /// Per-constituent par spreads in basis points (informational).
    pub constituents_spread_bp: Vec<ConstituentResult<f64>>,
    /// Par spread denominator methodology.
    pub method: ParSpreadMethod,
    /// Aggregated protection PV used in the total calculation.
    pub numerator_protection_pv: Money,
    /// Aggregated denominator used in the total calculation.
    pub denominator: f64,
}

/// CDS Index instrument definition
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CDSIndex {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Index name, e.g., "CDX.NA.IG", "CDX.NA.HY", "iTraxx Europe"
    pub index_name: String,
    /// Series number (e.g., 42)
    pub series: u16,
    /// Version number within series
    pub version: u16,
    /// Notional amount of the index
    pub notional: Money,
    /// Index factor (fraction of surviving notional since series inception)
    pub index_factor: f64,
    /// Protection buyer/seller perspective
    pub side: PayReceive,
    /// Regional ISDA convention
    pub convention: CDSConvention,
    /// Premium leg specification (coupon schedule and discounting)
    pub premium: PremiumLegSpec,
    /// Protection leg specification (credit curve and settlement)
    pub protection: ProtectionLegSpec,
    /// Pricing aggregation mode
    pub pricing: IndexPricing,
    /// Optional list of constituents when using `IndexPricing::Constituents`
    pub constituents: Vec<CDSIndexConstituent>,
    /// Number of reference entities in the index pool.
    ///
    /// Required for portfolio-level analytics (e.g. jump-to-default) when
    /// `constituents` is empty (`SingleCurve` mode), since the per-name
    /// risk number is `notional / N · LGD`. Index pool sizes drift with
    /// series — iTraxx Crossover has been 75 names only since Series 9,
    /// and CDX.NA.HY membership varies — so this is supplied explicitly
    /// rather than inferred from `index_name`. Standard presets populate
    /// it via `CDSIndex::from_preset`; set it directly with
    /// `with_num_constituents` for custom indices.
    #[serde(default)]
    #[builder(default)]
    pub num_constituents: Option<u32>,
    /// Pricing overrides (including upfront payment)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Optional OTC margin specification for VM/IM.
    ///
    /// CDS indices are typically cleared through ICE Clear Credit. Use
    /// `OtcMarginSpec::cleared("ICE", Currency::USD)` for standard
    /// cleared indices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_spec: Option<OtcMarginSpec>,
    /// Attributes for tagging and selection
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

impl CDSIndex {
    fn premium_with_standard_defaults(&self) -> PremiumLegSpec {
        let mut premium = self.premium.clone();
        if premium.calendar_id.is_none() {
            premium.calendar_id = Some(self.convention.default_calendar().to_string());
        }
        premium
    }

    fn protection_with_standard_defaults(&self) -> ProtectionLegSpec {
        let mut protection = self.protection.clone();
        if protection.settlement_delay == 0 {
            protection.settlement_delay = self.convention.settlement_delay();
        }
        protection
    }

    /// Create a canonical example CDS Index for testing and documentation.
    ///
    /// Returns a CDX.NA.IG series 42 index with standard conventions.
    pub fn example() -> Self {
        let convention = CDSConvention::IsdaNa;
        let dc = convention.day_count();
        let freq = convention.frequency();
        let bdc = convention.business_day_convention();
        let stub = convention.stub_convention();

        Self {
            id: InstrumentId::new("CDX-IG-42"),
            index_name: "CDX.NA.IG".to_string(),
            series: 42,
            version: 1,
            notional: Money::new(10_000_000.0, Currency::USD),
            index_factor: 1.0,
            side: PayReceive::Pay,
            convention,
            premium: PremiumLegSpec {
                start: date!(2024 - 03 - 20),
                end: date!(2029 - 12 - 20),
                frequency: freq,
                stub,
                bdc,
                calendar_id: Some(convention.default_calendar().to_string()),
                day_count: dc,
                spread_bp: Decimal::from(60),
                discount_curve_id: CurveId::new("USD-OIS"),
            },
            protection: ProtectionLegSpec {
                credit_curve_id: CurveId::new("CDX.NA.IG.HAZARD"),
                recovery_rate: 0.40,
                settlement_delay: convention.settlement_delay(),
            },
            pricing: IndexPricing::SingleCurve,
            constituents: Vec::new(),
            num_constituents: Some(125),
            pricing_overrides: PricingOverrides::default(),
            margin_spec: None,
            attributes: Attributes::new(),
        }
    }

    /// Construct a `CDSIndex` from a preset descriptor + trade-specific args.
    ///
    /// Replaces the previous `new_standard` constructor. Trade state that is
    /// orthogonal to the preset (constituents, index factor) is attached
    /// via the chained `with_*` methods on the returned instrument.
    ///
    /// # Example
    ///
    /// ```text
    /// let idx = CDSIndex::from_preset(
    ///     &CDSIndexParams::cdx_na_ig(42, 1, 100.0),
    ///     "CDX-IG-42",
    ///     Money::new(10_000_000.0, Currency::USD),
    ///     PayReceive::Pay,
    ///     start, end,
    ///     0.40,
    ///     "USD-OIS",
    ///     "CDX.NA.IG.HAZARD",
    /// )?
    /// .with_index_factor(0.96)
    /// .with_constituents(constituents);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if `preset.fixed_coupon_bp` cannot be represented
    /// as `Decimal`.
    #[allow(clippy::too_many_arguments)]
    pub fn from_preset(
        preset: &CDSIndexParams,
        id: impl Into<InstrumentId>,
        notional: Money,
        side: PayReceive,
        start: finstack_quant_core::dates::Date,
        end: finstack_quant_core::dates::Date,
        recovery_rate: f64,
        discount_curve_id: impl Into<CurveId>,
        credit_curve_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let convention = preset.convention;
        let dc = convention.day_count();
        let freq = convention.frequency();
        let bdc = convention.business_day_convention();
        let stub = convention.stub_convention();

        let spread_bp_decimal = Decimal::try_from(preset.fixed_coupon_bp).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "fixed_coupon_bp {} cannot be represented as Decimal: {}",
                preset.fixed_coupon_bp, e
            ))
        })?;

        Ok(Self {
            id: id.into(),
            index_name: preset.index_name.clone(),
            series: preset.series,
            version: preset.version,
            notional,
            index_factor: 1.0,
            side,
            convention,
            premium: PremiumLegSpec {
                start,
                end,
                frequency: freq,
                stub,
                bdc,
                calendar_id: Some(convention.default_calendar().to_string()),
                day_count: dc,
                spread_bp: spread_bp_decimal,
                discount_curve_id: discount_curve_id.into(),
            },
            protection: ProtectionLegSpec {
                credit_curve_id: credit_curve_id.into(),
                recovery_rate,
                settlement_delay: convention.settlement_delay(),
            },
            pricing: IndexPricing::SingleCurve,
            constituents: Vec::new(),
            num_constituents: preset.num_constituents,
            pricing_overrides: PricingOverrides::default(),
            margin_spec: None,
            attributes: Attributes::new(),
        })
    }

    /// Set the number of reference entities in the index pool.
    ///
    /// Use this for custom or off-series indices whose membership count is
    /// not carried by a standard preset. Required for jump-to-default and
    /// other per-name analytics when `constituents` is empty.
    pub fn with_num_constituents(mut self, num_constituents: u32) -> Self {
        self.num_constituents = Some(num_constituents);
        self
    }

    /// Set the index factor (fraction of original notional surviving).
    ///
    /// Defaults to 1.0. Set to less than 1.0 when modeling an index after
    /// one or more constituents have defaulted; pair with `defaulted=true`
    /// on the corresponding `CDSIndexConstituent`s. Validation occurs at
    /// pricing time and rejects `index_factor` values that exceed
    /// `1 − Σ defaulted weights`.
    pub fn with_index_factor(mut self, factor: f64) -> Self {
        self.index_factor = factor;
        self
    }

    /// Validate trade-state invariants shared by all pricing modes.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.index_factor.is_finite() || !(0.0..=1.0).contains(&self.index_factor) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CDS Index '{}' has invalid index_factor {} (must be finite and within [0, 1])",
                self.id, self.index_factor
            )));
        }
        if !self.notional.amount().is_finite() || self.notional.amount() == 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CDS Index '{}' notional must be finite and non-zero, got {}",
                self.id,
                self.notional.amount()
            )));
        }
        if let Some(upfront) = self.pricing_overrides.market_quotes.upfront_payment {
            if upfront.currency() != self.notional.currency() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CDS Index '{}' upfront override currency {} must match notional currency {}",
                    self.id,
                    upfront.currency(),
                    self.notional.currency()
                )));
            }
        }
        Ok(())
    }

    /// Map this index to a synthetic single-name CDS for valuation reuse.
    ///
    /// The synthetic CDS notional reflects `notional × index_factor`, i.e.
    /// the surviving notional. Pricing this synthetic CDS directly therefore
    /// produces the same dollar leg PVs as `IndexPricing::SingleCurve` mode
    /// on the index itself.
    pub fn to_synthetic_cds(&self) -> CreditDefaultSwap {
        CreditDefaultSwap {
            id: self.id.to_owned(),
            notional: Money::new(
                self.notional.amount() * self.index_factor,
                self.notional.currency(),
            ),
            side: self.side,
            convention: self.convention,
            premium: self.premium_with_standard_defaults(),
            protection: self.protection_with_standard_defaults(),
            pricing_overrides: self.pricing_overrides.clone(),
            valuation_convention:
                crate::instruments::credit_derivatives::cds::CdsValuationConvention::default(),
            upfront: None,
            doc_clause: None,
            protection_effective_date: None,
            margin_spec: self.margin_spec.clone(),
            attributes: self.attributes.clone(),
        }
    }

    /// Configure equal-weight constituents by credit parameter set per name.
    /// Each name receives weight = 1/(n names). Mode switches to `Constituents`.
    pub fn with_constituents_equal_weight(
        mut self,
        names: impl IntoIterator<Item = CreditParams>,
    ) -> Self {
        let list: Vec<CreditParams> = names.into_iter().collect();
        if list.is_empty() {
            self.constituents.clear();
            self.pricing = IndexPricing::SingleCurve;
            return self;
        }
        let w = 1.0 / (list.len() as f64);
        self.constituents = list
            .into_iter()
            .map(|credit| CDSIndexConstituent {
                credit,
                weight: w,
                defaulted: false,
            })
            .collect();
        self.pricing = IndexPricing::Constituents;
        self
    }

    /// Configure explicit constituents with custom weights.
    pub fn with_constituents(mut self, constituents: Vec<CDSIndexConstituent>) -> Self {
        if constituents.is_empty() {
            self.constituents.clear();
            self.pricing = IndexPricing::SingleCurve;
        } else {
            self.constituents = constituents;
            self.pricing = IndexPricing::Constituents;
        }
        self
    }

    /// Calculate protection leg PV
    pub fn pv_protection_leg(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let pricer = CDSIndexPricer::new();
        pricer.pv_protection_leg(self, curves, as_of)
    }

    /// Calculate premium leg PV
    pub fn pv_premium_leg(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let pricer = CDSIndexPricer::new();
        pricer.pv_premium_leg(self, curves, as_of)
    }

    /// Calculate par spread
    pub fn par_spread(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let pricer = CDSIndexPricer::new();
        pricer.par_spread(self, curves, as_of)
    }

    /// Calculate risky PV01
    pub fn risky_pv01(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let pricer = CDSIndexPricer::new();
        pricer.risky_pv01(self, curves, as_of)
    }

    /// Calculate CS01
    pub fn cs01(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let pricer = CDSIndexPricer::new();
        pricer.cs01(self, curves, as_of)
    }

    /// Calculate NPV with per-constituent breakdown (if applicable).
    pub fn npv_detailed(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<IndexResult<Money>> {
        let pricer = CDSIndexPricer::new();
        pricer.npv_detailed(self, curves, as_of)
    }

    /// Calculate protection leg PV with per-constituent breakdown.
    pub fn pv_protection_leg_detailed(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<IndexResult<Money>> {
        let pricer = CDSIndexPricer::new();
        pricer.pv_protection_leg_detailed(self, curves, as_of)
    }

    /// Calculate premium leg PV with per-constituent breakdown.
    pub fn pv_premium_leg_detailed(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<IndexResult<Money>> {
        let pricer = CDSIndexPricer::new();
        pricer.pv_premium_leg_detailed(self, curves, as_of)
    }

    /// Calculate par spread with per-constituent breakdown.
    pub fn par_spread_detailed(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<IndexParSpreadResult> {
        let pricer = CDSIndexPricer::new();
        pricer.par_spread_detailed(self, curves, as_of)
    }

    /// Calculate risky PV01 with per-constituent breakdown.
    pub fn risky_pv01_detailed(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<IndexResult<f64>> {
        let pricer = CDSIndexPricer::new();
        pricer.risky_pv01_detailed(self, curves, as_of)
    }

    /// Calculate CS01 with per-constituent breakdown.
    pub fn cs01_detailed(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<IndexResult<f64>> {
        let pricer = CDSIndexPricer::new();
        pricer.cs01_detailed(self, curves, as_of)
    }
}

impl crate::instruments::common_impl::traits::Instrument for CDSIndex {
    impl_instrument_base!(crate::pricer::InstrumentType::CDSIndex);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        CDSIndex::validate(self)
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::HazardRate
    }

    fn as_marginable(&self) -> Option<&dyn finstack_quant_margin::Marginable> {
        Some(self)
    }
    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        deps.add_discount_curve(self.premium.discount_curve_id.clone());
        deps.add_credit_curve(self.protection.credit_curve_id.clone());
        if self.pricing == IndexPricing::Constituents {
            for constituent in &self.constituents {
                if !constituent.defaulted {
                    deps.add_credit_curve(constituent.credit.credit_curve_id.clone());
                }
            }
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let pricer = CDSIndexPricer::new();
        pricer.npv(self, curves, as_of)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.premium.end)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.premium.start)
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for CDSIndex {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let pricer = CDSIndexPricer::new();
        let schedule = pricer.build_projected_schedule(self, curves, as_of)?;
        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

// Declare canonical market dependencies for the DV01 calculator.
// In Constituents mode, include per-constituent credit curves so that DV01/BucketedDV01
// correctly bump all credit curves, not just the index-level one.
