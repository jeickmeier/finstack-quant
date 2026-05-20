//! FX touch option (American binary option) instrument definition.

use super::pricer;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::PricingOverrides;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};

/// Touch type: one-touch (pays if barrier is hit) or no-touch (pays if not hit).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TouchType {
    /// Pays if the spot rate touches the barrier at any time before expiry.
    OneTouch,
    /// Pays if the spot rate does NOT touch the barrier before expiry.
    NoTouch,
}

impl std::fmt::Display for TouchType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OneTouch => write!(f, "one_touch"),
            Self::NoTouch => write!(f, "no_touch"),
        }
    }
}

impl std::str::FromStr for TouchType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "one_touch" | "onetouch" => Ok(Self::OneTouch),
            "no_touch" | "notouch" => Ok(Self::NoTouch),
            other => Err(format!(
                "Unknown touch type: '{}'. Valid: one_touch, no_touch",
                other
            )),
        }
    }
}

/// Barrier direction for touch options.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BarrierDirection {
    /// Barrier is above current spot (spot must rise to touch).
    Up,
    /// Barrier is below current spot (spot must fall to touch).
    Down,
}

impl std::fmt::Display for BarrierDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Up => write!(f, "up"),
            Self::Down => write!(f, "down"),
        }
    }
}

impl std::str::FromStr for BarrierDirection {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            other => Err(format!(
                "Unknown barrier direction: '{}'. Valid: up, down",
                other
            )),
        }
    }
}

/// Payout timing for touch options.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PayoutTiming {
    /// Payout occurs immediately when barrier is hit (for one-touch).
    AtHit,
    /// Payout is deferred to expiry regardless of when barrier is hit.
    AtExpiry,
}

impl std::fmt::Display for PayoutTiming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AtHit => write!(f, "at_hit"),
            Self::AtExpiry => write!(f, "at_expiry"),
        }
    }
}

impl std::str::FromStr for PayoutTiming {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "at_hit" | "athit" => Ok(Self::AtHit),
            "at_expiry" | "atexpiry" => Ok(Self::AtExpiry),
            other => Err(format!(
                "Unknown payout timing: '{}'. Valid: at_hit, at_expiry",
                other
            )),
        }
    }
}

/// FX touch option (American binary option).
///
/// Touch options pay a fixed amount if the spot rate touches a barrier
/// level at any time before expiry:
/// - One-touch: pays if barrier is touched
/// - No-touch: pays if barrier is NOT touched
///
/// # Pricing
///
/// Uses closed-form pricing for continuous monitoring (Rubinstein & Reiner 1991):
///
/// **Down-and-in one-touch (S > H, pay at expiry)**:
/// ```text
/// P = e^{-r_d T} × [(S/H)^{-(μ+λ)} × N(η·z) + (S/H)^{-(μ-λ)} × N(η·z')]
/// ```
///
/// where:
/// - μ = (r_d - r_f - σ²/2) / σ²
/// - λ = sqrt(μ² + 2r_d/σ²)
/// - z = ln(H/S)/(σ√T) + λσ√T
/// - z' = ln(H/S)/(σ√T) - λσ√T
/// - η = +1 for down barrier, -1 for up barrier
///
/// **No-touch**: P_no_touch = e^{-r_d T} × payout - P_one_touch
///
/// # References
///
/// - Rubinstein, M., & Reiner, E. (1991). "Unscrambling the Binary Code."
///   *Risk Magazine*, 4(9), 75-83.
/// - Wystup, U. (2006). *FX Options and Structured Products*. Wiley.
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct FxTouchOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Base currency (foreign currency)
    pub base_currency: Currency,
    /// Quote currency (domestic currency)
    pub quote_currency: Currency,
    /// Barrier level (exchange rate that triggers the touch)
    pub barrier_level: f64,
    /// Touch type (one-touch or no-touch)
    pub touch_type: TouchType,
    /// Barrier direction (up or down)
    pub barrier_direction: BarrierDirection,
    /// Fixed payout amount
    pub payout_amount: Money,
    /// Payout timing (at hit or at expiry)
    pub payout_timing: PayoutTiming,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Day count convention
    pub day_count: DayCount,
    /// Domestic currency discount curve ID
    pub domestic_discount_curve_id: CurveId,
    /// Foreign currency discount curve ID
    pub foreign_discount_curve_id: CurveId,
    /// FX volatility surface ID
    pub vol_surface_id: CurveId,
    /// Observed barrier event state for expired valuations.
    ///
    /// `Some(true)` means the barrier was touched during the option life,
    /// `Some(false)` means it was observed not to have touched, and `None`
    /// means the historical touch state is unavailable.
    ///
    /// This is only required once the option has expired. Without it, a
    /// touched-and-reverted path cannot be distinguished from an untouched path
    /// using the terminal spot alone.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_touch: Option<bool>,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

impl crate::instruments::common_impl::traits::CurveDependencies for FxTouchOption {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.domestic_discount_curve_id.clone())
            .discount(self.foreign_discount_curve_id.clone())
            .build()
    }
}

impl FxTouchOption {
    /// Create a canonical example FX touch option expiring on the
    /// project-wide stable example epoch.
    pub fn example() -> finstack_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("FXTOUCH-EURUSD-OT"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .barrier_level(1.05)
            .touch_type(TouchType::OneTouch)
            .barrier_direction(BarrierDirection::Down)
            .payout_amount(Money::new(1_000_000.0, Currency::USD))
            .payout_timing(PayoutTiming::AtExpiry)
            .expiry(crate::instruments::common_impl::example_constants::FAR_EXPIRY)
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
    }

    fn price_internal(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<Money> {
        pricer::compute_pv(self, market, as_of)
    }
}

impl crate::instruments::common_impl::traits::Instrument for FxTouchOption {
    impl_instrument_base!(crate::pricer::InstrumentType::FxTouchOption);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn base_value(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        self.price_internal(curves, as_of)
    }

    fn expiry(&self) -> Option<finstack_core::dates::Date> {
        Some(self.expiry)
    }

    fn effective_start_date(&self) -> Option<finstack_core::dates::Date> {
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

// Touch options use finite-difference Greeks (barrier discontinuities make
// analytical Greeks unreliable near the barrier).

impl crate::instruments::common_impl::traits::OptionGreeksProvider for FxTouchOption {
    fn option_delta(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        // Use FX spot bump via FX matrix
        let fx_matrix = market.fx().ok_or(finstack_core::Error::from(
            finstack_core::InputError::NotFound {
                id: "fx_matrix".to_string(),
            },
        ))?;
        let current_spot = fx_matrix
            .rate(finstack_core::money::fx::FxQuery::new(
                self.base_currency,
                self.quote_currency,
                as_of,
            ))?
            .rate;
        let bump_size = current_spot * crate::metrics::bump_sizes::SPOT;
        if bump_size <= 0.0 {
            return Ok(Some(0.0));
        }

        // Central-difference spot bump. Each scenario is a *fresh* `FxMatrix`
        // wrapping the same provider `Arc`. This is safe: `FxMatrix::set_quote`
        // writes a matrix-local explicit quote (its own LRU), and `FxMatrix`
        // never mutates the underlying provider — the explicit quote shadows
        // the provider on lookup. The base market's matrix is therefore
        // unaffected by the bumps. (Bumping must NOT go through
        // `SimpleFxProvider::set_quote`, which would mutate shared state.)
        let up_fx = {
            let fx_up = finstack_core::money::fx::FxMatrix::new(fx_matrix.provider().clone());
            fx_up.set_quote(
                self.base_currency,
                self.quote_currency,
                current_spot * (1.0 + crate::metrics::bump_sizes::SPOT),
            )?;
            market.clone().insert_fx(fx_up)
        };
        let dn_fx = {
            let fx_dn = finstack_core::money::fx::FxMatrix::new(fx_matrix.provider().clone());
            fx_dn.set_quote(
                self.base_currency,
                self.quote_currency,
                current_spot * (1.0 - crate::metrics::bump_sizes::SPOT),
            )?;
            market.clone().insert_fx(fx_dn)
        };

        let pv_up = self.value(&up_fx, as_of)?.amount();
        let pv_dn = self.value(&dn_fx, as_of)?.amount();

        Ok(Some((pv_up - pv_dn) / (2.0 * bump_size)))
    }

    fn option_gamma(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();

        let fx_matrix = market.fx().ok_or(finstack_core::Error::from(
            finstack_core::InputError::NotFound {
                id: "fx_matrix".to_string(),
            },
        ))?;
        let current_spot = fx_matrix
            .rate(finstack_core::money::fx::FxQuery::new(
                self.base_currency,
                self.quote_currency,
                as_of,
            ))?
            .rate;
        let bump_size = current_spot * crate::metrics::bump_sizes::SPOT;
        if bump_size <= 0.0 {
            return Ok(Some(0.0));
        }

        // See `option_delta` for why reusing the provider `Arc` is safe:
        // `FxMatrix::set_quote` is matrix-local and never mutates the provider.
        let up_fx = {
            let fx_up = finstack_core::money::fx::FxMatrix::new(fx_matrix.provider().clone());
            fx_up.set_quote(
                self.base_currency,
                self.quote_currency,
                current_spot * (1.0 + crate::metrics::bump_sizes::SPOT),
            )?;
            market.clone().insert_fx(fx_up)
        };
        let dn_fx = {
            let fx_dn = finstack_core::money::fx::FxMatrix::new(fx_matrix.provider().clone());
            fx_dn.set_quote(
                self.base_currency,
                self.quote_currency,
                current_spot * (1.0 - crate::metrics::bump_sizes::SPOT),
            )?;
            market.clone().insert_fx(fx_dn)
        };

        let pv_up = self.value(&up_fx, as_of)?.amount();
        let pv_dn = self.value(&dn_fx, as_of)?.amount();

        Ok(Some(
            (pv_up - 2.0 * base_pv + pv_dn) / (bump_size * bump_size),
        ))
    }

    fn option_vega(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();
        let bumped = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            crate::metrics::bump_sizes::VOLATILITY,
        )?;
        let pv_bumped = self.value(&bumped, as_of)?.amount();
        Ok(Some(
            (pv_bumped - base_pv) / crate::metrics::bump_sizes::VOLATILITY,
        ))
    }

    fn option_rho_bp(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();
        let bump_bp = self.pricing_overrides.rho_bump_bp();
        let bumped = crate::metrics::bump_discount_curve_parallel(
            market,
            &self.domestic_discount_curve_id,
            bump_bp,
        )?;
        let pv_bumped = self.value(&bumped, as_of)?.amount();
        Ok(Some((pv_bumped - base_pv) / bump_bp))
    }
}

crate::impl_empty_cashflow_provider!(
    FxTouchOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn touch_type_fromstr_display_roundtrip() {
        fn assert_touch_type(label: &str, expected: TouchType) {
            assert!(matches!(TouchType::from_str(label), Ok(value) if value == expected));
        }

        let variants = [TouchType::OneTouch, TouchType::NoTouch];
        for v in variants {
            let s = v.to_string();
            let parsed = TouchType::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        // Test aliases
        assert_touch_type("onetouch", TouchType::OneTouch);
        assert_touch_type("notouch", TouchType::NoTouch);
        assert!(TouchType::from_str("invalid").is_err());
    }

    #[test]
    fn barrier_direction_fromstr_display_roundtrip() {
        let variants = [BarrierDirection::Up, BarrierDirection::Down];
        for v in variants {
            let s = v.to_string();
            let parsed = BarrierDirection::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        assert!(BarrierDirection::from_str("invalid").is_err());
    }

    #[test]
    fn payout_timing_fromstr_display_roundtrip() {
        fn assert_payout_timing(label: &str, expected: PayoutTiming) {
            assert!(matches!(PayoutTiming::from_str(label), Ok(value) if value == expected));
        }

        let variants = [PayoutTiming::AtHit, PayoutTiming::AtExpiry];
        for v in variants {
            let s = v.to_string();
            let parsed = PayoutTiming::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        // Test aliases
        assert_payout_timing("athit", PayoutTiming::AtHit);
        assert_payout_timing("atexpiry", PayoutTiming::AtExpiry);
        assert!(PayoutTiming::from_str("invalid").is_err());
    }

    /// Item 8 regression: the FD spot bumps in `option_delta`/`option_gamma`
    /// rebuild `FxMatrix` via `provider().clone()` (an `Arc` clone — the
    /// provider is shared). This test pins that the bumped scenarios do NOT
    /// corrupt the base market's FX spot: `FxMatrix::set_quote` writes a
    /// matrix-local explicit quote and `FxMatrix` never mutates the provider.
    #[test]
    fn touch_fd_greeks_do_not_corrupt_base_market_fx_spot() {
        use crate::instruments::common_impl::traits::{Instrument, OptionGreeksProvider};
        use finstack_core::currency::Currency;
        use finstack_core::dates::Date;
        use finstack_core::market_data::context::MarketContext;
        use finstack_core::market_data::surfaces::VolSurface;
        use finstack_core::market_data::term_structures::DiscountCurve;
        use finstack_core::money::fx::{FxMatrix, FxQuery, SimpleFxProvider};
        use std::sync::Arc;
        use time::Month;

        let as_of = Date::from_calendar_date(2025, Month::January, 2).expect("date");
        let expiry = Date::from_calendar_date(2026, Month::January, 2).expect("date");

        let usd = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.04_f64).exp())])
            .build()
            .expect("usd curve");
        let eur = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.02_f64).exp())])
            .build()
            .expect("eur curve");
        let surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[1.0])
            .strikes(&[1.05])
            .row(&[0.11])
            .build()
            .expect("vol surface");
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");
        let market = MarketContext::new()
            .insert(usd)
            .insert(eur)
            .insert_surface(surface)
            .insert_fx(FxMatrix::new(provider));

        let touch = FxTouchOption::builder()
            .id(InstrumentId::new("FXTOUCH-CORRUPT-CHECK"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .barrier_level(1.05)
            .touch_type(TouchType::OneTouch)
            .barrier_direction(BarrierDirection::Down)
            .payout_amount(Money::new(1_000_000.0, Currency::USD))
            .payout_timing(PayoutTiming::AtExpiry)
            .expiry(expiry)
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("touch option");

        let spot_before = market
            .fx()
            .expect("fx")
            .rate(FxQuery::new(Currency::EUR, Currency::USD, as_of))
            .expect("spot")
            .rate;

        // Running the FD gamma exercises the bump-and-rebuild path.
        let _gamma = OptionGreeksProvider::option_gamma(&touch, &market, as_of).expect("gamma");

        let spot_after = market
            .fx()
            .expect("fx")
            .rate(FxQuery::new(Currency::EUR, Currency::USD, as_of))
            .expect("spot")
            .rate;
        assert!(
            (spot_before - spot_after).abs() < 1e-15,
            "base market FX spot must be unchanged after FD bumps: \
             before={spot_before} after={spot_after}"
        );

        // The base PV must also be reproducible (unchanged) after the bumps.
        let pv1 = touch.value(&market, as_of).expect("pv1").amount();
        let pv2 = touch.value(&market, as_of).expect("pv2").amount();
        assert!(
            (pv1 - pv2).abs() < 1e-9,
            "base PV must be stable after FD Greeks ran: pv1={pv1} pv2={pv2}"
        );
    }
}
