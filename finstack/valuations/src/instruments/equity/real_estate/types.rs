//! Real estate asset valuation types.

use super::pricer;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::{
    Attributes, CurveDependencies, Instrument, InstrumentCurves,
};
use crate::pricer::InstrumentType;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};

/// Broad property classification for reporting / tagging.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RealEstatePropertyType {
    /// Office (CBD/suburban, single-tenant or multi-tenant).
    Office,
    /// Multifamily / residential rental.
    Multifamily,
    /// Retail (strip, mall, big box, high street).
    Retail,
    /// Industrial (warehouse, logistics, manufacturing).
    Industrial,
    /// Hospitality (hotel, resort).
    Hospitality,
    /// Mixed-use (multiple property types).
    MixedUse,
    /// Other / uncategorized.
    Other,
}

/// Valuation method for a real estate asset.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RealEstateValuationMethod {
    /// Discounted cashflow using an explicit NOI schedule and discount rate.
    Dcf,
    /// Direct capitalization using a stabilized NOI and cap rate.
    DirectCap,
}

/// Real estate asset valuation instrument.
///
/// Supports DCF (explicit NOI schedule) and direct capitalization valuation.
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = RealEstateAsset::validate)]
#[serde(deny_unknown_fields, try_from = "RealEstateAssetUnchecked")]
pub struct RealEstateAsset {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Currency for valuation.
    pub currency: Currency,
    /// Valuation date (base date for discounting).
    #[schemars(with = "String")]
    pub valuation_date: Date,
    /// Valuation method (DCF or DirectCap).
    pub valuation_method: RealEstateValuationMethod,
    /// Optional property type classification (for reporting).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property_type: Option<RealEstatePropertyType>,
    /// Net operating income schedule (date, amount).
    #[schemars(with = "Vec<(String, f64)>")]
    pub noi_schedule: Vec<(Date, f64)>,
    /// Capital expenditure schedule (date, amount). Values are treated as **positive outflows**.
    ///
    /// When present, cashflows are valued as `NOI - CapEx` (unlevered net cash flow).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Vec<(String, f64)>>")]
    pub capex_schedule: Option<Vec<(Date, f64)>>,
    /// Discount rate for DCF (annualized).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount_rate: Option<f64>,
    /// Capitalization rate for direct cap (annualized).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_rate: Option<f64>,
    /// Optional stabilized NOI override for direct cap.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stabilized_noi: Option<f64>,
    /// Optional terminal cap rate for DCF (uses last NOI).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_cap_rate: Option<f64>,
    /// Optional terminal growth rate used to project `NOI_{N+1}` for exit valuation.
    ///
    /// Market convention for exit-cap terminal value is \(TV = NOI_{N+1} / cap\_rate\_exit\).
    /// When not provided, defaults to 0 (uses last NOI as-is).
    /// Validation range is \([-100\%, 20\%]\) to guard against configuration errors.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_growth_rate: Option<f64>,
    /// Optional sale/exit date that truncates the DCF horizon.
    ///
    /// When set, DCF only values unlevered flows up to and including `sale_date`.
    /// Terminal proceeds (if configured) are realized on `sale_date`.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub sale_date: Option<Date>,
    /// Optional explicit gross sale price (terminal proceeds), before disposition costs.
    ///
    /// When set, this takes precedence over `terminal_cap_rate` for terminal proceeds.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sale_price: Option<Money>,
    /// Optional purchase price (useful for IRR / cap rate metrics).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purchase_price: Option<Money>,
    /// Optional one-time acquisition cost deducted at `as_of` in DCF valuation.
    ///
    /// This is intended for closing costs, fees, and other transaction costs.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisition_cost: Option<f64>,
    /// Optional detailed acquisition cost line items (positive outflows) deducted at `as_of`.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acquisition_costs: Vec<Money>,
    /// Optional disposition cost percentage applied to terminal value.
    ///
    /// A value of `0.02` represents 2% selling costs. Must be in \([0, 1)\).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition_cost_pct: Option<f64>,
    /// Optional detailed disposition cost line items (positive outflows) deducted from terminal proceeds.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disposition_costs: Vec<Money>,
    /// Optional appraisal override value.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appraisal_value: Option<Money>,
    /// Day count convention for year fractions.
    pub day_count: DayCount,
    /// Discount curve identifier, used for risk attribution only.
    ///
    /// DCF valuation always discounts at [`discount_rate`](Self::discount_rate)
    /// regardless of whether this curve is loaded ; rate
    /// sensitivity (`Dv01`/`BucketedDv01`) bumps the risk-free component
    /// inside the rate.
    pub discount_curve_id: CurveId,
    /// Attributes for tagging and scenarios.
    #[builder(default)]
    #[serde(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

/// Mirror of `RealEstateAsset` used by serde to apply `validate()` after
/// deserialization. Not part of the public API.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RealEstateAssetUnchecked {
    id: InstrumentId,
    currency: Currency,
    #[schemars(with = "String")]
    valuation_date: Date,
    valuation_method: RealEstateValuationMethod,
    #[serde(default)]
    property_type: Option<RealEstatePropertyType>,
    #[schemars(with = "Vec<(String, f64)>")]
    noi_schedule: Vec<(Date, f64)>,
    #[serde(default)]
    #[schemars(with = "Option<Vec<(String, f64)>>")]
    capex_schedule: Option<Vec<(Date, f64)>>,
    #[serde(default)]
    discount_rate: Option<f64>,
    #[serde(default)]
    cap_rate: Option<f64>,
    #[serde(default)]
    stabilized_noi: Option<f64>,
    #[serde(default)]
    terminal_cap_rate: Option<f64>,
    #[serde(default)]
    terminal_growth_rate: Option<f64>,
    #[serde(default)]
    #[schemars(with = "Option<String>")]
    sale_date: Option<Date>,
    #[serde(default)]
    sale_price: Option<Money>,
    #[serde(default)]
    purchase_price: Option<Money>,
    #[serde(default)]
    acquisition_cost: Option<f64>,
    #[serde(default)]
    acquisition_costs: Vec<Money>,
    #[serde(default)]
    disposition_cost_pct: Option<f64>,
    #[serde(default)]
    disposition_costs: Vec<Money>,
    #[serde(default)]
    appraisal_value: Option<Money>,
    day_count: DayCount,
    discount_curve_id: CurveId,
    #[serde(default)]
    pricing_overrides: crate::instruments::PricingOverrides,
    #[serde(default)]
    attributes: Attributes,
}

impl TryFrom<RealEstateAssetUnchecked> for RealEstateAsset {
    type Error = finstack_core::Error;

    fn try_from(value: RealEstateAssetUnchecked) -> std::result::Result<Self, Self::Error> {
        let inst = Self {
            id: value.id,
            currency: value.currency,
            valuation_date: value.valuation_date,
            valuation_method: value.valuation_method,
            property_type: value.property_type,
            noi_schedule: value.noi_schedule,
            capex_schedule: value.capex_schedule,
            discount_rate: value.discount_rate,
            cap_rate: value.cap_rate,
            stabilized_noi: value.stabilized_noi,
            terminal_cap_rate: value.terminal_cap_rate,
            terminal_growth_rate: value.terminal_growth_rate,
            sale_date: value.sale_date,
            sale_price: value.sale_price,
            purchase_price: value.purchase_price,
            acquisition_cost: value.acquisition_cost,
            acquisition_costs: value.acquisition_costs,
            disposition_cost_pct: value.disposition_cost_pct,
            disposition_costs: value.disposition_costs,
            appraisal_value: value.appraisal_value,
            day_count: value.day_count,
            discount_curve_id: value.discount_curve_id,
            pricing_overrides: value.pricing_overrides,
            attributes: value.attributes,
        };
        inst.validate()?;
        Ok(inst)
    }
}

impl RealEstateAsset {
    /// Validate structural invariants required by both DCF and DirectCap pricers.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `noi_schedule` is empty (both methods need at least one NOI point)
    /// - `noi_schedule` is not strictly date-increasing, or contains
    ///   non-finite NOI amounts
    /// - `capex_schedule` (when set) is unsorted or contains non-finite values
    /// - `discount_rate`, `cap_rate`, `terminal_cap_rate` are set but non-finite
    /// - `cap_rate` or `terminal_cap_rate` are ≤ 0 (would divide-by-zero or
    ///   produce negative valuations)
    /// - `terminal_growth_rate` is set but outside `[-1.0, 0.20]` (sanity band:
    ///   prevents `1 + g <= 0` and unreasonably high terminal growth)
    /// - `disposition_cost_pct` is set but outside `[0.0, 1.0)`
    /// - `valuation_method == DirectCap` but neither `cap_rate` nor a way to
    ///   derive cap (sale_price + NOI) is set
    /// - `valuation_method == Dcf` without `discount_rate` (unless an
    ///   `appraisal_value` short-circuits pricing) — DCF always discounts at
    ///   the property rate 
    /// - `sale_date` (when set) is on or before `valuation_date`
    pub fn validate(&self) -> finstack_core::Result<()> {
        if self.noi_schedule.is_empty() {
            return Err(finstack_core::Error::Validation(format!(
                "RealEstateAsset '{}' noi_schedule must contain at least one entry",
                self.id.as_str()
            )));
        }
        for (i, (date, amount)) in self.noi_schedule.iter().enumerate() {
            if !amount.is_finite() {
                return Err(finstack_core::Error::Validation(format!(
                    "RealEstateAsset '{}' noi_schedule[{}] amount on {} must be finite, got {}",
                    self.id.as_str(),
                    i,
                    date,
                    amount
                )));
            }
        }
        for window in self.noi_schedule.windows(2) {
            if window[0].0 >= window[1].0 {
                return Err(finstack_core::Error::Validation(format!(
                    "RealEstateAsset '{}' noi_schedule dates must be strictly increasing; got {} >= {}",
                    self.id.as_str(),
                    window[0].0,
                    window[1].0
                )));
            }
        }
        if let Some(capex) = &self.capex_schedule {
            for (i, (date, amount)) in capex.iter().enumerate() {
                if !amount.is_finite() {
                    return Err(finstack_core::Error::Validation(format!(
                        "RealEstateAsset '{}' capex_schedule[{}] amount on {} must be finite, got {}",
                        self.id.as_str(), i, date, amount
                    )));
                }
            }
            for window in capex.windows(2) {
                if window[0].0 >= window[1].0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "RealEstateAsset '{}' capex_schedule dates must be strictly increasing; got {} >= {}",
                        self.id.as_str(),
                        window[0].0,
                        window[1].0
                    )));
                }
            }
        }
        if let Some(dr) = self.discount_rate {
            if !dr.is_finite() {
                return Err(finstack_core::Error::Validation(format!(
                    "RealEstateAsset '{}' discount_rate must be finite, got {}",
                    self.id.as_str(),
                    dr
                )));
            }
        }
        if let Some(cr) = self.cap_rate {
            if !cr.is_finite() || cr <= 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "RealEstateAsset '{}' cap_rate must be finite and positive, got {}",
                    self.id.as_str(),
                    cr
                )));
            }
        }
        if let Some(tcr) = self.terminal_cap_rate {
            if !tcr.is_finite() || tcr <= 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "RealEstateAsset '{}' terminal_cap_rate must be finite and positive, got {}",
                    self.id.as_str(),
                    tcr
                )));
            }
        }
        if let Some(g) = self.terminal_growth_rate {
            if !g.is_finite() || !(-1.0..=0.20).contains(&g) {
                return Err(finstack_core::Error::Validation(format!(
                    "RealEstateAsset '{}' terminal_growth_rate must be in [-1.0, 0.20], got {}",
                    self.id.as_str(),
                    g
                )));
            }
        }
        if let Some(dc) = self.disposition_cost_pct {
            if !dc.is_finite() || !(0.0..1.0).contains(&dc) {
                return Err(finstack_core::Error::Validation(format!(
                    "RealEstateAsset '{}' disposition_cost_pct must be in [0.0, 1.0), got {}",
                    self.id.as_str(),
                    dc
                )));
            }
        }
        if matches!(self.valuation_method, RealEstateValuationMethod::DirectCap)
            && self.cap_rate.is_none()
        {
            return Err(finstack_core::Error::Validation(format!(
                "RealEstateAsset '{}' uses DirectCap method but cap_rate is not set",
                self.id.as_str()
            )));
        }
        // DCF always discounts at the property rate , so
        // the rate is required unless an appraisal value short-circuits
        // pricing entirely.
        if matches!(self.valuation_method, RealEstateValuationMethod::Dcf)
            && self.appraisal_value.is_none()
            && self.discount_rate.is_none()
        {
            return Err(finstack_core::Error::Validation(format!(
                "RealEstateAsset '{}' uses Dcf method but discount_rate is not set",
                self.id.as_str()
            )));
        }
        if let Some(sd) = self.sale_date {
            if sd <= self.valuation_date {
                return Err(finstack_core::Error::Validation(format!(
                    "RealEstateAsset '{}' sale_date {} must be after valuation_date {}",
                    self.id.as_str(),
                    sd,
                    self.valuation_date
                )));
            }
        }
        Ok(())
    }

    /// Create a representative office building DCF valuation example.
    ///
    /// 5-year NOI schedule at $100K/year, 8% discount rate, 5.5% terminal
    /// cap rate, Act/365F day count convention.
    pub fn example() -> finstack_core::Result<Self> {
        use finstack_core::dates::DayCount;

        let valuation_date = time::macros::date!(2025 - 01 - 01);
        let noi_schedule: Vec<(Date, f64)> = (1..=5)
            .map(|y| {
                Date::from_calendar_date(2025 + y, time::Month::January, 1)
                    .map(|date| (date, 100_000.0))
                    .map_err(|error| finstack_core::Error::Validation(error.to_string()))
            })
            .collect::<finstack_core::Result<Vec<_>>>()?;

        Self::builder()
            .id(InstrumentId::new("RE-OFFICE-DCF"))
            .currency(Currency::USD)
            .valuation_date(valuation_date)
            .valuation_method(RealEstateValuationMethod::Dcf)
            .property_type_opt(Some(RealEstatePropertyType::Office))
            .noi_schedule(noi_schedule)
            .discount_rate_opt(Some(0.08))
            .terminal_cap_rate_opt(Some(0.055))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::default())
            .build()
    }

    pub(crate) fn acquisition_cost_total(&self) -> finstack_core::Result<f64> {
        pricer::acquisition_cost_total(self)
    }

    /// Compute net sale proceeds (undiscounted) realized at `exit_date`, if configured.
    ///
    /// Precedence:
    /// - If `sale_price` is set: use it as the gross proceeds.
    /// - Else if `terminal_cap_rate` is set: use exit-cap convention `TV = NOI_{N+1} / cap_rate_exit`
    ///   with NOI taken as the last schedule entry on/before `exit_date`.
    ///
    /// Then apply:
    /// - `disposition_cost_pct` (pct of gross proceeds), and
    /// - `disposition_costs` (dollar line items).
    pub(crate) fn sale_proceeds_at(
        &self,
        as_of: Date,
        exit_date: Date,
    ) -> finstack_core::Result<Option<(Date, f64)>> {
        pricer::sale_proceeds_at(self, as_of, exit_date)
    }

    /// First future NOI amount on/after `as_of`.
    pub(crate) fn first_noi(&self, as_of: Date) -> finstack_core::Result<f64> {
        pricer::first_noi(self, as_of)
    }

    /// Unlevered net cash flows (NOI - CapEx) on/after `as_of`.
    pub(crate) fn unlevered_flows(&self, as_of: Date) -> finstack_core::Result<Vec<(Date, f64)>> {
        pricer::unlevered_flows(self, as_of)
    }

    /// NOI cash flows on/after `as_of`.
    pub(crate) fn noi_flows(&self, as_of: Date) -> finstack_core::Result<Vec<(Date, f64)>> {
        pricer::noi_flows(self, as_of)
    }

    /// Compute net sale proceeds at the terminal date (undiscounted), if configured.
    ///
    /// Uses the exit-cap convention `TV = NOI_{N+1} / cap_rate_exit`, optionally applying
    /// `disposition_cost_pct`.
    pub(crate) fn terminal_sale_proceeds(
        &self,
        as_of: Date,
    ) -> finstack_core::Result<Option<(Date, f64)>> {
        pricer::terminal_sale_proceeds(self, as_of)
    }
}

impl Instrument for RealEstateAsset {
    impl_instrument_base!(InstrumentType::RealEstateAsset);

    fn base_value(&self, market: &MarketContext, as_of: Date) -> finstack_core::Result<Money> {
        pricer::compute_pv(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
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
impl CurveDependencies for RealEstateAsset {
    fn curve_dependencies(&self) -> finstack_core::Result<InstrumentCurves> {
        InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

impl crate::cashflow::traits::CashflowProvider for RealEstateAsset {
    fn cashflow_schedule(
        &self,
        _curves: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let mut flows: Vec<(Date, Money)> = self
            .unlevered_flows(as_of)?
            .into_iter()
            .map(|(date, amount)| (date, Money::new(amount, self.currency)))
            .collect();

        if let Some((date, amount)) = self.terminal_sale_proceeds(as_of)? {
            flows.push((date, Money::new(amount, self.currency)));
        }
        flows.sort_by_key(|(date, _)| *date);

        Ok(crate::cashflow::traits::schedule_from_dated_flows(
            flows,
            self.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                ..Default::default()
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use finstack_core::dates::DayCount;

    #[test]
    fn real_estate_cashflow_schedule_emits_projected_unlevered_flows() {
        let valuation_date =
            Date::from_calendar_date(2025, time::Month::January, 1).expect("valid valuation date");
        let noi1 = Date::from_calendar_date(2026, time::Month::January, 1).expect("valid noi date");
        let noi2 = Date::from_calendar_date(2027, time::Month::January, 1).expect("valid noi date");

        let asset = RealEstateAsset::builder()
            .id(InstrumentId::new("RE-SCHEDULE"))
            .currency(Currency::USD)
            .valuation_date(valuation_date)
            .valuation_method(RealEstateValuationMethod::Dcf)
            .noi_schedule(vec![(noi1, 100.0), (noi2, 100.0)])
            .discount_rate_opt(Some(0.10))
            .terminal_cap_rate_opt(Some(0.08))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Default::default())
            .build()
            .expect("asset should build");

        let schedule = asset
            .cashflow_schedule(&MarketContext::new(), valuation_date)
            .expect("real estate schedule");

        assert_eq!(
            schedule.meta.representation,
            crate::cashflow::builder::CashflowRepresentation::Projected
        );
        assert_eq!(schedule.flows.len(), 3);
        assert_eq!(schedule.flows[0].date, noi1);
        assert_eq!(schedule.flows[0].amount.amount(), 100.0);
        assert_eq!(schedule.flows[1].date, noi2);
        assert!(schedule.flows[2].amount.amount() > 100.0);
    }

    fn build_dcf_asset() -> RealEstateAsset {
        let valuation_date = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
        let n1 = Date::from_calendar_date(2026, time::Month::January, 1).unwrap();
        let n2 = Date::from_calendar_date(2027, time::Month::January, 1).unwrap();
        RealEstateAsset::builder()
            .id(InstrumentId::new("RE-VALID"))
            .currency(Currency::USD)
            .valuation_date(valuation_date)
            .valuation_method(RealEstateValuationMethod::Dcf)
            .noi_schedule(vec![(n1, 100.0), (n2, 100.0)])
            .discount_rate_opt(Some(0.08))
            .terminal_cap_rate_opt(Some(0.055))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Default::default())
            .build()
            .expect("base asset builds")
    }

    #[test]
    fn validate_rejects_empty_noi_schedule() {
        let mut asset = build_dcf_asset();
        asset.noi_schedule.clear();
        assert!(asset.validate().is_err());
    }

    #[test]
    fn validate_rejects_unsorted_noi_schedule() {
        let mut asset = build_dcf_asset();
        let n1 = Date::from_calendar_date(2027, time::Month::January, 1).unwrap();
        let n2 = Date::from_calendar_date(2026, time::Month::January, 1).unwrap();
        asset.noi_schedule = vec![(n1, 100.0), (n2, 100.0)];
        let err = asset.validate().expect_err("unsorted NOI must error");
        assert!(err.to_string().contains("strictly increasing"));
    }

    #[test]
    fn validate_rejects_zero_terminal_cap_rate() {
        let mut asset = build_dcf_asset();
        asset.terminal_cap_rate = Some(0.0);
        let err = asset
            .validate()
            .expect_err("zero terminal_cap_rate must error");
        assert!(err.to_string().contains("terminal_cap_rate"));
    }

    #[test]
    fn validate_rejects_negative_terminal_cap_rate() {
        let mut asset = build_dcf_asset();
        asset.terminal_cap_rate = Some(-0.05);
        assert!(asset.validate().is_err());
    }

    #[test]
    fn validate_rejects_directcap_without_cap_rate() {
        let mut asset = build_dcf_asset();
        asset.valuation_method = RealEstateValuationMethod::DirectCap;
        asset.cap_rate = None;
        let err = asset.validate().expect_err("DirectCap needs cap_rate");
        assert!(err.to_string().contains("DirectCap"));
    }

    #[test]
    fn validate_rejects_terminal_growth_above_band() {
        let mut asset = build_dcf_asset();
        asset.terminal_growth_rate = Some(1.5);
        assert!(asset.validate().is_err());
    }

    #[test]
    fn validate_rejects_disposition_cost_pct_at_one() {
        let mut asset = build_dcf_asset();
        asset.disposition_cost_pct = Some(1.0);
        assert!(asset.validate().is_err());
    }

    #[test]
    fn validate_rejects_sale_date_on_or_before_valuation_date() {
        let mut asset = build_dcf_asset();
        asset.sale_date = Some(asset.valuation_date);
        assert!(asset.validate().is_err());
    }
}
