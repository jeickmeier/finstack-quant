//! Levered real estate equity instrument.
//!
//! This instrument composes an unlevered [`RealEstateAsset`] with a financing stack
//! (e.g., term loans, bonds, convertibles) to provide:
//! - Equity value as `Asset PV - Financing PV`
//! - Levered deal-style metrics (IRR, MOIC, DSCR, LTV)

use super::levered_pricer;
use super::types::RealEstateAsset;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::{
    Attributes, CurveDependencies, Instrument, InstrumentCurves,
};
use crate::instruments::{InstrumentJson, MarketDependencies};
use crate::pricer::InstrumentType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// Levered real estate equity = unlevered asset + financing.
///
/// Value convention:
/// - `PV_equity = PV_asset - PV_financing` (financing valued from lender perspective).
///
/// Return/coverage metrics are computed from a simplified equity cashflow schedule:
/// - Initial outflow: `-(purchase_price + acquisition_cost)` at `as_of`
/// - Financing funding legs on/after `as_of` are included as equity inflows
/// - Interim equity CFs: `(NOI - CapEx) - debt_service_cash`
/// - Exit: `(sale_proceeds - financing_payoff)` at `exit_date`
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct LeveredRealEstateEquity {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Currency (must match asset currency; financing PV is validated at valuation time).
    pub currency: Currency,
    /// Underlying unlevered asset.
    pub asset: RealEstateAsset,
    /// Financing instruments (borrower liabilities), valued from lender perspective.
    ///
    /// PV convention nets these from the asset PV.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub financing: Vec<InstrumentJson>,
    /// Optional explicit exit/sale date. Defaults to the last NOI date on/after `as_of`.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub exit_date: Option<Date>,
    /// Discount curve identifier for equity PV attribution (typically same as asset curve).
    ///
    /// This is used only for curve dependency reporting; PV is computed as `asset - financing`.
    pub discount_curve_id: CurveId,
    /// Attributes for tagging and scenarios.
    #[builder(default)]
    #[serde(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,

    /// Attributes for tagging and scenarios.
    #[builder(default)]
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl LeveredRealEstateEquity {
    /// Create a representative levered office property example.
    ///
    /// Uses [`RealEstateAsset::example()`] as the underlying asset with no
    /// embedded financing for simplicity.
    pub fn example() -> finstack_quant_core::Result<Self> {
        let asset = RealEstateAsset::example()?;
        Self::builder()
            .id(InstrumentId::new("RE-LEVERED-OFFICE"))
            .currency(Currency::USD)
            .asset(asset)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::default())
            .build()
    }

    pub(crate) fn resolve_exit_date(&self, as_of: Date) -> finstack_quant_core::Result<Date> {
        levered_pricer::resolve_exit_date(self, as_of)
    }

    pub(crate) fn financing_schedules_supported(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Vec<crate::cashflow::builder::CashFlowSchedule>> {
        levered_pricer::financing_schedules_supported(self, market, as_of)
    }

    /// Build a dated equity cashflow schedule for internal levered return metrics.
    pub(crate) fn equity_cashflows(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Vec<(Date, f64)>> {
        levered_pricer::equity_cashflows(self, market, as_of)
    }

    /// Convenience: compute financing payoff amount at exit (absolute amount).
    pub fn financing_payoff_at_exit(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        levered_pricer::financing_payoff_at_exit(self, market, as_of)
    }

    pub(crate) fn irr_day_count(&self) -> DayCount {
        self.asset.day_count
    }
}

impl Instrument for LeveredRealEstateEquity {
    impl_instrument_base!(InstrumentType::LeveredRealEstateEquity);

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = self.asset.market_dependencies()?;
        for instrument in &self.financing {
            deps.merge(
                crate::instruments::common_impl::dependencies::MarketDependencies::from_instrument_json(
                    instrument,
                )?,
            );
        }
        deps.add_discount_curve(self.discount_curve_id.clone());
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        levered_pricer::compute_pv(self, market, as_of)
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

impl CurveDependencies for LeveredRealEstateEquity {
    fn curve_dependencies(&self) -> finstack_quant_core::Result<InstrumentCurves> {
        let mut deps = MarketDependencies::from_curve_dependencies(&self.asset)?;
        for inst in &self.financing {
            deps.merge(MarketDependencies::from_instrument_json(inst)?);
        }
        let mut curves = deps.curves;

        // Also include a top-level discount curve id for attribution if provided.
        if !curves
            .discount_curves
            .iter()
            .any(|c| c == &self.discount_curve_id)
        {
            curves.discount_curves.push(self.discount_curve_id.clone());
        }

        Ok(curves)
    }
}

impl crate::cashflow::traits::CashflowScheduleSource for LeveredRealEstateEquity {
    fn raw_cashflow_schedule(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let flows = self
            .equity_cashflows(market, as_of)?
            .into_iter()
            .map(|(date, amount)| (date, Money::new(amount, self.currency)))
            .collect();

        Ok(crate::cashflow::traits::schedule_from_dated_flows(
            flows,
            crate::cashflow::primitives::CFKind::Fixed,
            self.asset.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use crate::instruments::equity::real_estate::types::RealEstateValuationMethod;

    #[test]
    fn levered_real_estate_cashflow_schedule_emits_projected_equity_flows() {
        let as_of =
            Date::from_calendar_date(2025, time::Month::January, 1).expect("valid as_of date");
        let noi1 = Date::from_calendar_date(2026, time::Month::January, 1).expect("valid noi date");
        let noi2 = Date::from_calendar_date(2027, time::Month::January, 1).expect("valid noi date");

        let asset = RealEstateAsset::builder()
            .id(InstrumentId::new("RE-LEVERED-ASSET"))
            .currency(Currency::USD)
            .valuation_date(as_of)
            .valuation_method(RealEstateValuationMethod::Dcf)
            .noi_schedule(vec![(noi1, 100.0), (noi2, 100.0)])
            .purchase_price_opt(Some(Money::new(1_000.0, Currency::USD)))
            .sale_price_opt(Some(Money::new(1_100.0, Currency::USD)))
            .discount_rate_opt(Some(0.10))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Default::default())
            .build()
            .expect("asset should build");

        let equity = LeveredRealEstateEquity::builder()
            .id(InstrumentId::new("RE-LEVERED"))
            .currency(Currency::USD)
            .asset(asset)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Default::default())
            .build()
            .expect("levered equity should build");

        let schedule = equity
            .cashflow_schedule(&MarketContext::new(), as_of)
            .expect("levered schedule");

        assert_eq!(
            schedule.get_meta().representation,
            crate::cashflow::builder::CashflowRepresentation::Projected
        );
        assert_eq!(
            schedule.get_flows().first().expect("initial flow").date,
            as_of
        );
        assert!(
            schedule
                .get_flows()
                .first()
                .expect("initial flow")
                .amount
                .amount()
                < 0.0
        );
        assert!(
            schedule
                .get_flows()
                .last()
                .expect("exit flow")
                .amount
                .amount()
                > 0.0
        );
    }
}
