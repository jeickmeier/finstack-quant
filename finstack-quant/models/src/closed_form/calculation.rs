//! One closed-form option calculation: a price or an implied volatility, and
//! the Greeks at the resulting volatility.
//!
//! [`option_calculation`] dispatches an [`OptionCalculation`] onto the checked
//! scalar kernels ([`bs_price`], [`black76_price`], [`bachelier_price`], their
//! implied-volatility inverses and their Greeks), so a host sends one request
//! instead of choosing and sequencing those calls itself.
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_models::closed_form::{
//!     OptionCalculation, OptionModel, OptionSolve, option_calculation,
//! };
//!
//! let request = OptionCalculation {
//!     model: OptionModel::Black76 {
//!         forward: 100.0,
//!         strike: 100.0,
//!         df: 0.98,
//!         expiry: 1.0,
//!         is_call: true,
//!     },
//!     solve: OptionSolve::Price { vol: 0.2 },
//! };
//! let result = option_calculation(&request)?;
//! let inverse = option_calculation(&OptionCalculation {
//!     solve: OptionSolve::ImpliedVol { price: result.price },
//!     ..request
//! })?;
//! assert!((inverse.implied_vol - 0.2).abs() < 1e-8);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

use finstack_quant_core::Result;

use crate::closed_form::{
    BsGreeks, DEFAULT_THETA_DAYS_PER_YEAR, ForwardGreeks, bachelier_greeks, bachelier_price,
    black76_greeks, black76_implied_vol, black76_price, bs_greeks, bs_implied_vol, bs_price,
};
use crate::types::{OptionMarketParams, OptionType};
use crate::volatility::implied_vol_bachelier;

/// A closed-form model and the option terms it reads.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionModel {
    /// Black-Scholes-Merton on spot with continuous rate and dividend yield.
    BlackScholes {
        /// Underlying spot price, positive.
        spot: f64,
        /// Exercise price in the same units as `spot`, positive.
        strike: f64,
        /// Continuously compounded domestic risk-free rate as a decimal.
        rate: f64,
        /// Continuously compounded dividend yield or foreign rate as a decimal.
        div_yield: f64,
        /// Time to expiry in years.
        expiry: f64,
        /// `true` for a call, `false` for a put.
        is_call: bool,
        /// Theta denominator in days per year; omitted uses
        /// [`DEFAULT_THETA_DAYS_PER_YEAR`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        theta_days_per_year: Option<f64>,
    },
    /// Black-76 on a forward, discounted by `df`.
    Black76 {
        /// Forward price or rate at expiry, positive.
        forward: f64,
        /// Exercise price in the forward's units, positive.
        strike: f64,
        /// Discount factor from valuation to payment, positive.
        df: f64,
        /// Time to expiry in years.
        expiry: f64,
        /// `true` for a call, `false` for a put.
        is_call: bool,
    },
    /// Bachelier (normal) on a forward, undiscounted.
    Bachelier {
        /// Forward price or rate at expiry; any sign.
        forward: f64,
        /// Exercise price in the forward's units; any sign.
        strike: f64,
        /// Time to expiry in years.
        expiry: f64,
        /// `true` for a call, `false` for a put.
        is_call: bool,
    },
}

/// What the calculation solves for.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionSolve {
    /// Price the option at a volatility.
    Price {
        /// Annualized volatility: a decimal Black vol for Black-Scholes and
        /// Black-76, an absolute normal vol in the forward's units for
        /// Bachelier.
        vol: f64,
    },
    /// Invert a price to its implied volatility.
    ImpliedVol {
        /// Option price per unit in the model's convention: discounted for
        /// Black-Scholes and Black-76, undiscounted for Bachelier.
        price: f64,
    },
}

/// One option calculation request: a model with its terms and what to solve.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OptionCalculation {
    /// Closed-form model and option terms.
    pub model: OptionModel,
    /// Price at a volatility, or implied volatility of a price.
    pub solve: OptionSolve,
}

/// Greeks of the calculated option, by model family.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OptionGreeks {
    /// Black-Scholes Greeks.
    BlackScholes(BsGreeks),
    /// Black-76 or Bachelier forward Greeks.
    Forward(ForwardGreeks),
}

/// Result of an [`OptionCalculation`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OptionCalculationResult {
    /// Option price per unit: computed for a price solve, the supplied price
    /// for an implied-volatility solve.
    pub price: f64,
    /// Volatility the price and Greeks are at: the supplied vol for a price
    /// solve, the solved implied volatility otherwise.
    pub implied_vol: f64,
    /// Greeks at `implied_vol`.
    pub greeks: OptionGreeks,
}

/// Price an option or invert its price, then evaluate the Greeks at the
/// resulting volatility.
///
/// # Arguments
///
/// * `request` - Model, option terms and the solve; every value is validated
///   by the scalar kernel it reaches.
///
/// # Errors
///
/// Returns the kernel's validation error for an input outside the model's
/// domain or a price whose implied volatility cannot be solved.
pub fn option_calculation(request: &OptionCalculation) -> Result<OptionCalculationResult> {
    let (price, vol) = match request.solve {
        OptionSolve::Price { vol } => (price_at(&request.model, vol)?, vol),
        OptionSolve::ImpliedVol { price } => (price, implied_vol_of(&request.model, price)?),
    };
    Ok(OptionCalculationResult {
        price,
        implied_vol: vol,
        greeks: greeks_at(&request.model, vol)?,
    })
}

fn price_at(model: &OptionModel, vol: f64) -> Result<f64> {
    match *model {
        OptionModel::BlackScholes {
            spot,
            strike,
            rate,
            div_yield,
            expiry,
            is_call,
            ..
        } => bs_price(
            spot,
            strike,
            rate,
            div_yield,
            vol,
            expiry,
            OptionType::from(is_call),
        ),
        OptionModel::Black76 {
            forward,
            strike,
            df,
            expiry,
            is_call,
        } => black76_price(forward, strike, df, vol, expiry, OptionType::from(is_call)),
        OptionModel::Bachelier {
            forward,
            strike,
            expiry,
            is_call,
        } => bachelier_price(forward, strike, vol, expiry, OptionType::from(is_call)),
    }
}

fn implied_vol_of(model: &OptionModel, price: f64) -> Result<f64> {
    match *model {
        OptionModel::BlackScholes {
            spot,
            strike,
            rate,
            div_yield,
            expiry,
            is_call,
            ..
        } => bs_implied_vol(
            spot,
            strike,
            rate,
            div_yield,
            expiry,
            price,
            OptionType::from(is_call),
        ),
        OptionModel::Black76 {
            forward,
            strike,
            df,
            expiry,
            is_call,
        } => black76_implied_vol(
            forward,
            strike,
            df,
            expiry,
            price,
            OptionType::from(is_call),
        ),
        OptionModel::Bachelier {
            forward,
            strike,
            expiry,
            is_call,
        } => implied_vol_bachelier(price, forward, strike, expiry, is_call),
    }
}

fn greeks_at(model: &OptionModel, vol: f64) -> Result<OptionGreeks> {
    match *model {
        OptionModel::BlackScholes {
            spot,
            strike,
            rate,
            div_yield,
            expiry,
            is_call,
            theta_days_per_year,
        } => bs_greeks(
            &OptionMarketParams {
                spot,
                strike,
                rate,
                volatility: vol,
                time_to_expiry: expiry,
                dividend_yield: div_yield,
                option_type: OptionType::from(is_call),
            },
            theta_days_per_year.unwrap_or(DEFAULT_THETA_DAYS_PER_YEAR),
        )
        .map(OptionGreeks::BlackScholes),
        OptionModel::Black76 {
            forward,
            strike,
            expiry,
            is_call,
            ..
        } => black76_greeks(forward, strike, vol, expiry, OptionType::from(is_call))
            .map(OptionGreeks::Forward),
        OptionModel::Bachelier {
            forward,
            strike,
            expiry,
            is_call,
        } => bachelier_greeks(forward, strike, vol, expiry, OptionType::from(is_call))
            .map(OptionGreeks::Forward),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn black_scholes() -> OptionModel {
        OptionModel::BlackScholes {
            spot: 100.0,
            strike: 105.0,
            rate: 0.03,
            div_yield: 0.01,
            expiry: 0.75,
            is_call: false,
            theta_days_per_year: None,
        }
    }

    #[test]
    fn price_solve_matches_the_kernels() -> Result<()> {
        let result = option_calculation(&OptionCalculation {
            model: black_scholes(),
            solve: OptionSolve::Price { vol: 0.25 },
        })?;
        let put = OptionType::Put;
        assert_eq!(
            result.price,
            bs_price(100.0, 105.0, 0.03, 0.01, 0.25, 0.75, put)?
        );
        assert_eq!(result.implied_vol, 0.25);
        assert_eq!(
            result.greeks,
            OptionGreeks::BlackScholes(bs_greeks(
                &OptionMarketParams {
                    spot: 100.0,
                    strike: 105.0,
                    rate: 0.03,
                    volatility: 0.25,
                    time_to_expiry: 0.75,
                    dividend_yield: 0.01,
                    option_type: put
                },
                DEFAULT_THETA_DAYS_PER_YEAR
            )?)
        );
        Ok(())
    }

    #[test]
    fn implied_vol_solve_inverts_each_model() -> Result<()> {
        let models = [
            black_scholes(),
            OptionModel::Black76 {
                forward: 0.04,
                strike: 0.035,
                df: 0.97,
                expiry: 2.0,
                is_call: true,
            },
            OptionModel::Bachelier {
                forward: -0.002,
                strike: 0.001,
                expiry: 1.5,
                is_call: true,
            },
        ];
        for (model, vol) in models.into_iter().zip([0.25, 0.3, 0.006]) {
            let priced = option_calculation(&OptionCalculation {
                model,
                solve: OptionSolve::Price { vol },
            })?;
            let solved = option_calculation(&OptionCalculation {
                model,
                solve: OptionSolve::ImpliedVol {
                    price: priced.price,
                },
            })?;
            assert!((solved.implied_vol - vol).abs() < 1e-7, "{model:?}");
            assert_eq!(solved.price, priced.price);
        }
        Ok(())
    }

    #[test]
    fn wire_form_is_tagged_and_strict() -> Result<()> {
        let json = r#"{"model":{"kind":"black76","forward":100.0,"strike":100.0,"df":0.98,"expiry":1.0,"is_call":true},"solve":{"kind":"price","vol":0.2}}"#;
        let request: OptionCalculation = serde_json::from_str(json)
            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
        assert_eq!(serde_json::to_string(&request).ok().as_deref(), Some(json));
        let unknown = json.replace(r#""df":0.98"#, r#""df":0.98,"spot":1.0"#);
        assert!(serde_json::from_str::<OptionCalculation>(&unknown).is_err());
        let result = option_calculation(&request)?;
        let wire = serde_json::to_value(result)
            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
        assert_eq!(wire["greeks"]["kind"], "forward");
        Ok(())
    }

    #[test]
    fn invalid_terms_are_rejected_by_the_kernel() {
        let mut model = black_scholes();
        if let OptionModel::BlackScholes {
            theta_days_per_year,
            ..
        } = &mut model
        {
            *theta_days_per_year = Some(0.0);
        }
        assert!(
            option_calculation(&OptionCalculation {
                model,
                solve: OptionSolve::Price { vol: 0.25 },
            })
            .is_err()
        );
    }
}
