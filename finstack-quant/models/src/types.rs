//! Shared option-model input types.

use serde::{Deserialize, Serialize};

/// Option payoff direction used by analytical and numerical model engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OptionType {
    /// Call option.
    Call,
    /// Put option.
    Put,
}

impl From<bool> for OptionType {
    fn from(is_call: bool) -> Self {
        if is_call {
            Self::Call
        } else {
            Self::Put
        }
    }
}

impl std::fmt::Display for OptionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Call => write!(f, "call"),
            Self::Put => write!(f, "put"),
        }
    }
}

impl std::str::FromStr for OptionType {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "call" => Ok(Self::Call),
            "put" => Ok(Self::Put),
            _ => Err(format!("Unknown option type: {value}")),
        }
    }
}

/// Exercise schedule convention for option models.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ExerciseStyle {
    /// Exercise only at expiry.
    #[default]
    European,
    /// Exercise at any eligible time through expiry.
    American,
    /// Exercise on a finite schedule of eligible dates.
    Bermudan,
}

impl std::fmt::Display for ExerciseStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::European => write!(f, "european"),
            Self::American => write!(f, "american"),
            Self::Bermudan => write!(f, "bermudan"),
        }
    }
}

impl std::str::FromStr for ExerciseStyle {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "european" => Ok(Self::European),
            "american" => Ok(Self::American),
            "bermudan" => Ok(Self::Bermudan),
            _ => Err(format!("Unknown exercise style: {value}")),
        }
    }
}

/// Common market inputs for product-independent option model functions.
#[derive(Debug, Clone)]
pub struct OptionMarketParams {
    /// Current spot or forward price in the instrument's quote units.
    pub spot: f64,
    /// Strike price in the same quote units as `spot`.
    pub strike: f64,
    /// Continuously compounded annual risk-free rate as a decimal.
    pub rate: f64,
    /// Annualized volatility as a decimal.
    pub volatility: f64,
    /// Time to expiry in years.
    pub time_to_expiry: f64,
    /// Continuous dividend yield or cost of carry as a decimal.
    pub dividend_yield: f64,
    /// Call or put payoff direction.
    pub option_type: OptionType,
}

impl OptionMarketParams {
    /// Create option market parameters without validating them.
    ///
    /// # Arguments
    ///
    /// * `spot` - Positive spot or forward price in quote units.
    /// * `strike` - Positive strike in the same units as `spot`.
    /// * `rate` - Continuously compounded annual rate as a decimal.
    /// * `volatility` - Positive annualized volatility as a decimal.
    /// * `time_to_expiry` - Non-negative expiry time in years.
    /// * `dividend_yield` - Continuous annual carry or dividend yield as a decimal.
    /// * `option_type` - Call or put payoff direction.
    pub fn new(
        spot: f64,
        strike: f64,
        rate: f64,
        volatility: f64,
        time_to_expiry: f64,
        dividend_yield: f64,
        option_type: OptionType,
    ) -> Self {
        Self {
            spot,
            strike,
            rate,
            volatility,
            time_to_expiry,
            dividend_yield,
            option_type,
        }
    }

    /// Create call-option market parameters with zero dividend yield.
    pub fn call(spot: f64, strike: f64, rate: f64, volatility: f64, time_to_expiry: f64) -> Self {
        Self::new(
            spot,
            strike,
            rate,
            volatility,
            time_to_expiry,
            0.0,
            OptionType::Call,
        )
    }

    /// Create put-option market parameters with zero dividend yield.
    pub fn put(spot: f64, strike: f64, rate: f64, volatility: f64, time_to_expiry: f64) -> Self {
        Self::new(
            spot,
            strike,
            rate,
            volatility,
            time_to_expiry,
            0.0,
            OptionType::Put,
        )
    }

    /// Validate the structural invariants required by option models.
    ///
    /// # Errors
    ///
    /// Returns a core validation error for non-positive prices or volatility,
    /// negative expiry time, or non-finite rate/carry inputs.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        use finstack_quant_core::validation::{
            validate_f64_finite, validate_f64_non_negative, validate_f64_positive,
        };
        validate_f64_positive(self.volatility, "OptionMarketParams.volatility")?;
        validate_f64_positive(self.spot, "OptionMarketParams.spot")?;
        validate_f64_positive(self.strike, "OptionMarketParams.strike")?;
        validate_f64_finite(self.time_to_expiry, "OptionMarketParams.time_to_expiry")?;
        validate_f64_non_negative(self.time_to_expiry, "OptionMarketParams.time_to_expiry")?;
        validate_f64_finite(self.rate, "OptionMarketParams.rate")?;
        validate_f64_finite(self.dividend_yield, "OptionMarketParams.dividend_yield")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_enums_preserve_wire_and_display_values() {
        assert_eq!(OptionType::Call.to_string(), "call");
        assert_eq!("put".parse::<OptionType>(), Ok(OptionType::Put));
        assert_eq!(ExerciseStyle::default(), ExerciseStyle::European);
        assert_eq!(ExerciseStyle::Bermudan.to_string(), "bermudan");
    }

    #[test]
    fn constructors_set_all_fields() {
        let generic = OptionMarketParams::new(100.0, 95.0, 0.04, 0.20, 1.5, 0.015, OptionType::Put);
        assert_eq!(generic.spot, 100.0);
        assert_eq!(generic.strike, 95.0);
        assert_eq!(generic.rate, 0.04);
        assert_eq!(generic.volatility, 0.20);
        assert_eq!(generic.time_to_expiry, 1.5);
        assert_eq!(generic.dividend_yield, 0.015);
        assert_eq!(generic.option_type, OptionType::Put);
        assert!(generic.validate().is_ok());
    }

    #[test]
    fn call_and_put_helpers_default_dividend_yield_to_zero() {
        let call = OptionMarketParams::call(100.0, 95.0, 0.04, 0.20, 1.5);
        assert_eq!(call.option_type, OptionType::Call);
        assert_eq!(call.dividend_yield, 0.0);

        let put = OptionMarketParams::put(100.0, 95.0, 0.04, 0.20, 1.5);
        assert_eq!(put.option_type, OptionType::Put);
        assert_eq!(put.dividend_yield, 0.0);
    }

    #[test]
    fn validate_rejects_non_positive_volatility() {
        for volatility in [0.0, -0.1, f64::NAN, f64::INFINITY] {
            assert!(OptionMarketParams::call(100.0, 95.0, 0.04, volatility, 1.5)
                .validate()
                .is_err());
        }
    }

    #[test]
    fn validate_rejects_non_positive_spot_and_strike() {
        assert!(OptionMarketParams::call(0.0, 95.0, 0.04, 0.20, 1.5)
            .validate()
            .is_err());
        assert!(OptionMarketParams::call(100.0, -1.0, 0.04, 0.20, 1.5)
            .validate()
            .is_err());
    }

    #[test]
    fn validate_rejects_negative_or_non_finite_time_to_expiry() {
        for expiry in [-0.1, f64::NAN, f64::INFINITY] {
            assert!(OptionMarketParams::call(100.0, 95.0, 0.04, 0.20, expiry)
                .validate()
                .is_err());
        }
        assert!(OptionMarketParams::call(100.0, 95.0, 0.04, 0.20, 0.0)
            .validate()
            .is_ok());
    }

    #[test]
    fn validate_rejects_non_finite_rate_and_dividend_yield() {
        assert!(OptionMarketParams::call(100.0, 95.0, f64::NAN, 0.20, 1.5)
            .validate()
            .is_err());
        let mut params = OptionMarketParams::call(100.0, 95.0, 0.04, 0.20, 1.5);
        params.dividend_yield = f64::INFINITY;
        assert!(params.validate().is_err());
    }

    #[test]
    fn validate_catches_bad_params_on_struct_literal() {
        let params = OptionMarketParams {
            spot: 100.0,
            strike: 95.0,
            rate: 0.04,
            volatility: -0.20,
            time_to_expiry: 1.5,
            dividend_yield: 0.015,
            option_type: OptionType::Call,
        };
        assert!(params.validate().is_err());
    }
}
