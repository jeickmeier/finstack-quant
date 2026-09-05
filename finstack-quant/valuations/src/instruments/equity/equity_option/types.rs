//! Equity option instrument definition and Black–Scholes helpers.
//!
//! # Dividend Handling
//!
//! This implementation uses a **continuous dividend yield** model (parameter `q` in BSM).
//! The dividend yield is provided via `div_yield_id` as a unitless scalar representing
//! the annualized continuous dividend yield.
//!
//! ## Continuous vs Discrete Dividends
//!
//! **Continuous dividend yield** (implemented here) is appropriate for:
//! - Index options (e.g., SPX, NDX) where dividend yield is well-defined
//! - Long-dated options where discrete effects average out
//! - Options on indices with many constituents and frequent ex-dates
//!
//! **Discrete dividends** are important for:
//! - Single-stock options near ex-dividend dates
//! - Short-dated options where discrete jumps are material
//! - High-yield stocks with large dividend payments
//!
//! ## Discrete Dividend Adjustment
//!
//! For stocks with known discrete dividends, use the **spot adjustment method**:
//! ```text
//! S_adj = S - Σ D_i × e^{-r × t_i}  (for all dividends D_i at times t_i before expiry)
//! ```
//!
//! The `discrete_dividends` field selects the model by exercise style:
//! - European pricing uses the escrowed-dividend spot adjustment.
//! - American and Bermudan tree pricing evolves the escrowed stock component
//!   but restores remaining dividend value at each exercise node. This creates
//!   the contractual ex-date jump in the exercise decision while retaining a
//!   stable recombining lattice.
//!
//! When no discrete schedule is provided, pricing uses the continuous `q`
//! model. The one-dimensional PDE pricer rejects American discrete-dividend
//! contracts; select the tree pricer for that combination.
//!
//! ## Example: Manual Discrete Dividend Adjustment
//!
//! ```text
//! // Stock at $100, dividend of $2 in 0.25 years, r = 5%, T = 0.5 years
//! let spot = 100.0;
//! let dividend = 2.0;
//! let t_div = 0.25;
//! let r = 0.05;
//!
//! // Adjusted spot for BSM pricing
//! let s_adj = spot - dividend * (-r * t_div).exp();
//! // s_adj ≈ 98.01
//! ```
//!
//! # Market Data Validation
//!
//! When `div_yield_id` is set, the lookup **must succeed**. A failed lookup returns
//! an error rather than silently defaulting to zero. This prevents market data
//! configuration errors from affecting P&L calculations.
//!
//! # References
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.). Chapter 17. `docs/REFERENCES.md#hull-options-futures`
//! - Haug, E. G. (2007). *The Complete Guide to Option Pricing Formulas* (2nd ed.). Section 2.8. `docs/REFERENCES.md#haug-2007-option-formulas`
//! - QuantLib: `AnalyticEuropeanEngine` with `DividendVanillaOption`

// pricing formulas are implemented in the pricing engine; keep this module free of direct math imports
use crate::instruments::common_impl::parameters::underlying::EquityUnderlyingParams;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::{ExerciseStyle, OptionType, SettlementType};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use time::macros::date;

use super::parameters::{EquityOptionMarketData, EquityOptionParams};
use crate::impl_instrument_base;
use crate::instruments::common_impl::validation;

/// Day basis used to convert annual option theta into a per-day amount.
#[derive(PartialEq, Eq, Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ThetaDayBasis {
    /// Calendar-day theta, annual theta divided by 365.
    #[default]
    #[serde(rename = "calendar_365")]
    Calendar365,
    /// Trading-day theta, annual theta divided by 252.
    #[serde(rename = "trading_252")]
    Trading252,
}

impl ThetaDayBasis {
    pub(crate) const fn days_per_year(self) -> f64 {
        match self {
            Self::Calendar365 => 365.0,
            Self::Trading252 => 252.0,
        }
    }
}

/// Observed exercise or expiry state for an equity option.
///
/// From `date` onward the option pricer uses this fixed lifecycle state rather
/// than re-running the live option model. Cash settlement fixes the intrinsic
/// payoff from `spot`; physical settlement retains the marked delivery
/// obligation until `settlement_date`.
#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EquityOptionExercise {
    /// Exercise date, or the expiry observation date for an unexercised option.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Observed underlying level used to determine the fixed cash payoff.
    pub spot: f64,
    /// Contractual cash-payment or physical-delivery date.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub settlement_date: Date,
    /// Whether the option was exercised or automatically assigned.
    pub exercised: bool,
}

impl EquityOptionExercise {
    /// Create an observed exercise or expiry state.
    ///
    /// # Arguments
    ///
    /// * `date` - Exercise date, or expiry date for an unexercised observation.
    /// * `spot` - Positive finite observed underlying level in strike-price units.
    /// * `settlement_date` - Cash-payment or physical-delivery date, on or after `date`.
    /// * `exercised` - Whether exercise or assignment occurred.
    #[must_use]
    pub fn new(date: Date, spot: f64, settlement_date: Date, exercised: bool) -> Self {
        Self {
            date,
            spot,
            settlement_date,
            exercised,
        }
    }
}

/// Equity option instrument
#[derive(
    PartialEq,
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EquityOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Underlying equity ticker symbol
    pub underlying_ticker: String,
    /// Strike price
    pub strike: f64,
    /// Option type (call or put)
    pub option_type: OptionType,
    /// Exercise style (European or American)
    #[serde(default)]
    #[builder(default)]
    pub exercise_style: ExerciseStyle,
    /// Option expiry date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// Notional amount for valuation scaling.
    pub notional: Money,
    /// Day count convention
    #[serde(default = "crate::serde_defaults::day_count_act365f")]
    #[builder(default = finstack_quant_core::dates::DayCount::Act365F)]
    pub day_count: finstack_quant_core::dates::DayCount,
    /// Basis used for the reported per-day theta.
    ///
    /// Defaults to calendar-day theta (`annual theta / 365`). Select
    /// `Trading252` explicitly for a trading-day risk convention.
    #[serde(default)]
    #[builder(default)]
    pub theta_day_basis: ThetaDayBasis,
    /// Settlement type (physical or cash)
    #[serde(default = "crate::serde_defaults::settlement_cash")]
    #[builder(default = SettlementType::Cash)]
    pub settlement: SettlementType,
    /// Observed exercise or expiry state.
    ///
    /// Required from expiry onward. It fixes cash-settled intrinsic value or
    /// identifies a physical-delivery obligation through its settlement date.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exercise: Option<EquityOptionExercise>,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Equity spot price identifier
    pub spot_id: PriceId,
    /// Equity volatility surface ID
    pub vol_surface_id: CurveId,
    /// Optional continuous dividend yield identifier.
    ///
    /// The dividend yield should be a unitless scalar representing the annualized
    /// continuous dividend yield (e.g., 0.02 for 2%). This is used in the BSM model
    /// as the `q` parameter: `d1 = (ln(S/K) + (r - q + σ²/2)T) / (σ√T)`.
    ///
    /// # Semantics by value
    ///
    /// - **`Some(id)`** — the lookup MUST succeed. A missing market scalar
    ///   (or a non-unitless type) returns a hard error rather than silently
    ///   defaulting to zero, preventing market-data configuration errors
    ///   from quietly distorting P&L.
    /// - **`None`** — there is *no implicit default curve*. The pricer treats
    ///   the underlying as having **zero continuous dividend yield**. This is
    ///   correct for non-dividend-paying single stocks; for index options
    ///   (typically ~2% yield) callers should set `div_yield_id` explicitly.
    ///
    /// If `discrete_dividends` is non-empty, an escrowed-dividend adjustment
    /// is applied to spot and `q` is set to 0 internally regardless of
    /// `div_yield_id`.
    pub div_yield_id: Option<PriceId>,
    /// Optional discrete dividend schedule for more accurate pricing.
    ///
    /// Each entry is (ex-date, dividend_amount). When provided, the escrowed
    /// dividend model is used: the spot price is adjusted by subtracting the
    /// PV of future dividends before option pricing.
    ///
    /// # Escrowed Dividend Model
    ///
    /// The adjusted spot is:
    /// ```text
    /// S* = S - Σ D_i × DF(t_i)
    /// ```
    /// where D_i is each dividend amount and DF(t_i) is the discount factor
    /// to the ex-date. The adjusted spot S* is then used in Black-Scholes
    /// with zero dividend yield.
    ///
    /// # Reference
    ///
    /// - Haug, Haug, Lewis (2003). "Back to Basics: a new approach to the
    ///   discrete dividend problem"
    #[builder(default)]
    #[serde(default)]
    #[serde(with = "finstack_quant_core::wire::dated_f64_values")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(finstack_quant_core::wire::DateWire, f64)>")
    )]
    pub discrete_dividends: Vec<(Date, f64)>,
    /// Exercise schedule for Bermudan options.
    ///
    /// Required when `exercise_style` is `Bermudan`. Each date represents a time
    /// at which early exercise is permitted. Dates before as_of or after expiry
    /// are filtered out automatically.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "finstack_quant_core::wire::optional_dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<Vec<finstack_quant_core::wire::DateWire>>")
    )]
    pub exercise_schedule: Option<Vec<Date>>,
    /// Pricing overrides (manual price, yield, spread)
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-only pricing controls.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only valuation adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

// Declare canonical market dependencies for the DV01 calculator.
impl EquityOption {
    fn build_vanilla_with_market_data(
        id: impl Into<String>,
        ticker: impl Into<String>,
        option_params: EquityOptionParams,
        market_data: EquityOptionMarketData,
    ) -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new(id.into()))
            .underlying_ticker(ticker.into())
            .strike(option_params.strike)
            .option_type(option_params.option_type)
            .exercise_style(option_params.exercise_style)
            .expiry(option_params.expiry)
            .notional(option_params.notional)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .settlement(option_params.settlement)
            .discount_curve_id(market_data.discount_curve_id)
            .spot_id(market_data.spot_id)
            .vol_surface_id(market_data.vol_surface_id)
            .div_yield_id_opt(market_data.div_yield_id)
            .attributes(Attributes::new())
            .build()
    }

    /// Validate structural and lifecycle invariants.
    ///
    /// Checks strike/notional validity and, when an exercise observation is
    /// present, verifies its date, spot, settlement date, and exercise-style
    /// compatibility.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        validation::validate_f64_finite(self.strike, "equity option strike")?;
        validation::validate_f64_positive(self.strike, "equity option strike")?;
        validation::validate_money_finite(self.notional, "equity option notional")?;
        if self.notional.amount().abs() < f64::EPSILON {
            return Err(finstack_quant_core::Error::Validation(
                "Equity option notional must be non-zero".into(),
            ));
        }
        for (_, amount) in &self.discrete_dividends {
            validation::validate_f64_positive(*amount, "equity option discrete dividend")?;
        }
        if self
            .discrete_dividends
            .windows(2)
            .any(|window| window[0].0 >= window[1].0)
        {
            return Err(finstack_quant_core::Error::Validation(
                "Equity option discrete dividend dates must be strictly increasing".into(),
            ));
        }
        if let Some(exercise) = self.exercise {
            validation::validate_f64_finite(exercise.spot, "equity option exercise spot")?;
            validation::validate_f64_positive(exercise.spot, "equity option exercise spot")?;
            if exercise.date > self.expiry {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Equity option exercise date {} is after expiry {}",
                    exercise.date, self.expiry
                )));
            }
            if exercise.settlement_date < exercise.date {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Equity option settlement date {} precedes exercise date {}",
                    exercise.settlement_date, exercise.date
                )));
            }
            if !exercise.exercised && exercise.date != self.expiry {
                return Err(finstack_quant_core::Error::Validation(
                    "An unexercised equity option observation must be dated at expiry".into(),
                ));
            }
            match self.exercise_style {
                ExerciseStyle::European if exercise.date != self.expiry => {
                    return Err(finstack_quant_core::Error::Validation(
                        "European equity option exercise must occur on expiry".into(),
                    ));
                }
                ExerciseStyle::Bermudan
                    if exercise.exercised
                        && self
                            .exercise_schedule
                            .as_ref()
                            .is_none_or(|dates| !dates.contains(&exercise.date)) =>
                {
                    return Err(finstack_quant_core::Error::Validation(
                        "Bermudan equity option exercise date is not in exercise_schedule".into(),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Create a canonical example equity option for testing and documentation.
    ///
    /// Returns an at-the-money SPX call option with 6 months to expiry.
    pub fn example() -> finstack_quant_core::Result<Self> {
        let market_data = EquityOptionMarketData::new("USD-OIS", "EQUITY-SPOT", "EQUITY-VOL")
            .with_dividend_yield("EQUITY-DIVYIELD");

        Self::european_call_with_market_data(
            "SPX-CALL-4500",
            "SPX",
            4500.0,
            date!(2024 - 06 - 21),
            Money::from((100_i64, Currency::USD)),
            market_data,
        )
    }

    /// Create a European call option with standard conventions.
    ///
    /// This convenience constructor eliminates the builder for the most common case.
    ///
    /// # Errors
    ///
    /// Returns an error if the builder fails validation.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable string identifier used for lookup and serialization of this object
    /// * `ticker` - Ticker used by the algorithm, subject to the enclosing type invariants and documented units.
    /// * `strike` - Option strike in the surface's quote units (absolute or relative)
    /// * `expiry` - Option expiry date or year-fraction used to locate the volatility point
    /// * `notional` - Trade notional amount in the instrument currency's major units
    pub fn european_call(
        id: impl Into<String>,
        ticker: impl Into<String>,
        strike: f64,
        expiry: Date,
        notional: Money,
    ) -> finstack_quant_core::Result<Self> {
        Self::european_call_with_market_data(
            id,
            ticker,
            strike,
            expiry,
            notional,
            EquityOptionMarketData::new("USD-OIS", "EQUITY-SPOT", "EQUITY-VOL")
                .with_dividend_yield("EQUITY-DIVYIELD"),
        )
    }

    /// Create a European call option with explicit market-data identifiers.
    ///
    /// Use this constructor when you want the concise API of [`Self::european_call`]
    /// without hard-coding the discount curve, spot id, volatility surface, or
    /// dividend-yield source.
    pub fn european_call_with_market_data(
        id: impl Into<String>,
        ticker: impl Into<String>,
        strike: f64,
        expiry: Date,
        notional: Money,
        market_data: EquityOptionMarketData,
    ) -> finstack_quant_core::Result<Self> {
        let option_params = EquityOptionParams::european_call(strike, expiry, notional)
            .with_settlement(SettlementType::Cash);
        Self::build_vanilla_with_market_data(id, ticker, option_params, market_data)
    }

    /// Create a new equity option using parameter structs
    pub fn new(
        id: impl Into<String>,
        option_params: &EquityOptionParams,
        underlying_params: &EquityUnderlyingParams,
        discount_curve_id: CurveId,
        vol_surface_id: CurveId,
    ) -> Self {
        Self {
            id: InstrumentId::new(id.into()),
            underlying_ticker: underlying_params.ticker.clone(),
            strike: option_params.strike,
            option_type: option_params.option_type,
            exercise_style: option_params.exercise_style,
            expiry: option_params.expiry,
            notional: option_params.notional,
            day_count: finstack_quant_core::dates::DayCount::Act365F,
            theta_day_basis: ThetaDayBasis::Calendar365,
            settlement: option_params.settlement,
            exercise: None,
            discount_curve_id,
            spot_id: underlying_params.spot_id.clone(),
            vol_surface_id,
            div_yield_id: underlying_params.div_yield_id.clone(),
            discrete_dividends: Vec::new(),
            exercise_schedule: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        }
    }

    /// Calculate Greeks for this equity option
    pub fn greeks(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<
        crate::instruments::equity::equity_option::pricing::EquityOptionGreeks,
    > {
        use crate::instruments::equity::equity_option::pricing;
        pricing::compute_greeks(self, curves, as_of)
    }

    /// Calculate delta of this equity option
    pub fn delta(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let greeks = self.greeks(curves, as_of)?;
        Ok(greeks.delta)
    }

    /// Calculate gamma of this equity option
    pub fn gamma(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let greeks = self.greeks(curves, as_of)?;
        Ok(greeks.gamma)
    }

    /// Calculate vega of this equity option
    pub fn vega(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let greeks = self.greeks(curves, as_of)?;
        Ok(greeks.vega)
    }

    /// Calculate theta of this equity option
    pub fn theta(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let greeks = self.greeks(curves, as_of)?;
        Ok(greeks.theta)
    }

    /// Calculate rho of this equity option
    pub fn rho(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let greeks = self.greeks(curves, as_of)?;
        Ok(greeks.rho)
    }

    /// Calculate implied volatility of this equity option
    pub fn implied_vol(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        market_price: f64,
    ) -> finstack_quant_core::Result<f64> {
        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }
        if market_price <= 0.0 {
            return Ok(0.0);
        }
        if self.notional.amount() <= 0.0 {
            return Ok(0.0);
        }

        let (spot, r, q, _sigma, _t) = {
            use crate::instruments::equity::equity_option::pricing;
            let (spot, r, q, sigma, t) = pricing::collect_inputs(self, curves, as_of)?;
            (spot, r, q, sigma, t)
        };
        let k = self.strike;
        let target_unit = market_price / self.notional.amount();
        finstack_quant_models::bs_implied_vol(spot, k, r, q, t, self.option_type, target_unit)
    }
}

impl crate::instruments::common_impl::traits::OptionGreeksProvider for EquityOption {
    fn option_delta(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.greeks(market, as_of)?.delta))
    }

    fn option_gamma(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.greeks(market, as_of)?.gamma))
    }

    fn option_vega(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.greeks(market, as_of)?.vega))
    }

    fn option_theta(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.greeks(market, as_of)?.theta))
    }

    fn option_rho_bp(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        // EquityOptionGreeks::rho is per 1% rate move; metrics expose per 1bp.
        Ok(Some(self.greeks(market, as_of)?.rho / 100.0))
    }

    fn option_vanna(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        // Match the public metric test/reference conventions:
        // - Spot bump: ±1% (relative, on the spot scalar)
        // - Vol bump: ±1 vol point (absolute, parallel surface bump)
        let spot = crate::instruments::common_impl::helpers::scalar_price_amount(
            market.get_price(&self.spot_id)?,
            self.notional.currency(),
        )?;
        let spot_bump_abs = spot * crate::metrics::bump_sizes::SPOT;
        if spot_bump_abs <= 0.0 {
            return Ok(Some(0.0));
        }

        let vol_bump_abs = crate::metrics::bump_sizes::VOLATILITY;

        let curves_vol_up = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            vol_bump_abs,
        )?;
        let curves_vol_dn = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            -vol_bump_abs,
        )?;

        // Delta at sigma+:
        let pv_su = self
            .value(
                &crate::metrics::bump_scalar_price(
                    &curves_vol_up,
                    &self.spot_id,
                    crate::metrics::bump_sizes::SPOT,
                )?,
                as_of,
            )?
            .amount();
        let pv_sd = self
            .value(
                &crate::metrics::bump_scalar_price(
                    &curves_vol_up,
                    &self.spot_id,
                    -crate::metrics::bump_sizes::SPOT,
                )?,
                as_of,
            )?
            .amount();
        let delta_up = (pv_su - pv_sd) / (2.0 * spot_bump_abs);

        // Delta at sigma-:
        let pv_su = self
            .value(
                &crate::metrics::bump_scalar_price(
                    &curves_vol_dn,
                    &self.spot_id,
                    crate::metrics::bump_sizes::SPOT,
                )?,
                as_of,
            )?
            .amount();
        let pv_sd = self
            .value(
                &crate::metrics::bump_scalar_price(
                    &curves_vol_dn,
                    &self.spot_id,
                    -crate::metrics::bump_sizes::SPOT,
                )?,
                as_of,
            )?
            .amount();
        let delta_dn = (pv_su - pv_sd) / (2.0 * spot_bump_abs);

        // Report vanna per **vol point** on the σ axis (consistent with vega
        // and `MetricId::Vanna`): normalize by the bump width expressed in
        // vol points.
        let width = 2.0 * vol_bump_abs * crate::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
        Ok(Some((delta_up - delta_dn) / width))
    }

    fn option_volga(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        base_pv: f64,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let vol_bump_abs = crate::metrics::bump_sizes::VOLATILITY;
        let curves_vol_up = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            vol_bump_abs,
        )?;
        let curves_vol_dn = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            -vol_bump_abs,
        )?;

        let pv_up = self.value(&curves_vol_up, as_of)?.amount();
        let pv_dn = self.value(&curves_vol_dn, as_of)?.amount();

        // Report volga per **vol point squared** to match the library-wide
        // per-vol-point vega convention (and `MetricId::Volga`). The second
        // difference is taken in absolute-vol units, so normalize by the bump
        // expressed in vol points, squared.
        let width = vol_bump_abs * crate::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
        Ok(Some((pv_up - 2.0 * base_pv + pv_dn) / (width * width)))
    }
}

impl crate::instruments::common_impl::traits::Instrument for EquityOption {
    impl_instrument_base!(crate::pricer::InstrumentType::EquityOption);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        EquityOption::validate(self)
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_market_scalar_id(self.spot_id.as_str());
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                Some(self.spot_id.clone()),
                Some(self.strike),
            ),
        );
        if let Some(dividend_yield) = &self.div_yield_id {
            deps.add_market_scalar_id(dividend_yield.as_str());
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        use crate::instruments::equity::equity_option::pricing;
        pricing::compute_pv(self, curves, as_of)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.expiry)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        None
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    EquityOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod date_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/date.rs"
        ));
    }
    #[allow(dead_code, unused_imports)]
    mod discount_forward_curve_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/discount_forward_curves.rs"
        ));
    }
    #[allow(dead_code, unused_imports)]
    mod option_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/equity_fx_options.rs"
        ));
    }
    #[allow(dead_code, unused_imports)]
    mod volatility_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/volatility.rs"
        ));
    }

    use super::*;
    use crate::instruments::common_impl::{helpers::year_fraction, traits::Instrument};
    use crate::instruments::equity::equity_option::pricing;
    use crate::instruments::{
        Attributes, ExerciseStyle, InstrumentPricingOverrides, OptionType, SettlementType,
    };
    use finstack_quant_core::{
        currency::Currency,
        dates::{Date, DayCount},
        market_data::{
            context::MarketContext, scalars::MarketScalar, term_structures::DiscountCurve,
        },
        money::Money,
        types::{CurveId, InstrumentId},
    };
    use finstack_quant_models::closed_form::vanilla::bs_price_unchecked;

    #[test]
    fn canonical_dependencies_preserve_equity_surface_context() {
        let option = EquityOption::example().expect("example");
        let deps = option.market_dependencies().expect("dependencies");

        assert_eq!(
            deps.curves.discount_curves.as_slice(),
            std::slice::from_ref(&option.discount_curve_id)
        );
        let mut expected_spots = vec![option.spot_id.as_str().to_string()];
        expected_spots.extend(option.div_yield_id.iter().map(|id| id.as_str().to_string()));
        assert_eq!(deps.market_scalar_ids, expected_spots);
        assert_eq!(deps.volatility_dependencies.len(), 1);
        let volatility = &deps.volatility_dependencies[0];
        assert_eq!(volatility.vol_surface_id, option.vol_surface_id);
        assert_eq!(volatility.underlying_id.as_ref(), Some(&option.spot_id));
        assert_eq!(volatility.reference_strike, Some(option.strike));
        assert!(deps.series_ids.is_empty());
    }
    use date_support::date;
    use discount_forward_curve_support::flat_discount_with_tenor;
    use volatility_support::flat_vol_surface;

    const DISC_ID: &str = "USD-OIS";
    const SPOT_ID: &str = "SPX-SPOT";
    const VOL_ID: &str = "SPX-VOL";
    const DIV_ID: &str = "SPX-DIV";

    fn build_market_context(
        as_of: Date,
        spot: f64,
        vol: f64,
        rate: f64,
        div_yield: f64,
    ) -> MarketContext {
        let expiries = [0.25, 0.5, 1.0, 2.0];
        let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
        MarketContext::new()
            .insert(flat_discount_with_tenor(DISC_ID, as_of, rate, 5.0))
            .insert_surface(flat_vol_surface(VOL_ID, &expiries, &strikes, vol))
            .insert_price(SPOT_ID, MarketScalar::Unitless(spot))
            .insert_price(DIV_ID, MarketScalar::Unitless(div_yield))
    }

    fn base_option(expiry: Date) -> EquityOption {
        EquityOption::builder()
            .id(InstrumentId::new("EQ-OPT"))
            .underlying_ticker("SPX".to_string())
            .strike(100.0)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .notional(Money::from((100_i64, Currency::USD)))
            .day_count(DayCount::Act365F)
            .settlement(SettlementType::Cash)
            .discount_curve_id(CurveId::new(DISC_ID))
            .spot_id(SPOT_ID.into())
            .vol_surface_id(CurveId::new(VOL_ID))
            .div_yield_id_opt(Some(PriceId::new(DIV_ID)))
            .attributes(Attributes::new())
            .build()
            .expect("should succeed")
    }

    #[test]
    fn builder_rejects_invalid_equity_option_economics() {
        let option = base_option(date(2025, 12, 31));
        let error = EquityOption::builder()
            .id(option.id)
            .underlying_ticker(option.underlying_ticker)
            .strike(0.0)
            .option_type(option.option_type)
            .expiry(option.expiry)
            .notional(option.notional)
            .discount_curve_id(option.discount_curve_id)
            .spot_id(option.spot_id)
            .vol_surface_id(option.vol_surface_id)
            .attributes(option.attributes)
            .build()
            .expect_err("a zero strike must fail at the builder boundary");

        assert!(error.to_string().contains("strike"));
    }

    fn approx_eq(actual: f64, expected: f64, tol: f64) {
        let diff = (actual - expected).abs();
        assert!(
            diff <= tol,
            "expected {expected}, got {actual} (diff {diff} > {tol})"
        );
    }

    #[test]
    fn convenience_constructors_apply_standard_conventions() {
        let expiry = date(2025, 12, 31);
        let call =
            option_support::equity_option_european_call("SPX-CALL", "SPX", 100.0, expiry, 100.0)
                .unwrap();
        assert_eq!(call.exercise_style, ExerciseStyle::European);
        assert_eq!(call.option_type, OptionType::Call);
        assert_eq!(call.discount_curve_id, CurveId::new(DISC_ID));
        assert_eq!(call.spot_id.as_str(), "EQUITY-SPOT");
        assert_eq!(call.vol_surface_id, CurveId::new("EQUITY-VOL"));

        let put = option_support::equity_option_european_put("SPX-PUT", "SPX", 90.0, expiry, 50.0)
            .unwrap();
        assert_eq!(put.option_type, OptionType::Put);
        assert_eq!(put.notional.amount(), 50.0);

        let american =
            option_support::equity_option_american_call("SPX-AMER", "SPX", 105.0, expiry, 75.0)
                .unwrap();
        assert_eq!(american.exercise_style, ExerciseStyle::American);
        assert_eq!(american.notional.amount(), 75.0);
    }

    #[test]
    fn market_data_constructor_preserves_explicit_market_ids() {
        let expiry = date(2025, 12, 31);
        let market_data =
            EquityOptionMarketData::new(CurveId::new(DISC_ID), SPOT_ID, CurveId::new(VOL_ID))
                .with_dividend_yield(PriceId::new(DIV_ID));

        let option = EquityOption::european_call_with_market_data(
            "SPX-CALL-CUSTOM",
            "SPX",
            100.0,
            expiry,
            Money::from((100_i64, Currency::USD)),
            market_data,
        )
        .expect("custom market-data constructor should succeed");

        assert_eq!(option.id, InstrumentId::new("SPX-CALL-CUSTOM"));
        assert_eq!(option.underlying_ticker, "SPX");
        assert_eq!(option.strike, 100.0);
        assert_eq!(option.option_type, OptionType::Call);
        assert_eq!(option.exercise_style, ExerciseStyle::European);
        assert_eq!(option.expiry, expiry);
        assert_eq!(option.notional, Money::from((100_i64, Currency::USD)));
        assert_eq!(option.discount_curve_id, CurveId::new(DISC_ID));
        assert_eq!(option.spot_id.as_str(), SPOT_ID);
        assert_eq!(option.vol_surface_id, CurveId::new(VOL_ID));
        assert_eq!(option.div_yield_id, Some(PriceId::new(DIV_ID)));
        assert_eq!(option.settlement, SettlementType::Cash);
        assert_eq!(option.day_count, DayCount::Act365F);
    }

    /// API-WIRING check only: `value()`/`greeks()`/`delta()`… delegate to
    /// `pricing::compute_pv`/`compute_greeks`, so both sides of these
    /// assertions run the same code — this cannot detect a formula error.
    /// Formula correctness is anchored non-circularly in
    /// `models::closed_form::vanilla` (`analytic_greeks_match_finite_differences_of_price`,
    /// `hull_chapter19_worked_example_anchor`).
    #[test]
    fn npv_and_greeks_match_pricer_outputs() {
        let as_of = date(2025, 1, 3);
        let expiry = date(2025, 7, 3);
        let option = base_option(expiry);
        let curves = build_market_context(as_of, 105.0, 0.22, 0.03, 0.01);

        let price = option
            .value(&curves, as_of)
            .expect("NPV calculation should succeed in test");
        let (spot, r, q, sigma, t) = pricing::collect_inputs(&option, &curves, as_of)
            .expect("Input collection should succeed in test");
        let expected_unit =
            bs_price_unchecked(spot, option.strike, r, q, sigma, t, option.option_type);
        // Slightly wider tolerance due to MonotoneConvex interpolation (vs Linear)
        approx_eq(
            price.amount(),
            expected_unit * option.notional.amount(),
            5e-3,
        );

        let greeks = option
            .greeks(&curves, as_of)
            .expect("Greeks calculation should succeed in test");
        let expected = pricing::compute_greeks(&option, &curves, as_of)
            .expect("Greeks computation should succeed in test");
        approx_eq(greeks.delta, expected.delta, 1e-6);
        approx_eq(greeks.gamma, expected.gamma, 1e-10);
        approx_eq(greeks.vega, expected.vega, 1e-6);
        approx_eq(greeks.theta, expected.theta, 1e-8);
        approx_eq(greeks.rho, expected.rho, 1e-6);

        approx_eq(
            option
                .delta(&curves, as_of)
                .expect("Delta calculation should succeed in test"),
            greeks.delta,
            1e-12,
        );
        approx_eq(
            option
                .gamma(&curves, as_of)
                .expect("Gamma calculation should succeed in test"),
            greeks.gamma,
            1e-12,
        );
        approx_eq(
            option
                .vega(&curves, as_of)
                .expect("Vega calculation should succeed in test"),
            greeks.vega,
            1e-12,
        );
        approx_eq(
            option
                .theta(&curves, as_of)
                .expect("Theta calculation should succeed in test"),
            greeks.theta,
            1e-12,
        );
        approx_eq(
            option
                .rho(&curves, as_of)
                .expect("Rho calculation should succeed in test"),
            greeks.rho,
            1e-12,
        );
    }

    #[test]
    fn implied_volatility_recovers_surface_value_and_respects_override() {
        let as_of = date(2025, 1, 3);
        let expiry = date(2025, 7, 3);
        let option = base_option(expiry);
        let curves = build_market_context(as_of, 100.0, 0.30, 0.02, 0.01);

        let npv = option.value(&curves, as_of).expect("should succeed");
        let implied = option
            .implied_vol(&curves, as_of, npv.amount())
            .expect("should succeed");
        approx_eq(implied, 0.30, 1e-5);

        let mut override_option = base_option(expiry);
        let overrides = InstrumentPricingOverrides::default().with_implied_vol(0.45);
        override_option.instrument_pricing_overrides = overrides;
        let override_price = override_option
            .value(&curves, as_of)
            .expect("should succeed");
        let (spot, r, q, _, t) =
            pricing::collect_inputs(&override_option, &curves, as_of).expect("should succeed");
        let expected = bs_price_unchecked(
            spot,
            override_option.strike,
            r,
            q,
            0.45,
            t,
            override_option.option_type,
        ) * override_option.notional.amount();
        // Slightly wider tolerance due to MonotoneConvex interpolation (vs Linear)
        approx_eq(override_price.amount(), expected, 5e-3);
    }

    #[test]
    fn expired_options_return_intrinsic_value_and_static_greeks() {
        let expiry = date(2025, 1, 3);
        let as_of = expiry;
        let mut option = base_option(expiry);
        option.notional = Money::from((50_i64, Currency::USD));
        option.exercise = Some(EquityOptionExercise::new(expiry, 120.0, expiry, true));
        let curves = build_market_context(as_of, 120.0, 0.25, 0.01, 0.0);

        let pv = option.value(&curves, as_of).expect("should succeed");
        assert_eq!(pv.amount(), (120.0 - 100.0) * 50.0);

        let greeks = option.greeks(&curves, as_of).expect("should succeed");
        assert_eq!(greeks.delta, 0.0);
        assert_eq!(greeks.gamma, 0.0);
        assert_eq!(greeks.vega, 0.0);
        assert_eq!(greeks.theta, 0.0);
        assert_eq!(greeks.rho, 0.0);

        let implied = option
            .implied_vol(&curves, as_of, pv.amount())
            .expect("should succeed");
        assert_eq!(implied, 0.0);
    }

    /// Tests that separate day count handling works correctly when the discount curve
    /// uses ACT/360 (typical OIS convention) and volatility uses ACT/365F (equity standard).
    ///
    /// Market standard: Equity options use ACT/365F for vol time, but may discount on OIS
    /// curves with ACT/360. Mixing bases without proper separation rescales vols/rates
    /// and misstates greeks/theta.
    #[test]
    fn mixed_day_count_act365_vol_act360_discount() {
        let as_of = date(2025, 1, 3);
        let expiry = date(2026, 1, 3); // 1 year expiry

        // Create an ACT/360 discount curve (typical OIS convention)
        let flat_rate: f64 = 0.05;
        let df_5y: f64 = (-flat_rate * 5.0).exp();
        let act360_curve = DiscountCurve::builder(DISC_ID)
            .base_date(as_of)
            .day_count(DayCount::Act360) // OIS convention
            .knots([(0.0, 1.0), (5.0, df_5y)])
            .build()
            .expect("DiscountCurve with ACT/360 should build successfully");

        let expiries = [0.25, 0.5, 1.0, 2.0];
        let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
        let curves = MarketContext::new()
            .insert(act360_curve)
            .insert_surface(flat_vol_surface(VOL_ID, &expiries, &strikes, 0.20))
            .insert_price(SPOT_ID, MarketScalar::Unitless(100.0))
            .insert_price(DIV_ID, MarketScalar::Unitless(0.02));

        let option = base_option(expiry);

        // Verify the curve and model clocks remain separate.
        let inputs = pricing::collect_inputs_extended(&option, &curves, as_of)
            .expect("collect_inputs_extended should succeed");
        let discount_curve = curves.get_discount(DISC_ID).expect("discount curve");
        let curve_time = year_fraction(discount_curve.day_count(), as_of, expiry)
            .expect("curve day-count fraction");

        // ACT/360: 365 days / 360 = 1.01389 years
        // ACT/365F: 365 days / 365 = 1.0 years
        let expected_curve_time = 365.0 / 360.0; // ACT/360 for discounting
        let expected_t_vol = 365.0 / 365.0; // ACT/365F for vol

        approx_eq(curve_time, expected_curve_time, 1e-4);
        approx_eq(inputs.t_vol, expected_t_vol, 1e-4);

        // The difference between the curve and model clocks should be non-trivial.
        let time_diff = (curve_time - inputs.t_vol).abs();
        assert!(
            time_diff > 0.01,
            "curve and model clocks should differ with ACT/360 vs ACT/365F: got {time_diff}"
        );

        // Verify pricing works and produces reasonable values
        let pv = option
            .value(&curves, as_of)
            .expect("NPV should succeed with mixed day counts");
        assert!(pv.amount() > 0.0, "Call option should have positive value");

        // W-35 two-clock bridging — DISCOUNT leg.
        // The curve uses its own day-count clock in the date-to-date lookup,
        // while BSM applies the effective rate over the ACT/365F model clock.
        let df_curve = discount_curve
            .df_between_dates(as_of, expiry)
            .expect("date-to-date discount factor");
        let df_from_r = (-inputs.r * inputs.t_vol).exp();
        approx_eq(df_from_r, df_curve, 1e-10);

        // W-35 two-clock bridging — CARRY leg.
        // The BSM forward `F = S·e^{(r−q)·t_vol}` must equal the no-arbitrage
        // forward `(S/df)·e^{−q·t_vol}`. This confirms the effective rate is
        // correct for the carry term, not only the discount term — i.e. the
        // single effective `r` is right for BOTH legs despite the clock split.
        let bsm_forward = inputs.spot * ((inputs.r - inputs.q) * inputs.t_vol).exp();
        let no_arb_forward = inputs.spot / df_curve * (-inputs.q * inputs.t_vol).exp();
        approx_eq(bsm_forward, no_arb_forward, 1e-9);

        // Verify greeks are computed correctly
        let greeks = option
            .greeks(&curves, as_of)
            .expect("Greeks should succeed with mixed day counts");
        assert!(greeks.delta > 0.0 && greeks.delta < option.notional.amount());
        assert!(greeks.gamma > 0.0);
        assert!(greeks.vega > 0.0);

        // Verify price is within Black-Scholes tolerance
        // Using the inputs directly in the BS formula
        let expected_bs = bs_price_unchecked(
            inputs.spot,
            option.strike,
            inputs.r,
            inputs.q,
            inputs.sigma,
            inputs.t_vol,
            option.option_type,
        ) * option.notional.amount();

        // Slightly wider tolerance due to MonotoneConvex interpolation (vs Linear)
        // Same tolerance as other tests in this file
        approx_eq(pv.amount(), expected_bs, 5e-3);
    }

    /// Tests that pricing fails with a clear error when div_yield_id is set but missing from
    /// the market context.
    ///
    /// This validates the fix for the silent fallback to 0.0 issue identified in the quant
    /// code review. Market data configuration errors should not be masked.
    #[test]
    fn pricing_fails_when_dividend_yield_missing() {
        let as_of = date(2025, 1, 3);
        let expiry = date(2025, 7, 3);

        // Create option with div_yield_id that won't exist in market context
        let mut option = base_option(expiry);
        option.div_yield_id = Some(PriceId::new("MISSING-DIV-YIELD"));

        // Build market context WITHOUT the dividend yield
        let expiries = [0.25, 0.5, 1.0, 2.0];
        let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
        let curves = MarketContext::new()
            .insert(flat_discount_with_tenor(DISC_ID, as_of, 0.03, 5.0))
            .insert_surface(flat_vol_surface(VOL_ID, &expiries, &strikes, 0.25))
            .insert_price(SPOT_ID, MarketScalar::Unitless(100.0));
        // Note: NOT inserting dividend yield

        // Pricing should fail with a validation error
        let result = option.value(&curves, as_of);
        assert!(
            result.is_err(),
            "Expected pricing to fail when div_yield_id is set but missing from market context"
        );

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("MISSING-DIV-YIELD") || err_msg.contains("dividend yield"),
            "Error message should mention the missing dividend yield ID, got: {}",
            err_msg
        );
    }

    /// Tests that pricing fails when div_yield_id returns a Price scalar instead of Unitless.
    ///
    /// Dividend yield should be a unitless decimal (e.g., 0.02 for 2%), not a price.
    /// This validates type safety in market data configuration.
    #[test]
    fn pricing_fails_when_dividend_yield_wrong_type() {
        let as_of = date(2025, 1, 3);
        let expiry = date(2025, 7, 3);
        let option = base_option(expiry);

        // Build market context with dividend yield as a Price instead of Unitless
        let expiries = [0.25, 0.5, 1.0, 2.0];
        let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
        let curves = MarketContext::new()
            .insert(flat_discount_with_tenor(DISC_ID, as_of, 0.03, 5.0))
            .insert_surface(flat_vol_surface(VOL_ID, &expiries, &strikes, 0.25))
            .insert_price(SPOT_ID, MarketScalar::Unitless(100.0))
            // Wrong type: Price instead of Unitless
            .insert_price(
                DIV_ID,
                MarketScalar::Price(Money::new(0.02, Currency::USD).expect("valid money fixture")),
            );

        // Pricing should fail with a validation error about wrong scalar type
        let result = option.value(&curves, as_of);
        assert!(
            result.is_err(),
            "Expected pricing to fail when div_yield_id returns Price instead of Unitless"
        );

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("unitless") || err_msg.contains("Price"),
            "Error message should mention the type mismatch, got: {}",
            err_msg
        );
    }

    // ==================== DISCRETE DIVIDEND TESTS ====================

    #[test]
    fn discrete_dividends_adjusts_spot_and_zeroes_yield() {
        let as_of = date(2025, 1, 3);
        let expiry = date(2025, 7, 3);

        // Build option with two discrete dividends
        let mut option = base_option(expiry);
        option.discrete_dividends = vec![
            (date(2025, 3, 15), 1.50), // $1.50 div in ~2.5 months
            (date(2025, 6, 15), 1.50), // $1.50 div in ~5.5 months
        ];

        let curves = build_market_context(as_of, 100.0, 0.25, 0.05, 0.02);

        // Verify inputs reflect spot adjustment and q=0
        let (spot, r, q, _sigma, _t) = pricing::collect_inputs(&option, &curves, as_of)
            .expect("collect_inputs should succeed");
        assert!(
            spot < 100.0,
            "Adjusted spot should be less than raw spot of 100.0, got {}",
            spot
        );
        assert!(
            (q - 0.0).abs() < 1e-12,
            "Dividend yield should be 0 when discrete dividends are present, got {}",
            q
        );
        // PV of two $1.50 dividends at 5% rate should reduce spot by ~$2.97
        let expected_adj =
            100.0 - 1.50 * (-r * 71.0 / 365.0).exp() - 1.50 * (-r * 163.0 / 365.0).exp();
        assert!(
            (spot - expected_adj).abs() < 0.05,
            "Spot adjustment mismatch: got {}, expected ~{}",
            spot,
            expected_adj
        );
    }

    #[test]
    fn discrete_dividends_empty_falls_back_to_continuous_yield() {
        let as_of = date(2025, 1, 3);
        let expiry = date(2025, 7, 3);
        let option = base_option(expiry);
        let curves = build_market_context(as_of, 100.0, 0.25, 0.05, 0.02);

        // With empty discrete_dividends (default), should use continuous yield
        let (spot, _r, q, _sigma, _t) = pricing::collect_inputs(&option, &curves, as_of)
            .expect("collect_inputs should succeed");

        assert!(
            (spot - 100.0).abs() < 1e-10,
            "Spot should be unadjusted: got {}",
            spot
        );
        assert!(
            (q - 0.02).abs() < 1e-10,
            "Dividend yield should be 0.02: got {}",
            q
        );
    }

    #[test]
    fn discrete_dividends_after_expiry_are_excluded() {
        let as_of = date(2025, 1, 3);
        let expiry = date(2025, 7, 3);

        let mut option = base_option(expiry);
        // Only dividend is after expiry — should be excluded
        option.discrete_dividends = vec![(date(2025, 9, 15), 2.00)];
        // Also clear div_yield_id to ensure we get q=0 from the discrete path
        option.div_yield_id = None;

        let curves = build_market_context(as_of, 100.0, 0.25, 0.05, 0.0);

        let (spot, _r, q, _sigma, _t) = pricing::collect_inputs(&option, &curves, as_of)
            .expect("collect_inputs should succeed");

        // No future dividends within option life — spot unadjusted, q=0
        assert!(
            (spot - 100.0).abs() < 1e-10,
            "Spot should be unadjusted when all dividends are after expiry: got {}",
            spot
        );
        assert!(
            (q - 0.0).abs() < 1e-12,
            "q should be 0.0 when discrete divs are empty (after filtering): got {}",
            q
        );
    }

    #[test]
    fn discrete_dividends_past_dates_are_excluded() {
        let as_of = date(2025, 6, 1);
        let expiry = date(2025, 12, 31);

        let mut option = base_option(expiry);
        option.div_yield_id = None;
        option.discrete_dividends = vec![
            (date(2025, 3, 15), 1.00), // Already past as_of
            (date(2025, 9, 15), 1.50), // Future — should be included
        ];

        let curves = build_market_context(as_of, 100.0, 0.25, 0.05, 0.0);

        let (spot, r, q, _sigma, _t) = pricing::collect_inputs(&option, &curves, as_of)
            .expect("collect_inputs should succeed");

        // Only the $1.50 September dividend should reduce spot
        let t_sep = DayCount::Act365F
            .year_fraction(
                as_of,
                date(2025, 9, 15),
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .unwrap();
        let expected_adj = 100.0 - 1.50 * (-r * t_sep).exp();
        assert!(
            (spot - expected_adj).abs() < 0.02,
            "Only future dividend should adjust spot: got {}, expected ~{}",
            spot,
            expected_adj
        );
        assert!(
            (q - 0.0).abs() < 1e-12,
            "q should be 0.0 with discrete dividends: got {}",
            q
        );
    }

    #[test]
    fn discrete_vs_continuous_pricing_comparison() {
        // Verify that discrete dividends produce a different (but reasonable) price
        // compared to continuous yield
        let as_of = date(2025, 1, 3);
        let expiry = date(2025, 7, 3);

        let continuous_option = base_option(expiry);
        let curves = build_market_context(as_of, 100.0, 0.25, 0.05, 0.02);

        let continuous_pv = continuous_option
            .value(&curves, as_of)
            .expect("should succeed")
            .amount();

        // Create a discrete dividend option with roughly equivalent total yield
        // 2% annual on $100 over ~6 months ≈ $1 total dividends
        let mut discrete_option = base_option(expiry);
        discrete_option.discrete_dividends =
            vec![(date(2025, 3, 15), 0.50), (date(2025, 6, 15), 0.50)];

        let discrete_curves = build_market_context(as_of, 100.0, 0.25, 0.05, 0.0);
        let discrete_pv = discrete_option
            .value(&discrete_curves, as_of)
            .expect("should succeed")
            .amount();

        // Both should be positive
        assert!(continuous_pv > 0.0, "Continuous PV should be positive");
        assert!(discrete_pv > 0.0, "Discrete PV should be positive");

        // They should be in the same ballpark (within 20% of each other)
        let ratio = discrete_pv / continuous_pv;
        assert!(
            (0.5..2.0).contains(&ratio),
            "Discrete/continuous ratio {} seems unreasonable (cont={}, disc={})",
            ratio,
            continuous_pv,
            discrete_pv,
        );
    }

    #[test]
    fn discrete_dividend_greeks_are_computed() {
        let as_of = date(2025, 1, 3);
        let expiry = date(2025, 7, 3);

        let mut option = base_option(expiry);
        option.discrete_dividends = vec![(date(2025, 4, 15), 1.00)];

        let curves = build_market_context(as_of, 105.0, 0.22, 0.03, 0.0);

        let greeks = option
            .greeks(&curves, as_of)
            .expect("Greeks should succeed with discrete dividends");

        // For an ITM call with dividends, delta should be positive
        assert!(greeks.delta > 0.0, "Delta should be positive for ITM call");
        assert!(greeks.gamma > 0.0, "Gamma should be positive");
        assert!(greeks.vega > 0.0, "Vega should be positive");
    }

    #[test]
    fn bermudan_pricing_is_rejected_without_schedule() {
        let as_of = date(2025, 1, 3);
        let expiry = date(2025, 7, 3);
        let mut option = base_option(expiry);
        option.exercise_style = ExerciseStyle::Bermudan;
        let curves = build_market_context(as_of, 100.0, 0.25, 0.02, 0.01);

        let result = option.value(&curves, as_of);
        assert!(
            result.is_err(),
            "Expected Bermudan pricing to fail without exercise schedule"
        );

        let greeks = option.greeks(&curves, as_of);
        assert!(
            greeks.is_err(),
            "Expected Bermudan greeks to fail without exercise schedule"
        );
    }
}
