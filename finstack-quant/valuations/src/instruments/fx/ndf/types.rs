//! NDF types and implementations.
//!
//! Defines the `Ndf` instrument for non-deliverable forward contracts on
//! restricted currencies. Supports both pre-fixing (forward rate estimation)
//! and post-fixing (observed rate) valuation modes.

use crate::cashflow::builder::{CashFlowSchedule, Notional};
use crate::cashflow::primitives::CFKind;
use crate::cashflow::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Result;

/// Quote convention for NDF contract rates.
///
/// NDFs can be quoted in two conventions depending on the market:
///
/// # BasePerSettlement (default)
///
/// Rate is quoted as units of base currency per one unit of settlement currency.
/// Example: USD/CNY = 7.25 means 7.25 CNY per 1 USD.
///
/// Settlement formula:
/// ```text
/// Settlement = Notional_base × (1/F_contract - 1/F_fixing)
/// ```
///
/// This is the standard convention for most Asian NDF markets (CNY, KRW, INR, etc.)
/// where the restricted currency is the base and USD is the settlement currency.
///
/// # SettlementPerBase
///
/// Rate is quoted as units of settlement currency per one unit of base currency.
/// Example: CNY/USD = 0.138 means 0.138 USD per 1 CNY.
///
/// Settlement formula:
/// ```text
/// Settlement = Notional_base × (F_fixing - F_contract)
/// ```
///
/// This is less common but may be used in some markets or for consistency with
/// other FX instruments that quote in this direction.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NdfQuoteConvention {
    /// Rate quoted as base currency per settlement currency (e.g., 7.25 CNY per USD).
    /// Settlement = Notional_base × (1/F_contract - 1/F_fixing)
    #[default]
    BasePerSettlement,
    /// Rate quoted as settlement currency per base currency (e.g., 0.138 USD per CNY).
    /// Settlement = Notional_base × (F_fixing - F_contract)
    SettlementPerBase,
}

impl std::fmt::Display for NdfQuoteConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NdfQuoteConvention::BasePerSettlement => write!(f, "base_per_settlement"),
            NdfQuoteConvention::SettlementPerBase => write!(f, "settlement_per_base"),
        }
    }
}

impl std::str::FromStr for NdfQuoteConvention {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "base_per_settlement" | "basepersettlement" | "bps" => {
                Ok(NdfQuoteConvention::BasePerSettlement)
            }
            "settlement_per_base" | "settlementperbase" | "spb" => {
                Ok(NdfQuoteConvention::SettlementPerBase)
            }
            other => Err(format!("Unknown NDF quote convention: {}", other)),
        }
    }
}

/// Official NDF fixing source/benchmark.
///
/// NDF settlements reference official fixing rates published by central banks
/// or designated fixing bodies. Using the correct fixing source is critical
/// for proper settlement calculations.
///
/// # Market Standards
///
/// | Currency | Fixing Source | Publisher | Settlement |
/// |----------|---------------|-----------|------------|
/// | CNY | PBOC | People's Bank of China | USD T+2 |
/// | CNH | CNHFIX | Treasury Markets Association (HK) | USD T+2 |
/// | INR | RBI | Reserve Bank of India | USD T+2 |
/// | KRW | KFTC | Korea Financial Telecommunications | USD T+1 |
/// | BRL | PTAX | Banco Central do Brasil | USD T+2 |
/// | TWD | TAIFX | Taipei Forex Inc. | USD T+2 |
/// | PHP | PHP BVAL | Bankers Association of the Philippines | USD T+1 |
/// | IDR | JISDOR | Bank Indonesia | USD T+2 |
/// | MYR | BNM | Bank Negara Malaysia | USD T+2 |
///
/// # Example
///
/// ```rust
/// use finstack_quant_valuations::instruments::fx::ndf::{Ndf, NdfFixingSource, NdfQuoteConvention};
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let ndf = Ndf::builder()
///     .id(InstrumentId::new("USDCNY-NDF"))
///     .base_currency(Currency::CNY)
///     .settlement_currency(Currency::USD)
///     .fixing_date(Date::from_calendar_date(2025, Month::March, 13).unwrap())
///     .maturity(Date::from_calendar_date(2025, Month::March, 15).unwrap())
///     .notional(Money::new(10_000_000.0, Currency::CNY))
///     .contract_rate(7.25)
///     .domestic_discount_curve_id(CurveId::new("USD-OIS"))
///     .quote_convention(NdfQuoteConvention::BasePerSettlement)
///     .fixing_source_enum_opt(Some(NdfFixingSource::Pboc))
///     .build()
///     .expect("Valid NDF");
/// ```
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
#[non_exhaustive]
pub enum NdfFixingSource {
    /// PBOC - People's Bank of China CNY/USD fixing.
    /// Published daily at 9:15 AM Beijing time.
    Pboc,
    /// CNHFIX - Treasury Markets Association CNH/USD fixing (offshore CNY).
    /// Published daily at 11:15 AM Hong Kong time.
    Cnhfix,
    /// RBI - Reserve Bank of India INR/USD reference rate.
    /// Published daily around 1:30 PM Mumbai time.
    Rbi,
    /// KFTC - Korea Financial Telecommunications and Clearings Institute.
    /// KRW/USD fixing published at 3:30 PM Seoul time.
    Kftc,
    /// PTAX - Banco Central do Brasil BRL/USD reference rate.
    /// Published daily, settlement uses PTAX 800 (closing rate).
    Ptax,
    /// TAIFX - Taipei Forex Inc. TWD/USD fixing.
    /// Published daily at 11:00 AM Taipei time.
    Taifx,
    /// BVAL - Bankers Association of the Philippines PHP/USD reference rate.
    /// Also known as PHP BVAL or PDEx.
    PhpBval,
    /// JISDOR - Jakarta Interbank Spot Dollar Rate (Bank Indonesia).
    /// IDR/USD fixing published daily at 10:00 AM Jakarta time.
    Jisdor,
    /// BNM - Bank Negara Malaysia MYR/USD fixing.
    /// Published daily at 3:30 PM Kuala Lumpur time.
    Bnm,
    /// Custom or other fixing source not covered by the enum.
    Other,
}

impl NdfFixingSource {
    /// Get the typical currency for this fixing source.
    ///
    /// Note: CNHFIX returns CNY since offshore CNY (CNH) is typically
    /// represented as CNY in most currency enums.
    pub fn typical_currency(&self) -> Option<Currency> {
        match self {
            // CNHFIX is for offshore CNY, typically mapped to CNY
            NdfFixingSource::Pboc | NdfFixingSource::Cnhfix => Some(Currency::CNY),
            NdfFixingSource::Rbi => Some(Currency::INR),
            NdfFixingSource::Kftc => Some(Currency::KRW),
            NdfFixingSource::Ptax => Some(Currency::BRL),
            NdfFixingSource::Taifx => Some(Currency::TWD),
            NdfFixingSource::PhpBval => Some(Currency::PHP),
            NdfFixingSource::Jisdor => Some(Currency::IDR),
            NdfFixingSource::Bnm => Some(Currency::MYR),
            NdfFixingSource::Other => None,
        }
    }

    /// Get the typical fixing offset (business days before settlement).
    /// Most NDFs fix T-2, but some (KRW, PHP) fix T-1.
    pub fn typical_fixing_offset(&self) -> i64 {
        match self {
            NdfFixingSource::Kftc | NdfFixingSource::PhpBval => 1, // KRW/PHP fix T-1
            _ => 2,                                                // Most currencies fix T-2
        }
    }
}

impl std::fmt::Display for NdfFixingSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NdfFixingSource::Pboc => write!(f, "PBOC"),
            NdfFixingSource::Cnhfix => write!(f, "CNHFIX"),
            NdfFixingSource::Rbi => write!(f, "RBI"),
            NdfFixingSource::Kftc => write!(f, "KFTC"),
            NdfFixingSource::Ptax => write!(f, "PTAX"),
            NdfFixingSource::Taifx => write!(f, "TAIFX"),
            NdfFixingSource::PhpBval => write!(f, "PHP_BVAL"),
            NdfFixingSource::Jisdor => write!(f, "JISDOR"),
            NdfFixingSource::Bnm => write!(f, "BNM"),
            NdfFixingSource::Other => write!(f, "OTHER"),
        }
    }
}

impl std::str::FromStr for NdfFixingSource {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "PBOC" => Ok(NdfFixingSource::Pboc),
            "CNHFIX" | "CNH_FIX" => Ok(NdfFixingSource::Cnhfix),
            "RBI" => Ok(NdfFixingSource::Rbi),
            "KFTC" => Ok(NdfFixingSource::Kftc),
            "PTAX" => Ok(NdfFixingSource::Ptax),
            "TAIFX" => Ok(NdfFixingSource::Taifx),
            "PHP_BVAL" | "PHPBVAL" | "BVAL" | "PDEX" => Ok(NdfFixingSource::PhpBval),
            "JISDOR" => Ok(NdfFixingSource::Jisdor),
            "BNM" => Ok(NdfFixingSource::Bnm),
            "OTHER" => Ok(NdfFixingSource::Other),
            other => Err(format!("Unknown NDF fixing source: {}", other)),
        }
    }
}

/// Non-Deliverable Forward (NDF) instrument.
///
/// Represents a cash-settled forward contract on a restricted currency pair.
/// The position is long base currency (restricted) and short settlement currency.
///
/// # Quote Convention
///
/// NDFs support two quote conventions via the `quote_convention` field:
///
/// - **BasePerSettlement** (default): Rate quoted as base per settlement (e.g., 7.25 CNY/USD)
/// - **SettlementPerBase**: Rate quoted as settlement per base (e.g., 0.138 USD/CNY)
///
/// See [`NdfQuoteConvention`] for details on the settlement formulas.
///
/// # Pricing
///
/// ## Pre-Fixing (fixing_rate = None)
/// Forward rate is estimated via covered interest rate parity or fallback.
///
/// ## Post-Fixing (fixing_rate = Some)
/// Uses the observed fixing rate for settlement calculation.
///
/// The settlement formula depends on `quote_convention`:
///
/// **BasePerSettlement:**
/// ```text
/// Settlement = Notional_base × (1/F_contract - 1/F_fixing)
/// PV = Settlement × DF_settlement(T)
/// ```
///
/// **SettlementPerBase:**
/// ```text
/// Settlement = Notional_base × (F_fixing - F_contract)
/// PV = Settlement × DF_settlement(T)
/// ```
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fx::ndf::{Ndf, NdfQuoteConvention};
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let ndf = Ndf::builder()
///     .id(InstrumentId::new("USDCNY-NDF-3M"))
///     .base_currency(Currency::CNY)
///     .settlement_currency(Currency::USD)
///     .fixing_date(Date::from_calendar_date(2025, Month::March, 13).unwrap())
///     .maturity(Date::from_calendar_date(2025, Month::March, 15).unwrap())
///     .notional(Money::new(10_000_000.0, Currency::CNY))
///     .contract_rate(7.25)
///     .domestic_discount_curve_id(CurveId::new("USD-OIS"))
///     .quote_convention(NdfQuoteConvention::BasePerSettlement)
///     .build()
///     .expect("Valid NDF");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = Ndf::validate)]
#[serde(deny_unknown_fields, try_from = "NdfUnchecked")]
pub struct Ndf {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Base currency (restricted/non-deliverable currency, numerator).
    pub base_currency: Currency,
    /// Settlement currency (freely convertible, typically USD, denominator and PV currency).
    pub settlement_currency: Currency,
    /// Fixing date (rate observation date, typically T-2 before maturity).
    #[schemars(with = "String")]
    pub fixing_date: Date,
    /// Maturity/settlement date.
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Notional amount in base currency.
    pub notional: Money,
    /// Contract forward rate. Interpretation depends on `quote_convention`:
    /// - BasePerSettlement: base per settlement (e.g., 7.25 CNY per USD)
    /// - SettlementPerBase: settlement per base (e.g., 0.138 USD per CNY)
    pub contract_rate: f64,
    /// Settlement currency discount curve ID.
    pub domestic_discount_curve_id: CurveId,
    /// Quote convention for contract_rate and fixing_rate.
    pub quote_convention: NdfQuoteConvention,
    /// Optional foreign (base) currency discount curve ID.
    /// Required for pre-fixing forward estimation unless `forward_rate_override`
    /// is supplied.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreign_discount_curve_id: Option<CurveId>,
    /// Observed fixing rate. Interpretation depends on `quote_convention`.
    /// If Some, NDF is post-fixing.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixing_rate: Option<f64>,
    /// Official fixing source/benchmark enum for type-safe specification.
    ///
    /// Use this field for validated fixing sources.
    /// See [`NdfFixingSource`] for supported benchmarks and their typical currencies.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixing_source_enum: Option<NdfFixingSource>,
    /// Optional spot rate override for forward rate calculation.
    /// Interpretation depends on `quote_convention`.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot_rate_override: Option<f64>,
    /// Explicit pre-fixing forward rate in `quote_convention` units.
    ///
    /// Use this for NDF market quotes or basis-adjusted forwards when a base
    /// currency discount curve is unavailable.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forward_rate_override: Option<f64>,
    /// Optional base currency calendar.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_calendar_id: Option<String>,
    /// Optional settlement currency calendar.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_calendar_id: Option<String>,
    /// Attributes for tagging and selection.
    #[builder(default)]
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct NdfUnchecked {
    /// Unique instrument identifier.
    id: InstrumentId,
    /// Base currency (restricted/non-deliverable currency, numerator).
    base_currency: Currency,
    /// Settlement currency (freely convertible, typically USD, denominator and PV currency).
    settlement_currency: Currency,
    /// Fixing date (rate observation date, typically T-2 before maturity).
    #[schemars(with = "String")]
    fixing_date: Date,
    /// Maturity/settlement date.
    #[schemars(with = "String")]
    maturity: Date,
    /// Notional amount in base currency.
    notional: Money,
    /// Contract forward rate. Interpretation depends on `quote_convention`.
    contract_rate: f64,
    /// Settlement currency discount curve ID.
    domestic_discount_curve_id: CurveId,
    /// Quote convention for contract_rate and fixing_rate.
    quote_convention: NdfQuoteConvention,
    /// Optional foreign (base) currency discount curve ID.
    #[serde(default)]
    foreign_discount_curve_id: Option<CurveId>,
    /// Observed fixing rate. Interpretation depends on `quote_convention`.
    #[serde(default)]
    fixing_rate: Option<f64>,
    /// Official fixing source/benchmark enum for type-safe specification.
    #[serde(default)]
    fixing_source_enum: Option<NdfFixingSource>,
    /// Optional spot rate override for forward rate calculation.
    #[serde(default)]
    spot_rate_override: Option<f64>,
    /// Explicit pre-fixing forward rate in `quote_convention` units.
    #[serde(default)]
    forward_rate_override: Option<f64>,
    /// Optional base currency calendar.
    #[serde(default)]
    base_calendar_id: Option<String>,
    /// Optional settlement currency calendar.
    #[serde(default)]
    quote_calendar_id: Option<String>,
    /// Per-instrument pricing/sensitivity override knobs.
    #[serde(default)]
    pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging.
    attributes: Attributes,
}

impl TryFrom<NdfUnchecked> for Ndf {
    type Error = finstack_quant_core::Error;

    fn try_from(value: NdfUnchecked) -> std::result::Result<Self, Self::Error> {
        let ndf = Self {
            id: value.id,
            base_currency: value.base_currency,
            settlement_currency: value.settlement_currency,
            fixing_date: value.fixing_date,
            maturity: value.maturity,
            notional: value.notional,
            contract_rate: value.contract_rate,
            domestic_discount_curve_id: value.domestic_discount_curve_id,
            quote_convention: value.quote_convention,
            foreign_discount_curve_id: value.foreign_discount_curve_id,
            fixing_rate: value.fixing_rate,
            fixing_source_enum: value.fixing_source_enum,
            spot_rate_override: value.spot_rate_override,
            forward_rate_override: value.forward_rate_override,
            base_calendar_id: value.base_calendar_id,
            quote_calendar_id: value.quote_calendar_id,
            pricing_overrides: value.pricing_overrides,
            attributes: value.attributes,
        };
        ndf.validate()?;
        Ok(ndf)
    }
}

impl Ndf {
    const MIN_POSITIVE_RATE: f64 = 1.0e-12;

    /// Create a canonical example NDF for testing and documentation.
    ///
    /// Returns a 3-month USD/CNY NDF with realistic parameters.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        Self::builder()
            .id(InstrumentId::new("USDCNY-NDF-3M"))
            .base_currency(Currency::CNY)
            .settlement_currency(Currency::USD)
            .fixing_date(
                Date::from_calendar_date(2025, time::Month::March, 13).expect("Valid example date"),
            )
            .maturity(
                Date::from_calendar_date(2025, time::Month::March, 15).expect("Valid example date"),
            )
            .notional(Money::new(10_000_000.0, Currency::CNY))
            .contract_rate(7.25)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .quote_convention(NdfQuoteConvention::BasePerSettlement)
            .forward_rate_override_opt(Some(7.25))
            .fixing_source_enum_opt(Some(NdfFixingSource::Pboc))
            .attributes(
                Attributes::new()
                    .with_tag("ndf")
                    .with_meta("pair", "USDCNY"),
            )
            .build()
            .expect("Example NDF construction should not fail")
    }

    /// Validate that the fixing source is appropriate for the base currency.
    ///
    /// Returns an error if the fixing source enum is set and doesn't match
    /// the expected currency for that benchmark.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fx::ndf::{Ndf, NdfFixingSource, NdfQuoteConvention};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::{CurveId, InstrumentId};
    /// use time::Month;
    ///
    /// // This is valid: CNY with PBOC fixing
    /// let ndf_cny = Ndf::builder()
    ///     .id(InstrumentId::new("USDCNY"))
    ///     .base_currency(Currency::CNY)
    ///     .settlement_currency(Currency::USD)
    ///     .fixing_date(Date::from_calendar_date(2025, Month::March, 13).unwrap())
    ///     .maturity(Date::from_calendar_date(2025, Month::March, 15).unwrap())
    ///     .notional(Money::new(10_000_000.0, Currency::CNY))
    ///     .contract_rate(7.25)
    ///     .domestic_discount_curve_id(CurveId::new("USD-OIS"))
    ///     .quote_convention(NdfQuoteConvention::BasePerSettlement)
    ///     .fixing_source_enum_opt(Some(NdfFixingSource::Pboc))
    ///     .build()
    ///     .unwrap();
    /// assert!(ndf_cny.validate_fixing_source().is_ok());
    /// ```
    pub fn validate_fixing_source(&self) -> Result<()> {
        if let Some(fixing_source) = &self.fixing_source_enum {
            if let Some(expected_ccy) = fixing_source.typical_currency() {
                if expected_ccy != self.base_currency {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Fixing source {} is typically used for {} but NDF base currency is {}. \
                         Consider using the appropriate fixing source for this currency.",
                        fixing_source, expected_ccy, self.base_currency
                    )));
                }
            }
        }
        Ok(())
    }

    /// Validate NDF economics at construction boundaries.
    pub fn validate(&self) -> Result<()> {
        if self.base_currency == self.settlement_currency {
            return Err(finstack_quant_core::Error::Validation(format!(
                "NDF base_currency ({}) must differ from settlement_currency ({})",
                self.base_currency, self.settlement_currency
            )));
        }
        if self.notional.currency() != self.base_currency {
            return Err(finstack_quant_core::Error::Validation(format!(
                "NDF notional currency ({}) must match base_currency ({})",
                self.notional.currency(),
                self.base_currency
            )));
        }
        crate::instruments::common_impl::validation::validate_money_finite(
            self.notional,
            "NDF notional",
        )?;
        crate::instruments::common_impl::validation::validate_money_gt(
            self.notional,
            0.0,
            "NDF notional",
        )?;
        if self.fixing_date > self.maturity {
            return Err(finstack_quant_core::Error::Validation(format!(
                "NDF fixing_date ({}) must be <= maturity ({})",
                self.fixing_date, self.maturity
            )));
        }

        Self::validate_rate("contract_rate", self.contract_rate)?;
        if let Some(rate) = self.fixing_rate {
            Self::validate_rate("fixing_rate", rate)?;
        }
        if let Some(rate) = self.spot_rate_override {
            Self::validate_rate("spot_rate_override", rate)?;
        }
        if let Some(rate) = self.forward_rate_override {
            Self::validate_rate("forward_rate_override", rate)?;
        }
        self.validate_fixing_source()?;
        Ok(())
    }

    /// Get the effective fixing source as a string.
    ///
    /// Returns the enum display name if `fixing_source_enum` is set.
    pub fn effective_fixing_source(&self) -> Option<String> {
        self.fixing_source_enum
            .as_ref()
            .map(|fixing_enum| fixing_enum.to_string())
    }

    /// Construct an NDF from trade date and tenor using standard fixing offset.
    ///
    /// # Arguments
    ///
    /// * `id` - Instrument identifier
    /// * `base_currency` - Restricted currency (numerator)
    /// * `settlement_currency` - Convertible currency (denominator)
    /// * `trade_date` - Trade date
    /// * `tenor_days` - Days from spot to maturity
    /// * `notional` - Notional in base currency
    /// * `contract_rate` - Contract forward rate
    /// * `domestic_discount_curve_id` - Settlement/quote currency discount curve
    /// * `base_calendar_id` - Optional base currency calendar
    /// * `quote_calendar_id` - Optional quote/settlement currency calendar
    /// * `spot_lag_days` - Spot lag (typically 2)
    /// * `fixing_offset_days` - Days before maturity for fixing (typically 2)
    /// * `bdc` - Business day convention
    #[allow(clippy::too_many_arguments)]
    pub fn from_trade_date(
        id: impl Into<InstrumentId>,
        base_currency: Currency,
        settlement_currency: Currency,
        trade_date: Date,
        tenor_days: i64,
        notional: Money,
        contract_rate: f64,
        domestic_discount_curve_id: impl Into<CurveId>,
        base_calendar_id: Option<String>,
        quote_calendar_id: Option<String>,
        spot_lag_days: u32,
        fixing_offset_days: i64,
        bdc: finstack_quant_core::dates::BusinessDayConvention,
    ) -> finstack_quant_core::Result<Self> {
        use crate::instruments::common_impl::fx_dates::{
            adjust_joint_calendar, fx_spot_date_for_pair, ResolvedCalendarPair,
        };

        // CLS-consistent spot roll: a US holiday on an intermediate day does not
        // delay a USD pair's spot date (FX spot convention
        // finding). For NDFs the settlement currency (typically USD) is the
        // quote side of the pair.
        let spot_date = fx_spot_date_for_pair(
            trade_date,
            spot_lag_days,
            base_currency,
            settlement_currency,
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;
        let maturity_unadjusted = spot_date + time::Duration::days(tenor_days);
        let maturity = adjust_joint_calendar(
            maturity_unadjusted,
            bdc,
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;

        // Fixing date is typically T-2 before maturity using joint-business-day stepping.
        let joint_cal = ResolvedCalendarPair::resolve(
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;
        let mut fixing_unadjusted = maturity;
        if fixing_offset_days >= 0 {
            let mut remaining = fixing_offset_days as u32;
            while remaining > 0 {
                fixing_unadjusted -= time::Duration::days(1);
                if joint_cal.is_joint_business_day(fixing_unadjusted) {
                    remaining -= 1;
                }
            }
        } else {
            let n_days = u32::try_from(fixing_offset_days.unsigned_abs()).map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "NDF fixing_offset_days magnitude too large: {}",
                    fixing_offset_days
                ))
            })?;
            fixing_unadjusted = joint_cal.add_joint_business_days(fixing_unadjusted, n_days)?;
        }
        let fixing_date = adjust_joint_calendar(
            fixing_unadjusted,
            finstack_quant_core::dates::BusinessDayConvention::Preceding,
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;

        Self::builder()
            .id(id.into())
            .base_currency(base_currency)
            .settlement_currency(settlement_currency)
            .fixing_date(fixing_date)
            .maturity(maturity)
            .notional(notional)
            .contract_rate(contract_rate)
            .domestic_discount_curve_id(domestic_discount_curve_id.into())
            .quote_convention(NdfQuoteConvention::BasePerSettlement)
            .base_calendar_id_opt(base_calendar_id)
            .quote_calendar_id_opt(quote_calendar_id)
            .attributes(Attributes::new())
            .build()
    }

    /// Set the observed fixing rate (transitions NDF to post-fixing mode).
    ///
    /// # Errors
    ///
    /// Returns a `Validation` error if the fixing rate is non-finite or
    /// non-positive. Direct field assignment to `fixing_rate` bypasses this
    /// guard; prefer this constructor.
    pub fn with_fixing_rate(mut self, fixing_rate: f64) -> Result<Self> {
        Self::validate_rate("fixing_rate", fixing_rate)?;
        self.fixing_rate = Some(fixing_rate);
        Ok(self)
    }

    /// Check if NDF is in post-fixing mode.
    pub fn is_fixed(&self) -> bool {
        self.fixing_rate.is_some()
    }

    /// Estimate the forward rate when in pre-fixing mode.
    ///
    /// The forward rate is estimated in the same convention as `quote_convention`:
    /// - **BasePerSettlement**: Returns base per settlement (e.g., CNY/USD)
    /// - **SettlementPerBase**: Returns settlement per base (e.g., USD/CNY)
    fn estimate_forward_rate(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        use finstack_quant_core::money::fx::FxQuery;

        if let Some(rate) = self.forward_rate_override {
            Self::validate_rate("forward_rate_override", rate)?;
            return Ok(rate);
        }

        // Determine which direction to query based on convention.
        //
        // The `FxProvider::rate` contract is: the returned rate `r` satisfies
        // `amount_in_from * r = amount_in_to`, i.e. `rate(from, to)` is quoted
        // as units of `to` per unit of `from`. To obtain "base per settlement"
        // we must therefore query `(from = settlement, to = base)`.
        let (from_ccy, to_ccy) = match self.quote_convention {
            NdfQuoteConvention::BasePerSettlement => {
                // Want base per settlement -> query settlement->base.
                (self.settlement_currency, self.base_currency)
            }
            NdfQuoteConvention::SettlementPerBase => {
                // Want settlement per base -> query base->settlement.
                (self.base_currency, self.settlement_currency)
            }
        };

        // Try to get spot rate in the appropriate convention
        let spot = if let Some(rate) = self.spot_rate_override {
            rate
        } else if let Some(fx) = market.fx() {
            match (**fx).rate(FxQuery::new(from_ccy, to_ccy, as_of)) {
                Ok(fx_rate) => fx_rate.rate,
                Err(_) => {
                    // Try inverse and flip
                    let inverse = (**fx).rate(FxQuery::new(to_ccy, from_ccy, as_of))?;
                    1.0 / inverse.rate
                }
            }
        } else {
            return Err(finstack_quant_core::Error::Validation(format!(
                "NDF {} requires FxMatrix or spot_rate_override to estimate a forward rate",
                self.id
            )));
        };

        // Get settlement discount factor
        let settlement_disc = market.get_discount(self.domestic_discount_curve_id.as_str())?;
        let df_settlement = settlement_disc.df_between_dates(as_of, self.maturity)?;

        if let Some(ref foreign_curve_id) = self.foreign_discount_curve_id {
            let foreign_disc = market.get_discount(foreign_curve_id.as_str())?;
            let df_foreign = foreign_disc.df_between_dates(as_of, self.maturity)?;
            // Forward rate via covered interest rate parity (Hull Ch.5)
            // For BasePerSettlement (e.g. CNY per USD): F = S × DF_settlement / DF_base
            //   The "base" currency is the numerator (CNY), "settlement" is denominator (USD).
            //   CIRP: F/S = DF_denominator / DF_numerator = DF_settlement / DF_base
            // For SettlementPerBase (e.g. USD per CNY): F = S × DF_base / DF_settlement
            //   The "settlement" currency is the numerator (USD), "base" is denominator (CNY).
            //   CIRP: F/S = DF_denominator / DF_numerator = DF_base / DF_settlement
            let forward = match self.quote_convention {
                NdfQuoteConvention::BasePerSettlement => spot * df_settlement / df_foreign,
                NdfQuoteConvention::SettlementPerBase => spot * df_foreign / df_settlement,
            };
            return Ok(forward);
        }

        Err(finstack_quant_core::Error::Validation(format!(
            "NDF {} requires foreign_discount_curve_id or forward_rate_override for pre-fixing forward estimation",
            self.id
        )))
    }

    /// Set the quote convention.
    pub fn with_quote_convention(mut self, convention: NdfQuoteConvention) -> Self {
        self.quote_convention = convention;
        self
    }

    #[inline]
    fn validate_rate(name: &str, rate: f64) -> Result<()> {
        if !rate.is_finite() || rate <= Self::MIN_POSITIVE_RATE {
            return Err(finstack_quant_core::Error::Validation(format!(
                "NDF invalid {name}: {rate}. Must be finite and > {}",
                Self::MIN_POSITIVE_RATE
            )));
        }
        Ok(())
    }

    fn settlement_amount_for_schedule(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        let fixing_rate = if let Some(fixed_rate) = self.fixing_rate {
            fixed_rate
        } else if as_of >= self.fixing_date {
            return Err(finstack_quant_core::Error::Validation(format!(
                "NDF {} is past fixing date ({}) but no fixing_rate is set. \
                 Use with_fixing_rate() to set the observed rate.",
                self.id, self.fixing_date
            )));
        } else {
            self.estimate_forward_rate(market, as_of)?
        };
        Self::validate_rate("contract_rate", self.contract_rate)?;
        Self::validate_rate("fixing_rate", fixing_rate)?;

        let n_base = self.notional.amount();
        let settlement_amount = match self.quote_convention {
            NdfQuoteConvention::BasePerSettlement => {
                n_base * (1.0 / self.contract_rate - 1.0 / fixing_rate)
            }
            NdfQuoteConvention::SettlementPerBase => n_base * (fixing_rate - self.contract_rate),
        };
        Ok(settlement_amount)
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for Ndf {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        let mut builder = crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.domestic_discount_curve_id.clone());

        if let Some(ref foreign_curve) = self.foreign_discount_curve_id {
            builder = builder.discount(foreign_curve.clone());
        }

        builder.build()
    }
}

impl crate::instruments::common_impl::traits::Instrument for Ndf {
    impl_instrument_base!(crate::pricer::InstrumentType::Ndf);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        deps.add_fx_pair(self.base_currency, self.settlement_currency);
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        self.validate()?;
        // Reject economically impossible NDFs where the fixing date falls after
        // maturity. `Ndf::validate` enforces this at construction, but a direct
        // field assignment can bypass it; guarding here ensures the value path
        // never silently prices a malformed contract.
        if self.fixing_date > self.maturity {
            return Err(finstack_quant_core::Error::Validation(format!(
                "NDF {} fixing_date ({}) must be on or before maturity ({})",
                self.id, self.fixing_date, self.maturity
            )));
        }

        Self::validate_rate("contract_rate", self.contract_rate)?;
        if let Some(rate) = self.fixing_rate {
            Self::validate_rate("fixing_rate", rate)?;
        }
        if let Some(rate) = self.spot_rate_override {
            Self::validate_rate("spot_rate_override", rate)?;
        }

        // If maturity has passed, value is zero
        if self.maturity < as_of {
            return Ok(Money::new(0.0, self.settlement_currency));
        }

        // Get settlement discount curve
        let settlement_disc = market.get_discount(self.domestic_discount_curve_id.as_str())?;
        let df_settlement = settlement_disc.df_between_dates(as_of, self.maturity)?;

        // Determine the forward rate to use
        let effective_forward = if let Some(fixed_rate) = self.fixing_rate {
            // Post-fixing: use observed rate
            fixed_rate
        } else if as_of >= self.fixing_date {
            // Past fixing date but no rate set - this is an error condition
            return Err(finstack_quant_core::Error::Validation(format!(
                "NDF {} is past fixing date ({}) but no fixing_rate is set. \
                 Use with_fixing_rate() to set the observed rate.",
                self.id, self.fixing_date
            )));
        } else {
            // Pre-fixing: estimate forward rate
            self.estimate_forward_rate(market, as_of)?
        };
        Self::validate_rate("effective_forward", effective_forward)?;

        // Validate notional currency
        if self.notional.currency() != self.base_currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.base_currency,
                actual: self.notional.currency(),
            });
        }
        let n_base = self.notional.amount();

        // Compute settlement amount based on quote convention
        let settlement_amount = match self.quote_convention {
            NdfQuoteConvention::BasePerSettlement => {
                // Rate is base per settlement (e.g., 7.25 CNY per USD)
                // Settlement = N_base × (1/F_contract - 1/F_fixing)
                // Positive when F_fixing > F_contract (base currency depreciated)
                n_base * (1.0 / self.contract_rate - 1.0 / effective_forward)
            }
            NdfQuoteConvention::SettlementPerBase => {
                // Rate is settlement per base (e.g., 0.138 USD per CNY)
                // Settlement = N_base × (F_fixing - F_contract)
                // Positive when F_fixing > F_contract (base currency appreciated)
                n_base * (effective_forward - self.contract_rate)
            }
        };

        let pv = settlement_amount * df_settlement;
        Ok(Money::new(pv, self.settlement_currency))
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        None
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

impl CashflowProvider for Ndf {
    fn cashflow_schedule(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let settlement_amount = self.settlement_amount_for_schedule(market, as_of)?;
        let anchor = if as_of < self.maturity {
            as_of
        } else {
            self.maturity - time::Duration::days(1)
        };
        let ccy = self.settlement_currency;
        let mut builder = CashFlowSchedule::builder();
        let _ = builder.principal(Money::new(0.0, ccy), anchor, self.maturity);
        let _ = builder.add_principal_event(
            self.maturity,
            Money::new(0.0, ccy),
            Some(Money::new(-settlement_amount, ccy)),
            CFKind::Notional,
        );
        let mut schedule = builder.build_with_curves(None)?;
        schedule.notional = Notional::par(0.0, ccy);
        Ok(schedule.normalize_public(
            as_of,
            crate::cashflow::builder::CashflowRepresentation::Projected,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use std::sync::Arc;
    use time::Month;

    #[test]
    fn test_ndf_with_fixing_rate_rejects_non_positive() {
        let err = Ndf::example()
            .with_fixing_rate(0.0)
            .expect_err("zero rate should fail");
        assert!(err.to_string().contains("fixing_rate"));

        let err = Ndf::example()
            .with_fixing_rate(f64::NAN)
            .expect_err("NaN rate should fail");
        assert!(err.to_string().contains("fixing_rate"));
    }

    #[test]
    fn test_ndf_serde_rejects_invalid_contract_rate() {
        let ndf = Ndf::example();
        let mut json = serde_json::to_value(&ndf).expect("serialize");
        json["contract_rate"] = serde_json::json!(0.0);

        let err = serde_json::from_value::<Ndf>(json)
            .expect_err("invalid contract_rate should fail during deserialization");
        assert!(
            err.to_string().contains("contract_rate"),
            "error should mention contract_rate: {}",
            err
        );
    }

    #[test]
    fn test_ndf_quote_convention_with_builder() {
        let ndf = Ndf::builder()
            .id(InstrumentId::new("TEST-NDF"))
            .base_currency(Currency::CNY)
            .settlement_currency(Currency::USD)
            .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
            .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
            .notional(Money::new(10_000_000.0, Currency::CNY))
            .contract_rate(0.138) // USD per CNY
            .quote_convention(NdfQuoteConvention::SettlementPerBase)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        assert_eq!(ndf.quote_convention, NdfQuoteConvention::SettlementPerBase);
    }

    #[test]
    fn test_ndf_with_quote_convention() {
        let ndf = Ndf::example().with_quote_convention(NdfQuoteConvention::SettlementPerBase);
        assert_eq!(ndf.quote_convention, NdfQuoteConvention::SettlementPerBase);
    }

    #[test]
    fn test_ndf_quote_convention_display_and_parse() {
        let bps = NdfQuoteConvention::BasePerSettlement;
        let spb = NdfQuoteConvention::SettlementPerBase;

        assert_eq!(bps.to_string(), "base_per_settlement");
        assert_eq!(spb.to_string(), "settlement_per_base");

        assert_eq!(
            "base_per_settlement"
                .parse::<NdfQuoteConvention>()
                .expect("valid convention"),
            NdfQuoteConvention::BasePerSettlement
        );
        assert_eq!(
            "settlement_per_base"
                .parse::<NdfQuoteConvention>()
                .expect("valid convention"),
            NdfQuoteConvention::SettlementPerBase
        );
        assert_eq!(
            "bps"
                .parse::<NdfQuoteConvention>()
                .expect("valid convention"),
            NdfQuoteConvention::BasePerSettlement
        );
        assert_eq!(
            "spb"
                .parse::<NdfQuoteConvention>()
                .expect("valid convention"),
            NdfQuoteConvention::SettlementPerBase
        );
    }

    #[test]
    fn test_ndf_base_per_settlement_settlement_formula() {
        // Test the settlement formula for BasePerSettlement convention
        // Contract rate: 7.25 CNY/USD
        // Fixing rate: 7.30 CNY/USD (CNY depreciated)
        // Notional: 10,000,000 CNY
        //
        // Expected settlement (in USD):
        // = 10,000,000 * (1/7.25 - 1/7.30)
        // = 10,000,000 * (0.13793 - 0.13699)
        // = 10,000,000 * 0.00094
        // ≈ 9,430 USD (positive, we receive)

        let contract_rate = 7.25;
        let fixing_rate = 7.30;
        let notional = 10_000_000.0;

        let settlement: f64 = notional * (1.0 / contract_rate - 1.0 / fixing_rate);
        assert!(settlement > 0.0, "Settlement should be positive");
        assert!(
            (settlement - 9430.0).abs() < 100.0,
            "Settlement should be approximately 9,430 USD"
        );
    }

    #[test]
    fn test_ndf_settlement_per_base_settlement_formula() {
        // Test the settlement formula for SettlementPerBase convention
        // Contract rate: 0.138 USD/CNY
        // Fixing rate: 0.140 USD/CNY (CNY appreciated)
        // Notional: 10,000,000 CNY
        //
        // Expected settlement (in USD):
        // = 10,000,000 * (0.140 - 0.138)
        // = 10,000,000 * 0.002
        // = 20,000 USD (positive, we receive)

        let contract_rate = 0.138;
        let fixing_rate = 0.140;
        let notional = 10_000_000.0;

        let settlement: f64 = notional * (fixing_rate - contract_rate);
        assert!(settlement > 0.0, "Settlement should be positive");
        assert!(
            (settlement - 20_000.0).abs() < 1.0,
            "Settlement should be exactly 20,000 USD"
        );
    }

    #[test]
    fn test_ndf_past_fixing_without_rate_errors() {
        use crate::instruments::common_impl::traits::Instrument;
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;

        // Create a simple market context
        let as_of = Date::from_calendar_date(2025, Month::March, 14).expect("valid date");
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("should build");
        let market = MarketContext::new().insert(curve);

        // Create an NDF that's past fixing date but without fixing_rate
        let ndf = Ndf::builder()
            .id(InstrumentId::new("TEST-NDF"))
            .base_currency(Currency::CNY)
            .settlement_currency(Currency::USD)
            .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
            .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
            .notional(Money::new(10_000_000.0, Currency::CNY))
            .contract_rate(7.25)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .quote_convention(NdfQuoteConvention::BasePerSettlement)
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        // value() should error because we're past fixing date without a fixing rate
        let result = ndf.value(&market, as_of);
        assert!(
            result.is_err(),
            "Should error when past fixing without rate"
        );
        let err_msg = result.expect_err("expected an error").to_string();
        assert!(
            err_msg.contains("past fixing date"),
            "Error should mention past fixing date: {}",
            err_msg
        );
    }

    #[test]
    fn test_ndf_fixing_source_enum_display_and_parse() {
        // Test display
        assert_eq!(NdfFixingSource::Pboc.to_string(), "PBOC");
        assert_eq!(NdfFixingSource::Cnhfix.to_string(), "CNHFIX");
        assert_eq!(NdfFixingSource::Rbi.to_string(), "RBI");
        assert_eq!(NdfFixingSource::Kftc.to_string(), "KFTC");
        assert_eq!(NdfFixingSource::Ptax.to_string(), "PTAX");

        // Test parse
        assert_eq!(
            "PBOC".parse::<NdfFixingSource>().expect("valid source"),
            NdfFixingSource::Pboc
        );
        assert_eq!(
            "CNHFIX".parse::<NdfFixingSource>().expect("valid source"),
            NdfFixingSource::Cnhfix
        );
        assert_eq!(
            "cnh_fix".parse::<NdfFixingSource>().expect("valid source"),
            NdfFixingSource::Cnhfix
        );
        assert_eq!(
            "RBI".parse::<NdfFixingSource>().expect("valid source"),
            NdfFixingSource::Rbi
        );
    }

    #[test]
    fn test_ndf_fixing_source_typical_currency() {
        assert_eq!(
            NdfFixingSource::Pboc.typical_currency(),
            Some(Currency::CNY)
        );
        // CNHFIX maps to CNY (offshore CNY uses same currency code in most systems)
        assert_eq!(
            NdfFixingSource::Cnhfix.typical_currency(),
            Some(Currency::CNY)
        );
        assert_eq!(NdfFixingSource::Rbi.typical_currency(), Some(Currency::INR));
        assert_eq!(
            NdfFixingSource::Kftc.typical_currency(),
            Some(Currency::KRW)
        );
        assert_eq!(
            NdfFixingSource::Ptax.typical_currency(),
            Some(Currency::BRL)
        );
        assert_eq!(NdfFixingSource::Other.typical_currency(), None);
    }

    #[test]
    fn test_ndf_fixing_source_typical_fixing_offset() {
        // Most currencies fix T-2
        assert_eq!(NdfFixingSource::Pboc.typical_fixing_offset(), 2);
        assert_eq!(NdfFixingSource::Rbi.typical_fixing_offset(), 2);
        assert_eq!(NdfFixingSource::Ptax.typical_fixing_offset(), 2);

        // KRW and PHP fix T-1
        assert_eq!(NdfFixingSource::Kftc.typical_fixing_offset(), 1);
        assert_eq!(NdfFixingSource::PhpBval.typical_fixing_offset(), 1);
    }

    #[test]
    fn test_ndf_validate_fixing_source_valid() {
        let ndf = Ndf::builder()
            .id(InstrumentId::new("USDCNY"))
            .base_currency(Currency::CNY)
            .settlement_currency(Currency::USD)
            .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
            .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
            .notional(Money::new(10_000_000.0, Currency::CNY))
            .contract_rate(7.25)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .quote_convention(NdfQuoteConvention::BasePerSettlement)
            .fixing_source_enum_opt(Some(NdfFixingSource::Pboc))
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        // CNY with PBOC is valid
        assert!(ndf.validate_fixing_source().is_ok());
    }

    #[test]
    fn test_ndf_validate_fixing_source_mismatch_warns() {
        let err = Ndf::builder()
            .id(InstrumentId::new("USDINR"))
            .base_currency(Currency::INR)
            .settlement_currency(Currency::USD)
            .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
            .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
            .notional(Money::new(10_000_000.0, Currency::INR))
            .contract_rate(83.50)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .quote_convention(NdfQuoteConvention::BasePerSettlement)
            .fixing_source_enum_opt(Some(NdfFixingSource::Pboc)) // Wrong! PBOC is for CNY
            .attributes(Attributes::new())
            .build()
            .expect_err("builder should reject fixing source mismatch");

        let err_msg = err.to_string();
        assert!(
            err_msg.contains("CNY") && err_msg.contains("INR"),
            "Error should mention currency mismatch: {}",
            err_msg
        );
    }

    #[test]
    fn test_ndf_effective_fixing_source() {
        // With enum set
        let ndf_enum = Ndf::builder()
            .id(InstrumentId::new("USDCNY"))
            .base_currency(Currency::CNY)
            .settlement_currency(Currency::USD)
            .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
            .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
            .notional(Money::new(10_000_000.0, Currency::CNY))
            .contract_rate(7.25)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .quote_convention(NdfQuoteConvention::BasePerSettlement)
            .fixing_source_enum_opt(Some(NdfFixingSource::Pboc))
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        assert_eq!(ndf_enum.effective_fixing_source(), Some("PBOC".to_string()));
    }

    #[test]
    fn test_ndf_invalid_contract_rate_errors() {
        let err = Ndf::builder()
            .id(InstrumentId::new("USDCNY"))
            .base_currency(Currency::CNY)
            .settlement_currency(Currency::USD)
            .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
            .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
            .notional(Money::new(10_000_000.0, Currency::CNY))
            .contract_rate(0.0)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .quote_convention(NdfQuoteConvention::BasePerSettlement)
            .attributes(Attributes::new())
            .build()
            .expect_err("builder should reject invalid contract rate");
        assert!(
            err.to_string().contains("contract_rate"),
            "error should mention contract_rate: {err}"
        );
    }

    #[test]
    fn test_ndf_invalid_fixing_rate_errors() {
        let err = Ndf::builder()
            .id(InstrumentId::new("USDCNY"))
            .base_currency(Currency::CNY)
            .settlement_currency(Currency::USD)
            .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
            .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
            .notional(Money::new(10_000_000.0, Currency::CNY))
            .contract_rate(7.25)
            .fixing_rate_opt(Some(f64::INFINITY))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .quote_convention(NdfQuoteConvention::BasePerSettlement)
            .attributes(Attributes::new())
            .build()
            .expect_err("builder should reject invalid fixing rate");
        assert!(
            err.to_string().contains("fixing_rate"),
            "error should mention fixing_rate: {err}"
        );
    }

    #[test]
    fn test_cashflow_provider_emits_post_fixing_settlement_flow() {
        use finstack_quant_core::market_data::term_structures::DiscountCurve;

        let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let maturity = Date::from_calendar_date(2025, Month::March, 15).expect("valid date");
        let fixing_date = Date::from_calendar_date(2025, Month::March, 13).expect("valid date");
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("curve should build");
        let market = MarketContext::new().insert(curve);

        let ndf = Ndf::builder()
            .id(InstrumentId::new("USDCNY-FIXED-CF"))
            .base_currency(Currency::CNY)
            .settlement_currency(Currency::USD)
            .fixing_date(fixing_date)
            .maturity(maturity)
            .notional(Money::new(10_000_000.0, Currency::CNY))
            .contract_rate(7.25)
            .fixing_rate_opt(Some(7.30))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .quote_convention(NdfQuoteConvention::BasePerSettlement)
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        let flows = ndf
            .dated_cashflows(&market, as_of)
            .expect("post-fixing contractual settlement should build");

        assert_eq!(flows.len(), 1, "fixed NDF should emit one settlement flow");
        assert_eq!(flows[0].0, maturity);
        assert_eq!(flows[0].1.currency(), Currency::USD);
        assert!(flows[0].1.amount() > 0.0);
    }

    fn create_test_market(as_of: Date) -> MarketContext {
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};

        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.5, 0.9753), (1.0, 0.9512)])
            .build()
            .expect("should build");

        let fx_provider = {
            let p = Arc::new(SimpleFxProvider::new());
            p.set_quote(Currency::CNY, Currency::USD, 7.25)
                .expect("valid rate");
            p
        };
        let fx_matrix = FxMatrix::new(fx_provider);

        MarketContext::new().insert(usd_curve).insert_fx(fx_matrix)
    }

    /// Audit F2: the `fixing_date > maturity` guard, previously stranded in the
    /// unregistered `NdfDiscountingPricer`, now lives in `base_value` so it runs
    /// on every valuation. `Ndf::validate` rejects this at construction, so we
    /// mutate the field directly to reach the value path.
    #[test]
    fn test_ndf_value_rejects_fixing_after_maturity() {
        use crate::instruments::common_impl::traits::Instrument;

        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
        let market = create_test_market(as_of);

        let mut ndf = Ndf::builder()
            .id(InstrumentId::new("TEST"))
            .base_currency(Currency::CNY)
            .settlement_currency(Currency::USD)
            .fixing_date(Date::from_calendar_date(2024, Month::April, 13).expect("valid date"))
            .maturity(Date::from_calendar_date(2024, Month::April, 15).expect("valid date"))
            .notional(Money::new(10_000_000.0, Currency::CNY))
            .contract_rate(7.25)
            .forward_rate_override_opt(Some(7.25))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .quote_convention(NdfQuoteConvention::BasePerSettlement)
            .build()
            .expect("valid");

        // Directly bypass the builder validation to create a malformed NDF.
        ndf.maturity = Date::from_calendar_date(2024, Month::April, 12).expect("valid date");

        let err = ndf
            .value(&market, as_of)
            .expect_err("fixing_date after maturity must error");
        let msg = err.to_string();
        assert!(
            msg.contains("fixing_date") && msg.contains("maturity"),
            "error should mention the fixing/maturity ordering: {msg}"
        );
    }

    #[test]
    fn test_cashflow_schedule_is_tagged_projected() {
        let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let curve =
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots(vec![(0.0, 1.0), (1.0, 0.95)])
                .build()
                .expect("curve should build");
        let market = MarketContext::new().insert(curve);
        let schedule = Ndf::example()
            .cashflow_schedule(&market, as_of)
            .expect("ndf schedule");

        assert_eq!(
            schedule.meta.representation,
            crate::cashflow::builder::CashflowRepresentation::Projected
        );
    }
}
