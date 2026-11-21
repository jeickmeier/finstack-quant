//! Cross-asset options on futures for listed and bilateral contracts.
//!
//! The contract keeps all economics explicit: exercise style, premium
//! margining, cash versus futures delivery, multiplier, contract count, and
//! the post-exercise lifecycle are all contractual inputs. European options
//! use Black-76 or Bachelier directly. The same expectation prices American
//! cash-settled options with a fixed payment date: waiting dominates exercise
//! under deterministic rates and martingale futures prices. Future-delivery
//! American options use a multiplicative or additive recombining futures lattice.

use crate::instruments::Position;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::types::CurveId;
use finstack_quant_models::types::{ExerciseStyle, OptionType};

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
        /// Date on which the fixed exercise payoff is paid, including after
        /// early exercise; this date is not relative to the exercise date.
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        payment_date: Date,
    },
    /// Delivery of a futures position entered at the option strike.
    Future {
        /// Last trading date of the delivered underlying future.
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        underlying_last_trading_date: Date,
        /// Final settlement date of the delivered underlying future.
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
    /// Return the final cash or futures settlement date for this contract.
    pub fn terminal_date(&self) -> Date {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option_reference_price: Option<f64>,
    /// Optional change in the underlying futures price, in price points, for
    /// a one-basis-point increase in its mapped rate or yield risk factor.
    ///
    /// Rate-futures-option wrappers use this caller-supplied transform to
    /// report DV01 without inferring economics from an exchange symbol.
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
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// Cash payment or delivery of an underlying future.
    pub settlement: FutureOptionSettlement,
    /// Recorded early-exercise or expiry observation. Required from expiry onward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exercise: Option<FutureOptionExercise>,
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
            .model(FutureOptionModel::Black76)
            .premium_style(FutureOptionPremiumStyle::PremiumPaid)
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
    }
}
