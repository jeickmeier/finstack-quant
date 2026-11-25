//! Pricing, lifecycle dispatch, and Greeks for [`FutureOptionTerms`].

use super::types::{
    FutureOptionExercise, FutureOptionModel, FutureOptionPremiumStyle, FutureOptionSettlement,
    FutureOptionTerms,
};
use crate::instruments::{OptionGreekKind, OptionGreeks, OptionGreeksRequest};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::special_functions::norm_cdf;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_models::closed_form::{bachelier_price_with_annuity, d1_d2_black76};
use finstack_quant_models::trees::binomial_tree::BinomialTree;
use finstack_quant_models::types::{ExerciseStyle, OptionMarketParams, OptionType};

/// Official American lattice size when `tree_steps` is omitted on PV.
const OFFICIAL_TREE_STEPS: usize = 401;
/// Default American lattice size for finite-difference greeks.
const RISK_TREE_STEPS: usize = 201;

enum OptionLifecycleState {
    Terminal,
    PostExercise(FutureOptionExercise),
    Live,
}

fn ensure_finite_greek(name: &str, value: f64) -> finstack_quant_core::Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "FutureOptionTerms {name} produced non-finite value {value}"
        )))
    }
}

fn validate_positive_bump(name: &str, value: f64) -> finstack_quant_core::Result<f64> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "FutureOptionTerms {name} must be finite and positive, got {value}"
        )))
    }
}

impl FutureOptionTerms {
    /// Resolve the flat option volatility for a live (unexercised) option.
    fn live_volatility(implied_volatility: Option<f64>) -> finstack_quant_core::Result<f64> {
        let volatility = implied_volatility.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "a live futures option requires instrument_pricing_overrides.market_quotes.implied_volatility"
                    .to_string(),
            )
        })?;
        if !volatility.is_finite() || volatility < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "instrument_pricing_overrides.market_quotes.implied_volatility must be finite and non-negative, got {volatility}"
            )));
        }
        Ok(volatility)
    }

    pub(crate) fn intrinsic(&self, futures_price: f64) -> f64 {
        match self.option_type {
            OptionType::Call => (futures_price - self.strike).max(0.0),
            OptionType::Put => (self.strike - futures_price).max(0.0),
        }
    }

    pub(crate) fn is_in_the_money(&self, futures_price: f64) -> bool {
        match self.option_type {
            OptionType::Call => futures_price > self.strike,
            OptionType::Put => futures_price < self.strike,
        }
    }

    pub(crate) fn exercise_direction(&self) -> f64 {
        let option_direction = match self.option_type {
            OptionType::Call => 1.0,
            OptionType::Put => -1.0,
        };
        self.position.sign() * option_direction
    }

    fn time_to_expiry(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
        if as_of >= self.expiry {
            return Ok(0.0);
        }
        self.day_count
            .year_fraction(as_of, self.expiry, DayCountContext::default())
    }

    fn discount_factor(
        &self,
        market: &MarketContext,
        as_of: Date,
        date: Date,
    ) -> finstack_quant_core::Result<f64> {
        let df = market
            .get_discount(&self.discount_curve_id)?
            .df_between_dates(as_of, date)?;
        if !df.is_finite() || df <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FutureOptionTerms discount factor from {as_of} to {date} must be finite and positive, got {df}"
            )));
        }
        Ok(df)
    }

    pub(crate) fn pricing_discount_factor(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        if self.premium_style == FutureOptionPremiumStyle::FuturesStyle {
            return Ok(1.0);
        }
        let payment_date = match self.settlement {
            FutureOptionSettlement::Cash { payment_date } => payment_date,
            FutureOptionSettlement::Future { .. } => self.expiry,
        };
        self.discount_factor(market, as_of, payment_date)
    }

    fn european_unit_price(
        &self,
        futures_price: f64,
        volatility: f64,
        t: f64,
        df: f64,
    ) -> finstack_quant_core::Result<f64> {
        if t <= 0.0 || volatility <= 0.0 {
            return Ok(df * self.intrinsic(futures_price));
        }
        let undiscounted = match self.model {
            FutureOptionModel::Black76 => {
                if futures_price <= 0.0 || self.strike <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(
                        "FutureOptionTerms Black-76 requires positive bumped futures price and strike"
                            .to_string(),
                    ));
                }
                let (d1, d2) = d1_d2_black76(futures_price, self.strike, volatility, t);
                match self.option_type {
                    OptionType::Call => futures_price * norm_cdf(d1)? - self.strike * norm_cdf(d2)?,
                    OptionType::Put => {
                        self.strike * norm_cdf(-d2)? - futures_price * norm_cdf(-d1)?
                    }
                }
            }
            FutureOptionModel::Normal => bachelier_price_with_annuity(
                self.option_type,
                futures_price,
                self.strike,
                volatility,
                t,
                1.0,
            ),
        };
        Ok(df * undiscounted)
    }

    fn additive_american_unit_price(
        &self,
        futures_price: f64,
        volatility: f64,
        t: f64,
        rate: f64,
        steps: usize,
    ) -> finstack_quant_core::Result<f64> {
        if t <= 0.0 || volatility <= 0.0 {
            return Ok(self.intrinsic(futures_price));
        }
        if steps == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "FutureOptionTerms additive lattice requires at least one step".to_string(),
            ));
        }
        let dt = t / steps as f64;
        let move_size = volatility * dt.sqrt();
        let discount = (-rate * dt).exp();
        let mut values = (0..=steps)
            .map(|up_moves| {
                let terminal = futures_price + (2.0 * up_moves as f64 - steps as f64) * move_size;
                self.intrinsic(terminal)
            })
            .collect::<Vec<_>>();
        for step in (0..steps).rev() {
            for up_moves in 0..=step {
                let continuation = discount * 0.5 * (values[up_moves] + values[up_moves + 1]);
                let node_price = futures_price + (2.0 * up_moves as f64 - step as f64) * move_size;
                values[up_moves] = continuation.max(self.intrinsic(node_price));
            }
        }
        Ok(values[0])
    }

    pub(crate) fn live_unit_price(
        &self,
        market: &MarketContext,
        as_of: Date,
        futures_price: f64,
        volatility: f64,
        tree_steps: Option<usize>,
    ) -> finstack_quant_core::Result<f64> {
        let t = self.time_to_expiry(as_of)?;
        let df = self.pricing_discount_factor(market, as_of)?;
        if self.exercise_style == ExerciseStyle::European
            || matches!(self.settlement, FutureOptionSettlement::Cash { .. })
        {
            // Every exercise time has the same contractual cash payment date.
            // Convexity and the martingale futures price make waiting optimal.
            return self.european_unit_price(futures_price, volatility, t, df);
        }
        if t <= 0.0 || volatility <= 0.0 {
            // Delivery has exercise-time VM economics. With deterministic
            // futures and the lattice's constant effective rate, either
            // immediate exercise or waiting to expiry is optimal.
            return Ok(self.intrinsic(futures_price) * df.max(1.0));
        }
        let rate = if self.premium_style == FutureOptionPremiumStyle::FuturesStyle {
            0.0
        } else {
            crate::instruments::pricing::discounting::zero_rate_from_df(df, t, "FutureOptionTerms")?
        };
        let steps = tree_steps.unwrap_or(OFFICIAL_TREE_STEPS);
        match self.model {
            FutureOptionModel::Black76 => {
                let params = OptionMarketParams {
                    spot: futures_price,
                    strike: self.strike,
                    rate,
                    dividend_yield: rate,
                    volatility,
                    time_to_expiry: t,
                    option_type: self.option_type,
                };
                params.validate()?;
                BinomialTree::new(
                    steps,
                    finstack_quant_models::trees::binomial_tree::TreeType::LeisenReimer,
                )
                .price_american(&params, &[])
            }
            FutureOptionModel::Normal => {
                self.additive_american_unit_price(futures_price, volatility, t, rate, steps)
            }
        }
    }

    pub(crate) fn position_scale(&self) -> f64 {
        self.position.sign() * self.contracts * self.multiplier
    }

    pub(crate) fn position_value_from_unit_quote(
        &self,
        unit_quote: f64,
    ) -> finstack_quant_core::Result<f64> {
        if !unit_quote.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FutureOptionTerms option or delivered-future quote must be finite, got {unit_quote}"
            )));
        }
        let reference = match self.premium_style {
            FutureOptionPremiumStyle::PremiumPaid => 0.0,
            FutureOptionPremiumStyle::FuturesStyle => {
                self.option_reference_price.ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "FutureOptionTerms futures-style margining requires option_reference_price"
                            .to_string(),
                    )
                })?
            }
        };
        Ok(self.position_scale() * (unit_quote - reference))
    }

    /// Terms with the expiry observation a theta roll reaches, if any.
    ///
    /// Returns `Some` when expiry lies in `(as_of, rolled_date]` and no
    /// exercise is recorded: the copy expires at the quoted `futures_price`,
    /// which theta holds fixed.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date the roll starts from.
    /// * `rolled_date` - Date theta reprices at.
    pub fn expired_over_roll(
        &self,
        as_of: Date,
        rolled_date: Date,
    ) -> finstack_quant_core::Result<Option<Self>> {
        if self.exercise.is_some() || self.expiry <= as_of || self.expiry > rolled_date {
            return Ok(None);
        }
        let mut observed = self.clone();
        observed.exercise = Some(FutureOptionExercise::new(self.expiry, self.futures_price)?);
        Ok(Some(observed))
    }

    fn finite_difference_bump(&self, relative: f64, minimum: f64) -> f64 {
        let requested = (self.futures_price.abs() * relative).max(minimum);
        if self.model == FutureOptionModel::Black76 {
            requested.min(self.futures_price * 0.5)
        } else {
            requested
        }
    }

    fn lifecycle_state(
        &self,
        instrument_id: Option<&InstrumentId>,
        as_of: Date,
    ) -> finstack_quant_core::Result<OptionLifecycleState> {
        if as_of > self.settlement.terminal_date() {
            return Ok(OptionLifecycleState::Terminal);
        }
        if let Some(exercise) = self.exercise
            && as_of >= exercise.date
        {
            return Ok(OptionLifecycleState::PostExercise(exercise));
        }
        if as_of >= self.expiry {
            let label = instrument_id.map_or("futures option", |id| id.as_str());
            return Err(finstack_quant_core::Error::Validation(format!(
                "FutureOptionTerms '{}' requires an exercise/expiry observation from expiry {} onward",
                label, self.expiry
            )));
        }
        Ok(OptionLifecycleState::Live)
    }

    fn post_exercise_value(
        &self,
        instrument_id: &InstrumentId,
        market: &MarketContext,
        as_of: Date,
        exercise: FutureOptionExercise,
    ) -> finstack_quant_core::Result<f64> {
        let exercised = self.is_in_the_money(exercise.futures_price);
        match self.settlement {
            FutureOptionSettlement::Cash { payment_date } => {
                if as_of > payment_date {
                    return Ok(0.0);
                }
                let df = if self.premium_style == FutureOptionPremiumStyle::PremiumPaid {
                    self.discount_factor(market, as_of, payment_date)?
                } else {
                    1.0
                };
                self.position_value_from_unit_quote(self.intrinsic(exercise.futures_price) * df)
            }
            FutureOptionSettlement::Future {
                underlying_last_trading_date,
                underlying_settlement_date,
                underlying_settlement_price,
            } => {
                if as_of > underlying_settlement_date {
                    return Ok(0.0);
                }
                let mark = if as_of > underlying_last_trading_date {
                    underlying_settlement_price.ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "FutureOptionTerms '{}' requires underlying_settlement_price after underlying last trading date {}",
                            instrument_id, underlying_last_trading_date
                        ))
                    })?
                } else {
                    self.futures_price
                };
                let delivered_future_quote = if exercised {
                    let option_direction = match self.option_type {
                        OptionType::Call => 1.0,
                        OptionType::Put => -1.0,
                    };
                    option_direction * (mark - self.strike)
                } else {
                    0.0
                };
                self.position_value_from_unit_quote(delivered_future_quote)
            }
        }
    }

    /// Return the contract's signed fair value in settlement currency.
    ///
    /// From expiry onward, `exercise` is required to distinguish an expired
    /// out-of-the-money option from an exercised or assigned position.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Concrete asset-class instrument identifier used in lifecycle errors.
    /// * `tree_steps` - Optional lattice step count for American future
    ///   delivery; defaults to 401. Fixed-date cash settlement is analytic.
    /// * `implied_volatility` - Flat option volatility from
    ///   `instrument_pricing_overrides.market_quotes.implied_volatility`: decimal
    ///   lognormal for Black-76, futures-price points per square-root year for
    ///   the normal model. Required while the option is live; ignored once it
    ///   is exercised.
    /// * `market` - Market context containing the settlement discount curve.
    /// * `as_of` - Valuation date controlling live versus exercised lifecycle state.
    pub fn npv_raw(
        &self,
        instrument_id: &InstrumentId,
        tree_steps: Option<usize>,
        implied_volatility: Option<f64>,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        match self.lifecycle_state(Some(instrument_id), as_of)? {
            OptionLifecycleState::Terminal => Ok(0.0),
            OptionLifecycleState::PostExercise(exercise) => {
                self.post_exercise_value(instrument_id, market, as_of, exercise)
            }
            OptionLifecycleState::Live => {
                let volatility = Self::live_volatility(implied_volatility)?;
                let unit_quote = self.live_unit_price(
                    market,
                    as_of,
                    self.futures_price,
                    volatility,
                    tree_steps,
                )?;
                self.position_value_from_unit_quote(unit_quote)
            }
        }
    }

    fn cash_delta_with_bump(
        &self,
        tree_steps: Option<usize>,
        implied_volatility: Option<f64>,
        market: &MarketContext,
        as_of: Date,
        relative_spot_bump: f64,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        match self.lifecycle_state(None, as_of)? {
            OptionLifecycleState::Terminal => Ok(0.0),
            OptionLifecycleState::PostExercise(exercise) => match self.settlement {
                FutureOptionSettlement::Future {
                    underlying_last_trading_date,
                    underlying_settlement_date,
                    underlying_settlement_price,
                } if as_of <= underlying_settlement_date => {
                    if as_of > underlying_last_trading_date {
                        underlying_settlement_price.ok_or_else(|| {
                            finstack_quant_core::Error::Validation(format!(
                                "FutureOptionTerms requires underlying_settlement_price after underlying last trading date {underlying_last_trading_date}"
                            ))
                        })?;
                        Ok(0.0)
                    } else if self.is_in_the_money(exercise.futures_price) {
                        ensure_finite_greek(
                            "delta",
                            self.exercise_direction() * self.contracts * self.multiplier,
                        )
                    } else {
                        Ok(0.0)
                    }
                }
                _ => Ok(0.0),
            },
            OptionLifecycleState::Live => {
                let rel_bump = validate_positive_bump("spot bump", relative_spot_bump)?;
                let volatility = Self::live_volatility(implied_volatility)?;
                let bump = self.finite_difference_bump(rel_bump, rel_bump.min(1e-4));
                let steps = Some(tree_steps.unwrap_or(RISK_TREE_STEPS));
                let up = self.live_unit_price(
                    market,
                    as_of,
                    self.futures_price + bump,
                    volatility,
                    steps,
                )?;
                let down = self.live_unit_price(
                    market,
                    as_of,
                    self.futures_price - bump,
                    volatility,
                    steps,
                )?;
                ensure_finite_greek("delta", self.position_scale() * (up - down) / (2.0 * bump))
            }
        }
    }

    /// Cash delta for a one-point move in the underlying futures price.
    ///
    /// After delivery, live futures-price delta remains until last trading.
    /// Once official settlement fixes the mark, that live-price delta is zero;
    /// a missing required official settlement price returns a validation error.
    ///
    /// # Arguments
    ///
    /// * `tree_steps` - Optional American lattice step count; defaults to 201
    ///   for greeks and 401 for official PV.
    /// * `implied_volatility` - Flat option volatility from
    ///   `instrument_pricing_overrides.market_quotes.implied_volatility`, in the
    ///   model's units; required while the option is live.
    /// * `market` - Market context containing the settlement discount curve.
    /// * `as_of` - Valuation date.
    pub fn cash_delta(
        &self,
        tree_steps: Option<usize>,
        implied_volatility: Option<f64>,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.cash_delta_with_bump(tree_steps, implied_volatility, market, as_of, 1e-4)
    }

    fn cash_gamma_with_bump(
        &self,
        tree_steps: Option<usize>,
        implied_volatility: Option<f64>,
        market: &MarketContext,
        as_of: Date,
        relative_spot_bump: f64,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        match self.lifecycle_state(None, as_of)? {
            OptionLifecycleState::Terminal | OptionLifecycleState::PostExercise(_) => Ok(0.0),
            OptionLifecycleState::Live => {
                let rel_bump = validate_positive_bump("spot bump", relative_spot_bump)?;
                let volatility = Self::live_volatility(implied_volatility)?;
                let bump = self.finite_difference_bump(rel_bump, rel_bump.min(1e-3));
                let steps = Some(tree_steps.unwrap_or(RISK_TREE_STEPS));
                let base =
                    self.live_unit_price(market, as_of, self.futures_price, volatility, steps)?;
                let up = self.live_unit_price(
                    market,
                    as_of,
                    self.futures_price + bump,
                    volatility,
                    steps,
                )?;
                let down = self.live_unit_price(
                    market,
                    as_of,
                    self.futures_price - bump,
                    volatility,
                    steps,
                )?;
                ensure_finite_greek(
                    "gamma",
                    self.position_scale() * (up - 2.0 * base + down) / (bump * bump),
                )
            }
        }
    }

    /// Cash gamma for a one-point squared move in the underlying futures price.
    ///
    /// # Arguments
    ///
    /// * `tree_steps` - Optional American lattice step count; defaults to 201
    ///   for greeks and 401 for official PV.
    /// * `implied_volatility` - Flat option volatility from
    ///   `instrument_pricing_overrides.market_quotes.implied_volatility`, in the
    ///   model's units; required while the option is live.
    /// * `market` - Market context containing the settlement discount curve.
    /// * `as_of` - Valuation date.
    pub fn cash_gamma(
        &self,
        tree_steps: Option<usize>,
        implied_volatility: Option<f64>,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.cash_gamma_with_bump(tree_steps, implied_volatility, market, as_of, 1e-3)
    }

    fn cash_vega_with_bump(
        &self,
        tree_steps: Option<usize>,
        implied_volatility: Option<f64>,
        market: &MarketContext,
        as_of: Date,
        vol_bump: f64,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        match self.lifecycle_state(None, as_of)? {
            OptionLifecycleState::Terminal | OptionLifecycleState::PostExercise(_) => Ok(0.0),
            OptionLifecycleState::Live => {
                let bump = validate_positive_bump("volatility bump", vol_bump)?;
                let volatility = Self::live_volatility(implied_volatility)?;
                let steps = Some(tree_steps.unwrap_or(RISK_TREE_STEPS));
                let base =
                    self.live_unit_price(market, as_of, self.futures_price, volatility, steps)?;
                let up = self.live_unit_price(
                    market,
                    as_of,
                    self.futures_price,
                    volatility + bump,
                    steps,
                )?;
                ensure_finite_greek("vega", self.position_scale() * (up - base) * (0.01 / bump))
            }
        }
    }

    /// Cash vega for a +0.01 absolute bump in the `implied_volatility` units.
    ///
    /// For Black-76 this is one lognormal vol point. For the normal model it is
    /// 0.01 futures-price points per square-root year (one basis point when a
    /// rate future is quoted as `100 - rate_percent`).
    ///
    /// # Arguments
    ///
    /// * `tree_steps` - Optional American lattice step count; defaults to 201
    ///   for greeks and 401 for official PV.
    /// * `implied_volatility` - Flat option volatility from
    ///   `instrument_pricing_overrides.market_quotes.implied_volatility`, in the
    ///   model's units; required while the option is live.
    /// * `market` - Market context containing the settlement discount curve.
    /// * `as_of` - Valuation date.
    pub fn cash_vega(
        &self,
        tree_steps: Option<usize>,
        implied_volatility: Option<f64>,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.cash_vega_with_bump(tree_steps, implied_volatility, market, as_of, 0.01)
    }

    /// One-calendar-day theta with market quotes held fixed.
    ///
    /// # Arguments
    ///
    /// * `tree_steps` - Optional American lattice step count; defaults to 201
    ///   for greeks and 401 for official PV.
    /// * `implied_volatility` - Flat option volatility from
    ///   `instrument_pricing_overrides.market_quotes.implied_volatility`, in the
    ///   model's units; required while the option is live.
    /// * `market` - Market context containing the settlement discount curve.
    /// * `as_of` - Valuation date.
    pub fn cash_theta(
        &self,
        tree_steps: Option<usize>,
        implied_volatility: Option<f64>,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        match self.lifecycle_state(None, as_of)? {
            OptionLifecycleState::Terminal | OptionLifecycleState::PostExercise(_) => Ok(0.0),
            OptionLifecycleState::Live => {
                let volatility = Self::live_volatility(implied_volatility)?;
                let next_date = (as_of + time::Duration::days(1)).min(self.expiry);
                let steps = Some(tree_steps.unwrap_or(RISK_TREE_STEPS));
                let base =
                    self.live_unit_price(market, as_of, self.futures_price, volatility, steps)?;
                let next =
                    self.live_unit_price(market, next_date, self.futures_price, volatility, steps)?;
                ensure_finite_greek("theta", self.position_scale() * (next - base))
            }
        }
    }

    /// Compute the requested option Greek honoring the bump configuration.
    ///
    /// # Arguments
    ///
    /// * `tree_steps` - Optional American lattice step count; defaults to 201.
    /// * `implied_volatility` - Flat option volatility from
    ///   `instrument_pricing_overrides.market_quotes.implied_volatility`, in the
    ///   model's units; required while the option is live.
    /// * `market` - Market context containing the settlement discount curve.
    /// * `as_of` - Valuation date.
    /// * `request` - Metric request carrying the target [`OptionGreekKind`] and
    ///   finite-difference bump sizes.
    pub fn option_greeks(
        &self,
        tree_steps: Option<usize>,
        implied_volatility: Option<f64>,
        market: &MarketContext,
        as_of: Date,
        request: &OptionGreeksRequest,
    ) -> finstack_quant_core::Result<OptionGreeks> {
        let mut greeks = OptionGreeks::default();
        match request.greek {
            OptionGreekKind::Delta => {
                greeks.delta = Some(self.cash_delta_with_bump(
                    tree_steps,
                    implied_volatility,
                    market,
                    as_of,
                    request.bumps.spot_bump_decimal,
                )?);
            }
            OptionGreekKind::Gamma => {
                greeks.gamma = Some(self.cash_gamma_with_bump(
                    tree_steps,
                    implied_volatility,
                    market,
                    as_of,
                    request.bumps.spot_bump_decimal,
                )?);
            }
            OptionGreekKind::Vega => {
                greeks.vega = Some(self.cash_vega_with_bump(
                    tree_steps,
                    implied_volatility,
                    market,
                    as_of,
                    request.bumps.vol_bump_decimal,
                )?);
            }
            OptionGreekKind::Theta => {
                greeks.theta =
                    Some(self.cash_theta(tree_steps, implied_volatility, market, as_of)?);
            }
            _ => {}
        }
        Ok(greeks)
    }
}
