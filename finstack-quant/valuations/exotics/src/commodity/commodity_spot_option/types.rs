//! Commodity option instrument definition and builder methods.

use super::valuation::{CommoditySpotOptionInputs, implied_carry};
use crate::commodity::CommodityUnderlyingParams;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_models::monte_carlo::engine::McPricingInputs;
use finstack_quant_models::monte_carlo::process::schwartz_smith::SchwartzSmithParams;
use finstack_quant_models::types::{ExerciseStyle, OptionType};
use finstack_quant_valuations_core::instruments::SettlementType;

/// Monte Carlo configuration for commodity option pricing.
///
/// When provided, enables simulation-based pricing using the Schwartz-Smith
/// two-factor model instead of the default Black-76 analytical formula.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CommodityMcParams {
    /// Risk-neutral Schwartz-Smith two-factor process parameters.
    pub params: SchwartzSmithParams,
    /// Number of independent Monte Carlo paths (estimators).
    pub num_paths: usize,
    /// Number of time steps per path.
    pub num_steps: usize,
    /// Optional random seed for reproducibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

/// Recorded exercise or expiry of a spot commodity option.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CommoditySpotOptionExercise {
    /// Exercise date, or expiry observation date for an unexercised option.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Positive observed commodity spot price in strike units.
    pub spot: f64,
    /// Actual cash-payment or physical-delivery date, on or after exercise.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub settlement_date: Date,
    /// True for exercise/assignment; false records expiration without exercise.
    pub exercised: bool,
}

/// Option on spot commodity. Options on a specified futures contract use
/// [`crate::commodity::commodity_future_option::CommodityFutureOption`], including futures delivery.
///
/// # Pricing
///
/// - **European options**: Black-76 model using the forward price from the `PriceCurve`
/// - **American options**: Binomial tree (Leisen-Reimer) with cost-of-carry derived from
///   the forward/spot relationship
///
/// Analytical delta measures currency per unit change in the resolved forward,
/// holding discount factors and volatility fixed. At zero volatility or expiry,
/// the intrinsic-payoff derivative is used; delta is defined as zero exactly at
/// the strike, where the mathematical derivative does not exist.
///
/// # American Option Assumptions
///
/// For American exercise, the model requires a spot price to build the binomial tree.
/// `spot_id` must identify an observed spot for early exercise and Monte Carlo.
/// A numeric scalar is in the underlying currency; a Money scalar must carry
/// that same currency.
/// The commodity forward is never substituted for the spot underlying.
/// The convenience yield (cost-of-carry) is implied from the forward/spot ratio:
/// `q = r - ln(F/S)/T`
///
/// Bermudan exercise uses the configured exercise schedule and a binomial
/// tree. The schedule must contain at least one strictly future date when the
/// option is priced before expiry.
///
/// # Forward Price Retrieval
///
/// Forward prices are retrieved from a `PriceCurve` (not a `ForwardCurve`).
/// The curve must be added via `MarketContext::insert_price_curve()`.
/// If `quoted_forward` is provided, it overrides the curve lookup.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(deny_unknown_fields))]
#[serde(remote = "Self")]
pub struct CommoditySpotOption {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Commodity underlying parameters (commodity_type, underlying_ticker, unit, currency).
    #[serde(flatten)]
    pub underlying: CommodityUnderlyingParams,
    /// Strike price per unit.
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_positive_f64",
        deserialize_with = "finstack_quant_core::wire::deserialize_positive_f64"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::PositiveF64Wire")
    )]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<Vec<finstack_quant_core::wire::DateWire>>")
    )]
    pub exercise_dates: Option<Vec<Date>>,
    /// Option expiry date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// Contract quantity in units.
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_positive_f64",
        deserialize_with = "finstack_quant_core::wire::deserialize_positive_f64"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::PositiveF64Wire")
    )]
    pub quantity: f64,
    /// Contract multiplier (typically 1.0 for OTC options).
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_positive_f64",
        deserialize_with = "finstack_quant_core::wire::deserialize_positive_f64"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::PositiveF64Wire")
    )]
    pub multiplier: f64,
    /// Settlement type (physical or cash).
    ///
    /// Cash fixes the payoff on exercise; physical retains the marked delivery
    /// claim until its recorded settlement date.
    pub settlement: SettlementType,
    /// Recorded exercise or expiry; required after expiry while settlement remains unpaid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exercise: Option<CommoditySpotOptionExercise>,
    /// Forward/futures curve ID for price interpolation.
    pub forward_curve_id: CurveId,
    /// Discount curve ID for present value.
    pub discount_curve_id: CurveId,
    /// Volatility surface ID for implied vol.
    pub vol_surface_id: CurveId,
    /// Optional spot price ID (for spot-based pricing and American options).
    /// Numeric prices use the underlying currency; Money prices must match it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot_id: Option<PriceId>,
    /// Optional quoted forward price (overrides curve lookup).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quoted_forward: Option<f64>,
    /// Day count convention for time to expiry.
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::day_count_act365f")]
    #[builder(default = DayCount::Act365F)]
    pub day_count: DayCount,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides,
    /// Attributes for tagging and selection.
    #[builder(default)]
    #[serde(default)]
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
    /// Rejects unknown JSON fields (restores `deny_unknown_fields` despite the
    /// `#[serde(flatten)]` on `underlying`).
    #[serde(flatten)]
    #[cfg_attr(feature = "json-schema", schemars(skip))]
    #[builder(default)]
    pub(crate) unknown_fields: finstack_quant_core::serde_guard::UnknownFieldGuard,
}

finstack_quant_valuations_core::validate_on_deserialize!(
    CommoditySpotOption,
    CommoditySpotOption::validate
);

impl CommoditySpotOption {
    /// Validate the static economic inputs shared by builders and JSON loads.
    pub fn validate(&self) -> Result<()> {
        self.underlying.validate("CommoditySpotOption")?;
        finstack_quant_core::validation::validate_f64_positive(
            self.strike,
            "CommoditySpotOption strike",
        )?;
        finstack_quant_core::validation::validate_f64_positive(
            self.quantity,
            "CommoditySpotOption quantity",
        )?;
        finstack_quant_core::validation::validate_f64_positive(
            self.multiplier,
            "CommoditySpotOption multiplier",
        )?;
        if !matches!(self.exercise_style, ExerciseStyle::European) && self.spot_id.is_none() {
            return Err(finstack_quant_core::Error::Validation(
                "Spot early exercise requires spot_id; futures options use CommodityFutureOption"
                    .into(),
            ));
        }
        if let Some(exercise) = &self.exercise {
            finstack_quant_core::validation::validate_f64_positive(
                exercise.spot,
                "commodity exercise spot",
            )?;
            if exercise.date > self.expiry
                || exercise.settlement_date < exercise.date
                || (!exercise.exercised && exercise.date != self.expiry)
                || (matches!(self.exercise_style, ExerciseStyle::European)
                    && exercise.date != self.expiry)
                || (matches!(self.exercise_style, ExerciseStyle::Bermudan)
                    && exercise.date != self.expiry
                    && !self
                        .exercise_dates
                        .as_ref()
                        .is_some_and(|dates| dates.contains(&exercise.date)))
            {
                return Err(finstack_quant_core::Error::Validation(
                    "Commodity spot exercise and settlement dates conflict with the contract"
                        .into(),
                ));
            }
        }
        if let Some(schedule) = &self.exercise_dates {
            if schedule.windows(2).any(|w| w[0] >= w[1]) {
                return Err(finstack_quant_core::Error::Validation(
                    "CommoditySpotOption exercise_dates must be strictly increasing".to_string(),
                ));
            }
            if schedule.iter().any(|date| *date > self.expiry) {
                return Err(finstack_quant_core::Error::Validation(
                    "CommoditySpotOption exercise_dates cannot extend beyond expiry".to_string(),
                ));
            }
        }
        if matches!(self.exercise_style, ExerciseStyle::Bermudan)
            && self.exercise_dates.as_ref().is_none_or(Vec::is_empty)
        {
            return Err(finstack_quant_core::Error::Validation(
                "Commodity Bermudan option requires exercise_dates".to_string(),
            ));
        }
        Ok(())
    }

    /// Create a canonical example commodity option for testing and documentation.
    ///
    /// Returns a WTI European call option.
    pub fn example() -> finstack_quant_core::Result<Self> {
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
            .expiry(time::macros::date!(2025 - 06 - 15))
            .quantity(1000.0)
            .multiplier(1.0)
            .settlement(SettlementType::Cash)
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("WTI-VOL"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
    }

    pub(super) fn required_spot(&self, market: &MarketContext) -> Result<f64> {
        let spot = self.spot_price(market)?.ok_or_else(|| finstack_quant_core::Error::Validation(
            "CommoditySpotOption requires observed spot_id for spot exercise or simulation; use CommodityFutureOption for futures".into()
        ))?;
        finstack_quant_core::validation::validate_f64_positive(spot, "commodity spot")?;
        Ok(spot)
    }

    /// Value only the remaining exercise settlement claim, when resolved.
    pub(super) fn resolved_claim(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Option<Money>> {
        let Some(exercise) = self
            .exercise
            .filter(|observation| observation.date <= as_of)
        else {
            if as_of > self.expiry {
                return Err(finstack_quant_core::Error::Validation(
                    "CommoditySpotOption requires recorded expiry observation after expiry".into(),
                ));
            }
            if as_of == self.expiry {
                let spot = if self.spot_id.is_some() {
                    self.required_spot(market)?
                } else {
                    self.forward_price(market, as_of)?
                };
                return Money::from_model_value(
                    self.intrinsic_value(spot) * self.quantity * self.multiplier,
                    self.underlying.currency,
                )
                .map(Some);
            }
            return Ok(None);
        };
        let unit = if !exercise.exercised || as_of > exercise.settlement_date {
            0.0
        } else {
            let disc = market.get_discount(&self.discount_curve_id)?;
            let df = disc.df_between_dates(as_of, exercise.settlement_date)?;
            match self.settlement {
                SettlementType::Cash => self.intrinsic_value(exercise.spot) * df,
                SettlementType::Physical => {
                    let curve = market.get_price_curve(&self.forward_curve_id)?;
                    let t = curve.day_count().year_fraction(
                        curve.base_date(),
                        exercise.settlement_date,
                        DayCountContext::default(),
                    )?;
                    let delivery_forward = curve.price(t)?;
                    let sign = if self.option_type == OptionType::Call {
                        1.0
                    } else {
                        -1.0
                    };
                    sign * (delivery_forward - self.strike) * df
                }
            }
        };
        Money::from_model_value(
            unit * self.quantity * self.multiplier,
            self.underlying.currency,
        )
        .map(Some)
    }

    pub(super) fn intrinsic_value(&self, underlying: f64) -> f64 {
        match self.option_type {
            OptionType::Call => (underlying - self.strike).max(0.0),
            OptionType::Put => (self.strike - underlying).max(0.0),
        }
    }

    pub(super) fn time_to_expiry(&self, as_of: Date) -> Result<f64> {
        self.day_count
            .year_fraction(as_of, self.expiry, DayCountContext::default())
            .map(|t| t.max(0.0))
    }

    pub(super) fn collect_inputs(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<CommoditySpotOptionInputs> {
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let t = self.time_to_expiry(as_of)?;
        let df = disc.df_between_dates(as_of, self.expiry)?;
        let r =
            finstack_quant_valuations_core::instruments::pricing::discounting::zero_rate_from_df(
                df,
                t,
                "commodity-option model rate",
            )?;

        let sigma =
            finstack_quant_valuations_core::instruments::pricing::volatility::resolve_sigma_at(
                &self.instrument_pricing_overrides.market_quotes,
                market,
                self.vol_surface_id.as_str(),
                t,
                self.strike,
            )?;

        let forward = self.forward_price(market, as_of)?;
        let spot = if matches!(self.exercise_style, ExerciseStyle::European) {
            // European pricing uses the expiry forward directly; this local
            // coordinate is unused by its payoff and does not define a spot.
            forward
        } else {
            self.required_spot(market)?
        };
        let q = implied_carry(spot, forward, r, t);

        Ok(CommoditySpotOptionInputs {
            forward,
            spot,
            r,
            q,
            sigma,
            t,
            df,
        })
    }

    pub(super) fn spot_price(&self, market: &MarketContext) -> Result<Option<f64>> {
        let Some(spot_id) = &self.spot_id else {
            return Ok(None);
        };
        let scalar = market.get_price(spot_id)?;
        let spot =
            finstack_quant_valuations_core::instruments::pricing::market_inputs::scalar_price_amount(
                scalar,
                self.underlying.currency,
            )?;
        Ok(Some(spot))
    }

    /// Get the forward price for this option.
    ///
    /// Uses `quoted_forward` when supplied, otherwise the price curve at expiry.
    /// A missing curve is an error; spot alone cannot establish commodity carry.
    ///
    /// # Arguments
    ///
    /// * `market` - Market containing the dated commodity price curve unless overridden.
    /// * `_as_of` - Valuation date; the curve retains its own base date and the contract's expiry.
    pub fn forward_price(&self, market: &MarketContext, _as_of: Date) -> Result<f64> {
        let validate = |price: f64| {
            if price.is_finite() && price > 0.0 {
                Ok(price)
            } else {
                Err(finstack_quant_core::Error::Validation(format!(
                    "CommoditySpotOption '{}' requires a finite positive forward for its lognormal model, got {price}",
                    self.id
                )))
            }
        };
        validate(match self.quoted_forward {
            Some(forward) => forward,
            None => market
                .get_price_curve(&self.forward_curve_id)?
                .price_on_date(self.expiry)?,
        })
    }

    pub(crate) fn price_mc(
        &self,
        mc_params: &CommodityMcParams,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<(
        Money,
        Option<finstack_quant_valuations_core::results::MonteCarloValuationDetails>,
    )> {
        use finstack_quant_core::math::time_grid::TimeGrid;
        use finstack_quant_models::monte_carlo::discretization::schwartz_smith::ExactSchwartzSmith;
        use finstack_quant_models::monte_carlo::engine::{McEngine, McEngineConfig};
        use finstack_quant_models::monte_carlo::payoff::vanilla::European;
        use finstack_quant_models::monte_carlo::process::schwartz_smith::SchwartzSmithProcess;
        use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;

        self.validate()?;
        if let Some(claim) = self.resolved_claim(market, as_of)? {
            return Ok((claim, None));
        }
        if !matches!(self.exercise_style, ExerciseStyle::European) {
            return Err(finstack_quant_core::Error::Validation(
                "CommoditySpotOption Monte Carlo supports European exercise only".into(),
            ));
        }
        let t = self.time_to_expiry(as_of)?;

        let p = &mc_params.params;

        // Build risk-neutral Schwartz-Smith process. Schwartz & Smith
        // (2000) risk-neutralize the short-term factor with a
        // CONSTANT drift shift at unchanged mean-reversion speed:
        //   dX = (-kappa·X - lambda_x) dt + sigma_x dW*_X
        // (inflating kappa by lambda_x would distort the futures
        // volatility term structure e^{-kappa·tau}).
        let ss_params = SchwartzSmithParams::new(p.kappa, p.sigma_x, p.mu_y, p.sigma_y, p.rho_xy)?
            .with_lambda_x(p.lambda_x)?;

        let initial_spot = self.required_spot(market)?;

        let mut process = SchwartzSmithProcess::from_spot(ss_params, initial_spot, None)?;

        // Pin the simulated forward to the market: the closed-form
        // futures curve F_model(0,T) (Schwartz-Smith 2000, eq. 9) is
        // calibrated to the market forward by shifting the long-term
        // drift — A(T) is linear in mu_y with coefficient T, so
        //   mu_y* = mu_y + ln(F_mkt / F_model) / T
        // makes E^Q[S_T] equal the same forward the Black-76 branch
        // discounts. Without this, the SS-MC price embeds an
        // arbitrary carry inconsistent with the price curve.
        let market_forward = self.forward_price(market, as_of)?;
        if market_forward > 0.0 && t > 0.0 {
            let model_forward = process.futures_price(t);
            let mu_adjustment = (market_forward / model_forward).ln() / t;
            let pinned_params = SchwartzSmithParams::new(
                p.kappa,
                p.sigma_x,
                p.mu_y + mu_adjustment,
                p.sigma_y,
                p.rho_xy,
            )?
            .with_lambda_x(p.lambda_x)?;
            process = SchwartzSmithProcess::from_spot(pinned_params, initial_spot, None)?;
        }
        let disc_scheme = ExactSchwartzSmith::new();

        // Discount factor
        let disc_curve = market.get_discount(self.discount_curve_id.as_str())?;
        let df = disc_curve.df_between_dates(as_of, self.expiry)?;

        let seed = mc_params.seed.unwrap_or(42);
        let time_grid = TimeGrid::uniform(t, mc_params.num_steps)?;
        let time_grid_values = time_grid.times().to_vec();
        let engine_config = McEngineConfig::new(mc_params.num_paths, time_grid).parallel(true);
        let engine = McEngine::new(engine_config);

        let rng = PhiloxRng::new(seed);
        let initial_state = process.initial_state().to_vec();
        let maturity_step = mc_params.num_steps;

        let payoff = European::new(self.option_type, self.strike, 1.0, maturity_step);
        let estimate = engine.price(McPricingInputs {
            rng: &rng,
            process: &process,
            disc: &disc_scheme,
            initial_state: &initial_state,
            payoff: &payoff,
            currency: self.underlying.currency,
            discount_factor: df,
        })?;

        let scale = self.quantity * self.multiplier;
        Ok((
            Money::from_model_value(estimate.mean.amount() * scale, self.underlying.currency)?,
            Some(
                finstack_quant_valuations_core::results::MonteCarloValuationDetails {
                    model_key:
                        finstack_quant_valuations_core::pricer::ModelKey::MonteCarloSchwartzSmith,
                    standard_error: estimate.stderr * scale.abs(),
                    training_paths: 0,
                    training_simulated_paths: 0,
                    make_whole_training_paths: 0,
                    make_whole_training_simulated_paths: 0,
                    estimator_paths: estimate.num_paths,
                    simulated_paths: estimate.num_simulated_paths,
                    seed,
                    time_grid: time_grid_values,
                    antithetic: false,
                    sobol: false,
                    brownian_bridge: false,
                },
            ),
        ))
    }
}
