//! Cliquet option instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::PricingOverrides;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId, PriceId};
use time::macros::date;

/// Cliquet option instrument.
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = CliquetOption::validate)]
#[serde(deny_unknown_fields, try_from = "CliquetOptionUnchecked")]
pub struct CliquetOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Underlying asset ticker symbol
    pub underlying_ticker: String,
    /// Reset dates for periodic return locking
    #[schemars(with = "Vec<String>")]
    pub reset_dates: Vec<Date>,
    /// Explicit terminal expiry date for the structure.
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Local cap on individual period returns
    pub local_cap: f64,
    /// Local floor on individual period returns (default 0.0)
    pub local_floor: f64,
    /// Global cap on sum of all period returns
    pub global_cap: f64,
    /// Global floor on sum of all period returns (default 0.0)
    pub global_floor: f64,
    /// Notional amount
    pub notional: Money,
    /// Day count convention
    pub day_count: finstack_core::dates::DayCount,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Spot price identifier
    pub spot_id: PriceId,
    /// Volatility surface ID
    pub vol_surface_id: CurveId,
    /// Optional dividend yield curve ID.
    ///
    /// `Some(id)`: lookup MUST succeed (a missing or non-unitless scalar
    /// returns an error). `None`: no implicit default; treated as zero
    /// continuous dividend yield. Set explicitly for index underlyings.
    pub div_yield_id: Option<CurveId>,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
    /// Payoff aggregation type (default: Additive)
    #[builder(default)]
    #[serde(default)]
    pub payoff_type: CliquetPayoffType,
}

/// Cliquet payoff aggregation type.
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
pub enum CliquetPayoffType {
    /// Additive: Sum of period returns
    #[default]
    Additive,
    /// Multiplicative: Product of (1 + period returns) - 1
    Multiplicative,
}

/// Mirror of `CliquetOption` used by serde to apply `validate()` after
/// deserialization. Not part of the public API.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CliquetOptionUnchecked {
    id: InstrumentId,
    underlying_ticker: String,
    #[schemars(with = "Vec<String>")]
    reset_dates: Vec<Date>,
    #[schemars(with = "String")]
    expiry: Date,
    local_cap: f64,
    local_floor: f64,
    global_cap: f64,
    global_floor: f64,
    notional: Money,
    day_count: finstack_core::dates::DayCount,
    discount_curve_id: CurveId,
    spot_id: PriceId,
    vol_surface_id: CurveId,
    #[serde(default)]
    div_yield_id: Option<CurveId>,
    #[serde(default)]
    pricing_overrides: PricingOverrides,
    attributes: Attributes,
    #[serde(default)]
    payoff_type: CliquetPayoffType,
}

impl TryFrom<CliquetOptionUnchecked> for CliquetOption {
    type Error = finstack_core::Error;

    fn try_from(value: CliquetOptionUnchecked) -> std::result::Result<Self, Self::Error> {
        let opt = Self {
            id: value.id,
            underlying_ticker: value.underlying_ticker,
            reset_dates: value.reset_dates,
            expiry: value.expiry,
            local_cap: value.local_cap,
            local_floor: value.local_floor,
            global_cap: value.global_cap,
            global_floor: value.global_floor,
            notional: value.notional,
            day_count: value.day_count,
            discount_curve_id: value.discount_curve_id,
            spot_id: value.spot_id,
            vol_surface_id: value.vol_surface_id,
            div_yield_id: value.div_yield_id,
            pricing_overrides: value.pricing_overrides,
            attributes: value.attributes,
            payoff_type: value.payoff_type,
        };
        opt.validate()?;
        Ok(opt)
    }
}

// Implement CurveDependencies for DV01 calculator
impl crate::instruments::common_impl::traits::CurveDependencies for CliquetOption {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

impl CliquetOption {
    /// Validate structural invariants required by the pricing engine.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `reset_dates` is empty (the schedule needs at least one observation)
    /// - `reset_dates` are not strictly increasing
    /// - any reset date is strictly after `expiry`
    /// - `local_floor > local_cap` or `global_floor > global_cap`
    /// - `notional.amount()` is not finite
    pub fn validate(&self) -> finstack_core::Result<()> {
        if self.reset_dates.is_empty() {
            return Err(finstack_core::Error::Validation(
                "CliquetOption requires at least one reset_dates entry".into(),
            ));
        }
        for window in self.reset_dates.windows(2) {
            if window[0] >= window[1] {
                return Err(finstack_core::Error::Validation(format!(
                    "CliquetOption reset_dates must be strictly increasing; got {} >= {}",
                    window[0], window[1]
                )));
            }
        }
        // Safe: reset_dates is non-empty (checked above).
        if let Some(&last_reset) = self.reset_dates.last() {
            if last_reset > self.expiry {
                return Err(finstack_core::Error::Validation(format!(
                    "CliquetOption last reset date {} is after expiry {}",
                    last_reset, self.expiry
                )));
            }
        }
        if self.local_floor > self.local_cap {
            return Err(finstack_core::Error::Validation(format!(
                "CliquetOption local_floor ({}) must be <= local_cap ({})",
                self.local_floor, self.local_cap
            )));
        }
        if self.global_floor > self.global_cap {
            return Err(finstack_core::Error::Validation(format!(
                "CliquetOption global_floor ({}) must be <= global_cap ({})",
                self.global_floor, self.global_cap
            )));
        }
        if !self.notional.amount().is_finite() {
            return Err(finstack_core::Error::Validation(
                "CliquetOption notional amount must be finite".into(),
            ));
        }
        Ok(())
    }

    /// Create a canonical example cliquet option (quarterly resets with local/global caps).
    pub fn example() -> finstack_core::Result<Self> {
        use finstack_core::currency::Currency;
        use finstack_core::dates::DayCount;
        let reset_dates = vec![
            date!(2024 - 03 - 29),
            date!(2024 - 06 - 28),
            date!(2024 - 09 - 30),
            date!(2024 - 12 - 31),
        ];
        CliquetOption::builder()
            .id(InstrumentId::new("CLIQ-SPX-QTR"))
            .underlying_ticker("SPX".to_string())
            .reset_dates(reset_dates)
            .expiry(date!(2024 - 12 - 31))
            .local_cap(0.05) // 5% per period
            .local_floor(0.0) // 0% per period (min)
            .global_cap(0.20) // 20% max cumulative
            .global_floor(0.0) // 0% min cumulative
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
    }
}

impl crate::instruments::common_impl::traits::Instrument for CliquetOption {
    impl_instrument_base!(crate::pricer::InstrumentType::CliquetOption);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::MonteCarloGBM
    }

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        crate::instruments::common_impl::dependencies::MarketDependencies::from_curves_and_equity(
            self,
        )
    }

    fn base_value(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        use crate::instruments::equity::cliquet_option::pricer;
        pricer::compute_pv(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        self.reset_dates.first().copied()
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.expiry)
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

crate::impl_empty_cashflow_provider!(
    CliquetOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod validation_tests {
    use super::*;
    use crate::instruments::Attributes;
    use crate::metrics::HasExpiry;
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;

    #[test]
    fn builder_rejects_empty_reset_dates() {
        let result = CliquetOption::builder()
            .id(InstrumentId::new("CLIQ-EMPTY"))
            .underlying_ticker("SPX".to_string())
            .reset_dates(vec![])
            .expiry(date!(2024 - 12 - 31))
            .local_cap(0.05)
            .local_floor(0.0)
            .global_cap(0.20)
            .global_floor(0.0)
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build();
        assert!(result.is_err(), "empty reset_dates must be rejected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("reset_dates"),
            "error message should mention reset_dates: {}",
            msg
        );
    }

    #[test]
    fn builder_rejects_reset_dates_after_expiry() {
        let result = CliquetOption::builder()
            .id(InstrumentId::new("CLIQ-PAST-EXPIRY"))
            .underlying_ticker("SPX".to_string())
            .reset_dates(vec![date!(2025 - 01 - 01)])
            .expiry(date!(2024 - 12 - 31))
            .local_cap(0.05)
            .local_floor(0.0)
            .global_cap(0.20)
            .global_floor(0.0)
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build();
        assert!(result.is_err(), "reset_dates after expiry must be rejected");
    }

    #[test]
    fn builder_rejects_unsorted_reset_dates() {
        let result = CliquetOption::builder()
            .id(InstrumentId::new("CLIQ-UNSORTED"))
            .underlying_ticker("SPX".to_string())
            .reset_dates(vec![date!(2024 - 06 - 30), date!(2024 - 03 - 30)])
            .expiry(date!(2024 - 12 - 31))
            .local_cap(0.05)
            .local_floor(0.0)
            .global_cap(0.20)
            .global_floor(0.0)
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build();
        assert!(result.is_err(), "unsorted reset_dates must be rejected");
    }

    #[test]
    fn has_expiry_returns_explicit_expiry_field() {
        // After construction the explicit expiry, not the last reset date,
        // is the contract maturity.
        let opt = CliquetOption::example().expect("example builds");
        assert_eq!(HasExpiry::expiry(&opt), opt.expiry);
    }

    #[test]
    fn has_expiry_does_not_panic_on_empty_reset_dates() {
        // Construct via builder (validated) then mutate to simulate corrupted
        // state from an unsanitised JSON path. expiry() must not panic.
        let mut opt = CliquetOption::example().expect("example builds");
        opt.reset_dates.clear();
        let _ = HasExpiry::expiry(&opt);
    }
}
