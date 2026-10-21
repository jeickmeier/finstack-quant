//! Typed FX instrument classes and their fluent builders.
//!
//! Every FX class exposes `fromJson` / `toJson`, `price`, `metric` and
//! Greeks; `FxForward` and `FxOption` add one getter per public Rust field,
//! the `Instrument` trait surface, Rust presets and the `FinancialBuilder`
//! setters. A bare spec becomes a class through
//! `fromJson(instrumentEnvelopeFromSpec(type, spec))`.
//!
//! # Monte-Carlo determinism
//!
//! The `price` method accepts Monte-Carlo models for
//! path-dependent FX products (e.g. barrier / touch options). As with the
//! generic `priceInstrument` bindings, no explicit RNG-seed parameter is
//! exposed: the seed is an instrument-level concern. When an instrument's
//! `instrument_pricing_overrides.model_config.mc_seed_scenario` is `None`, the core MC pricers
//! derive a stable seed deterministically from the instrument ID, so repricing
//! the same instrument JSON is bit-reproducible. Callers needing a distinct
//! deterministic stream set that label inside the instrument JSON.

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use crate::utils::input::{from_js_json, js_f64, js_opt_string, js_string, json_text};
use crate::utils::to_js_err;
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

macro_rules! fx_class {
    ($rust_name:ident, $js_name:literal, $rust_ty:ty) => {
        instrument_class!(
            #[doc = concat!("Typed WASM wrapper for the Rust FX instrument ", $js_name, ".")]
            $rust_name,
            $js_name,
            $rust_ty
        );
        instrument_pricing!($rust_name, $js_name);
    };
}

/// Emit an FX option class: the `fx_class!` surface plus one method per
/// applicable Greek (through `greek_methods!`) and `greeks()`.
///
/// Each Greek entry is `(method, jsName, "metric_id", "summary doc", "@returns doc")`. The
/// method list is pinned against the Rust metric registry by
/// `tests::option_greek_methods_match_the_metric_registry`.
macro_rules! fx_option_class {
    (
        $rust_name:ident, $js_name:literal, $rust_ty:ty, $instrument_type:ident,
        [$(($method:ident, $js_method:ident, $metric:literal, $summary:literal, $returns:literal)),+ $(,)?]
    ) => {
        fx_class!($rust_name, $js_name, $rust_ty);

        impl $rust_name {
            /// Metric IDs of the Greek methods this class exposes, in
            /// `STANDARD_OPTION_GREEKS` order.
            #[cfg(test)]
            const GREEK_METHODS: &'static [&'static str] = &[$($metric),+];
            /// Rust instrument type whose registry decides which Greeks apply.
            #[cfg(test)]
            const INSTRUMENT_TYPE: finstack_quant_valuations::pricer::InstrumentType =
                finstack_quant_valuations::pricer::InstrumentType::$instrument_type;
        }

        // Plain-string doc attributes, not `///`: a `///` line passed through a
        // macro argument reaches wasm-bindgen as a raw string and keeps its
        // `r"` prefix in the generated declarations.
        greek_methods!($rust_name, $js_name, {
            $(
                #[doc = $summary]
                #[doc = " @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data."]
                #[doc = " @param as_of - ISO-8601 valuation date used to resolve date-dependent market data."]
                #[doc = " @param model - Optional pricing-model identifier; omit to use the instrument's default model."]
                #[doc = $returns]
                #[doc = ""]
                #[doc = " # Errors"]
                #[doc = ""]
                #[doc = " Throws a JavaScript exception if the instrument or market JSON,"]
                #[doc = " `asOf`, or `model` is invalid; required market data is missing;"]
                #[doc = " pricing fails; or the Greek is not produced by the selected model."]
                $method as $js_method => $metric,
            )+
        });

        #[wasm_bindgen(js_class = $js_name)]
        impl $rust_name {
            /// Compute every Greek that applies to this instrument as a JavaScript object.
            /// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
            /// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
            /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
            /// @returns Map of Greek name to value, with keys in the Rust
            /// `STANDARD_OPTION_GREEKS` order (`delta`, `gamma`, `vega`, `theta`, `rho`, …);
            /// only the Greeks that apply to the instrument are present.
            ///
            /// # Errors
            ///
            /// Throws a JavaScript exception if the instrument or market JSON,
            /// `asOf`, or `model` is invalid; required market data is missing;
            /// pricing fails; a returned Greek is non-finite; or the result cannot
            /// be converted to a JavaScript value.
            pub fn greeks(
                &self,
                market_json: JsValue,
                as_of: JsValue,
                model: Option<JsValue>,
            ) -> Result<JsValue, JsValue> {
                let market_json = json_text(&market_json, "marketJson")?;
                let as_of = js_string(&as_of, "asOf")?;
                let model = js_opt_string(model.as_ref(), "model")?;
                super::option_greeks(
                    Box::new(self.inner.clone()),
                    &market_json,
                    &as_of,
                    model.as_deref(),
                )
            }
        }
    };
}

/// An FX option class whose Greek methods start with spot delta, gamma, vega,
/// theta and domestic rho.
///
/// `$theta_returns` documents the class's theta; each `$extra` appends one
/// more Greek entry in the `fx_option_class!` form.
macro_rules! fx_option {
    (
        $rust_name:ident, $js_name:literal, $rust_ty:ty, $instrument_type:ident,
        $theta_returns:literal $(, $extra:tt)* $(,)?
    ) => {
        fx_option_class!(
            $rust_name, $js_name, $rust_ty, $instrument_type,
            [
                (delta, delta, "delta", "Spot delta of the option.", "@returns Cash delta `dPV/dS`: PV change per unit move in the spot rate, including notional scaling."),
                (gamma, gamma, "gamma", "Spot gamma of the option.", "@returns Cash gamma `d²PV/dS²`: PV per unit-spot squared, including notional scaling."),
                (vega, vega, "vega", "Vega of the option.", "@returns Cash vega: PV change for a 1 vol-point (0.01 absolute) move in implied volatility."),
                (theta, theta, "theta", "Theta of the option.", $theta_returns),
                (rho, rho, "rho", "Domestic rate rho of the option.", "@returns Domestic rho: PV change for a 1bp (0.0001) move in the domestic discount rate."),
                $($extra),*
            ]
        );
    };
}

use crate::api::valuations::pricing::parse_market;
use crate::utils::wire::{js_date, js_wire};
use finstack_quant_valuations::instruments;

fx_class!(JsFxSpot, "FxSpot", instruments::FxSpot);
fx_class!(JsFxForward, "FxForward", instruments::FxForward);
fx_class!(JsFxSwap, "FxSwap", instruments::FxSwap);
fx_class!(JsNdf, "Ndf", instruments::Ndf);
fx_class!(
    JsFxVarianceSwap,
    "FxVarianceSwap",
    instruments::FxVarianceSwap
);

/// An FX option class exposing every standard Greek: the `fx_option!` five
/// plus foreign rho, vanna and volga.
macro_rules! fx_option_with_all_greeks {
    ($rust_name:ident, $js_name:literal, $rust_ty:ty, $instrument_type:ident, $theta_returns:literal) => {
        fx_option!(
            $rust_name, $js_name, $rust_ty, $instrument_type, $theta_returns,
            (foreign_rho, foreignRho, "foreign_rho", "Foreign rate rho of the option.", "@returns Foreign rho: PV change for a 1bp (0.0001) move in the foreign discount rate."),
            (vanna, vanna, "vanna", "Vanna of the option.", "@returns Vanna `d²PV/(dS dσ)`: PV per unit spot per vol point (0.01 absolute vol)."),
            (volga, volga, "volga", "Volga of the option.", "@returns Volga `d²PV/dσ²`: PV per vol-point squared (0.01 absolute vol)."),
        );
    };
}

// `FxOption` registers the model (analytic) theta; `QuantoOption` keeps the
// universal theta-horizon P&L.
fx_option_with_all_greeks!(
    JsFxOption,
    "FxOption",
    instruments::FxOption,
    FxOption,
    "@returns Model theta per day: annual analytic theta divided by the `metric_pricing_overrides.theta_day_basis` day count (default 365 calendar days); not annualized."
);
fx_option_with_all_greeks!(
    JsQuantoOption,
    "QuantoOption",
    instruments::QuantoOption,
    QuantoOption,
    "@returns Theta: P&L over the theta horizon (default one day, capped at expiry) with spot held at its `asOf` level, so a roll into the monitoring window observes that spot; not annualized."
);
fx_option!(
    JsFxDigitalOption,
    "FxDigitalOption",
    instruments::FxDigitalOption,
    FxDigitalOption,
    "@returns Model theta per day on the `metric_pricing_overrides.theta_day_basis` day count (default 365 calendar days): analytic for cash-or-nothing payouts, a one-day finite-difference decay for asset-or-nothing; not annualized."
);
fx_option!(
    JsFxTouchOption,
    "FxTouchOption",
    instruments::FxTouchOption,
    FxTouchOption,
    "@returns Theta: P&L over the theta horizon (default one day, capped at expiry) with spot held at its `asOf` level, so a roll into the monitoring window observes that spot; not annualized."
);
fx_option!(
    JsFxBarrierOption,
    "FxBarrierOption",
    instruments::FxBarrierOption,
    FxBarrierOption,
    "@returns Theta: P&L over the theta horizon (default one day, capped at expiry) with spot held at its `asOf` level, so a roll into the monitoring window observes that spot; not annualized.",
    (vanna, vanna, "vanna", "Vanna of the option.", "@returns Vanna `d²PV/(dS dσ)`: PV per unit spot per vol point (0.01 absolute vol)."),
    (volga, volga, "volga", "Volga of the option.", "@returns Volga `d²PV/dσ²`: PV per vol-point squared (0.01 absolute vol)."),
);

fn parse_currency(
    value: &JsValue,
    label: &str,
) -> Result<finstack_quant_core::currency::Currency, JsValue> {
    js_string(value, label)?.parse().map_err(to_js_err)
}

#[wasm_bindgen(js_class = FxForward)]
impl JsFxForward {
    /// Market-standard spot lag in business days for a currency pair.
    ///
    /// Mirrors Rust `FxForward::standard_settlement_days`: T+1 for USD/CAD,
    /// USD/TRY and USD/RUB, T+2 otherwise.
    /// @param base - Base (foreign) currency ISO code.
    /// @param quote - Quote (domestic) currency ISO code.
    /// @returns Spot lag in business days (1 or 2).
    /// @throws Error - Throws with kind `validation` if a currency code is not ISO-4217.
    #[wasm_bindgen(js_name = standardSettlementDays)]
    pub fn standard_settlement_days(base: JsValue, quote: JsValue) -> Result<u32, JsValue> {
        Ok(instruments::FxForward::standard_settlement_days(
            parse_currency(&base, "base")?,
            parse_currency(&quote, "quote")?,
        ))
    }

    /// Build an at-market forward from a trade date and a standard FX tenor.
    ///
    /// Mirrors Rust `FxForward::from_trade_date`: spot is rolled from
    /// `tradeDate` by the spot lag (CLS-consistent pair roll), then `tenor` is
    /// added with the FX end-of-month rule. Omitted options take the Rust
    /// defaults: the pair's standard spot lag, Modified Following, weekends-only
    /// calendars and no end-of-month rule.
    /// @param id - Unique instrument identifier.
    /// @param baseCurrency - Base (foreign) currency ISO code; the notional currency.
    /// @param quoteCurrency - Quote (domestic) currency ISO code.
    /// @param tradeDate - Trade date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param tenor - Standard FX tenor from spot (e.g. `Tenor.parse("3M")`).
    /// @param notional - Notional in `baseCurrency`.
    /// @param domesticDiscountCurveId - Quote-currency discount curve identifier.
    /// @param foreignDiscountCurveId - Base-currency discount curve identifier.
    /// @param options - Optional `{ base_calendar_id?, quote_calendar_id?, settlement_days?, business_day_convention?, end_of_month? }`; `business_day_convention` uses the serde names (e.g. `"modified_following"`).
    /// @returns The validated forward without a contract rate; chain `withForwardPoints` / `withForwardPips` to fix it.
    /// @throws Error - Throws with kind `validation` if a currency, date or option is malformed, the currencies coincide, or the notional currency differs from `baseCurrency`; kind `not_found` if a calendar identifier is unknown.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = fromTradeDate)]
    pub fn from_trade_date(
        id: JsValue,
        base_currency: JsValue,
        quote_currency: JsValue,
        trade_date: JsValue,
        tenor: &crate::api::core::dates::JsTenor,
        notional: &crate::api::core::money::JsMoney,
        domestic_discount_curve_id: JsValue,
        foreign_discount_curve_id: JsValue,
        options: Option<JsValue>,
    ) -> Result<JsFxForward, JsValue> {
        let options: FromTradeDateOptions = match options.as_ref() {
            Some(value) if !value.is_undefined() && !value.is_null() => {
                from_js_json(value, "options")?
            }
            _ => FromTradeDateOptions::default(),
        };
        let inner = instruments::FxForward::from_trade_date(
            js_string(&id, "id")?,
            parse_currency(&base_currency, "baseCurrency")?,
            parse_currency(&quote_currency, "quoteCurrency")?,
            js_date(&trade_date, "tradeDate")?,
            tenor.inner,
            notional.inner,
            js_string(&domestic_discount_curve_id, "domesticDiscountCurveId")?,
            js_string(&foreign_discount_curve_id, "foreignDiscountCurveId")?,
            options.base_calendar_id,
            options.quote_calendar_id,
            options.settlement_days,
            options.business_day_convention,
            options.end_of_month,
        )
        .map_err(to_js_err)?;
        Ok(JsFxForward { inner })
    }

    /// Return a copy whose contract rate is `spotRate + forwardPoints`.
    ///
    /// Mirrors Rust `FxForward::with_forward_points`; forward points are in
    /// rate units (`0.0025` is 25 pips on EUR/USD). The receiver is unchanged.
    /// @param spotRate - Spot rate, quote currency per unit of base currency; must be positive.
    /// @param forwardPoints - Forward points in rate units, added to `spotRate`.
    /// @returns A new forward with the contract rate set.
    /// @throws Error - Throws with kind `validation` if `spotRate` is not positive and finite or the resulting rate is not positive.
    #[wasm_bindgen(js_name = withForwardPoints)]
    pub fn with_forward_points(
        &self,
        spot_rate: JsValue,
        forward_points: JsValue,
    ) -> Result<JsFxForward, JsValue> {
        self.inner
            .clone()
            .with_forward_points(
                js_f64(&spot_rate, "spotRate")?,
                js_f64(&forward_points, "forwardPoints")?,
            )
            .map(|inner| JsFxForward { inner })
            .map_err(to_js_err)
    }

    /// Return a copy whose contract rate is `spotRate + pips * pipSize`.
    ///
    /// Mirrors Rust `FxForward::with_forward_pips`; the pip size follows
    /// market convention (`0.01` for JPY/KRW/HUF pairs, `0.0001` otherwise).
    /// @param spotRate - Spot rate, quote currency per unit of base currency; must be positive.
    /// @param pips - Forward points quoted in pips.
    /// @returns A new forward with the contract rate set.
    /// @throws Error - Throws with kind `validation` if `pips` or `spotRate` is not finite or the resulting rate is not positive.
    #[wasm_bindgen(js_name = withForwardPips)]
    pub fn with_forward_pips(
        &self,
        spot_rate: JsValue,
        pips: JsValue,
    ) -> Result<JsFxForward, JsValue> {
        self.inner
            .clone()
            .with_forward_pips(js_f64(&spot_rate, "spotRate")?, js_f64(&pips, "pips")?)
            .map(|inner| JsFxForward { inner })
            .map_err(to_js_err)
    }
}

market_metrics!(JsFxForward, "FxForward", {
        /// Covered-interest-parity forward rate implied by the market.
        ///
        /// Mirrors Rust `FxForward::market_forward_rate`:
        /// `F = S × DF_foreign(T) / DF_domestic(T)` at the forward's maturity.
        /// @param marketJson - Serialized `MarketContext` carrying both discount curves and the FX spot (or an explicit `quoted_spot` on the instrument).
        /// @param asOf - Valuation date as an ISO-8601 string.
        /// @returns Forward rate, quote currency per unit of base currency.
        /// @throws Error - Throws with kind `not_found` if a discount curve or the FX spot is missing, and kind `validation` if the market JSON or `asOf` is malformed.
        market_forward_rate as marketForwardRate,
});

factories!(JsFxForward, "FxForward", instruments::FxForward, {
        /// Canonical example forward (mirrors Rust `FxForward::example`).
        ///
        /// A 6-month EUR/USD forward: EUR 1,000,000 at contract rate 1.12,
        /// maturity 2025-06-15, curves `USD-OIS` / `EUR-OIS`.
        /// @returns The example forward.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

/// Optional arguments of `FxForward.fromTradeDate`; omitted fields take the
/// Rust `FxForward::from_trade_date` defaults.
#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FromTradeDateOptions {
    #[serde(default)]
    base_calendar_id: Option<String>,
    #[serde(default)]
    quote_calendar_id: Option<String>,
    #[serde(default)]
    settlement_days: Option<u32>,
    #[serde(default)]
    business_day_convention: Option<finstack_quant_core::dates::BusinessDayConvention>,
    #[serde(default)]
    end_of_month: bool,
}

#[wasm_bindgen(js_class = FxOption)]
impl JsFxOption {
    /// Build a European FX option with currency-derived OIS curves.
    ///
    /// Mirrors Rust `FxOption::european`: discount curves are `<QUOTE>-OIS`
    /// (domestic) and `<BASE>-OIS` (foreign), with the EUR/USD and GBP/USD
    /// underlying presets when they apply.
    /// @param id - Unique instrument identifier.
    /// @param baseCurrency - Base (foreign) currency ISO code; the notional currency.
    /// @param quoteCurrency - Quote (domestic) currency ISO code.
    /// @param strike - Strike, quote currency per unit of base currency.
    /// @param expiry - Expiry date as an ISO-8601 string.
    /// @param notional - Notional in `baseCurrency`.
    /// @param volSurfaceId - FX volatility surface identifier.
    /// @param optionType - `"call"` or `"put"` on the base currency.
    /// @param deltaConventionKind - `"spot"`, `"forward"`, `"premium_adjusted_spot"` or `"premium_adjusted_forward"`.
    /// @param premiumCurrency - ISO code of the premium currency (base or quote).
    /// @param venue - Non-empty market venue or quoting-source identifier.
    /// @returns The validated option.
    /// @throws Error - Throws with kind `validation` if a code, date or enum is malformed, the currencies coincide, the premium currency is neither leg, the venue is blank, or the notional is not positive.
    #[allow(clippy::too_many_arguments)]
    pub fn european(
        id: JsValue,
        base_currency: JsValue,
        quote_currency: JsValue,
        strike: JsValue,
        expiry: JsValue,
        notional: &crate::api::core::money::JsMoney,
        vol_surface_id: JsValue,
        option_type: JsValue,
        delta_convention_kind: JsValue,
        premium_currency: JsValue,
        venue: JsValue,
    ) -> Result<JsFxOption, JsValue> {
        let delta_convention = instruments::fx::fx_option::FxDeltaConvention::new(
            js_string(&delta_convention_kind, "deltaConventionKind")?
                .parse()
                .map_err(to_js_err)?,
            parse_currency(&premium_currency, "premiumCurrency")?,
            js_string(&venue, "venue")?,
        )
        .map_err(to_js_err)?;
        let inner = instruments::FxOption::european(
            js_string(&id, "id")?,
            parse_currency(&base_currency, "baseCurrency")?,
            parse_currency(&quote_currency, "quoteCurrency")?,
            js_f64(&strike, "strike")?,
            js_date(&expiry, "expiry")?,
            notional.inner,
            js_string(&vol_surface_id, "volSurfaceId")?,
            js_string(&option_type, "optionType")?
                .parse()
                .map_err(to_js_err)?,
            delta_convention,
        )
        .map_err(to_js_err)?;
        Ok(JsFxOption { inner })
    }

    /// Implied volatility that reproduces `targetPrice`.
    ///
    /// Mirrors Rust `FxOption::implied_vol` (Garman–Kohlhagen inversion).
    /// @param marketJson - Serialized `MarketContext` carrying both discount curves and the FX spot.
    /// @param asOf - Valuation date as an ISO-8601 string.
    /// @param targetPrice - Observed option PV in quote currency, on the same scale as `price`.
    /// @returns Annualized lognormal volatility as a decimal (`0.10` = 10%).
    /// @throws Error - Throws with kind `not_found` if a curve or the spot is missing, kind `validation` if an input is malformed, and kind `computation` if the root search does not converge (price outside the no-arbitrage bounds).
    #[wasm_bindgen(js_name = impliedVol)]
    pub fn implied_vol(
        &self,
        market_json: JsValue,
        as_of: JsValue,
        target_price: JsValue,
    ) -> Result<f64, JsValue> {
        let market = parse_market(&market_json)?;
        let as_of = js_date(&as_of, "asOf")?;
        self.inner
            .implied_vol(&market, as_of, js_f64(&target_price, "targetPrice")?)
            .map_err(to_js_err)
    }
}

factories!(JsFxOption, "FxOption", instruments::FxOption, {
        /// Canonical example option (mirrors Rust `FxOption::example`).
        ///
        /// A EUR/USD call, strike 1.12, EUR 1,000,000, forward-delta convention
        /// with premium in USD; curves `USD-OIS` / `EUR-OIS`, surface `EURUSD-VOL`.
        /// @returns The example option.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations::metrics::{standard_registry, MetricId};
    use finstack_quant_valuations::pricer::{InstrumentType, STANDARD_OPTION_GREEKS};

    #[test]
    fn public_json_routes_validate_instrument_before_market_json() {
        assert!(
            crate::api::valuations::pricing::tests::not_a_market_request()
                .instrument("{}")
                .is_err()
        );
        assert!(super::super::option_greeks(
            Box::new(instruments::FxOption::example().expect("example")),
            "not-market-json",
            "not-a-date",
            Some("not-a-model"),
        )
        .is_err());
    }

    /// The Greeks the registry computes for `instrument_type`, in
    /// `STANDARD_OPTION_GREEKS` order.
    fn registry_greeks(instrument_type: InstrumentType) -> Vec<String> {
        let menu: Vec<MetricId> = STANDARD_OPTION_GREEKS
            .iter()
            .map(|metric| MetricId::parse_strict(metric).expect("standard greek"))
            .collect();
        standard_registry()
            .applicable_subset(&menu, instrument_type)
            .iter()
            .map(|id| id.as_str().to_string())
            .collect()
    }

    #[test]
    fn option_greek_methods_match_the_metric_registry() {
        let classes: [(&[&str], InstrumentType); 5] = [
            (JsFxOption::GREEK_METHODS, JsFxOption::INSTRUMENT_TYPE),
            (
                JsQuantoOption::GREEK_METHODS,
                JsQuantoOption::INSTRUMENT_TYPE,
            ),
            (
                JsFxDigitalOption::GREEK_METHODS,
                JsFxDigitalOption::INSTRUMENT_TYPE,
            ),
            (
                JsFxTouchOption::GREEK_METHODS,
                JsFxTouchOption::INSTRUMENT_TYPE,
            ),
            (
                JsFxBarrierOption::GREEK_METHODS,
                JsFxBarrierOption::INSTRUMENT_TYPE,
            ),
        ];
        for (methods, instrument_type) in classes {
            assert_eq!(
                methods.to_vec(),
                registry_greeks(instrument_type),
                "{instrument_type:?} Greek methods drifted from the metric registry"
            );
        }
    }
}

instrument_surface!(JsFxForward, "FxForward", JsFxForwardBuilder, unpriced);

getters!(JsFxForward, "FxForward", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Base (foreign) currency; the notional currency.
        /// @returns The base currency.
        base_currency as baseCurrency => currency(i.base_currency),
        /// Quote (domestic) currency; the PV currency.
        /// @returns The quote currency.
        quote_currency as quoteCurrency => currency(i.quote_currency),
        /// Maturity / settlement date.
        /// @returns The maturity date.
        maturity as maturity => date(i.maturity),
        /// Notional amount in the base currency.
        /// @returns Currency-tagged notional.
        notional as notional => money(i.notional),
        /// Contract forward rate (quote per base).
        /// @returns The rate, or `null` when at-market.
        contract_rate as contractRate => json(i.contract_rate),
        /// Domestic (quote-currency) discount curve identifier.
        /// @returns The curve id.
        domestic_discount_curve_id as domesticDiscountCurveId => text(i.domestic_discount_curve_id),
        /// Foreign (base-currency) discount curve identifier.
        /// @returns The curve id.
        foreign_discount_curve_id as foreignDiscountCurveId => text(i.foreign_discount_curve_id),
        /// Explicit spot override (quote per base).
        /// @returns The override, or `null` to use the FX matrix.
        quoted_spot as quotedSpot => json(i.quoted_spot),
        /// Base-currency holiday calendar identifier.
        /// @returns The calendar id, or `null`.
        base_calendar_id as baseCalendarId => opt_text(i.base_calendar_id),
        /// Quote-currency holiday calendar identifier.
        /// @returns The calendar id, or `null`.
        quote_calendar_id as quoteCalendarId => opt_text(i.quote_calendar_id),
        /// Expiry as seen by the pricer.
        /// @returns `null`: FX forwards carry no option expiry.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

common_getters!(JsFxForward, "FxForward");

builder_class!(
    /// Fluent builder for `FxForward`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    JsFxForwardBuilder,
    "FxForwardBuilder",
    finstack_quant_valuations::instruments::fx::fx_forward::FxForwardBuilder,
    JsFxForward,
    |b| b.build()
);

setters!(JsFxForwardBuilder, "FxForwardBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the FX forward.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the maturity/settlement date.
        /// @param value - Maturity/settlement date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity => date,
        /// Set the contract forward rate (quote per base).
        /// @param value - Contract forward rate; when never set the forward is valued at-market (zero PV at inception).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        contract_rate as contractRate => num,
        /// Set the domestic (quote currency) discount curve identifier.
        /// @param value - Domestic (quote currency) discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        domestic_discount_curve_id as domesticDiscountCurveId => id,
        /// Set the foreign (base currency) discount curve identifier.
        /// @param value - Foreign (base currency) discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        foreign_discount_curve_id as foreignDiscountCurveId => id,
        /// Set an explicit spot rate override (quote per base).
        /// @param value - Spot FX rate; when never set the spot is sourced from the market's FX matrix.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quoted_spot as quotedSpot => num,
        /// Set the base currency calendar identifier for business day adjustment.
        /// @param value - Base currency holiday calendar identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        base_calendar_id as baseCalendarId => id,
        /// Set the quote currency calendar identifier for business day adjustment.
        /// @param value - Quote currency holiday calendar identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quote_calendar_id as quoteCalendarId => id,
});

common_setters!(JsFxForwardBuilder, "FxForwardBuilder");

handle_setters!(JsFxForwardBuilder, "FxForwardBuilder", {
        /// Set the base currency (foreign currency, numerator of the pair).
        /// @param value - Base currency (foreign currency, numerator of the pair), as a `Currency` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        base_currency as baseCurrency => JsCurrency,
        /// Set the quote currency (domestic currency, denominator of the pair).
        /// @param value - Quote currency (domestic currency, denominator of the pair), as a `Currency` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        quote_currency as quoteCurrency => JsCurrency,
        /// Set the notional amount in base currency.
        /// @param value - Notional amount in base currency, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

instrument_surface!(JsFxOption, "FxOption", JsFxOptionBuilder, unpriced);

getters!(JsFxOption, "FxOption", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Base (foreign) currency; the notional currency.
        /// @returns The base currency.
        base_currency as baseCurrency => currency(i.base_currency),
        /// Quote (domestic) currency.
        /// @returns The quote currency.
        quote_currency as quoteCurrency => currency(i.quote_currency),
        /// Strike, quote currency per unit of base currency.
        /// @returns The strike rate.
        strike as strike => json(i.strike),
        /// Option type on the base currency.
        /// @returns `"call"` or `"put"`.
        option_type as optionType => json(i.option_type),
        /// Delta convention.
        /// @returns `{"kind", "premium_currency", "venue"}`.
        delta_convention as deltaConvention => json(i.delta_convention),
        /// Last date on which the option may be exercised.
        /// @returns Unadjusted calendar expiry date. Time to expiry used in pricing is measured from the valuation date to this date under the pricing day-count convention.
        expiry as expiry => date(i.expiry),
        /// Day count for the time-to-expiry year fraction.
        /// @returns Act/365F unless set otherwise.
        day_count as dayCount => day_count(i.day_count),
        /// Notional amount in the base currency.
        /// @returns Currency-tagged notional.
        notional as notional => money(i.notional),
        /// Domestic (quote-currency) discount curve identifier.
        /// @returns The curve id.
        domestic_discount_curve_id as domesticDiscountCurveId => text(i.domestic_discount_curve_id),
        /// Foreign (base-currency) discount curve identifier.
        /// @returns The curve id.
        foreign_discount_curve_id as foreignDiscountCurveId => text(i.foreign_discount_curve_id),
        /// FX volatility surface identifier.
        /// @returns The surface id.
        vol_surface_id as volSurfaceId => text(i.vol_surface_id),
});

common_getters!(JsFxOption, "FxOption");

builder_class!(
    /// Fluent builder for `FxOption`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    JsFxOptionBuilder,
    "FxOptionBuilder",
    finstack_quant_valuations::instruments::fx::fx_option::FxOptionBuilder,
    JsFxOption,
    |b| b.build()
);

setters!(JsFxOptionBuilder, "FxOptionBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the FX option.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the strike exchange rate (quote per base).
        /// @param value - Strike exchange rate, quote currency per unit of base currency.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        strike as strike => num,
        /// Set the option type: `"call"` or `"put"` on base currency.
        /// @param value - Option type of the FX option.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        option_type as optionType => en,
        /// Set the option expiry date.
        /// @param value - Option expiry date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        expiry as expiry => date,
        /// Set the domestic currency discount curve identifier.
        /// @param value - Domestic currency discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        domestic_discount_curve_id as domesticDiscountCurveId => id,
        /// Set the foreign currency discount curve identifier.
        /// @param value - Foreign currency discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        foreign_discount_curve_id as foreignDiscountCurveId => id,
        /// Set the FX volatility surface identifier.
        /// @param value - FX volatility surface identifier for option pricing.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId => id,
});

common_setters!(JsFxOptionBuilder, "FxOptionBuilder");

handle_setters!(JsFxOptionBuilder, "FxOptionBuilder", {
        /// Set the base currency (foreign currency).
        /// @param value - Base currency (foreign currency), as a `Currency` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        base_currency as baseCurrency => JsCurrency,
        /// Set the quote currency (domestic currency).
        /// @param value - Quote currency (domestic currency), as a `Currency` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        quote_currency as quoteCurrency => JsCurrency,
        /// Set the model day count for volatility, dividend carry and exercise times.
        /// @param value - Model day count for volatility, dividend carry and exercise times, as a `DayCount` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        day_count as dayCount => JsDayCount,
        /// Set the notional amount in base currency.
        /// @param value - Notional amount in base currency, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

#[wasm_bindgen(js_class = FxOptionBuilder)]
impl JsFxOptionBuilder {
    /// Set the delta convention used to read the volatility surface.
    ///
    /// Mirrors Rust `FxDeltaConvention::new`; the convention decides which
    /// delta (spot or forward, premium-adjusted or not) a surface pillar means.
    /// @param kind - Delta convention kind: `spot`, `forward`, `spot_premium_adjusted`, or `forward_premium_adjusted`.
    /// @param premium_currency - Currency the option premium is paid in.
    /// @param venue - Market or broker whose quoting convention this is (for example `"interbank"`); must not be blank.
    /// @returns The builder, for chaining.
    /// @throws Error - Throws with kind `invalid_type` if `kind` or `venue` is not a string, and kind `validation` if `kind` is unknown, `venue` is blank, or the builder was already consumed by `build()`.
    #[wasm_bindgen(js_name = deltaConvention)]
    pub fn delta_convention(
        &self,
        kind: JsValue,
        premium_currency: &JsCurrency,
        venue: JsValue,
    ) -> Result<JsFxOptionBuilder, JsValue> {
        let convention =
            finstack_quant_valuations::instruments::fx::fx_option::FxDeltaConvention::new(
                js_wire(&kind, "kind")?,
                premium_currency.inner,
                crate::utils::input::js_string(&venue, "venue")?,
            )
            .map_err(to_js_err)?;
        let inner = self.inner.apply(|b| b.delta_convention(convention))?;
        Ok(Self { inner })
    }
}
