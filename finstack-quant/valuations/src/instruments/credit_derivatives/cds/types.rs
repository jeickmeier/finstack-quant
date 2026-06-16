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
//! For European or Asian CDS, explicitly specify `CDSConvention::IsdaEu` or
//! `CDSConvention::IsdaAs` respectively. Use [`CDSConvention::detect_from_currency`]
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
//! ```ignore
//! // Detect from currency (recommended for cross-regional portfolios)
//! let convention = CDSConvention::detect_from_currency(Currency::EUR);
//! assert_eq!(convention, CDSConvention::IsdaEu);
//!
//! // Or specify explicitly
//! let european_cds = CreditDefaultSwap::builder()
//!     .convention(CDSConvention::IsdaEu)
//!     // ...
//!     .build()?;
//! ```

use crate::constants::isda::STANDARD_RECOVERY_SENIOR;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::instruments::PricingOverrides;
use crate::market::conventions::ids::CdsDocClause;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_margin::types::OtcMarginSpec;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::macros::date;

use crate::impl_instrument_base;
use crate::instruments::credit_derivatives::cds::pricer::CDSPricer;
use std::sync::OnceLock;

// Re-export PayReceive from common parameters (works for both IRS and CDS)
pub use crate::instruments::common_impl::parameters::legs::PayReceive;

/// ISDA CDS conventions
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CDSConvention {
    /// Standard North American convention (quarterly, Act/360)
    IsdaNa,
    /// Standard European convention (quarterly, Act/360)
    IsdaEu,
    /// Standard Asian convention (quarterly, Act/365)
    IsdaAs,
    /// Custom convention
    Custom,
}

/// Valuation presentation and pricing policy for CDS marks.
///
/// Each variant bundles a coherent set of choices (premium-leg accrual schedule,
/// clean/dirty NPV, par-spread denominator). Mixing those choices via separate
/// boolean overrides is intentionally not supported — the variants here are the
/// only conventions traded in practice.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
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

impl CDSConvention {
    fn registry_id(&self) -> &'static str {
        match self {
            CDSConvention::IsdaNa => "ANY:isda_na",
            CDSConvention::IsdaEu => "ANY:isda_eu",
            CDSConvention::IsdaAs => "ANY:isda_as",
            CDSConvention::Custom => "ANY:custom",
        }
    }

    /// Look up resolved conventions from the embedded registry.
    ///
    /// # Panics
    ///
    /// Panics if the embedded registry is missing the canonical entry for this
    /// convention. The four enum variants are mirrored 1:1 in
    /// `cds_conventions.json` (`ANY:isda_na`, `ANY:isda_eu`, `ANY:isda_as`,
    /// `ANY:custom`); a missing entry indicates a corrupted build artifact and
    /// must fail loudly rather than silently returning North American defaults.
    #[allow(clippy::panic)] // Build-artifact corruption is unrecoverable
    fn registry(&self) -> &'static CdsConventionResolved {
        let id = self.registry_id();
        cds_conventions_registry().get(id).unwrap_or_else(|| {
            panic!(
                "Missing CDS conventions registry entry for '{id}'. \
                 The embedded cds_conventions.json file is corrupted; \
                 this is a build/packaging error and cannot be recovered at runtime."
            )
        })
    }

    /// Get the standard day count convention.
    ///
    /// Per ISDA standards:
    /// - North America/Europe: ACT/360
    /// - Asia: ACT/365F
    #[must_use]
    pub fn day_count(&self) -> DayCount {
        self.registry().day_count
    }

    /// Get the standard payment frequency (quarterly for all conventions).
    #[must_use]
    pub fn frequency(&self) -> Tenor {
        self.registry().frequency
    }

    /// Get the standard business day convention.
    ///
    /// Per ISDA 2014 Credit Derivatives Definitions Section 4.12, CDS payment
    /// dates use **Modified Following** to prevent dates from rolling into
    /// the next month.
    #[must_use]
    pub fn business_day_convention(&self) -> BusinessDayConvention {
        self.registry().bdc
    }

    /// Get the standard stub convention.
    #[must_use]
    pub fn stub_convention(&self) -> StubKind {
        self.registry().stub_convention
    }

    /// Get the standard settlement delay in business days.
    ///
    /// Returns the number of business days between trade date and settlement
    /// for standard CDS conventions by region.
    #[must_use]
    pub fn settlement_delay(&self) -> u16 {
        self.registry().settlement_delay_days
    }

    /// Get the default holiday calendar identifier for this convention.
    ///
    /// Returns the standard calendar for business day adjustments:
    /// - North America: `nyse` (New York Stock Exchange)
    /// - Europe: `target2` (TARGET2 / ECB)
    /// - Asia: `jpto` (Tokyo Stock Exchange)
    #[must_use]
    pub fn default_calendar(&self) -> &'static str {
        self.registry().default_calendar_id.as_str()
    }

    /// Detect the appropriate CDS convention based on currency.
    ///
    /// This helper automatically selects the regional convention based on the
    /// currency of the CDS notional. Useful for cross-regional portfolios
    /// where convention should match the underlying credit market.
    ///
    /// # Currency Mapping
    ///
    /// - **USD, CAD**: North American (`IsdaNa`) - T+3, ACT/360, NYSE calendar
    /// - **EUR, GBP, CHF**: European (`IsdaEu`) - T+1, ACT/360, TARGET2 calendar (post-2009 Big Bang)
    /// - **JPY, AUD, HKD, SGD**: Asian (`IsdaAs`) - T+3, ACT/365F, Tokyo calendar
    /// - **Other**: North American (default)
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_valuations::instruments::credit_derivatives::cds::CDSConvention;
    ///
    /// let na = CDSConvention::detect_from_currency(Currency::USD);
    /// assert_eq!(na, CDSConvention::IsdaNa);
    ///
    /// let eu = CDSConvention::detect_from_currency(Currency::EUR);
    /// assert_eq!(eu, CDSConvention::IsdaEu);
    ///
    /// let asia = CDSConvention::detect_from_currency(Currency::JPY);
    /// assert_eq!(asia, CDSConvention::IsdaAs);
    /// ```
    #[must_use]
    pub fn detect_from_currency(currency: Currency) -> Self {
        match currency {
            // European currencies
            Currency::EUR | Currency::GBP | Currency::CHF => Self::IsdaEu,
            // Asian/Pacific currencies
            Currency::JPY | Currency::AUD | Currency::HKD | Currency::SGD => Self::IsdaAs,
            // Default to North American for others (most liquid CDS market)
            _ => Self::IsdaNa,
        }
    }
}

impl Default for CDSConvention {
    /// Returns the default CDS convention.
    ///
    /// # Warning
    ///
    /// The default convention is **ISDA North American (IsdaNa)**. For non-US
    /// credits, consider using [`CDSConvention::detect_from_currency`] or
    /// explicitly specifying the convention.
    ///
    /// See module-level documentation for convention selection guidance.
    fn default() -> Self {
        Self::IsdaNa
    }
}

impl std::fmt::Display for CDSConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IsdaNa => write!(f, "isda_na"),
            Self::IsdaEu => write!(f, "isda_eu"),
            Self::IsdaAs => write!(f, "isda_as"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for CDSConvention {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "isda_na" | "isdana" | "na" => Ok(Self::IsdaNa),
            "isda_eu" | "isdaeu" | "eu" => Ok(Self::IsdaEu),
            "isda_as" | "isdaas" | "as" | "asia" => Ok(Self::IsdaAs),
            "custom" => Ok(Self::Custom),
            _ => Err(format!(
                "Unknown CDS convention: '{}'. Expected one of: isda_na, isda_eu, isda_as, custom",
                s
            )),
        }
    }
}

pub(crate) fn resolve_market_conventions(
    currency: Currency,
    doc_clause: Option<&str>,
) -> finstack_quant_core::Result<&'static CdsConventionResolved> {
    let ccy = currency.to_string();

    let normalize_clause = |s: &str| {
        let t = s.trim();
        if t.eq_ignore_ascii_case("default") {
            return "DEFAULT".to_string();
        }
        let canon = t.to_ascii_lowercase().replace('-', "_");
        match canon.as_str() {
            "isdana" | "isda_na" => "isda_na".to_string(),
            "isdaeu" | "isda_eu" => "isda_eu".to_string(),
            "isdaas" | "isda_as" => "isda_as".to_string(),
            "custom" => "custom".to_string(),
            _ => t.to_string(),
        }
    };

    let key = if let Some(clause) = doc_clause {
        format!("{}:{}", ccy, normalize_clause(clause))
    } else {
        format!("{}:DEFAULT", ccy)
    };

    // If caller specified a doc clause, do not silently change it. Fall back only for the
    // "no clause provided" case, or for missing currency defaults.
    if let Some(found) = cds_conventions_registry().get(&key) {
        return Ok(found);
    }

    if doc_clause.is_some() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Unknown CDS market conventions key '{}'. Add it to finstack-quant/valuations/data/conventions/cds_conventions.json",
            key
        )));
    }

    // Currency default missing: fall back to the canonical North American default.
    cds_conventions_registry()
        .get("ANY:isda_na")
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "Missing CDS market conventions entry 'ANY:isda_na'".to_string(),
            )
        })
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CdsConventionResolved {
    pub doc_clause: CDSConvention,
    pub day_count: DayCount,
    pub frequency: Tenor,
    pub bdc: BusinessDayConvention,
    pub stub_convention: StubKind,
    pub settlement_delay_days: u16,
    pub default_calendar_id: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CdsConventionRecord {
    doc_clause: CDSConvention,
    day_count: DayCount,
    payment_frequency: String,
    bdc: BusinessDayConvention,
    stub_convention: StubKind,
    settlement_days: u16,
    calendar_id: String,
}

impl CdsConventionRecord {
    fn try_into_resolved(self) -> finstack_quant_core::Result<CdsConventionResolved> {
        let frequency = Tenor::parse(&self.payment_frequency).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "Invalid `payment_frequency` in CDS conventions registry: '{}': {}",
                self.payment_frequency, e
            ))
        })?;
        Ok(CdsConventionResolved {
            doc_clause: self.doc_clause,
            day_count: self.day_count,
            frequency,
            bdc: self.bdc,
            stub_convention: self.stub_convention,
            settlement_delay_days: self.settlement_days,
            default_calendar_id: self.calendar_id,
        })
    }
}

fn normalize_cds_key(id: &str) -> String {
    id.trim().to_string()
}

/// Returns the global CDS conventions registry, lazily initialized from embedded JSON.
///
/// # Panics
///
/// Panics if the embedded `cds_conventions.json` file is corrupted or malformed.
/// This is intentional: corrupted embedded data represents a build/packaging error
/// that cannot be recovered at runtime and should fail fast during startup.
#[allow(clippy::expect_used)]
fn cds_conventions_registry() -> &'static finstack_quant_core::HashMap<String, CdsConventionResolved>
{
    static REGISTRY: OnceLock<finstack_quant_core::HashMap<String, CdsConventionResolved>> =
        OnceLock::new();
    REGISTRY.get_or_init(|| {
        let json = include_str!("../../../../data/conventions/cds_conventions.json");
        let file: crate::market::conventions::loaders::json::RegistryFile<CdsConventionRecord> =
            serde_json::from_str(json)
                .expect("Failed to parse embedded CDS conventions registry JSON");

        // Build the registry, converting each record to resolved form
        let mut map = finstack_quant_core::HashMap::default();
        for entry in file.entries {
            // Each entry can have multiple alias IDs
            match entry.record.clone().try_into_resolved() {
                Ok(resolved) => {
                    for id in &entry.ids {
                        let key = normalize_cds_key(id);
                        map.insert(key, resolved.clone());
                    }
                }
                Err(e) => {
                    tracing::warn!(ids = ?entry.ids, error = %e, "Failed to load CDS convention");
                }
            }
        }
        map
    })
}

// Re-export from common parameters
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
/// ## Integration Methods
///
/// Multiple numerical integration methods available:
/// - **ISDA Exact:** Analytical integration at exact cashflow dates (default)
/// - **Gaussian Quadrature:** 8-point Gauss-Legendre for smooth integration
/// - **Adaptive Simpson:** Adaptive refinement for complex survival curves
///
/// ## References
///
/// - ISDA 2014 Credit Derivatives Definitions
/// - "Modelling Single-name and Multi-name Credit Derivatives" by O'Kane (2008)
/// - ISDA CDS Standard Model Implementation (Markit, 2009)
/// - Bloomberg CDSW function documentation
///
/// See unit tests and `examples/` for usage.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
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
    pub convention: CDSConvention,
    /// Premium leg specification
    pub premium: PremiumLegSpec,
    /// Protection leg specification
    pub protection: ProtectionLegSpec,
    /// Pricing overrides (including upfront payment)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Valuation presentation convention.
    #[serde(default)]
    #[builder(default)]
    pub valuation_convention: CdsValuationConvention,
    /// Upfront payment (Date, Money).
    ///
    /// The amount is defined as a payment from Protection Buyer to Protection Seller.
    /// - If positive: Buyer pays Seller.
    /// - If negative: Seller pays Buyer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<(String, Money)>")]
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
    #[schemars(with = "Option<String>")]
    pub protection_effective_date: Option<Date>,
    /// Optional OTC margin specification for VM/IM.
    ///
    /// For cleared CDS (e.g., via ICE Clear Credit), use
    /// `OtcMarginSpec::cleared("ICE", Currency::USD)`. For bilateral
    /// CDS, use `OtcMarginSpec::bilateral_simm(...)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_spec: Option<OtcMarginSpec>,
    /// Additional attributes
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

impl CreditDefaultSwap {
    /// Create a canonical example CDS for testing and documentation.
    ///
    /// Returns a 5-year investment-grade CDS with standard ISDA conventions.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        let convention = CDSConvention::IsdaNa;
        let dc = convention.day_count();
        let freq = convention.frequency();
        let bdc = convention.business_day_convention();
        let stub = convention.stub_convention();

        let spread_bp_decimal = Decimal::try_from(100.0)
            .expect("Example CDS spread 100bp should always be representable as Decimal");

        let cds = CreditDefaultSwap::builder()
            .id(InstrumentId::new("CDS-CORP-5Y"))
            .notional(Money::new(10_000_000.0, Currency::USD))
            .side(PayReceive::Pay)
            .convention(convention)
            .premium(PremiumLegSpec {
                start: date!(2024 - 03 - 20),
                end: date!(2029 - 03 - 20),
                frequency: freq,
                stub,
                bdc,
                calendar_id: Some(convention.default_calendar().to_string()),
                day_count: dc,
                spread_bp: spread_bp_decimal,
                discount_curve_id: finstack_quant_core::types::CurveId::new("USD-OIS"),
            })
            .protection(ProtectionLegSpec {
                credit_curve_id: finstack_quant_core::types::CurveId::new("CORP-HAZARD"),
                recovery_rate: STANDARD_RECOVERY_SENIOR,
                settlement_delay: convention.settlement_delay(),
            })
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("Example CDS construction should not fail");

        cds.validate()
            .expect("Example CDS validation should not fail");

        cds
    }

    /// Create a forward-starting CDS example with explicit doc clause.
    ///
    /// Returns a 5-year CDS with ISDA EU conventions, Modified-Modified
    /// Restructuring (MM14) doc clause, deferred protection start (3 months
    /// after premium start), and a 2% upfront payment.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example_forward_start() -> Self {
        let convention = CDSConvention::IsdaEu;
        let dc = convention.day_count();
        let freq = convention.frequency();
        let bdc = convention.business_day_convention();
        let stub = convention.stub_convention();

        let premium_start = date!(2024 - 06 - 20);
        let premium_end = date!(2029 - 06 - 20);
        // Protection starts 3 months after premium start
        let protection_effective = date!(2024 - 09 - 20);
        // Upfront payment date (T+1 for EU)
        let upfront_date = date!(2024 - 06 - 21);
        let notional = Money::new(10_000_000.0, Currency::EUR);

        let spread_bp_decimal = Decimal::try_from(150.0)
            .expect("Example CDS spread 150bp should always be representable as Decimal");

        let cds = CreditDefaultSwap::builder()
            .id(InstrumentId::new("CDS-FWD-EU-5Y"))
            .notional(notional)
            .side(PayReceive::Pay)
            .convention(convention)
            .premium(PremiumLegSpec {
                start: premium_start,
                end: premium_end,
                frequency: freq,
                stub,
                bdc,
                calendar_id: Some(convention.default_calendar().to_string()),
                day_count: dc,
                spread_bp: spread_bp_decimal,
                discount_curve_id: finstack_quant_core::types::CurveId::new("EUR-OIS"),
            })
            .protection(ProtectionLegSpec {
                credit_curve_id: finstack_quant_core::types::CurveId::new("EU-CORP-HAZARD"),
                recovery_rate: STANDARD_RECOVERY_SENIOR,
                settlement_delay: convention.settlement_delay(),
            })
            .pricing_overrides(PricingOverrides::default())
            .upfront_opt(Some((
                upfront_date,
                Money::new(200_000.0, Currency::EUR), // 2% of 10M notional
            )))
            .doc_clause_opt(Some(CdsDocClause::Mm14))
            .protection_effective_date_opt(Some(protection_effective))
            .attributes(Attributes::new())
            .build()
            .expect("Example forward-start CDS construction should not fail");

        cds.validate()
            .expect("Example forward-start CDS validation should not fail");

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
        convention: CDSConvention,
        spread_bp: Decimal,
        start: finstack_quant_core::dates::Date,
        end: finstack_quant_core::dates::Date,
        recovery_rate: f64,
        discount_curve_id: impl Into<finstack_quant_core::types::CurveId>,
        credit_id: impl Into<finstack_quant_core::types::CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let dc = convention.day_count();
        let freq = convention.frequency();
        let bdc = convention.business_day_convention();
        let stub = convention.stub_convention();

        let cds = Self {
            id: id.into(),
            notional,
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
                spread_bp,
                discount_curve_id: discount_curve_id.into(),
            },
            protection: ProtectionLegSpec {
                credit_curve_id: credit_id.into(),
                recovery_rate,
                settlement_delay: convention.settlement_delay(),
            },
            pricing_overrides: PricingOverrides::default(),
            valuation_convention: CdsValuationConvention::default(),
            upfront: None,
            doc_clause: None,
            protection_effective_date: None,
            margin_spec: None,
            attributes: Attributes::new(),
        };

        // Validate all parameters including recovery rate
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
    /// ```ignore
    /// let cds = CreditDefaultSwap::builder()
    ///     .id("CDS-EXAMPLE".into())
    ///     .notional(Money::new(10_000_000.0, Currency::USD))
    ///     .side(PayReceive::Pay)
    ///     .convention(CDSConvention::IsdaNa)
    ///     .premium(PremiumLegSpec {
    ///         start: date!(2024 - 03 - 20),
    ///         end: date!(2029 - 03 - 20),
    ///         freq: CDSConvention::IsdaNa.frequency(),
    ///         stub: CDSConvention::IsdaNa.stub_convention(),
    ///         bdc: CDSConvention::IsdaNa.business_day_convention(),
    ///         calendar_id: Some(CDSConvention::IsdaNa.default_calendar().to_string()),
    ///         dc: CDSConvention::IsdaNa.day_count(),
    ///         spread_bp: Decimal::try_from(100.0).expect("valid bps"),
    ///         discount_curve_id: CurveId::new("USD-OIS"),
    ///     })
    ///     .protection(ProtectionLegSpec {
    ///         credit_curve_id: CurveId::new("CORP-HAZARD"),
    ///         recovery_rate: STANDARD_RECOVERY_SENIOR,
    ///         settlement_delay: CDSConvention::IsdaNa.settlement_delay(),
    ///     })
    ///     .pricing_overrides(PricingOverrides::default())
    ///     .attributes(Attributes::new())
    ///     .build()?;
    /// cds.validate()?; // Validates all parameters
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
                CDSConvention::IsdaNa | CDSConvention::IsdaAs | CDSConvention::Custom => {
                    CdsDocClause::Xr14
                }
                CDSConvention::IsdaEu => CdsDocClause::Mm14,
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
                Ok(finstack_quant_core::cashflow::CashFlow {
                    date: end,
                    reset_date: None,
                    amount: Money::new(
                        self.notional.amount() * spread * accrual,
                        self.notional.currency(),
                    ),
                    kind: finstack_quant_core::cashflow::CFKind::Fixed,
                    accrual_factor: accrual,
                    rate: Some(spread),
                })
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
    /// - Dates are based on IMM 20th of Mar/Jun/Sep/Dec, with business day
    ///   adjustment per the instrument's calendar and BDC.
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
    impl_instrument_base!(crate::pricer::InstrumentType::CDS);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::HazardRate
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
            self,
        )
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
        Ok(finstack_quant_core::money::Money::new(
            npv_amount,
            self.notional.currency(),
        ))
    }

    fn value_raw(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        self.npv_raw_internal(market, as_of)
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

impl crate::instruments::common_impl::traits::CurveDependencies for CreditDefaultSwap {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.premium.discount_curve_id.clone())
            .credit(self.protection.credit_curve_id.clone())
            .build()
    }
}

impl crate::cashflow::traits::CashflowProvider for CreditDefaultSwap {
    fn notional(&self) -> Option<finstack_quant_core::money::Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let mut schedule = self.build_premium_leg_schedule()?;

        if let Some((dt, amount)) = self.upfront {
            schedule
                .flows
                .push(finstack_quant_core::cashflow::CashFlow {
                    date: dt,
                    reset_date: None,
                    amount,
                    kind: finstack_quant_core::cashflow::CFKind::Fee,
                    accrual_factor: 0.0,
                    rate: None,
                });
            schedule.flows.sort_by_key(|cf| cf.date);
        }

        // Apply holder-view sign: protection buyer (Pay) pays premium,
        // protection seller (Receive) receives premium.
        let sign = match self.side {
            PayReceive::Pay => -1.0,
            PayReceive::Receive => 1.0,
        };
        for cf in &mut schedule.flows {
            cf.amount = Money::new(cf.amount.amount() * sign, cf.amount.currency());
        }

        schedule.meta.representation = crate::cashflow::builder::CashflowRepresentation::Projected;
        Ok(schedule)
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
            Money::new(10_000_000.0, Currency::USD),
            PayReceive::Pay,
            CDSConvention::IsdaNa,
            Decimal::try_from(100.0).expect("valid spread_bp"),
            date!(2025 - 03 - 20),
            date!(2030 - 03 - 20),
            0.40,
            "USD-OIS",
            "CORP-HAZARD",
        )
        .expect("ISDA CDS constructor should succeed");

        assert_eq!(cds.id, InstrumentId::new("CDS-CORP-5Y"));
        assert_eq!(cds.notional, Money::new(10_000_000.0, Currency::USD));
        assert_eq!(cds.side, PayReceive::Pay);
        assert_eq!(cds.convention, CDSConvention::IsdaNa);
        assert_eq!(cds.premium.start, date!(2025 - 03 - 20));
        assert_eq!(cds.premium.end, date!(2030 - 03 - 20));
        assert_eq!(cds.premium.day_count, CDSConvention::IsdaNa.day_count());
        assert_eq!(cds.premium.frequency, CDSConvention::IsdaNa.frequency());
        assert_eq!(
            cds.premium.bdc,
            CDSConvention::IsdaNa.business_day_convention()
        );
        assert_eq!(
            cds.premium.calendar_id.as_deref(),
            Some(CDSConvention::IsdaNa.default_calendar())
        );
        assert_eq!(cds.premium.spread_bp.to_f64(), Some(100.0));
        assert_eq!(cds.premium.discount_curve_id, CurveId::new("USD-OIS"));
        assert_eq!(cds.protection.credit_curve_id, CurveId::new("CORP-HAZARD"));
        assert_eq!(cds.protection.recovery_rate, 0.40);
        assert_eq!(
            cds.protection.settlement_delay,
            CDSConvention::IsdaNa.settlement_delay()
        );
    }

    #[test]
    fn missing_currency_default_falls_back_to_isda_na_alias() {
        let conv = resolve_market_conventions(Currency::BRL, None)
            .expect("missing currency default should fall back");

        let isda_na = CDSConvention::IsdaNa.registry();

        assert_eq!(conv, isda_na);
    }
}
