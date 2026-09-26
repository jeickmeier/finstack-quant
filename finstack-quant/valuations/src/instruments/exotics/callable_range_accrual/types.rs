//! Callable Range Accrual instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::exotics::range_accrual::{BoundsType, RangeAccrualTerms};
use crate::instruments::rates::hw1f::bermudan_call::BermudanCallProvision;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// Callable Range Accrual.
///
/// Extends the existing range accrual concept with a Bermudan call provision
/// allowing the issuer to terminate early on specified call dates.
///
/// The call decision interacts with the range accrual feature: the issuer
/// will call when the expected future value of remaining range accrual
/// coupons exceeds the call price (par). Pricing requires backward
/// induction (LSMC or HW tree) combined with forward range accrual
/// coupon simulation.
///
/// # Pricing
///
/// - **LSMC**: Simulate paths with HW1F short rate model. At each call
///   date, compute continuation value via regression. Exercise if
///   the call amount `notional * price_pct_of_par / 100` is below the
///   continuation value.
/// - **HW Tree**: Build trinomial tree, attach range accrual cashflows
///   at each node, apply backward induction with call decision.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CallableRangeAccrual {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Range accrual contract terms (no identity, attributes or overrides of their own).
    pub range_accrual: RangeAccrualTerms,
    /// Bermudan call provision.
    pub call_provision: BermudanCallProvision,
    /// Instrument-owned pricing inputs.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes.
    pub attributes: Attributes,
}

impl CallableRangeAccrual {
    /// Validate the callable range accrual parameters.
    ///
    /// Validates both the underlying range accrual spec and the call provision.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.range_accrual.validate()?;
        self.call_provision.validate()?;
        if self.range_accrual.index_id.is_none() {
            return Err(finstack_quant_core::Error::Validation(
                "CallableRangeAccrual requires explicit index_id, forward_curve_id, and index_tenor; asset-style spot/dividend inputs cannot be priced with Hull-White"
                    .to_string(),
            ));
        }
        if self.range_accrual.forward_curve_id.as_ref()
            != Some(&self.range_accrual.discount_curve_id)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CallableRangeAccrual '{}' HW1F implementation is currently single-curve; forward_curve_id must equal discount_curve_id until basis-adjusted projection is implemented",
                self.id
            )));
        }
        Ok(())
    }

    /// Create a canonical example callable range accrual for testing.
    #[allow(clippy::expect_used)]
    pub fn example() -> Self {
        use finstack_quant_core::currency::Currency;
        use time::macros::date;
        use time::Month;

        let observation_dates = vec![
            Date::from_calendar_date(2026, Month::January, 31).expect("valid"),
            Date::from_calendar_date(2026, Month::February, 28).expect("valid"),
            Date::from_calendar_date(2026, Month::March, 31).expect("valid"),
            Date::from_calendar_date(2026, Month::April, 30).expect("valid"),
            Date::from_calendar_date(2026, Month::May, 31).expect("valid"),
            Date::from_calendar_date(2026, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2026, Month::July, 31).expect("valid"),
            Date::from_calendar_date(2026, Month::August, 31).expect("valid"),
            Date::from_calendar_date(2026, Month::September, 30).expect("valid"),
            Date::from_calendar_date(2026, Month::October, 31).expect("valid"),
            Date::from_calendar_date(2026, Month::November, 30).expect("valid"),
            Date::from_calendar_date(2026, Month::December, 31).expect("valid"),
        ];

        let call_dates = vec![
            Date::from_calendar_date(2026, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2026, Month::September, 30).expect("valid"),
            Date::from_calendar_date(2026, Month::December, 31).expect("valid"),
        ];

        CallableRangeAccrual {
            id: InstrumentId::new("CALLABLE-RA-SOFR-1Y"),
            range_accrual: RangeAccrualTerms::builder()
                .underlying_ticker("SOFR".to_string())
                .observation_dates(observation_dates)
                .lower_bound(0.04)
                .upper_bound(0.06)
                .bounds_type(BoundsType::Absolute)
                .coupon_rate(0.065)
                .notional(Money::from((1_000_000_i64, Currency::USD)))
                .day_count(DayCount::Act360)
                .accrual_start_date(date!(2025 - 12 - 31))
                .index_id(finstack_quant_core::types::IndexId::new("SOFR"))
                .forward_curve_id(CurveId::new("USD-OIS"))
                .index_tenor(finstack_quant_core::dates::Tenor::quarterly())
                .discount_curve_id(CurveId::new("USD-OIS"))
                .spot_id("SOFR-RATE".into())
                .vol_surface_id(CurveId::new("SOFR-VOL"))
                .div_yield_id_opt(None)
                .payment_date_opt(None)
                .past_observations_in_range_opt(None)
                .total_past_observations_opt(None)
                .build()
                .expect("example range accrual terms should build"),
            call_provision: BermudanCallProvision::new(call_dates, 100.0, 1),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        }
    }
}

impl crate::instruments::common_impl::traits::Instrument for CallableRangeAccrual {
    impl_instrument_base!(crate::pricer::InstrumentType::CallableRangeAccrual);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        CallableRangeAccrual::validate(self)
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::MonteCarloHullWhite1F
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        self.range_accrual.market_dependencies()
    }

    fn base_value(
        &self,
        _market: &finstack_quant_core::market_data::context::MarketContext,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        self.validate()?;
        Err(finstack_quant_core::Error::Validation(
            "Callable Range Accrual pricing requires LSMC or HW tree. \
             Use price_with_metrics with a MC pricer."
                .to_string(),
        ))
    }

    fn effective_start_date(&self) -> Option<Date> {
        self.range_accrual.observation_dates.first().copied()
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    CallableRangeAccrual,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_validates() {
        let cra = CallableRangeAccrual::example();
        assert!(cra.validate().is_ok());
    }

    #[test]
    fn invalid_range_fails() {
        let mut cra = CallableRangeAccrual::example();
        cra.range_accrual.lower_bound = 0.06;
        cra.range_accrual.upper_bound = 0.04;
        assert!(cra.validate().is_err());
    }

    #[test]
    fn invalid_call_provision_fails() {
        let mut cra = CallableRangeAccrual::example();
        cra.call_provision.call_dates = vec![];
        assert!(cra.validate().is_err());
    }

    #[test]
    fn instrument_trait() {
        use crate::instruments::common_impl::traits::Instrument;
        let cra = CallableRangeAccrual::example();
        assert_eq!(cra.id(), "CALLABLE-RA-SOFR-1Y");
        assert_eq!(
            cra.key(),
            crate::pricer::InstrumentType::CallableRangeAccrual
        );
    }

    #[test]
    fn serde_roundtrip() {
        let cra = CallableRangeAccrual::example();
        let json = serde_json::to_string(&cra).expect("serialize");
        let deser: CallableRangeAccrual = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.id, cra.id);
        assert!((deser.range_accrual.coupon_rate - cra.range_accrual.coupon_rate).abs() < 1e-12);
    }

    #[test]
    fn nested_range_accrual_rejects_retired_envelope_keys() {
        // `range_accrual` now carries RangeAccrualTerms only: the callable note has
        // one id, one attributes map and one override set, all at its top level.
        for (key, retired) in [
            ("id", serde_json::json!("CALLABLE-RA-SOFR-1Y-RANGE")),
            ("attributes", serde_json::json!({})),
            (
                // schema-rejection-test
                "instrument_pricing_overrides",
                serde_json::json!({"model_config": {"mc_paths": 8}}),
            ),
            (
                // schema-rejection-test
                "metric_pricing_overrides",
                serde_json::json!({"theta_period": {"count": 1, "unit": "weeks"}}),
            ),
            (
                // schema-rejection-test
                "scenario_pricing_overrides",
                serde_json::json!({"scenario_price_shock_decimal": 0.01}),
            ),
        ] {
            let mut value =
                serde_json::to_value(CallableRangeAccrual::example()).expect("serialize");
            value["range_accrual"][key] = retired;
            let error = serde_json::from_value::<CallableRangeAccrual>(value)
                .expect_err("retired range_accrual envelope key must be rejected");
            assert!(
                error
                    .to_string()
                    .contains(&format!("unknown field `{key}`")),
                "range_accrual.{key}: {error}"
            );
        }
    }

    #[test]
    fn value_requires_explicit_pricer() {
        use crate::instruments::common_impl::traits::Instrument;
        use finstack_quant_core::market_data::context::MarketContext;
        use time::macros::date;

        let cra = CallableRangeAccrual::example();
        let market = MarketContext::default();
        let as_of = date!(2025 - 01 - 01);
        let err = cra
            .value(&market, as_of)
            .expect_err("value must require LSMC or HW tree pricer");
        let msg = err.to_string();
        assert!(
            msg.contains("LSMC") && msg.contains("HW tree"),
            "expected LSMC/HW tree pricer in error message, got: {msg}"
        );
    }
}
