//! Credit Default Swap (CDS) types and implementations.
//!
//! # Convention Defaults
//!
//! This module uses **ISDA North American (IsdaNa)** as the default convention
//! when no explicit convention is specified. This choice aligns with:
//!
//! - The ISDA CDS Standard Model (2014) which was developed primarily for
//!   US/Canadian CDS markets
//! - Bloomberg and Markit pricing tools which default to NA conventions
//! - The dominance of US credit markets in global CDS trading volume
//!
//! For European or Asian CDS, explicitly specify `CdsConvention::IsdaEu` or
//! `CdsConvention::IsdaAs` respectively. Use [`CdsConvention::detect_from_currency`]
//! for automatic detection based on currency.
//!
//! ## Regional Convention Summary
//!
//! | Region | Convention | Day Count | Payment Frequency | Settlement | Calendar |
//! |--------|-----------|-----------|-------------------|------------|----------|
//! | North America | `IsdaNa` | ACT/360 | Quarterly | T+3 | NYSE |
//! | Europe | `IsdaEu` | ACT/360 | Quarterly | T+1 | TARGET2 |
//! | Asia | `IsdaAs` | ACT/365F | Quarterly | T+3 | Tokyo |
//!
//! **Note**: European CDS settlement changed from T+3 to T+1 on June 20, 2009 as part of the
//! ISDA "Big Bang" protocol. This implementation uses the post-2009 T+1 standard.
//!
//! ## Example: Explicit Convention Selection
//!
//! ```
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_valuations::market::conventions::CdsConvention;
//!
//! // Detect from currency (recommended for cross-regional portfolios).
//! assert_eq!(
//!     CdsConvention::detect_from_currency(Currency::EUR),
//!     CdsConvention::IsdaEu
//! );
//! assert_eq!(
//!     CdsConvention::detect_from_currency(Currency::JPY),
//!     CdsConvention::IsdaAs
//! );
//!
//! // Anything else falls back to the most liquid market, North America.
//! assert_eq!(
//!     CdsConvention::detect_from_currency(Currency::USD),
//!     CdsConvention::IsdaNa
//! );
//! ```

use crate::constants::isda::STANDARD_RECOVERY_SENIOR;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::market::conventions::ids::CdsDocClause;
use crate::market::conventions::CdsConvention;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_margin::types::OtcMarginSpec;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::macros::date;

use crate::impl_instrument_base;
use crate::instruments::credit_derivatives::cds::pricing::CDSPricer;

pub use crate::instruments::common_impl::parameters::legs::PayReceive;

/// Valuation presentation and pricing policy for CDS marks.
///
/// Each variant bundles a coherent set of choices (premium-leg accrual schedule,
/// clean/dirty NPV, par-spread denominator). Mixing those choices via separate
/// boolean overrides is intentionally not supported — the variants here are the
/// only conventions traded in practice.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CdsValuationConvention {
    /// ISDA-style dirty model PV.
    ///
    /// Premium-leg cashflows accrue between unadjusted IMM dates, the final
    /// coupon excludes the maturity day, and the reported NPV is the dirty
    /// model PV (no add-back of accrued premium). Par spread uses the risky
    /// annuity denominator. Use only for direct reproduction of academic ISDA
    /// Standard Model literature.
    IsdaDirty,
    /// Bloomberg CDSW clean principal presentation and premium-leg policy.
    ///
    /// This is the industry-standard convention used by Bloomberg CDSW and
    /// the ISDA Standard Upfront Model. It is the default for new
    /// `CreditDefaultSwap` instances:
    ///
    /// - Premium cashflows accrue between business-day-adjusted dates that
    ///   match the Bloomberg CDSW cashflow schedule.
    /// - The final coupon period is inclusive of the maturity date (extra
    ///   day) per CDSW convention.
    /// - The reported NPV is the clean principal value (Bloomberg
    ///   "Principal" line). Cash settlement is `Principal + Accrued`.
    /// - Par spread uses the risky annuity denominator (matches the CDSW
    ///   screen for investment-grade credits).
    /// - Hazard rebootstrap inside risk metrics (CS01, recovery01, etc.)
    ///   inherits the same CDSW pricer convention so sensitivities are
    ///   self-consistent with the base PV.
    #[default]
    BloombergCdswClean,
    /// Bloomberg CDSW clean principal with full premium leg in the par-spread
    /// denominator (distressed-credit variant).
    ///
    /// Identical to [`Self::BloombergCdswClean`] except the par spread
    /// denominator includes accrual-on-default. The difference vs. risky
    /// annuity is typically < 1bp for investment grade and 2-5bps for
    /// distressed credits (hazard rate > 3%).
    BloombergCdswCleanFullPremium,
    /// QuantLib `IsdaCdsEngine` parity convention.
    ///
    /// Reproduces QuantLib's CDS output: dirty PV (no clean add-back),
    /// business-day-adjusted premium accrual periods, and full premium leg in
    /// the par-spread denominator. Combine with the QuantLib day-count
    /// pricing overrides (`cds_aod_half_day_bias`,
    /// `cds_act360_include_last_day`) for full bit-level reproduction.
    QuantLibIsdaParity,
}

impl CdsValuationConvention {
    /// Whether the convention reports clean principal (with accrued add-back).
    #[must_use]
    pub fn uses_clean_price(self) -> bool {
        matches!(
            self,
            Self::BloombergCdswClean | Self::BloombergCdswCleanFullPremium
        )
    }

    /// Whether the convention uses business-day-adjusted premium accrual periods.
    #[must_use]
    pub fn uses_adjusted_premium_accrual_dates(self) -> bool {
        matches!(
            self,
            Self::BloombergCdswClean
                | Self::BloombergCdswCleanFullPremium
                | Self::QuantLibIsdaParity
        )
    }

    /// Whether the par-spread denominator includes accrual-on-default.
    #[must_use]
    pub fn par_spread_uses_full_premium(self) -> bool {
        matches!(
            self,
            Self::BloombergCdswCleanFullPremium | Self::QuantLibIsdaParity
        )
    }

    /// Calendar days between `as_of` and the protection step-in (effective)
    /// date used as the lower bound of the protection-leg integral.
    ///
    /// - ISDA Standard Model / Markit step protection in at T+1: a default on
    ///   the valuation date itself is not covered (~$5/$10M/day on a 100bp
    ///   name).
    /// - QuantLib's `IsdaCdsEngine` integrates the default leg from the
    ///   valuation date even when the instrument carries a T+1 protection
    ///   start (verified against QuantLib 1.42.1; the in-tree
    ///   `cds_quantlib_flat_hazard_decomposition` golden pins this), so the
    ///   parity convention uses 0 days.
    /// - Bloomberg CDSW/CDSO integrate from the valuation date itself
    ///   ("Protection starts immediately", DOCS 2057273 §3); the in-tree
    ///   Bloomberg parity goldens pin this behaviour.
    #[must_use]
    pub fn protection_step_in_days(self) -> i64 {
        match self {
            Self::IsdaDirty => 1,
            Self::QuantLibIsdaParity
            | Self::BloombergCdswClean
            | Self::BloombergCdswCleanFullPremium => 0,
        }
    }
}

pub use crate::instruments::common_impl::parameters::legs::{PremiumLegSpec, ProtectionLegSpec};

/// Resolve a meta documentation clause (e.g., `IsdaNa`, `IsdaEu`) to its
/// concrete restructuring variant. Concrete clauses pass through unchanged.
fn resolve_doc_clause(clause: CdsDocClause) -> CdsDocClause {
    match clause {
        CdsDocClause::IsdaNa
        | CdsDocClause::IsdaAs
        | CdsDocClause::IsdaAu
        | CdsDocClause::IsdaNz => CdsDocClause::Xr14,
        CdsDocClause::IsdaEu => CdsDocClause::Mm14,
        // Concrete clauses pass through
        other => other,
    }
}

/// Credit Default Swap instrument.
///
/// # Market Standards & Citations (Week 5)
///
/// ## ISDA Standards
///
/// This implementation follows the **ISDA 2014 Credit Derivatives Definitions**:
/// - **Section 1.1:** General Terms and Credit Events
/// - **Section 3.2:** Fixed Payments (Premium Leg)
/// - **Section 3.3:** Floating Payments (Protection Leg)
/// - **Section 7.1:** Settlement Terms
///
/// ## ISDA CDS Standard Model
///
/// The pricing engine implements the **ISDA CDS Standard Model (2009)**:
/// - Quarterly premium payments (20th of Mar/Jun/Sep/Dec - IMM dates)
/// - ACT/360 day count
/// - Modified Following business day convention
/// - Accrual-on-default included in premium leg
/// - Settlement: T+3 (North America), T+1 (Europe post-2009)
///
/// ## Integration Method
///
/// Protection and accrual-on-default legs use one piecewise-analytical
/// integration engine. Hazard- and discount-curve knots are mandatory
/// boundaries; configurable intra-knot substeps control the additional
/// resolution between those boundaries.
///
/// ## References
///
/// - ISDA 2014 Credit Derivatives Definitions `docs/REFERENCES.md#isda-2014-credit-definitions`
/// - "Modelling Single-name and Multi-name Credit Derivatives" by O'Kane (2008) `docs/REFERENCES.md#o-kane-2008`
/// - ISDA CDS Standard Model Implementation (Markit, 2009) `docs/REFERENCES.md#isda-cds-standard-model`
/// - Bloomberg CDSW / *The Bloomberg CDS Model* (DOCS 2057273)
///   `docs/REFERENCES.md#bloomberg-cds-model`
///
/// See unit tests and `examples/` for usage.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
// Note: JsonSchema derive requires finstack-quant-core types to implement JsonSchema
// #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CreditDefaultSwap {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Notional amount
    pub notional: Money,
    /// Buyer/seller perspective
    pub side: PayReceive,
    /// ISDA convention
    pub convention: CdsConvention,
    /// Premium leg specification
    pub premium: PremiumLegSpec,
    /// Protection leg specification
    pub protection: ProtectionLegSpec,
    /// Instrument-owned pricing overrides (including upfront payment).
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-only pricing controls.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only valuation adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Valuation presentation convention.
    ///
    /// Defaults to Bloomberg CDSW clean. Set `IsdaDirty` only when
    /// reproducing academic ISDA Standard Model literature.
    #[serde(default)]
    #[builder(default)]
    pub valuation_convention: CdsValuationConvention,
    /// Upfront payment (Date, Money).
    ///
    /// The amount is defined as a payment from Protection Buyer to Protection Seller.
    /// - If positive: Buyer pays Seller.
    /// - If negative: Seller pays Buyer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "finstack_quant_core::wire::optional_dated_money")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<(finstack_quant_core::wire::DateWire, Money)>")
    )]
    pub upfront: Option<(Date, Money)>,
    /// ISDA documentation clause for restructuring credit events.
    ///
    /// Controls which restructuring events trigger protection payments and
    /// the maximum deliverable obligation maturity upon restructuring:
    ///
    /// - **Cr14** (Full Restructuring): All restructuring events trigger; no maturity cap.
    /// - **Mr14** (Modified Restructuring): Restructuring triggers with 30-month maturity cap.
    /// - **Mm14** (Modified-Modified Restructuring): Restructuring triggers with 60-month cap.
    /// - **Xr14** (No Restructuring): Restructuring does not trigger protection.
    ///
    /// If `None`, the effective clause is derived from the CDS convention:
    /// - `IsdaNa` / `IsdaAs` -> `Xr14` (no restructuring, North American / Asian standard)
    /// - `IsdaEu` -> `Mm14` (modified-modified restructuring, European standard)
    ///
    /// See [`doc_clause_effective`](Self::doc_clause_effective) for resolution logic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_clause: Option<CdsDocClause>,
    /// Optional protection effective date for forward-starting CDS.
    ///
    /// When `Some(date)`, protection begins on the specified date rather than
    /// the premium leg start date. This allows a CDS where premium accrues
    /// from the original start date but credit protection only kicks in later.
    ///
    /// Must satisfy: `premium.start <= protection_effective_date <= premium.end`.
    ///
    /// When `None`, protection starts on the premium leg start date (standard CDS).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub protection_effective_date: Option<Date>,
    /// Optional OTC margin specification for VM/IM.
    ///
    /// For cleared CDS (e.g., via ICE Clear Credit), use
    /// `OtcMarginSpec::cleared("ICE", Currency::USD)`. For bilateral
    /// CDS, use `OtcMarginSpec::bilateral_simm(...)`; an explicit
    /// `SimmCreditClassification` is required so CS01 is routed to the correct CQ sector
    /// bucket or the credit non-qualifying risk class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_spec: Option<OtcMarginSpec>,
    /// Additional attributes
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

impl CreditDefaultSwap {
    /// Return the CDS par spread implied by an immutable market snapshot.
    ///
    /// The result is expressed in basis points and uses this contract's
    /// valuation convention, premium schedule, discount curve, hazard curve,
    /// and recovery assumption. This narrow entry point supports calibration
    /// residuals without exposing the internal CDS pricer.
    ///
    /// # Arguments
    ///
    /// * `market` - Market containing the discount and hazard curves named by
    ///   this CDS. Curve recovery metadata must match the contract recovery.
    /// * `as_of` - Valuation date used for settlement and accrued-premium
    ///   conventions.
    ///
    /// # Errors
    ///
    /// Returns an error when the instrument is invalid, required curves are
    /// missing, recovery assumptions conflict, or par-spread pricing fails.
    pub fn get_par_spread(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::traits::Instrument::validate_for_pricing(self)?;
        let discount = market.get_discount(self.premium.discount_curve_id.as_str())?;
        let hazard = market.get_hazard(self.protection.credit_curve_id.as_str())?;
        super::pricing::CDSPricer::with_config(super::pricing::CDSPricerConfig::from_cds(self))
            .par_spread(self, discount.as_ref(), hazard.as_ref(), as_of)
    }

    /// Create a canonical example CDS for testing and documentation.
    ///
    /// Returns a 5-year investment-grade CDS with standard ISDA conventions.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        let convention = CdsConvention::IsdaNa;
        let day_count = convention.day_count();
        let frequency = convention.frequency();
        let business_day_convention = convention.business_day_convention();
        let stub = convention.stub_convention();

        let spread_bp_decimal = Decimal::try_from(100.0)
            .expect("Example CDS spread 100bp should always be representable as Decimal");

        let cds = CreditDefaultSwap::builder()
            .id(InstrumentId::new("CDS-CORP-5Y"))
            .notional(Money::from((10_000_000_i64, Currency::USD)))
            .side(PayReceive::Pay)
            .convention(convention)
            .premium(PremiumLegSpec {
                standard_imm_dates: true,
                start: date!(2024 - 03 - 20),
                end: date!(2029 - 03 - 20),
                frequency,
                stub,
                business_day_convention,
                calendar_id: Some(convention.default_calendar().to_string()),
                day_count,
                spread_bp: spread_bp_decimal,
                discount_curve_id: finstack_quant_core::types::CurveId::new("USD-OIS"),
            })
            .protection(ProtectionLegSpec {
                credit_curve_id: finstack_quant_core::types::CurveId::new("CORP-HAZARD"),
                recovery_rate: STANDARD_RECOVERY_SENIOR,
                settlement_delay: convention.settlement_delay(),
            })
            .instrument_pricing_overrides(Default::default())
            .attributes(Attributes::new())
            .build()
            .expect("Example CDS construction should not fail");

        cds.validate()
            .expect("Example CDS validation should not fail");

        cds
    }

    /// Create a new CDS with standard ISDA conventions using explicit inputs.
    ///
    /// Internal helper used by synthetic CDS creation in `cds_option` and
    /// `cds_index`. For the public API, use [`builder()`](Self::builder).
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails (e.g., recovery rate out of bounds).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_isda(
        id: impl Into<InstrumentId>,
        notional: Money,
        side: PayReceive,
        convention: CdsConvention,
        spread_bp: Decimal,
        start: finstack_quant_core::dates::Date,
        end: finstack_quant_core::dates::Date,
        recovery_rate: f64,
        discount_curve_id: impl Into<finstack_quant_core::types::CurveId>,
        credit_id: impl Into<finstack_quant_core::types::CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let day_count = convention.day_count();
        let frequency = convention.frequency();
        let business_day_convention = convention.business_day_convention();
        let stub = convention.stub_convention();

        let cds = Self {
            id: id.into(),
            notional,
            side,
            convention,
            premium: PremiumLegSpec {
                standard_imm_dates: true,
                start,
                end,
                frequency,
                stub,
                business_day_convention,
                calendar_id: Some(convention.default_calendar().to_string()),
                day_count,
                spread_bp,
                discount_curve_id: discount_curve_id.into(),
            },
            protection: ProtectionLegSpec {
                credit_curve_id: credit_id.into(),
                recovery_rate,
                settlement_delay: convention.settlement_delay(),
            },
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            valuation_convention: CdsValuationConvention::default(),
            upfront: None,
            doc_clause: None,
            protection_effective_date: None,
            margin_spec: None,
            attributes: Attributes::new(),
        };

        cds.validate()?;
        Ok(cds)
    }

    /// Validate all CDS parameters.
    ///
    /// Performs comprehensive validation of the CDS instrument:
    /// - Premium leg start date must be before end date
    /// - Recovery rate must be in [0, 1]
    ///
    /// Note: Zero notional and negative spreads are allowed as they represent
    /// valid edge cases (testing scenarios, unusual market conditions).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` with a descriptive message if any validation fails.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::CurveId;
    /// use finstack_quant_valuations::constants::isda::STANDARD_RECOVERY_SENIOR;
    /// use finstack_quant_valuations::instruments::credit_derivatives::cds::CdsConvention;
    /// use finstack_quant_valuations::instruments::{
    ///     Attributes, CreditDefaultSwap, InstrumentPricingOverrides, PayReceive, PremiumLegSpec,
    ///     ProtectionLegSpec,
    /// };
    /// use rust_decimal::Decimal;
    /// use time::macros::date;
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let cds = CreditDefaultSwap::builder()
    ///     .id("CDS-EXAMPLE".into())
    ///     .notional(Money::from((10_000_000_i64, Currency::USD)))
    ///     .side(PayReceive::Pay)
    ///     .convention(CdsConvention::IsdaNa)
    ///     .premium(PremiumLegSpec {
    ///         standard_imm_dates: true,
    ///         start: date!(2024 - 03 - 20),
    ///         end: date!(2029 - 03 - 20),
    ///         frequency: CdsConvention::IsdaNa.frequency(),
    ///         stub: CdsConvention::IsdaNa.stub_convention(),
    ///         business_day_convention: CdsConvention::IsdaNa.business_day_convention(),
    ///         calendar_id: Some(CdsConvention::IsdaNa.default_calendar().to_string()),
    ///         day_count: CdsConvention::IsdaNa.day_count(),
    ///         spread_bp: Decimal::try_from(100.0).expect("valid bp"),
    ///         discount_curve_id: CurveId::new("USD-OIS"),
    ///     })
    ///     .protection(ProtectionLegSpec {
    ///         credit_curve_id: CurveId::new("CORP-HAZARD"),
    ///         recovery_rate: STANDARD_RECOVERY_SENIOR,
    ///         settlement_delay: CdsConvention::IsdaNa.settlement_delay(),
    ///     })
    ///     .instrument_pricing_overrides(InstrumentPricingOverrides::default())
    ///     .attributes(Attributes::new())
    ///     .build()?;
    /// cds.validate()?; // Validates all parameters
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        // Validate date ordering (start must not be after end)
        // Note: start == end is allowed for "expired" CDS (valuation handles this edge case)
        validation::validate_date_range_non_strict(
            self.premium.start,
            self.premium.end,
            "CDS premium",
        )?;

        // Validate recovery rate (must be in [0, 1])
        validation::validate_recovery_rate(self.protection.recovery_rate)?;

        // Validate protection_effective_date bounds if set
        if let Some(ped) = self.protection_effective_date {
            if ped < self.premium.start {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CDS protection_effective_date ({}) must be >= premium start date ({})",
                    ped, self.premium.start
                )));
            }
            if ped > self.premium.end {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CDS protection_effective_date ({}) must be <= premium end date ({})",
                    ped, self.premium.end
                )));
            }
        }

        let currency = self.notional.currency();
        if let Some((_, upfront)) = self.upfront {
            if upfront.currency() != currency {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CDS upfront currency {} must match notional currency {}",
                    upfront.currency(),
                    currency
                )));
            }
        }
        if let Some(upfront) = self
            .instrument_pricing_overrides
            .market_quotes
            .upfront_payment
        {
            if upfront.currency() != currency {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CDS upfront override currency {} must match notional currency {}",
                    upfront.currency(),
                    currency
                )));
            }
        }

        if let Some(margin_spec) = &self.margin_spec {
            margin_spec.validate_for_credit()?;
        }

        // Note: Zero notional is allowed for testing scenarios
        // Note: Negative spreads are allowed (theoretically possible in unusual market conditions)

        Ok(())
    }

    /// Resolve the effective documentation clause.
    ///
    /// If an explicit `doc_clause` is set on the instrument, returns it directly.
    /// Otherwise, derives the standard clause from the CDS convention:
    ///
    /// | Convention | Default Clause | Rationale |
    /// |-----------|---------------|-----------|
    /// | `IsdaNa` | `Xr14` | NA standard: no restructuring (post-Big Bang) |
    /// | `IsdaEu` | `Mm14` | European standard: modified-modified restructuring |
    /// | `IsdaAs` | `Xr14` | Asian standard: follows NA convention |
    /// | `Custom` | `Xr14` | Conservative default |
    ///
    /// For meta-clauses (`IsdaNa`, `IsdaEu`, `IsdaAs` on `CdsDocClause`), the
    /// method further resolves them to their concrete restructuring variant.
    #[must_use]
    pub fn doc_clause_effective(&self) -> CdsDocClause {
        match self.doc_clause {
            Some(clause) => resolve_doc_clause(clause),
            None => match self.convention {
                CdsConvention::IsdaNa | CdsConvention::IsdaAs | CdsConvention::Custom => {
                    CdsDocClause::Xr14
                }
                CdsConvention::IsdaEu => CdsDocClause::Mm14,
            },
        }
    }

    /// Returns the effective protection start date.
    ///
    /// For a forward-starting CDS, this returns the `protection_effective_date`.
    /// For a standard (spot) CDS, this returns `premium.start`.
    #[must_use]
    pub fn protection_start(&self) -> Date {
        self.protection_effective_date.unwrap_or(self.premium.start)
    }

    pub(crate) fn uses_clean_price(&self) -> bool {
        self.valuation_convention.uses_clean_price()
    }

    pub(crate) fn uses_full_premium_par_spread_denominator(&self) -> bool {
        self.valuation_convention.par_spread_uses_full_premium()
    }

    pub(crate) fn uses_adjusted_premium_accrual_dates(&self) -> bool {
        self.valuation_convention
            .uses_adjusted_premium_accrual_dates()
    }

    fn build_premium_leg_schedule(
        &self,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let spread = self.premium.spread_bp.to_f64().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "premium spread_bp cannot be represented as f64".to_string(),
            )
        })? / 10_000.0;
        let pricer = CDSPricer::new();
        let payment_accruals = if self.uses_adjusted_premium_accrual_dates() {
            pricer.premium_cashflow_accruals(self, self.premium.start)?
        } else {
            pricer
                .generate_isda_schedule(self)?
                .windows(2)
                .map(|window| {
                    let accrual = self.premium.day_count.year_fraction(
                        window[0],
                        window[1],
                        finstack_quant_core::dates::DayCountContext::default(),
                    )?;
                    Ok((window[1], accrual))
                })
                .collect::<finstack_quant_core::Result<Vec<_>>>()?
        };
        let flows = payment_accruals
            .into_iter()
            .map(|(end, accrual)| {
                Ok(finstack_quant_core::cashflow::CashFlow::new(
                    end,
                    None,
                    Money::new(
                        self.notional.amount() * spread * accrual,
                        self.notional.currency(),
                    )?,
                    finstack_quant_core::cashflow::CFKind::Fixed,
                    accrual,
                    Some(spread),
                ))
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;

        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            self.premium.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(self.notional),
                ..Default::default()
            },
        ))
    }

    /// ISDA-standard coupon date schedule (IMM 20th dates).
    ///
    /// This is **not** a pricing entry point; it is a schedule helper that
    /// exposes the convention-driven coupon dates used by the CDS pricer.
    ///
    /// - With `premium.standard_imm_dates`, dates use the prescribed quarterly
    ///   CDS 20th grid. Bespoke legs use `premium.frequency` and `premium.stub`.
    /// - Payments use the instrument calendar and business-day convention;
    ///   absent calendars leave payment dates unadjusted.
    /// - The returned schedule includes the start date and the (possibly adjusted)
    ///   maturity date.
    pub fn isda_coupon_schedule(&self) -> finstack_quant_core::Result<Vec<Date>> {
        self.validate()?;
        let pricer = CDSPricer::new();
        pricer.generate_isda_schedule(self)
    }

    fn npv_raw_internal(
        &self,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        let disc = market.get_discount(&self.premium.discount_curve_id)?;
        let surv = market.get_hazard(&self.protection.credit_curve_id)?;
        CDSPricer::new().npv_full(self, disc.as_ref(), surv.as_ref(), as_of)
    }

    // (no public/raw-NPV helper; use `Instrument::value_raw()` instead)
}

impl crate::instruments::common_impl::traits::Instrument for CreditDefaultSwap {
    impl_instrument_base!(crate::pricer::InstrumentType::Cds);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        CreditDefaultSwap::validate(self)
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::HazardRate
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.premium.discount_curve_id.clone());
        deps.add_credit_curve(self.protection.credit_curve_id.clone());
        Ok(deps)
    }
    fn as_marginable(&self) -> Option<&dyn finstack_quant_margin::Marginable> {
        Some(self)
    }
    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let npv_amount = self.npv_raw_internal(market, as_of)?;
        finstack_quant_core::money::Money::new(npv_amount, self.notional.currency())
    }

    fn base_value_raw(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        self.npv_raw_internal(market, as_of)
    }

    fn base_value_raw_with_currency(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(f64, finstack_quant_core::currency::Currency)> {
        Ok((
            self.npv_raw_internal(market, as_of)?,
            self.notional.currency(),
        ))
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.premium.end)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.premium.start)
    }

    crate::impl_focused_pricing_overrides!();
}

impl crate::cashflow::traits::CashflowScheduleSource for CreditDefaultSwap {
    fn notional(&self) -> finstack_quant_core::Result<Option<finstack_quant_core::money::Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let mut schedule = self.build_premium_leg_schedule()?;

        if let Some((dt, amount)) = self.upfront {
            schedule.push_flow(finstack_quant_core::cashflow::CashFlow::new(
                dt,
                None,
                amount,
                finstack_quant_core::cashflow::CFKind::Fee,
                0.0,
                None,
            ));
        }

        // Apply holder-view sign: protection buyer (Pay) pays premium,
        // protection seller (Receive) receives premium.
        let sign = match self.side {
            PayReceive::Pay => -1.0,
            PayReceive::Receive => 1.0,
        };
        schedule.update_flows(|cf| {
            cf.amount = if sign < 0.0 {
                cf.amount.checked_neg()
            } else {
                cf.amount
            };
        });

        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::types::CurveId;
    use rust_decimal::prelude::ToPrimitive;
    use time::macros::date;

    #[test]
    fn new_isda_applies_standard_convention_fields() {
        let cds = CreditDefaultSwap::new_isda(
            InstrumentId::new("CDS-CORP-5Y"),
            Money::from((10_000_000_i64, Currency::USD)),
            PayReceive::Pay,
            CdsConvention::IsdaNa,
            Decimal::try_from(100.0).expect("valid spread_bp"),
            date!(2025 - 03 - 20),
            date!(2030 - 03 - 20),
            0.40,
            "USD-OIS",
            "CORP-HAZARD",
        )
        .expect("ISDA CDS constructor should succeed");

        assert_eq!(cds.id, InstrumentId::new("CDS-CORP-5Y"));
        assert_eq!(cds.notional, Money::from((10_000_000_i64, Currency::USD)));
        assert_eq!(cds.side, PayReceive::Pay);
        assert_eq!(cds.convention, CdsConvention::IsdaNa);
        assert_eq!(cds.premium.start, date!(2025 - 03 - 20));
        assert_eq!(cds.premium.end, date!(2030 - 03 - 20));
        assert_eq!(cds.premium.day_count, CdsConvention::IsdaNa.day_count());
        assert_eq!(cds.premium.frequency, CdsConvention::IsdaNa.frequency());
        assert_eq!(
            cds.premium.business_day_convention,
            CdsConvention::IsdaNa.business_day_convention()
        );
        assert_eq!(
            cds.premium.calendar_id.as_deref(),
            Some(CdsConvention::IsdaNa.default_calendar())
        );
        assert_eq!(cds.premium.spread_bp.to_f64(), Some(100.0));
        assert_eq!(cds.premium.discount_curve_id, CurveId::new("USD-OIS"));
        assert_eq!(cds.protection.credit_curve_id, CurveId::new("CORP-HAZARD"));
        assert_eq!(cds.protection.recovery_rate, 0.40);
        assert_eq!(
            cds.protection.settlement_delay,
            CdsConvention::IsdaNa.settlement_delay()
        );
    }

    #[test]
    fn builder_rejects_invalid_cds_recovery_rate() {
        let cds = CreditDefaultSwap::example();
        let mut protection = cds.protection;
        protection.recovery_rate = 1.1;

        let error = CreditDefaultSwap::builder()
            .id(cds.id)
            .notional(cds.notional)
            .side(cds.side)
            .convention(cds.convention)
            .premium(cds.premium)
            .protection(protection)
            .attributes(cds.attributes)
            .build()
            .expect_err("recovery above one must fail at the builder boundary");

        assert!(error.to_string().to_lowercase().contains("recovery"));
    }

    #[test]
    fn simm_margin_spec_without_credit_classification_is_rejected() {
        let mut cds = CreditDefaultSwap::example();
        cds.margin_spec =
            Some(finstack_quant_margin::OtcMarginSpec::usd_bilateral().expect("margin spec"));

        let error = cds.validate().expect_err("classification is mandatory");
        assert!(error.to_string().contains("simm_credit_classification"));
    }

    #[test]
    fn classified_simm_margin_spec_is_valid() {
        let mut cds = CreditDefaultSwap::example();
        cds.margin_spec = Some(
            finstack_quant_margin::OtcMarginSpec::usd_bilateral()
                .expect("margin spec")
                .with_simm_credit_classification(
                    finstack_quant_margin::SimmCreditClassification::Qualifying {
                        sector: finstack_quant_margin::SimmCreditSector::Financial,
                    },
                ),
        );

        cds.validate().expect("classified CDS");
    }
}
