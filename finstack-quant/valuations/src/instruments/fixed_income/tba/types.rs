//! Agency TBA (To-Be-Announced) forward types.
//!
//! TBA trades are forward contracts on agency MBS pools where the specific
//! pools to be delivered are not known at trade time. Instead, pools must
//! meet good delivery standards (coupon, term, agency).

use crate::cashflow::traits::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::fixed_income::mbs_passthrough::{AgencyMbsPassthrough, AgencyProgram};
use crate::instruments::PricingOverrides;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, SifmaSettlementClass};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// TBA term enumeration (original loan term).
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
pub enum TbaTerm {
    /// 15-year original term
    FifteenYear,
    /// 20-year original term
    TwentyYear,
    /// 30-year original term
    ThirtyYear,
}

impl TbaTerm {
    /// Get the term in years.
    pub fn years(&self) -> u32 {
        match self {
            TbaTerm::FifteenYear => 15,
            TbaTerm::TwentyYear => 20,
            TbaTerm::ThirtyYear => 30,
        }
    }

    /// Get the term in months.
    pub fn months(&self) -> u32 {
        self.years() * 12
    }
}

impl std::fmt::Display for TbaTerm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TbaTerm::FifteenYear => write!(f, "15Y"),
            TbaTerm::TwentyYear => write!(f, "20Y"),
            TbaTerm::ThirtyYear => write!(f, "30Y"),
        }
    }
}

/// TBA settlement information.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TbaSettlement {
    /// Good delivery (settlement) date
    #[schemars(with = "String")]
    pub settlement_date: Date,
    /// Last trading day (48 hours before settlement)
    #[schemars(with = "String")]
    pub notification_date: Date,
}

/// TBA (To-Be-Announced) forward trade.
///
/// Represents a forward contract to buy or sell agency MBS at a specified
/// price for a future settlement date. The specific pools delivered are
/// not known at trade time.
///
/// # Good Delivery Standards
///
/// TBA trades must meet SIFMA good delivery guidelines:
/// - Pool must be from the specified agency program
/// - Pool coupon must match the TBA coupon
/// - Pool term must match the TBA term
/// - Variance rules for face amount (±0.01% of trade amount)
///
/// # Pricing
///
/// TBA value is calculated as the difference between the forward value
/// of assumed pool characteristics and the trade price, discounted to
/// the valuation date.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::tba::{AgencyTba, TbaTerm};
/// use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyProgram;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let tba = AgencyTba::builder()
///     .id(InstrumentId::new("FN30-4.0-202703"))
///     .agency(AgencyProgram::Fnma)
///     .coupon(0.04)
///     .term(TbaTerm::ThirtyYear)
///     .settlement_year(2027)
///     .settlement_month(3)
///     .notional(Money::new(10_000_000.0, Currency::USD))
///     .trade_price(98.5)
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .build()
///     .expect("Valid TBA");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct AgencyTba {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Agency program (FNMA, FHLMC, GNMA).
    pub agency: AgencyProgram,
    /// Pass-through coupon rate (e.g., 0.04 for 4%).
    pub coupon: f64,
    /// Original loan term.
    pub term: TbaTerm,
    /// Settlement year.
    pub settlement_year: i32,
    /// Settlement month (1-12).
    pub settlement_month: u8,
    /// SIFMA settlement class override.
    ///
    /// When `None`, inferred from agency + term using
    /// [`SifmaSettlementClass::from_agency_term`].
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_class: Option<SifmaSettlementClass>,
    /// Trade notional (par amount).
    pub notional: Money,
    /// Trade price (percentage of par, e.g., 98.5).
    pub trade_price: f64,
    /// Trade date.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub trade_date: Option<Date>,
    /// Expected pool factor for valuation.
    /// Defaults to 1.0 (newly issued) if not specified.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_factor: Option<f64>,
    /// Optional assumed pool for valuation.
    /// If not provided, generic pool characteristics are assumed.
    #[builder(optional)]
    #[serde(skip)]
    pub assumed_pool: Option<Box<AgencyMbsPassthrough>>,
    /// Discount curve identifier.
    pub discount_curve_id: CurveId,
    /// Pricing overrides.
    #[builder(default)]
    #[serde(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for tagging and selection.
    #[builder(default)]
    #[serde(default)]
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl AgencyTba {
    /// Create a canonical example TBA for testing and documentation.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("FN30-4.0-202703"))
            .agency(AgencyProgram::Fnma)
            .coupon(0.04)
            .term(TbaTerm::ThirtyYear)
            .settlement_year(2027)
            .settlement_month(3)
            .notional(Money::new(10_000_000.0, Currency::USD))
            .trade_price(98.5)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(
                Attributes::new()
                    .with_tag("tba")
                    .with_tag("agency")
                    .with_meta("program", "fnma"),
            )
            .build()
    }

    /// Builder helper for settlement month.
    pub fn settlement_month(mut self, year: i32, month: u8) -> Self {
        self.settlement_year = year;
        self.settlement_month = month;
        self
    }

    /// Get the settlement date using the SIFMA calendar.
    ///
    /// Uses the explicit `settlement_class` if set, otherwise infers the
    /// class from `agency` and `term`.
    pub fn get_settlement_date(&self) -> finstack_quant_core::Result<Date> {
        let month = time::Month::try_from(self.settlement_month)
            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
        let class = self.effective_settlement_class();
        finstack_quant_core::dates::sifma_settlement_date_for_class(
            month,
            self.settlement_year,
            class,
        )
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "No published SIFMA settlement date for {:02}/{} and class {:?}",
                self.settlement_month, self.settlement_year, class
            ))
        })
    }

    /// Effective settlement class (explicit or inferred from agency + term).
    pub fn effective_settlement_class(&self) -> SifmaSettlementClass {
        self.settlement_class.unwrap_or_else(|| {
            let agency_str = format!("{:?}", self.agency);
            SifmaSettlementClass::from_agency_term(&agency_str, self.term.years())
        })
    }

    /// Get TBA identifier string (e.g., "FN30 4.0 Mar24").
    pub fn tba_identifier(&self) -> String {
        let agency_str = match self.agency {
            AgencyProgram::Fnma => "FN",
            AgencyProgram::Fhlmc => "FH",
            AgencyProgram::Gnma | AgencyProgram::GnmaII | AgencyProgram::GnmaI => "GN",
        };
        let term_str = match self.term {
            TbaTerm::FifteenYear => "15",
            TbaTerm::TwentyYear => "20",
            TbaTerm::ThirtyYear => "30",
        };
        let month_str = match self.settlement_month {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            12 => "Dec",
            _ => "???",
        };
        let year_str = (self.settlement_year % 100) as u8;

        format!(
            "{}{} {:.1} {}{:02}",
            agency_str,
            term_str,
            self.coupon * 100.0,
            month_str,
            year_str
        )
    }

    /// Calculate trade value (notional × price).
    pub fn trade_value(&self) -> Money {
        Money::new(
            self.notional.amount() * self.trade_price / 100.0,
            self.notional.currency(),
        )
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for AgencyTba {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

impl CashflowProvider for AgencyTba {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        _curves: &finstack_quant_core::market_data::context::MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        Err(finstack_quant_core::Error::Validation(format!(
            "AgencyTba '{}' is a physically settled forward and has no standalone cashflow schedule; represent the delivered pool separately after settlement",
            self.id
        )))
    }
}

impl crate::instruments::common_impl::traits::Instrument for AgencyTba {
    impl_instrument_base!(crate::pricer::InstrumentType::AgencyTba);

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        crate::instruments::fixed_income::tba::pricer::price_tba(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        self.trade_date
    }

    fn expiry(&self) -> Option<Date> {
        self.get_settlement_date().ok()
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

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    #[test]
    fn test_tba_example() {
        let tba = AgencyTba::example().expect("AgencyTba example is valid");
        assert_eq!(tba.agency, AgencyProgram::Fnma);
        assert!((tba.coupon - 0.04).abs() < 1e-10);
        assert_eq!(tba.term, TbaTerm::ThirtyYear);
    }

    #[test]
    fn test_tba_identifier() {
        let tba = AgencyTba::example().expect("AgencyTba example is valid");
        let id = tba.tba_identifier();
        assert!(id.contains("FN30"));
        assert!(id.contains("4.0"));
    }

    #[test]
    fn test_trade_value() {
        let tba = AgencyTba::example().expect("AgencyTba example is valid");
        let value = tba.trade_value();
        // 10M at 98.5 = 9.85M
        assert!((value.amount() - 9_850_000.0).abs() < 1.0);
    }

    #[test]
    fn test_settlement_date() {
        let tba = AgencyTba::example().expect("AgencyTba example is valid");
        let settle = tba.get_settlement_date().expect("valid date");
        assert_eq!(settle.month(), Month::March);
        assert_eq!(settle.year(), 2027);
    }

    #[test]
    fn test_tba_term() {
        assert_eq!(TbaTerm::ThirtyYear.years(), 30);
        assert_eq!(TbaTerm::ThirtyYear.months(), 360);
        assert_eq!(TbaTerm::FifteenYear.years(), 15);
    }
}
