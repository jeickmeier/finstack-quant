//! CMS Spread Option instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::instruments::rates::cms_common::CmsReferenceSwap;
use crate::instruments::OptionType;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::types::{CurveId, InstrumentId};
use rust_decimal::Decimal;

/// CMS Spread Option.
///
/// Option on the spread between two CMS rates of different tenors.
///
/// ```text
/// Payoff = max(CMS_long - CMS_short - strike, 0) * notional    [for a call]
/// Payoff = max(strike - (CMS_long - CMS_short), 0) * notional   [for a put]
/// ```
///
/// Typically: long tenor = 10Y or 30Y CMS, short tenor = 2Y CMS.
///
/// # Pricing Approach
///
/// 1. Each CMS rate has a lognormal payment-measure marginal from a flat-strike
///    Black volatility surface (or a tenor-axis ATM surface).
/// 2. A Gaussian copula couples the two rates.
/// 3. The shared first-order CMS convexity approximation sets each marginal mean.
///
/// Nonflat smiles and SABR cubes are unsupported and return a validation error.
/// The registered `StaticReplication` model key selects this approximation;
/// it does not implement SABR smile replication.
///
/// # References
///
/// - Hagan, P. S. (2003). "Convexity Conundrums." *Wilmott Magazine*. `docs/REFERENCES.md#hagan-2003-cms-convexity`
/// - Antonov, A., Konikov, M., & Spector, M. (2013). "SABR Spreads." *Risk*. `docs/REFERENCES.md#hagan-2002-sabr`
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CmsSpreadOption {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Long CMS tenor (e.g., 10Y).
    pub long_cms_tenor: Tenor,
    /// Short CMS tenor (e.g., 2Y).
    pub short_cms_tenor: Tenor,
    /// Strike on the CMS spread as a decimal rate (0.005 = 50bp).
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub strike: Decimal,
    /// Call or put on the spread `CMS_long - CMS_short`: a call pays
    /// `max(spread - strike, 0)`, a put pays `max(strike - spread, 0)`.
    pub option_type: OptionType,
    /// Notional amount.
    pub notional: Money,
    /// Option expiry date.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// Payment date (may differ from expiry).
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub payment_date: Date,
    /// Black swaption volatility surface for the long tenor. The selected
    /// expiry slice must be flat across strikes; tenor-axis ATM surfaces are valid.
    pub long_vol_surface_id: CurveId,
    /// Black swaption volatility surface for the short tenor, subject to the
    /// same flat-strike restriction as the long-tenor surface.
    pub short_vol_surface_id: CurveId,
    /// Discount curve ID.
    pub discount_curve_id: CurveId,
    /// Forward curve ID (for swap rate projection).
    pub forward_curve_id: CurveId,
    /// Gaussian-copula correlation between the two CMS rates, a decimal in `[-1, 1]`.
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_correlation",
        deserialize_with = "finstack_quant_core::wire::deserialize_correlation"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::CorrelationWire")
    )]
    pub correlation: f64,
    /// Day count convention.
    pub day_count: DayCount,

    //
    // The forward swap rate of each CMS leg must be projected on the correct
    // annuity / day-count basis. These fields plumb the actual instrument /
    // market swap conventions through to `resolve_leg`.
    /// Rate-index convention-registry key of the underlying CMS swaps (e.g. `EUR-ESTR-OIS`).
    ///
    /// Required for USD CMS; other supported currencies use their registered
    /// overnight-index default when omitted. Supplies calendar, settlement lag
    /// and default leg conventions. Individual leg fields override the index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_id: Option<IndexId>,
    /// Fixed leg frequency of the underlying CMS swaps (overrides convention).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_fixed_frequency: Option<Tenor>,
    /// Floating leg frequency of the underlying CMS swaps (overrides convention).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_float_frequency: Option<Tenor>,
    /// Fixed leg day count of the underlying CMS swaps (overrides convention).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_fixed_day_count: Option<DayCount>,
    /// Floating leg day count of the underlying CMS swaps (overrides convention).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_float_day_count: Option<DayCount>,
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

impl CmsSpreadOption {
    /// Strike as an `f64` decimal rate for the pricing kernels.
    ///
    /// # Errors
    ///
    /// Returns an error when `strike` cannot be represented as `f64`.
    pub(crate) fn strike_rate(&self) -> finstack_quant_core::Result<f64> {
        finstack_quant_core::decimal::decimal_to_f64(self.strike)
    }

    /// Validate the CMS spread option parameters.
    ///
    /// Checks:
    /// - Long tenor must be strictly longer than short tenor
    /// - Strike is finite
    /// - Expiry date is before or on payment date
    /// - Correlation is in [-1, 1]
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        // Tenor comparison: long must be > short (compare months if both are month/year based)
        validation::require_with(
            self.long_cms_tenor.months() > self.short_cms_tenor.months(),
            || {
                format!(
                    "CmsSpreadOption long_cms_tenor ({}) must be longer than short_cms_tenor ({})",
                    self.long_cms_tenor, self.short_cms_tenor
                )
            },
        )?;

        self.strike_rate()?;

        validation::require_with(self.payment_date >= self.expiry, || {
            format!(
                "CmsSpreadOption payment_date ({}) must be on or after expiry ({})",
                self.payment_date, self.expiry
            )
        })?;

        validation::require_with((-1.0..=1.0).contains(&self.correlation), || {
            format!(
                "CmsSpreadOption correlation ({}) must be in [-1, 1]",
                self.correlation
            )
        })?;

        Ok(())
    }

    /// Reference swap observed by this instrument's CMS fixings.
    pub fn reference_swap(&self) -> CmsReferenceSwap<'_> {
        CmsReferenceSwap {
            label: "CMS spread option",
            currency: self.notional.currency(),
            index_id: self.index_id.as_ref(),
            swap_fixed_frequency: self.swap_fixed_frequency,
            swap_float_frequency: self.swap_float_frequency,
            swap_fixed_day_count: self.swap_fixed_day_count,
            swap_float_day_count: self.swap_float_day_count,
            discount_curve_id: &self.discount_curve_id,
            forward_curve_id: &self.forward_curve_id,
        }
    }

    /// Create a canonical example CMS spread option for testing.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;

        Ok(CmsSpreadOption {
            id: InstrumentId::new("CMS-SPREAD-10Y2Y"),
            long_cms_tenor: Tenor::new(10, finstack_quant_core::dates::TenorUnit::Years)?,
            short_cms_tenor: Tenor::new(2, finstack_quant_core::dates::TenorUnit::Years)?,
            strike: Decimal::new(5, 3), // 50bp
            option_type: OptionType::Call,
            notional: Money::from((10_000_000_i64, Currency::USD)),
            expiry: time::macros::date!(2027 - 03 - 29),
            payment_date: time::macros::date!(2027 - 03 - 31),
            long_vol_surface_id: CurveId::new("USD-SWAPTION-VOL-10Y"),
            short_vol_surface_id: CurveId::new("USD-SWAPTION-VOL-2Y"),
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            correlation: 0.85,
            day_count: DayCount::Act360,
            index_id: Some(IndexId::new("USD-SOFR-OIS")),
            swap_fixed_frequency: None,
            swap_float_frequency: None,
            swap_fixed_day_count: None,
            swap_float_day_count: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        })
    }
}

impl crate::instruments::common_impl::traits::Instrument for CmsSpreadOption {
    impl_instrument_base!(crate::pricer::InstrumentType::CmsSpreadOption);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::StaticReplication
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_forward_curve(self.forward_curve_id.clone());
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.long_vol_surface_id.clone(),
                None,
                Some(self.strike_rate()?),
            ),
        );
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.short_vol_surface_id.clone(),
                None,
                Some(self.strike_rate()?),
            ),
        );
        let long_tenor_years = self.long_cms_tenor.months().map(f64::from).ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "CMS long tenor must be month- or year-based".to_string(),
            )
        })? / 12.0;
        let short_tenor_years = self
            .short_cms_tenor
            .months()
            .map(f64::from)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "CMS short tenor must be month- or year-based".to_string(),
                )
            })?
            / 12.0;
        deps.add_series_id(
            finstack_quant_core::market_data::fixings::cms_fixing_series_id(
                self.forward_curve_id.as_str(),
                long_tenor_years,
            ),
        );
        deps.add_series_id(
            finstack_quant_core::market_data::fixings::cms_fixing_series_id(
                self.forward_curve_id.as_str(),
                short_tenor_years,
            ),
        );
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        self.validate()?;
        crate::instruments::rates::cms_spread_option::pricer::CmsSpreadOptionPricer::new()
            .price_internal(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.expiry)
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    CmsSpreadOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::PricingOptions;
    use crate::pricer::{standard_pricer_registry, ModelKey};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    fn flat_surface(id: &str, vol: f64) -> VolSurface {
        VolSurface::builder(id)
            .expiries(&[0.25, 1.0, 5.0])
            .strikes(&[0.005, 0.04, 0.10])
            .row(&[vol; 3])
            .row(&[vol; 3])
            .row(&[vol; 3])
            .build()
            .expect("flat Black surface")
    }

    fn market(as_of: Date, vol: f64) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (30.0, (-0.035_f64 * 30.0).exp())])
            .build()
            .expect("discount curve");
        let forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.025), (2.0, 0.030), (10.0, 0.045), (30.0, 0.055)])
            .build()
            .expect("forward curve");

        MarketContext::new()
            .insert(discount)
            .insert(forward)
            .insert_surface(flat_surface("USD-SWAPTION-VOL-10Y", vol))
            .insert_surface(flat_surface("USD-SWAPTION-VOL-2Y", vol))
    }

    fn price_amount(opt: &CmsSpreadOption, market: &MarketContext, as_of: Date) -> f64 {
        standard_pricer_registry()
            .price_with_metrics(
                opt,
                ModelKey::StaticReplication,
                market,
                as_of,
                &[],
                PricingOptions::default(),
            )
            .expect("cms spread option price")
            .value
            .amount()
    }

    #[test]
    fn example_validates() {
        let opt = CmsSpreadOption::example().expect("example");
        assert!(opt.validate().is_ok());
    }

    #[test]
    fn long_tenor_shorter_than_short_fails() {
        let mut opt = CmsSpreadOption::example().expect("example");
        // Swap the tenors so long < short
        opt.long_cms_tenor = Tenor::new(2, finstack_quant_core::dates::TenorUnit::Years)
            .expect("valid tenor fixture");
        opt.short_cms_tenor = Tenor::new(10, finstack_quant_core::dates::TenorUnit::Years)
            .expect("valid tenor fixture");
        assert!(opt.validate().is_err());
    }

    #[test]
    fn correlation_out_of_range_fails() {
        let mut opt = CmsSpreadOption::example().expect("example");
        opt.correlation = 1.5;
        assert!(opt.validate().is_err());
    }

    #[test]
    fn payment_before_expiry_fails() {
        use time::Month;
        let mut opt = CmsSpreadOption::example().expect("example");
        opt.payment_date = Date::from_calendar_date(2027, Month::March, 28).expect("valid");
        assert!(opt.validate().is_err());
    }

    #[test]
    fn instrument_trait() {
        use crate::instruments::common_impl::traits::Instrument;
        let opt = CmsSpreadOption::example().expect("example");
        assert_eq!(opt.id(), "CMS-SPREAD-10Y2Y");
        assert_eq!(opt.key(), crate::pricer::InstrumentType::CmsSpreadOption);
    }

    #[test]
    fn serde_roundtrip() {
        let opt = CmsSpreadOption::example().expect("example");
        let json = serde_json::to_string(&opt).expect("serialize");
        let deser: CmsSpreadOption = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.id, opt.id);
        assert_eq!(deser.strike, opt.strike);
    }

    #[test]
    fn static_replication_pricer_returns_positive_price() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.30);
        let mut opt = CmsSpreadOption::example().expect("example");
        opt.expiry = date(2026, Month::January, 1);
        opt.payment_date = date(2026, Month::January, 5);
        opt.strike = Decimal::new(5, 3);
        opt.correlation = 0.50;

        let amount = price_amount(&opt, &market, as_of);
        let via_value = opt.value(&market, as_of).expect("direct value");

        assert!(amount > 0.0);
        assert!((via_value.amount() - amount).abs() < 1e-12);
    }

    #[test]
    fn lower_correlation_increases_curve_spread_option_value() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.35);
        let mut low_corr = CmsSpreadOption::example().expect("example");
        low_corr.expiry = date(2026, Month::January, 1);
        low_corr.payment_date = date(2026, Month::January, 5);
        low_corr.strike = Decimal::new(10, 3);
        low_corr.correlation = 0.0;

        let mut high_corr = low_corr.clone();
        high_corr.correlation = 0.95;

        let low_corr_value = price_amount(&low_corr, &market, as_of);
        let high_corr_value = price_amount(&high_corr, &market, as_of);

        assert!(low_corr_value > high_corr_value);
    }

    #[test]
    fn higher_flat_volatility_increases_option_value() {
        let as_of = date(2025, Month::January, 1);
        let mut opt = CmsSpreadOption::example().expect("example");
        opt.expiry = date(2026, Month::January, 1);
        opt.payment_date = date(2026, Month::January, 5);
        opt.strike = Decimal::new(10, 3);
        opt.correlation = 0.50;

        let low_vol = price_amount(&opt, &market(as_of, 0.15), as_of);
        let high_vol = price_amount(&opt, &market(as_of, 0.60), as_of);

        assert!(high_vol > low_vol);
    }

    /// Regression test (item 2): the underlying-swap conventions must be
    /// plumbed through, not hard-coded to USD (semi/30360 fixed,
    /// quarterly/Act360 float).
    ///
    /// `resolve_leg` previously hard-coded `Tenor::semi_annual()` /
    /// `DayCount::Thirty360` / `Tenor::quarterly()` / `DayCount::Act360`. A
    /// `EurEstr` CMS spread has an *annual* fixed leg, so its forward swap
    /// rate is projected on a different annuity. This test verifies the
    /// resolved conventions pick up the instrument's `index_id`.
    #[test]
    fn swap_conventions_resolve_from_index_id() {
        use finstack_quant_core::dates::{Tenor, TenorUnit};

        let mut opt = CmsSpreadOption::example().expect("example");
        opt.index_id = None;
        assert!(opt.reference_swap().resolved_fixed_frequency().is_err());

        // EUR convention -> annual fixed leg (the case the hard-coded path got wrong).
        opt.index_id = Some(IndexId::new("EUR-ESTR-OIS"));
        assert_eq!(
            opt.reference_swap()
                .resolved_fixed_frequency()
                .expect("registry"),
            Tenor::annual(),
            "EUR CMS swap fixed leg must be annual, not the hard-coded semi-annual"
        );

        // Explicit per-field override beats the convention.
        opt.swap_fixed_frequency =
            Some(Tenor::new(3, TenorUnit::Months).expect("valid tenor fixture"));
        assert_eq!(
            opt.reference_swap()
                .resolved_fixed_frequency()
                .expect("registry"),
            Tenor::new(3, TenorUnit::Months).expect("valid tenor fixture")
        );
    }

    /// A CMS spread must use its index's complete reference-swap conventions,
    /// including calendars and settlement lag, for each projected forward.
    #[test]
    fn non_usd_convention_changes_price() {
        let as_of = date(2025, Month::January, 1);
        let market = market(as_of, 0.35);

        let mut usd = CmsSpreadOption::example().expect("example");
        // Juneteenth closes the USD calendar on June 19, 2026;
        // TARGET remains open. Identical annual ACT/360 coupons otherwise
        // need not give different prices just because their index names differ.
        usd.expiry = date(2026, Month::June, 17);
        usd.payment_date = date(2026, Month::June, 24);
        usd.strike = Decimal::ZERO;
        usd.correlation = 0.50;
        usd.index_id = Some(IndexId::new("USD-SOFR-OIS"));

        let mut eur = usd.clone();
        eur.index_id = Some(IndexId::new("EUR-ESTR-OIS"));

        assert_ne!(
            usd.reference_swap()
                .reference_swap_start(usd.expiry)
                .expect("USD start"),
            eur.reference_swap()
                .reference_swap_start(eur.expiry)
                .expect("EUR start"),
        );
        let usd_value = price_amount(&usd, &market, as_of);
        let eur_value = price_amount(&eur, &market, as_of);

        assert!(usd_value > 0.0 && eur_value > 0.0);
        assert!(
            (usd_value - eur_value).abs() > 1e-9,
            "CMS spread price must depend on the underlying-swap convention \
             (annuity/day-count basis); USD and EUR priced identically: \
             usd={usd_value}, eur={eur_value}"
        );
    }
}
