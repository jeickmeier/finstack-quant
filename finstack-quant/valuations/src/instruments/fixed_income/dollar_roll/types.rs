//! Dollar roll types.
//!
//! A dollar roll is a simultaneous sale and purchase of agency MBS TBAs
//! for different settlement months, used for financing and carry trades.

use crate::cashflow::builder::CashFlowSchedule;
use crate::cashflow::primitives::CFKind;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::fixed_income::mbs_passthrough::AgencyProgram;
use crate::instruments::fixed_income::tba::{AgencyTba, TbaTerm};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, SifmaSettlementClass};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// Dollar roll - simultaneous sale and purchase of TBAs for different months.
///
/// A dollar roll involves:
/// 1. Selling TBA for near-month settlement
/// 2. Buying TBA for far-month settlement
///
/// The price difference between the two legs represents the "drop" and
/// implies a financing rate.
///
/// # Financing and Carry
///
/// Dollar rolls are used for:
/// - **Financing**: Implied repo rate is often cheaper than repo
/// - **Carry trades**: Profit from drop vs. expected prepayment
/// - **Roll specialness**: When roll drops exceed fair value
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::dollar_roll::DollarRoll;
/// use finstack_quant_valuations::instruments::fixed_income::tba::TbaTerm;
/// use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyProgram;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::types::{CurveId, InstrumentId};
///
/// let roll = DollarRoll::builder()
///     .id(InstrumentId::new("FN30-4.0-ROLL-0326-0426"))
///     .agency(AgencyProgram::Fnma)
///     .coupon(0.04)
///     .term(TbaTerm::ThirtyYear)
///     .notional(Money::new(10_000_000.0, Currency::USD))
///     .front_settlement_year(2026)
///     .front_settlement_month(3)
///     .back_settlement_year(2026)
///     .back_settlement_month(4)
///     .front_price(98.5)
///     .back_price(98.0)
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .build()
///     .expect("Valid dollar roll");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
#[serde(deny_unknown_fields)]
pub struct DollarRoll {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Agency program.
    pub agency: AgencyProgram,
    /// Pass-through coupon rate.
    pub coupon: f64,
    /// Original loan term.
    pub term: TbaTerm,
    /// Trade notional (par amount).
    pub notional: Money,
    /// Front-month settlement year.
    pub front_settlement_year: i32,
    /// Front-month settlement month (1-12).
    pub front_settlement_month: u8,
    /// Back-month settlement year.
    pub back_settlement_year: i32,
    /// Back-month settlement month (1-12).
    pub back_settlement_month: u8,
    /// SIFMA settlement class override.
    ///
    /// When `None`, inferred from agency + term.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_class: Option<SifmaSettlementClass>,
    /// Explicit front-month settlement date override.
    ///
    /// When set, bypasses the SIFMA calendar lookup for the front leg.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub front_settlement_date: Option<Date>,
    /// Explicit back-month settlement date override.
    ///
    /// When set, bypasses the SIFMA calendar lookup for the back leg.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub back_settlement_date: Option<Date>,
    /// Front-month price (sell price).
    pub front_price: f64,
    /// Back-month price (buy price).
    pub back_price: f64,
    /// Trade date.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub trade_date: Option<Date>,
    /// Discount curve identifier.
    pub discount_curve_id: CurveId,
    /// Optional repo/financing curve identifier (carry-only).
    ///
    /// Used exclusively for implied financing rate and roll specialness
    /// calculations (see [`crate::instruments::fixed_income::dollar_roll::carry`]
    /// module). Does **not** affect
    /// the mark-to-market PV, which always discounts both legs at
    /// `discount_curve_id`.
    ///
    /// When `None`, the discount curve rate is used as the reference
    /// financing rate for carry analytics.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_curve_id: Option<CurveId>,
    /// Pricing overrides.
    #[builder(default)]
    #[serde(default)]
    /// Instrument-owned pricing inputs.
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    #[builder(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    #[builder(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for tagging and selection.
    #[builder(default)]
    #[serde(default)]
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl DollarRoll {
    /// Create a canonical example dollar roll for testing.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("FN30-4.0-ROLL-0326-0426"))
            .agency(AgencyProgram::Fnma)
            .coupon(0.04)
            .term(TbaTerm::ThirtyYear)
            .notional(Money::new(10_000_000.0, Currency::USD))
            .front_settlement_year(2026)
            .front_settlement_month(3)
            .back_settlement_year(2026)
            .back_settlement_month(4)
            .front_price(98.5)
            .back_price(98.0)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(
                Attributes::new()
                    .with_tag("dollar_roll")
                    .with_tag("agency")
                    .with_meta("program", "fnma"),
            )
            .build()
    }

    /// Get the drop (price difference between front and back month).
    ///
    /// Positive drop means front month trades at premium to back month.
    pub fn drop(&self) -> f64 {
        self.front_price - self.back_price
    }

    /// Get the drop in 32nds (common market convention).
    pub fn drop_32nds(&self) -> f64 {
        self.drop() * 32.0
    }

    /// Effective settlement class (explicit or inferred from agency + term).
    pub fn effective_settlement_class(&self) -> SifmaSettlementClass {
        self.settlement_class.unwrap_or_else(|| {
            SifmaSettlementClass::from_agency_term(self.agency.as_str(), self.term.years())
        })
    }

    /// Resolve the front-month settlement date.
    pub fn front_settle_date(&self) -> finstack_quant_core::Result<Date> {
        if let Some(d) = self.front_settlement_date {
            return Ok(d);
        }
        self.front_leg()?.get_settlement_date()
    }

    /// Resolve the back-month settlement date.
    pub fn back_settle_date(&self) -> finstack_quant_core::Result<Date> {
        if let Some(d) = self.back_settlement_date {
            return Ok(d);
        }
        self.back_leg()?.get_settlement_date()
    }

    /// Create the front-month TBA leg.
    pub fn front_leg(&self) -> finstack_quant_core::Result<AgencyTba> {
        AgencyTba::builder()
            .id(InstrumentId::new(format!("{}-FRONT", self.id.as_str())))
            .agency(self.agency)
            .coupon(self.coupon)
            .term(self.term)
            .settlement_year(self.front_settlement_year)
            .settlement_month(self.front_settlement_month)
            .settlement_class_opt(Some(self.effective_settlement_class()))
            .notional(self.notional)
            .trade_price(self.front_price)
            .discount_curve_id(self.discount_curve_id.clone())
            .build()
    }

    /// Create the back-month TBA leg.
    pub fn back_leg(&self) -> finstack_quant_core::Result<AgencyTba> {
        AgencyTba::builder()
            .id(InstrumentId::new(format!("{}-BACK", self.id.as_str())))
            .agency(self.agency)
            .coupon(self.coupon)
            .term(self.term)
            .settlement_year(self.back_settlement_year)
            .settlement_month(self.back_settlement_month)
            .settlement_class_opt(Some(self.effective_settlement_class()))
            .notional(self.notional)
            .trade_price(self.back_price)
            .discount_curve_id(self.discount_curve_id.clone())
            .build()
    }

    /// Calculate days between settlement dates.
    pub fn settlement_days(&self) -> finstack_quant_core::Result<i64> {
        let front = self.front_settle_date()?;
        let back = self.back_settle_date()?;
        let days = (back - front).whole_days();
        if days <= 0 {
            return Err(finstack_quant_core::Error::Validation(
                "Back settlement date must be after front settlement date".to_string(),
            ));
        }
        Ok(days)
    }

    fn trade_cash_amount(&self, price: f64) -> f64 {
        self.notional.amount() * price / 100.0
    }
}

impl crate::instruments::common_impl::traits::Instrument for DollarRoll {
    impl_instrument_base!(crate::pricer::InstrumentType::DollarRoll);

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(repo_curve_id) = &self.repo_curve_id {
            deps.add_forward_curve(repo_curve_id.clone());
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        crate::instruments::fixed_income::dollar_roll::pricer::price_dollar_roll(
            self, market, as_of,
        )
    }

    fn effective_start_date(&self) -> Option<Date> {
        self.trade_date
    }

    crate::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for DollarRoll {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn raw_cashflow_schedule(
        &self,
        _market: &finstack_quant_core::market_data::context::MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let front_date = self.front_settle_date()?;
        let back_date = self.back_settle_date()?;
        let ccy = self.notional.currency();
        let schedule = crate::cashflow::traits::schedule_from_dated_flows(
            vec![
                (
                    front_date,
                    Money::new(self.trade_cash_amount(self.front_price), ccy),
                ),
                (
                    back_date,
                    Money::new(-self.trade_cash_amount(self.back_price), ccy),
                ),
            ],
            CFKind::Notional,
            finstack_quant_core::dates::DayCount::Act365F,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(self.notional),
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Contractual,
                    ..Default::default()
                },
            },
        );
        Ok(schedule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;

    #[test]
    fn test_dollar_roll_example() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");
        assert_eq!(roll.agency, AgencyProgram::Fnma);
        assert!((roll.coupon - 0.04).abs() < 1e-10);
    }

    #[test]
    fn test_drop_calculation() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");
        let drop = roll.drop();

        // Front price 98.5 - back price 98.0 = 0.5
        assert!((drop - 0.5).abs() < 1e-10);

        // 0.5 points = 16/32nds
        let drop_32 = roll.drop_32nds();
        assert!((drop_32 - 16.0).abs() < 1e-10);
    }

    #[test]
    fn test_leg_creation() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");

        let front = roll.front_leg().expect("front leg construction");
        let back = roll.back_leg().expect("back leg construction");

        assert_eq!(front.agency, roll.agency);
        assert_eq!(back.agency, roll.agency);
        assert!((front.trade_price - roll.front_price).abs() < 1e-10);
        assert!((back.trade_price - roll.back_price).abs() < 1e-10);
    }

    #[test]
    fn test_settlement_days() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");
        let days = roll.settlement_days().expect("valid dates");

        // One month apart should be roughly 28-31 days
        assert!((25..=35).contains(&days));
    }

    #[test]
    fn test_cashflow_provider_emits_front_and_back_trade_flows() {
        let as_of = Date::from_calendar_date(2024, time::Month::January, 15).expect("valid date");
        let roll = DollarRoll::example().expect("DollarRoll example is valid");
        let market = finstack_quant_core::market_data::context::MarketContext::new();

        let flows = roll
            .dated_cashflows(&market, as_of)
            .expect("contractual settlement schedule should build");

        assert_eq!(
            flows.len(),
            2,
            "dollar roll should emit front and back settlements"
        );
        assert_eq!(flows[0].0, roll.front_settle_date().expect("front settle"));
        assert_eq!(flows[1].0, roll.back_settle_date().expect("back settle"));
        assert!(flows[0].1.amount() > 0.0, "front sale should be a receipt");
        assert!(
            flows[1].1.amount() < 0.0,
            "back purchase should be a payment"
        );
    }
}
