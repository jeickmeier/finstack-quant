//! Commodity option instrument definition and pricing logic.

use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::{
    CommodityConvention, CommodityUnderlyingParams, OptionMarketParams,
};
use crate::instruments::common_impl::traits::{
    Attributes, CurveDependencies, Instrument, InstrumentCurves,
};
use crate::instruments::{ExerciseStyle, OptionType, PricingOverrides, SettlementType};
use crate::models::trees::binomial_tree::BinomialTree;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_core::Result;

// ---------------------------------------------------------------------------
// Monte Carlo types (feature-gated)
// ---------------------------------------------------------------------------

/// Monte Carlo configuration for commodity option pricing.
///
/// When provided, enables simulation-based pricing using the specified
/// stochastic model instead of the default Black-76 analytical formula.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CommodityMcParams {
    /// Pricing model to use for simulation.
    pub model: CommodityPricingModel,
    /// Number of Monte Carlo paths.
    pub n_paths: usize,
    /// Number of time steps per path.
    pub n_steps: usize,
    /// Optional random seed for reproducibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

/// Commodity option pricing model selection.
///
/// Determines the stochastic dynamics used for Monte Carlo simulation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum CommodityPricingModel {
    /// Analytical Black-76 (no MC needed; included for completeness).
    BlackScholes,
    /// Schwartz-Smith two-factor model.
    ///
    /// Models commodity prices as S(t) = exp(X(t) + Y(t)) where:
    /// - X(t): short-term mean-reverting deviation (OU process)
    /// - Y(t): long-term equilibrium trend (arithmetic Brownian motion)
    SchwartzSmith {
        /// Mean reversion speed for short-term component (kappa).
        kappa: f64,
        /// Volatility of short-term component (sigma_x).
        sigma_x: f64,
        /// Volatility of long-term component (sigma_y).
        sigma_y: f64,
        /// Correlation between short-term and long-term factors (rho_xy).
        rho_xy: f64,
        /// Drift of long-term trend (mu_y).
        mu_y: f64,
        /// Risk premium for short-term factor (lambda_x).
        ///
        /// Under risk-neutral measure: dX = -(kappa + lambda_x) X dt + sigma_x dW_X
        lambda_x: f64,
    },
}

/// Commodity option (option on commodity forward or spot).
///
/// # Pricing
///
/// - **European options**: Black-76 model using the forward price from the `PriceCurve`
/// - **American options**: Binomial tree (Leisen-Reimer) with cost-of-carry derived from
///   the forward/spot relationship
///
/// # American Option Assumptions
///
/// For American exercise, the model requires a spot price to build the binomial tree.
/// If `spot_id` is provided, it uses that spot price. Otherwise, the forward
/// price is used as a proxy for spot, which may underestimate early exercise value.
/// The convenience yield (cost-of-carry) is implied from the forward/spot ratio:
/// `q = r - ln(F/S)/T`
///
/// # Bermudan Exercise Status
///
/// Bermudan exercise is currently **rejected**. The prior American-style
/// approximation was removed because it materially overstates option value
/// for sparse exercise schedules.
///
/// # Forward Price Retrieval
///
/// Forward prices are retrieved from a `PriceCurve` (not a `ForwardCurve`).
/// The curve must be added via `MarketContext::insert_price_curve()`.
/// If `quoted_forward` is provided, it overrides the curve lookup.
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct CommodityOption {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Commodity underlying parameters (commodity_type, ticker, unit, currency).
    #[serde(flatten)]
    pub underlying: CommodityUnderlyingParams,
    /// Strike price per unit.
    pub strike: f64,
    /// Option type (call or put).
    pub option_type: OptionType,
    /// Exercise style (European or American).
    #[serde(default)]
    #[builder(default)]
    pub exercise_style: ExerciseStyle,
    /// Optional Bermudan exercise schedule.
    ///
    /// Required when `exercise_style == ExerciseStyle::Bermudan`.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Vec<String>>")]
    pub exercise_schedule: Option<Vec<Date>>,
    /// Option expiry date.
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Contract quantity in units.
    pub quantity: f64,
    /// Contract multiplier (typically 1.0 for OTC options).
    pub multiplier: f64,
    /// Settlement type (physical or cash).
    ///
    /// Defaults to cash settlement when omitted in serialized payloads.
    #[serde(default = "crate::serde_defaults::settlement_cash")]
    #[builder(default = SettlementType::Cash)]
    pub settlement: SettlementType,
    /// Forward/futures curve ID for price interpolation.
    pub forward_curve_id: CurveId,
    /// Discount curve ID for present value.
    pub discount_curve_id: CurveId,
    /// Volatility surface ID for implied vol.
    pub vol_surface_id: CurveId,
    /// Optional spot price ID (for spot-based pricing and American options).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot_id: Option<String>,
    /// Optional quoted forward price (overrides curve lookup).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quoted_forward: Option<f64>,
    /// Day count convention for time to expiry.
    #[serde(default = "crate::serde_defaults::day_count_act365f")]
    #[builder(default = DayCount::Act365F)]
    pub day_count: DayCount,
    /// Pricing overrides (implied vol, tree steps, etc.).
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Optional market convention for this commodity.
    ///
    /// When set, provides default premium settlement days and calendar.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub convention: Option<CommodityConvention>,
    /// Premium settlement lag in business days after trade date.
    ///
    /// Standard: T+1 for most exchange-traded options, T+2 for OTC.
    /// If not set and `convention` is provided, uses convention default.
    /// Otherwise defaults to 1 (T+1).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premium_settlement_days: Option<u32>,
    /// Attributes for tagging and selection.
    #[builder(default)]
    #[serde(default)]
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl CommodityOption {
    /// Create a canonical example commodity option for testing and documentation.
    ///
    /// Returns a WTI European call option.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        Self::builder()
            .id(InstrumentId::new("WTI-OPT-2025M06"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .strike(75.0)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(
                Date::from_calendar_date(2025, time::Month::June, 15).expect("valid example date"),
            )
            .quantity(1000.0)
            .multiplier(1.0)
            .settlement(SettlementType::Cash)
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("WTI-VOL"))
            .day_count(DayCount::Act365F)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("Example commodity option construction should not fail")
    }

    fn intrinsic_value(&self, underlying: f64) -> f64 {
        match self.option_type {
            OptionType::Call => (underlying - self.strike).max(0.0),
            OptionType::Put => (self.strike - underlying).max(0.0),
        }
    }

    fn time_to_expiry(&self, as_of: Date) -> Result<f64> {
        self.day_count
            .year_fraction(as_of, self.expiry, DayCountContext::default())
            .map(|t| t.max(0.0))
    }

    fn collect_inputs(&self, market: &MarketContext, as_of: Date) -> Result<CommodityOptionInputs> {
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let curve_dc = disc.day_count();
        let t_rate = curve_dc.year_fraction(as_of, self.expiry, DayCountContext::default())?;
        let r = disc.zero(t_rate.max(0.0));
        let t = self.time_to_expiry(as_of)?;

        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &self.pricing_overrides.market_quotes,
            market,
            self.vol_surface_id.as_str(),
            t,
            self.strike,
        )?;

        let forward = self.forward_price(market, as_of)?;
        let df = disc.df_between_dates(as_of, self.expiry)?;

        let spot = if let Some(spot) = self.spot_price(market)? {
            spot
        } else {
            forward
        };
        let q = implied_carry(spot, forward, r, t);

        Ok(CommodityOptionInputs {
            forward,
            spot,
            r,
            q,
            sigma,
            t,
            df,
        })
    }

    fn spot_price(&self, market: &MarketContext) -> Result<Option<f64>> {
        let Some(spot_id) = &self.spot_id else {
            return Ok(None);
        };
        let scalar = market.get_price(spot_id)?;
        let spot = match scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };
        Ok(Some(spot))
    }

    /// Get the forward price for this option.
    ///
    /// Uses `quoted_forward` if provided, otherwise retrieves from the `PriceCurve`
    /// specified by `forward_curve_id`. If no `PriceCurve` is found but `spot_id`
    /// is provided, falls back to cost-of-carry model: F = S × exp(r × T).
    ///
    /// # Note on PriceCurve Evaluation
    ///
    /// When using a `PriceCurve`, this method uses `price_on_date(expiry)` which
    /// respects the curve's own day count convention rather than hard-coding Act365F.
    pub fn forward_price(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        // 1. Direct override takes precedence
        if let Some(price) = self.quoted_forward {
            return Ok(price);
        }

        // 2. Try to get price from PriceCurve using date-based evaluation
        if let Ok(price_curve) = market.get_price_curve(self.forward_curve_id.as_str()) {
            // At or past expiry, return spot price from curve
            if self.expiry <= as_of {
                return Ok(price_curve.spot_price());
            }
            // Use price_on_date to respect the curve's day count convention
            return price_curve.price_on_date(self.expiry);
        }

        // 3. Fallback: cost-of-carry model if spot is available
        if let Some(spot) = self.spot_price(market)? {
            let t = DayCount::Act365F
                .year_fraction(as_of, self.expiry, DayCountContext::default())?
                .max(0.0);
            let disc = market.get_discount(self.discount_curve_id.as_str())?;
            let r = disc.zero(t);
            return Ok(spot * (r * t).exp());
        }

        // 4. No PriceCurve and no spot - error with helpful message
        Err(finstack_core::Error::Input(
            finstack_core::error::InputError::NotFound {
                id: format!(
                    "PriceCurve '{}' not found. \
                     Use MarketContext::insert_price_curve() to add a commodity forward price curve.",
                    self.forward_curve_id
                ),
            },
        ))
    }

    /// Get the effective premium settlement lag in business days.
    ///
    /// Resolution order:
    /// 1. `premium_settlement_days` if explicitly set
    /// 2. `convention.settlement_days()` if convention is set
    /// 3. Default: 1 (T+1, standard for exchange-traded options)
    pub fn effective_premium_settlement_days(&self) -> u32 {
        self.premium_settlement_days
            .or_else(|| self.convention.map(|c| c.settlement_days()))
            .unwrap_or(1)
    }

    /// Price commodity option using Monte Carlo with Schwartz-Smith dynamics.
    ///
    /// Simulates paths under the Schwartz-Smith two-factor model and
    /// computes discounted expected payoff.
    ///
    /// # Arguments
    ///
    /// * `mc_params` - Monte Carlo configuration (model, paths, steps, seed)
    /// * `market` - Market data context for discount curves and forward prices
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    ///
    /// Present value as `Money` in the underlying currency.
    pub fn npv_mc(
        &self,
        mc_params: &CommodityMcParams,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        use finstack_monte_carlo::prelude::*;

        let t = self.time_to_expiry(as_of)?;
        if t <= 0.0 {
            // At or past expiry: return intrinsic value
            let underlying = if let Some(spot) = self.spot_price(market)? {
                spot
            } else {
                self.forward_price(market, as_of)?
            };
            let intrinsic = self.intrinsic_value(underlying);
            return Ok(Money::new(
                intrinsic * self.quantity * self.multiplier,
                self.underlying.currency,
            ));
        }

        match &mc_params.model {
            CommodityPricingModel::BlackScholes => {
                // Fall back to analytical Black-76
                let inputs = self.collect_inputs(market, as_of)?;
                let unit_price = black76_unit_price(
                    inputs.forward,
                    self.strike,
                    inputs.sigma,
                    inputs.t,
                    inputs.df,
                    self.option_type,
                );
                Ok(Money::new(
                    unit_price * self.quantity * self.multiplier,
                    self.underlying.currency,
                ))
            }
            CommodityPricingModel::SchwartzSmith {
                kappa,
                sigma_x,
                sigma_y,
                rho_xy,
                mu_y,
                lambda_x,
            } => {
                // Build risk-neutral Schwartz-Smith process.
                // Under Q: dX = -(kappa + lambda_x) X dt + sigma_x dW_X
                let rn_kappa = kappa + lambda_x;
                if rn_kappa <= 0.0 {
                    return Err(finstack_core::Error::Validation(
                        "Risk-neutral kappa (kappa + lambda_x) must be positive".to_string(),
                    ));
                }

                let ss_params =
                    SchwartzSmithParams::new(rn_kappa, *sigma_x, *mu_y, *sigma_y, *rho_xy)?;

                // Get initial spot price
                let initial_spot = if let Some(spot) = self.spot_price(market)? {
                    spot
                } else {
                    self.forward_price(market, as_of)?
                };

                let process = SchwartzSmithProcess::from_spot(ss_params, initial_spot, None);
                let disc_scheme = ExactSchwartzSmith::from_process(&process)?;

                // Discount factor
                let disc_curve = market.get_discount(self.discount_curve_id.as_str())?;
                let df = disc_curve.df_between_dates(as_of, self.expiry)?;

                // Build MC engine
                let seed = mc_params.seed.unwrap_or(42);
                let time_grid = TimeGrid::uniform(t, mc_params.n_steps)?;
                let engine_config =
                    McEngineConfig::new(mc_params.n_paths, time_grid).with_parallel(true);
                let engine = McEngine::new(engine_config);

                let rng = PhiloxRng::new(seed);
                let initial_state = process.initial_state().to_vec();
                let maturity_step = mc_params.n_steps;

                // Dispatch on option type (EuropeanCall / EuropeanPut are
                // distinct concrete types, so we branch here).
                let unit_pv = match self.option_type {
                    OptionType::Call => {
                        let payoff = EuropeanCall::new(self.strike, 1.0, maturity_step);
                        let est = engine.price(
                            &rng,
                            &process,
                            &disc_scheme,
                            &initial_state,
                            &payoff,
                            self.underlying.currency,
                            df,
                        )?;
                        est.mean.amount()
                    }
                    OptionType::Put => {
                        let payoff = EuropeanPut::new(self.strike, 1.0, maturity_step);
                        let est = engine.price(
                            &rng,
                            &process,
                            &disc_scheme,
                            &initial_state,
                            &payoff,
                            self.underlying.currency,
                            df,
                        )?;
                        est.mean.amount()
                    }
                };

                Ok(Money::new(
                    unit_pv * self.quantity * self.multiplier,
                    self.underlying.currency,
                ))
            }
        }
    }
}

struct CommodityOptionInputs {
    forward: f64,
    spot: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    df: f64,
}

fn implied_carry(spot: f64, forward: f64, r: f64, t: f64) -> f64 {
    // Guard against near-zero time: ln(F/S)/t amplifies noise when t is tiny.
    // For t < 1e-5 (~5 seconds), the carry estimate is unreliable.
    //
    // Also guard against non-finite `forward` / `spot` (W-13): a NaN or
    // infinite input passes the `<= 0.0` checks (NaN comparisons are false)
    // and would otherwise feed `(forward/spot).ln()/t` — and hence `q` — a
    // non-finite value straight into the binomial tree. Fall back to the
    // risk-free rate (zero cost-of-carry) when any input is unusable.
    if t < 1e-5
        || !t.is_finite()
        || !spot.is_finite()
        || !forward.is_finite()
        || spot <= 0.0
        || forward <= 0.0
    {
        return r;
    }
    let carry = (forward / spot).ln() / t;
    if carry.is_finite() {
        r - carry
    } else {
        r
    }
}

fn black76_unit_price(
    forward: f64,
    strike: f64,
    sigma: f64,
    t: f64,
    df: f64,
    option_type: OptionType,
) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        let intrinsic = match option_type {
            OptionType::Call => (forward - strike).max(0.0),
            OptionType::Put => (strike - forward).max(0.0),
        };
        return intrinsic * df;
    }

    let d1 = crate::models::d1_black76(forward, strike, sigma, t);
    let d2 = crate::models::d2_black76(forward, strike, sigma, t);

    let price = match option_type {
        OptionType::Call => {
            forward * finstack_core::math::norm_cdf(d1) - strike * finstack_core::math::norm_cdf(d2)
        }
        OptionType::Put => {
            strike * finstack_core::math::norm_cdf(-d2)
                - forward * finstack_core::math::norm_cdf(-d1)
        }
    };
    price * df
}

impl CurveDependencies for CommodityOption {
    fn curve_dependencies(&self) -> finstack_core::Result<InstrumentCurves> {
        InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .forward(self.forward_curve_id.clone())
            .build()
    }
}

impl Instrument for CommodityOption {
    impl_instrument_base!(crate::pricer::InstrumentType::CommodityOption);

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        if let Some(spot_id) = self.spot_id.as_deref() {
            deps.add_spot_id(spot_id);
        }
        deps.add_vol_surface_id(self.vol_surface_id.as_str());
        Ok(deps)
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        // Post-expiry: option is fully settled, value is 0
        if as_of > self.expiry {
            return Ok(Money::new(0.0, self.underlying.currency));
        }

        let t = self.time_to_expiry(as_of)?;
        if t <= 0.0 {
            // At expiry: return intrinsic value
            let underlying = if let Some(spot) = self.spot_price(market)? {
                spot
            } else {
                self.forward_price(market, as_of)?
            };
            let intrinsic = self.intrinsic_value(underlying);
            return Ok(Money::new(
                intrinsic * self.quantity * self.multiplier,
                self.underlying.currency,
            ));
        }

        let inputs = self.collect_inputs(market, as_of)?;

        let unit_price = match self.exercise_style {
            ExerciseStyle::European => black76_unit_price(
                inputs.forward,
                self.strike,
                inputs.sigma,
                inputs.t,
                inputs.df,
                self.option_type,
            ),
            ExerciseStyle::American => {
                let steps = self
                    .pricing_overrides
                    .model_config
                    .tree_steps
                    .unwrap_or(201);
                let tree = BinomialTree::leisen_reimer_odd(steps);
                let params = OptionMarketParams {
                    spot: inputs.spot,
                    strike: self.strike,
                    rate: inputs.r,
                    dividend_yield: inputs.q,
                    volatility: inputs.sigma,
                    time_to_expiry: inputs.t,
                    option_type: self.option_type,
                };
                tree.price_american(&params)?
            }
            ExerciseStyle::Bermudan => {
                let schedule = self.exercise_schedule.as_ref().ok_or_else(|| {
                    finstack_core::Error::Validation(
                        "Commodity Bermudan option requires exercise_schedule".to_string(),
                    )
                })?;
                let steps = self
                    .pricing_overrides
                    .model_config
                    .tree_steps
                    .unwrap_or(201);
                let tree = BinomialTree::leisen_reimer_odd(steps);
                let params = OptionMarketParams {
                    spot: inputs.spot,
                    strike: self.strike,
                    rate: inputs.r,
                    dividend_yield: inputs.q,
                    volatility: inputs.sigma,
                    time_to_expiry: inputs.t,
                    option_type: self.option_type,
                };
                let exercise_times: Vec<f64> = schedule
                    .iter()
                    .filter_map(|date| {
                        let yf = DayCount::Act365F
                            .year_fraction(as_of, *date, DayCountContext::default())
                            .ok()?;
                        if yf > 0.0 && yf <= inputs.t {
                            Some(yf)
                        } else {
                            None
                        }
                    })
                    .collect();
                if exercise_times.is_empty() {
                    return Err(finstack_core::Error::Validation(
                        "Commodity Bermudan option requires at least one future exercise date"
                            .to_string(),
                    ));
                }
                tree.price_bermudan(&params, &exercise_times)?
            }
        };

        Ok(Money::new(
            unit_price * self.quantity * self.multiplier,
            self.underlying.currency,
        ))
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

impl crate::instruments::common_impl::traits::OptionGreeksProvider for CommodityOption {
    fn option_delta(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<Option<f64>> {
        use finstack_core::math::special_functions::norm_cdf;

        let t = self
            .day_count
            .year_fraction(as_of, self.expiry, DayCountContext::default())?
            .max(0.0);
        if t <= 0.0 {
            let forward = self.forward_price(market, as_of)?;
            let intrinsic = match self.option_type {
                OptionType::Call => {
                    if forward > self.strike {
                        1.0
                    } else {
                        0.0
                    }
                }
                OptionType::Put => {
                    if forward < self.strike {
                        -1.0
                    } else {
                        0.0
                    }
                }
            };
            return Ok(Some(intrinsic * self.quantity * self.multiplier));
        }

        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &self.pricing_overrides.market_quotes,
            market,
            self.vol_surface_id.as_str(),
            t,
            self.strike,
        )?;
        if sigma <= 0.0 {
            return Ok(Some(0.0));
        }

        let forward = self.forward_price(market, as_of)?;
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let df = disc.df_between_dates(as_of, self.expiry)?;
        let d1 = crate::models::d1_black76(forward, self.strike, sigma, t);
        let nd1 = norm_cdf(d1);

        let delta_unit = match self.option_type {
            OptionType::Call => df * nd1,
            OptionType::Put => df * (nd1 - 1.0),
        };
        Ok(Some(delta_unit * self.quantity * self.multiplier))
    }

    fn option_vega(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<Option<f64>> {
        use finstack_core::math::special_functions::norm_pdf;

        let t = self
            .day_count
            .year_fraction(as_of, self.expiry, DayCountContext::default())?
            .max(0.0);
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &self.pricing_overrides.market_quotes,
            market,
            self.vol_surface_id.as_str(),
            t,
            self.strike,
        )?;
        if sigma <= 0.0 {
            return Ok(Some(0.0));
        }

        let forward = self.forward_price(market, as_of)?;
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let df = disc.df_between_dates(as_of, self.expiry)?;
        let d1 = crate::models::d1_black76(forward, self.strike, sigma, t);
        let vega_abs = df * forward * norm_pdf(d1) * t.sqrt();
        Ok(Some(vega_abs * 0.01 * self.quantity * self.multiplier))
    }

    fn option_gamma(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        #[derive(Debug)]
        enum ForwardDriver {
            QuotedForward(f64),
            PriceCurve,
            SpotScalar(String),
        }

        let driver = if let Some(fwd) = self.quoted_forward {
            ForwardDriver::QuotedForward(fwd)
        } else if market
            .get_price_curve(self.forward_curve_id.as_str())
            .is_ok()
        {
            ForwardDriver::PriceCurve
        } else if let Some(ref spot_id) = self.spot_id {
            ForwardDriver::SpotScalar(spot_id.clone())
        } else {
            return Err(finstack_core::Error::Validation(
                "Cannot compute gamma: no quoted_forward, PriceCurve, or spot_id available"
                    .to_string(),
            ));
        };

        let bump_pct = crate::metrics::bump_sizes::SPOT;
        let forward_price = self.forward_price(market, as_of)?;
        let bump_size = forward_price * bump_pct;
        if bump_size <= 0.0 {
            return Ok(Some(0.0));
        }

        let pv_base = self.value(market, as_of)?.amount();

        let (pv_up, pv_down) = match driver {
            ForwardDriver::QuotedForward(fwd) => {
                let mut up = self.clone();
                up.quoted_forward = Some(fwd * (1.0 + bump_pct));
                let pv_up = up.value(market, as_of)?.amount();

                let mut down = self.clone();
                down.quoted_forward = Some(fwd * (1.0 - bump_pct));
                let pv_down = down.value(market, as_of)?.amount();
                (pv_up, pv_down)
            }
            ForwardDriver::PriceCurve => {
                use finstack_core::market_data::bumps::{
                    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
                };
                let curve_id = CurveId::new(self.forward_curve_id.as_str());
                let up = market.bump([MarketBump::Curve {
                    id: curve_id.clone(),
                    spec: BumpSpec {
                        bump_type: BumpType::Parallel,
                        mode: BumpMode::Additive,
                        units: BumpUnits::Percent,
                        value: bump_pct * 100.0,
                    },
                }])?;
                let pv_up = self.value(&up, as_of)?.amount();

                let down = market.bump([MarketBump::Curve {
                    id: curve_id,
                    spec: BumpSpec {
                        bump_type: BumpType::Parallel,
                        mode: BumpMode::Additive,
                        units: BumpUnits::Percent,
                        value: -bump_pct * 100.0,
                    },
                }])?;
                let pv_down = self.value(&down, as_of)?.amount();
                (pv_up, pv_down)
            }
            ForwardDriver::SpotScalar(ref spot_id) => {
                let up = crate::metrics::bump_scalar_price(market, spot_id, bump_pct)?;
                let down = crate::metrics::bump_scalar_price(market, spot_id, -bump_pct)?;
                let pv_up = self.value(&up, as_of)?.amount();
                let pv_down = self.value(&down, as_of)?.amount();
                (pv_up, pv_down)
            }
        };

        Ok(Some(
            (pv_up - 2.0 * pv_base + pv_down) / (bump_size * bump_size),
        ))
    }

    fn option_vanna(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<Option<f64>> {
        #[derive(Debug)]
        enum ForwardDriver {
            QuotedForward(f64),
            PriceCurve,
            SpotScalar(String),
        }

        let driver = if let Some(fwd) = self.quoted_forward {
            ForwardDriver::QuotedForward(fwd)
        } else if market
            .get_price_curve(self.forward_curve_id.as_str())
            .is_ok()
        {
            ForwardDriver::PriceCurve
        } else if let Some(ref spot_id) = self.spot_id {
            ForwardDriver::SpotScalar(spot_id.clone())
        } else {
            return Err(finstack_core::Error::Validation(
                "Cannot compute vanna: no quoted_forward, PriceCurve, or spot_id available"
                    .to_string(),
            ));
        };

        let fwd_bump_pct = crate::metrics::bump_sizes::SPOT;
        let vol_bump = crate::metrics::bump_sizes::VOLATILITY;

        let forward_price = self.forward_price(market, as_of)?;
        let fwd_bump_size = forward_price * fwd_bump_pct;
        if fwd_bump_size <= 0.0 {
            return Ok(Some(0.0));
        }

        let pv_with_bumps = |fwd_bump_pct: f64, vol_bump: f64| -> finstack_core::Result<f64> {
            match driver {
                ForwardDriver::QuotedForward(fwd) => {
                    let mut inst = self.clone();
                    inst.quoted_forward = Some(fwd * (1.0 + fwd_bump_pct));
                    let bumped = crate::metrics::bump_surface_vol_absolute(
                        market,
                        self.vol_surface_id.as_str(),
                        vol_bump,
                    )?;
                    Ok(inst.value(&bumped, as_of)?.amount())
                }
                ForwardDriver::PriceCurve => {
                    use finstack_core::market_data::bumps::{
                        BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
                    };
                    let curve_id = CurveId::new(self.forward_curve_id.as_str());
                    let bumped_price = market.bump([MarketBump::Curve {
                        id: curve_id,
                        spec: BumpSpec {
                            bump_type: BumpType::Parallel,
                            mode: BumpMode::Additive,
                            units: BumpUnits::Percent,
                            value: fwd_bump_pct * 100.0,
                        },
                    }])?;
                    let bumped = crate::metrics::bump_surface_vol_absolute(
                        &bumped_price,
                        self.vol_surface_id.as_str(),
                        vol_bump,
                    )?;
                    Ok(self.value(&bumped, as_of)?.amount())
                }
                ForwardDriver::SpotScalar(ref spot_id) => {
                    let bumped_spot =
                        crate::metrics::bump_scalar_price(market, spot_id, fwd_bump_pct)?;
                    let bumped = crate::metrics::bump_surface_vol_absolute(
                        &bumped_spot,
                        self.vol_surface_id.as_str(),
                        vol_bump,
                    )?;
                    Ok(self.value(&bumped, as_of)?.amount())
                }
            }
        };

        let pv_up_up = pv_with_bumps(fwd_bump_pct, vol_bump)?;
        let pv_up_dn = pv_with_bumps(fwd_bump_pct, -vol_bump)?;
        let pv_dn_up = pv_with_bumps(-fwd_bump_pct, vol_bump)?;
        let pv_dn_dn = pv_with_bumps(-fwd_bump_pct, -vol_bump)?;

        Ok(Some(
            (pv_up_up - pv_up_dn - pv_dn_up + pv_dn_dn) / (4.0 * fwd_bump_size * vol_bump),
        ))
    }

    fn option_volga(
        &self,
        market: &MarketContext,
        as_of: Date,
        base_pv: f64,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let vol_bump = crate::metrics::bump_sizes::VOLATILITY;
        let up = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            vol_bump,
        )?;
        let dn = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            -vol_bump,
        )?;
        let pv_up = self.value(&up, as_of)?.amount();
        let pv_dn = self.value(&dn, as_of)?.amount();
        Ok(Some(
            (pv_up - 2.0 * base_pv + pv_dn) / (vol_bump * vol_bump),
        ))
    }
}

crate::impl_empty_cashflow_provider!(
    CommodityOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::{DiscountCurve, PriceCurve};

    #[test]
    fn test_default_settlement_type_is_cash() {
        assert_eq!(
            crate::serde_defaults::settlement_cash(),
            SettlementType::Cash
        );
    }

    #[test]
    fn test_serde_defaults_settlement_to_cash_when_omitted() {
        let mut value = serde_json::to_value(CommodityOption::example()).expect("serialize");
        let obj = value
            .as_object_mut()
            .expect("CommodityOption should serialize to an object");
        obj.remove("settlement");
        let option: CommodityOption = serde_json::from_value(value).expect("deserialize");
        assert_eq!(option.settlement, SettlementType::Cash);
    }

    #[test]
    fn bermudan_exercise_requires_schedule() {
        let mut option = CommodityOption::example();
        option.exercise_style = ExerciseStyle::Bermudan;
        let as_of = Date::from_calendar_date(2025, time::Month::January, 15).expect("valid");
        let market = finstack_core::market_data::context::MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 0.97)])
                    .build()
                    .expect("discount curve"),
            )
            .insert(
                PriceCurve::builder("WTI-FORWARD")
                    .base_date(as_of)
                    .spot_price(75.0)
                    .knots([(0.0, 75.0), (1.0, 76.0)])
                    .build()
                    .expect("price curve"),
            )
            .insert_surface(
                VolSurface::builder("WTI-VOL")
                    .expiries(&[0.25, 0.5, 1.0])
                    .strikes(&[60.0, 75.0, 90.0])
                    .row(&[0.25, 0.25, 0.25])
                    .row(&[0.25, 0.25, 0.25])
                    .row(&[0.25, 0.25, 0.25])
                    .build()
                    .expect("vol surface"),
            );
        let err = option
            .value(&market, as_of)
            .expect_err("Bermudan commodity options should require an exercise schedule");
        assert!(err.to_string().contains("exercise_schedule"));
    }

    /// W-13: `implied_carry` must guard against non-finite `forward` / `spot`.
    /// A NaN or infinite input passes the `<= 0.0` checks (NaN comparisons are
    /// always false), so without an `is_finite()` guard a non-finite cost-of-
    /// carry `q` would flow straight into the binomial tree. The guard makes
    /// `implied_carry` fall back to the risk-free rate (zero carry).
    #[test]
    fn w13_implied_carry_guards_against_non_finite_inputs() {
        let r = 0.05;
        let t = 0.5;

        // NaN forward → fall back to r.
        let q_nan_fwd = implied_carry(100.0, f64::NAN, r, t);
        assert_eq!(
            q_nan_fwd, r,
            "NaN forward must fall back to the risk-free rate, got {q_nan_fwd}"
        );
        assert!(q_nan_fwd.is_finite());

        // NaN spot → fall back to r.
        let q_nan_spot = implied_carry(f64::NAN, 100.0, r, t);
        assert_eq!(
            q_nan_spot, r,
            "NaN spot must fall back to the risk-free rate"
        );

        // Infinite forward → fall back to r.
        let q_inf_fwd = implied_carry(100.0, f64::INFINITY, r, t);
        assert_eq!(q_inf_fwd, r, "infinite forward must fall back to r");
        assert!(q_inf_fwd.is_finite());

        // Sanity: a well-formed input still yields the true cost-of-carry.
        let q_ok = implied_carry(100.0, 105.0, r, t);
        let expected = r - (105.0_f64 / 100.0).ln() / t;
        assert!(
            (q_ok - expected).abs() < 1e-12,
            "well-formed inputs must yield the true implied carry"
        );
    }
}
