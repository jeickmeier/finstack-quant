//! Cross-asset options on futures for listed and bilateral contracts.
//!
//! The contract keeps all economics explicit: exercise style, premium
//! margining, cash versus futures delivery, multiplier, contract count, and
//! the post-exercise lifecycle are all contractual inputs. European options
//! use Black-76 or Bachelier directly. American Black-76 options use the shared
//! multiplicative tree with zero futures drift; American normal options use an
//! additive recombining futures lattice.

use crate::instruments::common_impl::parameters::{ExerciseStyle, OptionMarketParams, OptionType};
use crate::instruments::{Position, SettlementType};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::norm_cdf;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::trees::binomial_tree::BinomialTree;
use finstack_quant_models::volatility::black::d1_d2_black76;
use finstack_quant_models::volatility::normal::bachelier_price;

/// Official American lattice size when `tree_steps` is omitted on PV.
const OFFICIAL_TREE_STEPS: usize = 401;
/// Default American lattice size for finite-difference greeks.
const RISK_TREE_STEPS: usize = 201;

/// Quotation model used for an option on a futures price.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FutureOptionModel {
    /// Black-76 with decimal lognormal volatility and positive price/strike.
    Black76,
    /// Bachelier with normal volatility in futures-price points per square-root year.
    Normal,
}

/// Premium-settlement convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FutureOptionPremiumStyle {
    /// Premium is paid up front, so the expected expiry payoff is discounted.
    PremiumPaid,
    /// Option value is variation-margined, so its quoted value is undiscounted.
    FuturesStyle,
}

/// Settlement delivered by exercise or assignment.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FutureOptionSettlement {
    /// Cash payment of intrinsic value on the supplied date.
    Cash {
        /// Date on which the fixed exercise payoff is paid.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        payment_date: Date,
    },
    /// Delivery of a futures position entered at the option strike.
    Future {
        /// Last trading date of the delivered underlying future.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        underlying_last_trading_date: Date,
        /// Final settlement date of the delivered underlying future.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        underlying_settlement_date: Date,
        /// Official final settlement of the delivered future once trading has ended.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        underlying_settlement_price: Option<f64>,
    },
}

impl FutureOptionSettlement {
    fn settlement_type(&self) -> SettlementType {
        match self {
            Self::Cash { .. } => SettlementType::Cash,
            Self::Future { .. } => SettlementType::Physical,
        }
    }

    pub(crate) fn terminal_date(&self) -> Date {
        match self {
            Self::Cash { payment_date } => *payment_date,
            Self::Future {
                underlying_settlement_date,
                ..
            } => *underlying_settlement_date,
        }
    }
}

/// Exercise or assignment observation for an option on a future.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FutureOptionExercise {
    /// Exercise date. European options require this to equal contractual expiry.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Official underlying futures price used to determine exercise.
    pub futures_price: f64,
}

impl FutureOptionExercise {
    /// Construct a recorded exercise or expiry observation.
    ///
    /// # Arguments
    ///
    /// * `date` - Exercise or contractual-expiry date.
    /// * `futures_price` - Finite official underlying futures price in contract points.
    pub fn new(date: Date, futures_price: f64) -> finstack_quant_core::Result<Self> {
        if !futures_price.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "future-option exercise futures_price must be finite".to_string(),
            ));
        }
        Ok(Self {
            date,
            futures_price,
        })
    }
}

/// Shared contractual and pricing terms for an asset-owned option on a future.
#[derive(
    Clone,
    Debug,
    PartialEq,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[builder(validate = FutureOptionTerms::validate)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FutureOptionTerms {
    /// Underlying futures identifier or descriptive label.
    pub underlying: String,
    /// Current futures mark in the contract's price units.
    pub futures_price: f64,
    /// Option trade or settlement reference price in option price points.
    ///
    /// Required for [`FutureOptionPremiumStyle::FuturesStyle`]. Set this to the
    /// trade price for cumulative P&L since inception or to the preceding
    /// official settlement price for one-day variation margin.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option_reference_price: Option<f64>,
    /// Optional change in the underlying futures price, in price points, for
    /// a one-basis-point increase in its mapped rate or yield risk factor.
    ///
    /// Rate-futures-option wrappers use this caller-supplied transform to
    /// report DV01 without inferring economics from an exchange symbol.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underlying_price_change_per_bp: Option<f64>,
    /// Option strike in the same futures-price points.
    pub strike: f64,
    /// Number of option contracts.
    pub contracts: f64,
    /// Settlement-currency value of one underlying futures price point per contract.
    pub multiplier: f64,
    /// Premium, variation-margin, and settlement currency.
    pub currency: Currency,
    /// Call or put payoff.
    pub option_type: OptionType,
    /// Long-holder or short-writer position.
    #[builder(default)]
    #[serde(default)]
    pub position: Position,
    /// European or American exercise convention.
    pub exercise_style: ExerciseStyle,
    /// Contractual option expiry.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// Cash payment or delivery of an underlying future.
    pub settlement: FutureOptionSettlement,
    /// Recorded early-exercise or expiry observation. Required from expiry onward.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exercise: Option<FutureOptionExercise>,
    /// Annualized volatility. Decimal lognormal units for Black-76; futures-price
    /// points per square-root year for the normal model.
    pub volatility: f64,
    /// Black-76 or normal quotation model.
    pub model: FutureOptionModel,
    /// Up-front premium or futures-style variation-margin convention.
    pub premium_style: FutureOptionPremiumStyle,
    /// Day-count convention for option time and discount-rate inference.
    pub day_count: DayCount,
    /// Settlement-currency discount curve.
    pub discount_curve_id: CurveId,
}

impl FutureOptionTerms {
    /// Validate contract economics, model domain, and lifecycle dates.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if self.underlying.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "FutureOptionTerms underlying must not be empty".to_string(),
            ));
        }
        for (name, value) in [
            ("futures_price", self.futures_price),
            ("strike", self.strike),
            ("contracts", self.contracts),
            ("multiplier", self.multiplier),
        ] {
            if !value.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FutureOptionTerms {name} must be finite"
                )));
            }
        }
        if self.contracts <= 0.0 || self.multiplier <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "FutureOptionTerms contracts and multiplier must be positive".to_string(),
            ));
        }
        if !self.volatility.is_finite() || self.volatility < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "FutureOptionTerms volatility must be finite and non-negative".to_string(),
            ));
        }
        if self
            .option_reference_price
            .is_some_and(|price| !price.is_finite() || price < 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(
                "FutureOptionTerms option_reference_price must be finite and non-negative"
                    .to_string(),
            ));
        }
        if self
            .underlying_price_change_per_bp
            .is_some_and(|change| !change.is_finite())
        {
            return Err(finstack_quant_core::Error::Validation(
                "FutureOptionTerms underlying_price_change_per_bp must be finite when supplied"
                    .to_string(),
            ));
        }
        if self.premium_style == FutureOptionPremiumStyle::FuturesStyle
            && self.option_reference_price.is_none()
        {
            return Err(finstack_quant_core::Error::Validation(
                "FutureOptionTerms futures-style margining requires option_reference_price"
                    .to_string(),
            ));
        }
        if self.model == FutureOptionModel::Black76
            && (self.futures_price <= 0.0 || self.strike <= 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(
                "FutureOptionTerms Black-76 requires positive futures_price and strike".to_string(),
            ));
        }
        if self.exercise_style == ExerciseStyle::Bermudan {
            return Err(finstack_quant_core::Error::Validation(
                "FutureOptionTerms supports European or American exercise; Bermudan requires an explicit exercise schedule"
                    .to_string(),
            ));
        }
        match &self.settlement {
            FutureOptionSettlement::Cash { payment_date } => {
                if *payment_date < self.expiry {
                    return Err(finstack_quant_core::Error::Validation(
                        "FutureOptionTerms cash payment_date must be on or after expiry"
                            .to_string(),
                    ));
                }
            }
            FutureOptionSettlement::Future {
                underlying_last_trading_date,
                underlying_settlement_date,
                underlying_settlement_price,
            } => {
                if *underlying_last_trading_date < self.expiry
                    || *underlying_settlement_date < *underlying_last_trading_date
                {
                    return Err(finstack_quant_core::Error::Validation(
                        "FutureOptionTerms delivered-future dates must satisfy expiry <= last trading <= settlement"
                            .to_string(),
                    ));
                }
                if underlying_settlement_price.is_some_and(|price| !price.is_finite()) {
                    return Err(finstack_quant_core::Error::Validation(
                        "FutureOptionTerms underlying_settlement_price must be finite".to_string(),
                    ));
                }
            }
        }
        if let Some(exercise) = self.exercise {
            if !exercise.futures_price.is_finite() || exercise.date > self.expiry {
                return Err(finstack_quant_core::Error::Validation(
                    "FutureOptionTerms exercise must have a finite price and date no later than expiry"
                        .to_string(),
                ));
            }
            if self.exercise_style == ExerciseStyle::European && exercise.date != self.expiry {
                return Err(finstack_quant_core::Error::Validation(
                    "FutureOptionTerms European exercise observation must be on expiry".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Create neutral terms for schema, serde, and documentation examples.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        Self::builder()
            .underlying("UNDERLYING-FUTURE".to_string())
            .futures_price(100.0)
            .strike(100.0)
            .contracts(1.0)
            .multiplier(1.0)
            .currency(Currency::USD)
            .option_type(OptionType::Call)
            .position(Position::Long)
            .exercise_style(ExerciseStyle::European)
            .expiry(date!(2026 - 12 - 14))
            .settlement(FutureOptionSettlement::Cash {
                payment_date: date!(2026 - 12 - 14),
            })
            .volatility(0.20)
            .model(FutureOptionModel::Black76)
            .premium_style(FutureOptionPremiumStyle::PremiumPaid)
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
    }

    fn intrinsic(&self, futures_price: f64) -> f64 {
        match self.option_type {
            OptionType::Call => (futures_price - self.strike).max(0.0),
            OptionType::Put => (self.strike - futures_price).max(0.0),
        }
    }

    fn is_in_the_money(&self, futures_price: f64) -> bool {
        match self.option_type {
            OptionType::Call => futures_price > self.strike,
            OptionType::Put => futures_price < self.strike,
        }
    }

    fn exercise_direction(&self) -> f64 {
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
        market
            .get_discount(&self.discount_curve_id)?
            .df_between_dates(as_of, date)
    }

    fn pricing_discount_factor(
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
                    OptionType::Call => futures_price * norm_cdf(d1) - self.strike * norm_cdf(d2),
                    OptionType::Put => self.strike * norm_cdf(-d2) - futures_price * norm_cdf(-d1),
                }
            }
            FutureOptionModel::Normal => bachelier_price(
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

    fn live_unit_price(
        &self,
        market: &MarketContext,
        as_of: Date,
        futures_price: f64,
        volatility: f64,
        tree_steps: Option<usize>,
    ) -> finstack_quant_core::Result<f64> {
        let t = self.time_to_expiry(as_of)?;
        let df = self.pricing_discount_factor(market, as_of)?;
        if self.exercise_style == ExerciseStyle::European {
            return self.european_unit_price(futures_price, volatility, t, df);
        }
        if t <= 0.0 || volatility <= 0.0 {
            return Ok(self.intrinsic(futures_price));
        }
        let rate = if self.premium_style == FutureOptionPremiumStyle::FuturesStyle {
            0.0
        } else {
            -df.ln() / t
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
                BinomialTree::leisen_reimer(steps).price_american(&params)
            }
            FutureOptionModel::Normal => {
                self.additive_american_unit_price(futures_price, volatility, t, rate, steps)
            }
        }
    }

    fn position_scale(&self) -> f64 {
        self.position.sign() * self.contracts * self.multiplier
    }

    fn position_value_from_unit_quote(&self, unit_quote: f64) -> finstack_quant_core::Result<f64> {
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

    fn finite_difference_bump(&self, relative: f64, minimum: f64) -> f64 {
        let requested = (self.futures_price.abs() * relative).max(minimum);
        if self.model == FutureOptionModel::Black76 {
            requested.min(self.futures_price * 0.5)
        } else {
            requested
        }
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
    /// * `tree_steps` - Optional American lattice step count; defaults to 401.
    /// * `market` - Market context containing the settlement discount curve.
    /// * `as_of` - Valuation date controlling live versus exercised lifecycle state.
    pub fn npv_raw(
        &self,
        instrument_id: &InstrumentId,
        tree_steps: Option<usize>,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        if as_of > self.settlement.terminal_date() {
            return Ok(0.0);
        }
        if let Some(exercise) = self.exercise {
            if as_of >= exercise.date {
                return self.post_exercise_value(instrument_id, market, as_of, exercise);
            }
        }
        if as_of >= self.expiry {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FutureOptionTerms '{}' requires an exercise/expiry observation from expiry {} onward",
                instrument_id, self.expiry
            )));
        }
        let unit_quote = self.live_unit_price(
            market,
            as_of,
            self.futures_price,
            self.volatility,
            tree_steps,
        )?;
        self.position_value_from_unit_quote(unit_quote)
    }

    /// Cash delta for a one-point move in the underlying futures price.
    ///
    /// # Arguments
    ///
    /// * `tree_steps` - Optional American lattice step count; defaults to 201
    ///   for greeks and 401 for official PV.
    /// * `market` - Market context containing the settlement discount curve.
    /// * `as_of` - Valuation date.
    pub fn cash_delta(
        &self,
        tree_steps: Option<usize>,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        if let Some(exercise) = self.exercise {
            if as_of >= exercise.date {
                return Ok(
                    if self.is_in_the_money(exercise.futures_price)
                        && self.settlement.settlement_type() == SettlementType::Physical
                        && as_of <= self.settlement.terminal_date()
                    {
                        self.exercise_direction() * self.contracts * self.multiplier
                    } else {
                        0.0
                    },
                );
            }
        }
        if as_of >= self.expiry {
            return Ok(0.0);
        }
        let bump = self.finite_difference_bump(1e-4, 1e-4);
        let steps = Some(tree_steps.unwrap_or(RISK_TREE_STEPS));
        let up = self.live_unit_price(
            market,
            as_of,
            self.futures_price + bump,
            self.volatility,
            steps,
        )?;
        let down = self.live_unit_price(
            market,
            as_of,
            self.futures_price - bump,
            self.volatility,
            steps,
        )?;
        Ok(self.position_scale() * (up - down) / (2.0 * bump))
    }

    /// Cash gamma for a one-point squared move in the underlying futures price.
    ///
    /// # Arguments
    ///
    /// * `tree_steps` - Optional American lattice step count; defaults to 201
    ///   for greeks and 401 for official PV.
    /// * `market` - Market context containing the settlement discount curve.
    /// * `as_of` - Valuation date.
    pub fn cash_gamma(
        &self,
        tree_steps: Option<usize>,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        if as_of >= self.expiry || self.exercise.is_some_and(|exercise| as_of >= exercise.date) {
            return Ok(0.0);
        }
        let bump = self.finite_difference_bump(1e-3, 1e-3);
        let steps = Some(tree_steps.unwrap_or(RISK_TREE_STEPS));
        let base =
            self.live_unit_price(market, as_of, self.futures_price, self.volatility, steps)?;
        let up = self.live_unit_price(
            market,
            as_of,
            self.futures_price + bump,
            self.volatility,
            steps,
        )?;
        let down = self.live_unit_price(
            market,
            as_of,
            self.futures_price - bump,
            self.volatility,
            steps,
        )?;
        Ok(self.position_scale() * (up - 2.0 * base + down) / (bump * bump))
    }

    /// Cash vega for a +0.01 absolute bump in the configured volatility units.
    ///
    /// For Black-76 this is one lognormal vol point. For the normal model it is
    /// 0.01 futures-price points per square-root year (one basis point when a
    /// rate future is quoted as `100 - rate_percent`).
    ///
    /// # Arguments
    ///
    /// * `tree_steps` - Optional American lattice step count; defaults to 201
    ///   for greeks and 401 for official PV.
    /// * `market` - Market context containing the settlement discount curve.
    /// * `as_of` - Valuation date.
    pub fn cash_vega(
        &self,
        tree_steps: Option<usize>,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        if as_of >= self.expiry || self.exercise.is_some_and(|exercise| as_of >= exercise.date) {
            return Ok(0.0);
        }
        let steps = Some(tree_steps.unwrap_or(RISK_TREE_STEPS));
        let base =
            self.live_unit_price(market, as_of, self.futures_price, self.volatility, steps)?;
        let up = self.live_unit_price(
            market,
            as_of,
            self.futures_price,
            self.volatility + 0.01,
            steps,
        )?;
        Ok(self.position_scale() * (up - base))
    }

    /// One-calendar-day theta with market quotes held fixed.
    ///
    /// # Arguments
    ///
    /// * `tree_steps` - Optional American lattice step count; defaults to 201
    ///   for greeks and 401 for official PV.
    /// * `market` - Market context containing the settlement discount curve.
    /// * `as_of` - Valuation date.
    pub fn cash_theta(
        &self,
        tree_steps: Option<usize>,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        if as_of >= self.expiry || self.exercise.is_some_and(|exercise| as_of >= exercise.date) {
            return Ok(0.0);
        }
        let next_date = (as_of + time::Duration::days(1)).min(self.expiry);
        let steps = Some(tree_steps.unwrap_or(RISK_TREE_STEPS));
        let base =
            self.live_unit_price(market, as_of, self.futures_price, self.volatility, steps)?;
        let next = self.live_unit_price(
            market,
            next_date,
            self.futures_price,
            self.volatility,
            steps,
        )?;
        Ok(self.position_scale() * (next - base))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::macros::date;

    fn market(rate: f64) -> MarketContext {
        MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(date!(2026 - 01 - 01))
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (2.0, (-2.0 * rate).exp())])
                .build()
                .expect("discount curve"),
        )
    }

    fn option(model: FutureOptionModel, option_type: OptionType) -> FutureOptionTerms {
        FutureOptionTerms::builder()
            .underlying("TEST-Z6".to_string())
            .futures_price(100.0)
            .strike(100.0)
            .contracts(1.0)
            .multiplier(10.0)
            .currency(Currency::USD)
            .option_type(option_type)
            .position(Position::Long)
            .exercise_style(ExerciseStyle::European)
            .expiry(date!(2027 - 01 - 01))
            .settlement(FutureOptionSettlement::Cash {
                payment_date: date!(2027 - 01 - 01),
            })
            .volatility(match model {
                FutureOptionModel::Black76 => 0.20,
                FutureOptionModel::Normal => 5.0,
            })
            .model(model)
            .premium_style(FutureOptionPremiumStyle::PremiumPaid)
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("option")
    }

    #[test]
    fn put_call_parity_holds_for_black_and_normal() {
        let market = market(0.05);
        let as_of = date!(2026 - 01 - 01);
        for model in [FutureOptionModel::Black76, FutureOptionModel::Normal] {
            let call = option(model, OptionType::Call)
                .npv_raw(&InstrumentId::new("TEST-FOP"), None, &market, as_of)
                .expect("call");
            let put = option(model, OptionType::Put)
                .npv_raw(&InstrumentId::new("TEST-FOP"), None, &market, as_of)
                .expect("put");
            assert!((call - put).abs() < 1e-10);
        }
    }

    #[test]
    fn normal_atm_matches_closed_form() {
        let market = market(0.0);
        let as_of = date!(2026 - 01 - 01);
        let option = option(FutureOptionModel::Normal, OptionType::Call);
        let expected = 5.0 / (2.0 * std::f64::consts::PI).sqrt() * 10.0;
        let actual = option
            .npv_raw(&InstrumentId::new("TEST-FOP"), None, &market, as_of)
            .expect("normal pv");
        assert!((actual - expected).abs() < 1e-10 * expected);
    }

    #[test]
    fn futures_style_value_is_pnl_against_reference_quote() {
        let market = market(0.05);
        let as_of = date!(2026 - 01 - 01);
        let premium_paid = option(FutureOptionModel::Black76, OptionType::Call);
        let mut futures_style = premium_paid.clone();
        futures_style.premium_style = FutureOptionPremiumStyle::FuturesStyle;
        let paid = premium_paid
            .npv_raw(&InstrumentId::new("TEST-FOP"), None, &market, as_of)
            .expect("paid");
        let undiscounted_quote = futures_style
            .live_unit_price(
                &market,
                as_of,
                futures_style.futures_price,
                futures_style.volatility,
                None,
            )
            .expect("undiscounted quote");
        assert!(
            (undiscounted_quote * futures_style.multiplier / paid - 0.05_f64.exp()).abs() < 1e-10
        );

        futures_style.option_reference_price = Some(undiscounted_quote);
        let pnl = futures_style
            .npv_raw(&InstrumentId::new("TEST-FOP"), None, &market, as_of)
            .expect("margined pnl");
        assert!(pnl.abs() < 1e-10);
    }

    #[test]
    fn futures_style_requires_reference_quote() {
        let mut terms = option(FutureOptionModel::Black76, OptionType::Call);
        terms.premium_style = FutureOptionPremiumStyle::FuturesStyle;
        assert!(terms.validate().is_err());
    }

    #[test]
    fn black_greeks_keep_bumped_futures_prices_positive() {
        let market = market(0.0);
        let as_of = date!(2026 - 01 - 01);
        let mut terms = option(FutureOptionModel::Black76, OptionType::Call);
        terms.futures_price = 1.0e-8;
        terms.strike = 1.0e-8;
        assert!(terms
            .cash_delta(None, &market, as_of)
            .expect("delta")
            .is_finite());
        assert!(terms
            .cash_gamma(None, &market, as_of)
            .expect("gamma")
            .is_finite());
    }

    #[test]
    fn american_normal_lattice_does_not_undervalue_european() {
        let market = market(0.08);
        let as_of = date!(2026 - 01 - 01);
        let european = option(FutureOptionModel::Normal, OptionType::Put);
        let mut american = european.clone();
        american.exercise_style = ExerciseStyle::American;
        let european_pv = european
            .npv_raw(&InstrumentId::new("TEST-FOP"), None, &market, as_of)
            .expect("european");
        let american_pv = american
            .npv_raw(&InstrumentId::new("TEST-FOP"), None, &market, as_of)
            .expect("american");
        assert!(american_pv + 1e-10 >= european_pv);
    }

    #[test]
    fn delivered_future_keeps_directional_delta_after_exercise() {
        let market = market(0.0);
        let mut option = option(FutureOptionModel::Black76, OptionType::Call);
        option.expiry = date!(2026 - 06 - 01);
        option.futures_price = 104.0;
        option.strike = 100.0;
        option.settlement = FutureOptionSettlement::Future {
            underlying_last_trading_date: date!(2026 - 12 - 30),
            underlying_settlement_date: date!(2026 - 12 - 31),
            underlying_settlement_price: None,
        };
        option.exercise =
            Some(FutureOptionExercise::new(date!(2026 - 06 - 01), 102.0).expect("exercise"));
        let as_of = date!(2026 - 06 - 02);
        assert_eq!(
            option
                .npv_raw(&InstrumentId::new("TEST-FOP"), None, &market, as_of)
                .expect("pnl"),
            40.0
        );
        assert_eq!(
            option.cash_delta(None, &market, as_of).expect("delta"),
            10.0
        );
    }

    #[test]
    fn expiry_requires_official_observation() {
        let market = market(0.0);
        let option = option(FutureOptionModel::Normal, OptionType::Call);
        let error = option
            .npv_raw(&InstrumentId::new("TEST-FOP"), None, &market, option.expiry)
            .expect_err("missing expiry observation");
        assert!(error
            .to_string()
            .contains("requires an exercise/expiry observation"));
    }
}
