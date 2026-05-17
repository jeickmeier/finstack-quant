//! Option market parameters used by pricing models.

use super::market::OptionType;
use finstack_core::types::{Percentage, Rate};

/// Option market parameters for pricing models.
///
/// Groups market data parameters commonly used in option pricing functions.
#[derive(Debug, Clone)]
pub struct OptionMarketParams {
    /// Current spot/forward price
    pub spot: f64,
    /// Strike price
    pub strike: f64,
    /// Risk-free rate
    pub rate: f64,
    /// Volatility
    pub volatility: f64,
    /// Time to expiry in years
    pub time_to_expiry: f64,
    /// Dividend yield or cost of carry
    pub dividend_yield: f64,
    /// Option type (Call/Put)
    pub option_type: OptionType,
}

impl OptionMarketParams {
    /// Create option market parameters.
    ///
    /// Validation is provided separately by [`OptionMarketParams::validate`]
    /// so this constructor's signature stays infallible; callers that price
    /// off these parameters should call `validate` first.
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

    /// Create option market parameters using typed rates/volatility.
    pub fn new_typed(
        spot: f64,
        strike: f64,
        rate: Rate,
        volatility: Percentage,
        time_to_expiry: f64,
        dividend_yield: Percentage,
        option_type: OptionType,
    ) -> Self {
        Self {
            spot,
            strike,
            rate: rate.as_decimal(),
            volatility: volatility.as_decimal(),
            time_to_expiry,
            dividend_yield: dividend_yield.as_decimal(),
            option_type,
        }
    }

    /// Create call option market parameters
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

    /// Create call option market parameters using typed rates/volatility.
    pub fn call_typed(
        spot: f64,
        strike: f64,
        rate: Rate,
        volatility: Percentage,
        time_to_expiry: f64,
    ) -> Self {
        Self {
            spot,
            strike,
            rate: rate.as_decimal(),
            volatility: volatility.as_decimal(),
            time_to_expiry,
            dividend_yield: Percentage::ZERO.as_decimal(),
            option_type: OptionType::Call,
        }
    }

    /// Create put option market parameters
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

    /// Create put option market parameters using typed rates/volatility.
    pub fn put_typed(
        spot: f64,
        strike: f64,
        rate: Rate,
        volatility: Percentage,
        time_to_expiry: f64,
    ) -> Self {
        Self {
            spot,
            strike,
            rate: rate.as_decimal(),
            volatility: volatility.as_decimal(),
            time_to_expiry,
            dividend_yield: Percentage::ZERO.as_decimal(),
            option_type: OptionType::Put,
        }
    }

    /// Set dividend yield
    #[must_use]
    pub fn with_dividend_yield(mut self, dividend_yield: f64) -> Self {
        self.dividend_yield = dividend_yield;
        self
    }

    /// Set dividend yield using a typed percentage.
    #[must_use]
    pub fn with_dividend_yield_pct(mut self, dividend_yield: Percentage) -> Self {
        self.dividend_yield = dividend_yield.as_decimal();
        self
    }

    /// Validate the structural invariants required for option pricing.
    ///
    /// Enforces:
    /// - `volatility > 0` — a non-positive volatility makes the Black /
    ///   Bachelier pricing formulas degenerate (`d1`/`d2` and the normal CDF
    ///   collapse).
    /// - `spot > 0` and `strike > 0` — both are prices and must be strictly
    ///   positive for the lognormal model to be well-defined.
    /// - `time_to_expiry` finite and `>= 0` — `0` is permitted (the at-expiry
    ///   intrinsic-value limit); a negative or non-finite value is rejected.
    /// - `rate` and `dividend_yield` finite — non-finite carry parameters
    ///   propagate `NaN`/`inf` through every PV and Greek.
    ///
    /// The constructors ([`OptionMarketParams::new`] and friends) do not call
    /// this — the struct also has public fields — so a pricer that consumes
    /// these parameters should call `validate` before evaluating any model.
    ///
    /// # Errors
    /// Returns an error stating the attempted value and what failed for the
    /// first violated invariant.
    pub fn validate(&self) -> finstack_core::Result<()> {
        use crate::instruments::common_impl::validation::{
            validate_f64_finite, validate_f64_non_negative, validate_f64_positive,
        };
        validate_f64_positive(self.volatility, "OptionMarketParams.volatility")?;
        validate_f64_positive(self.spot, "OptionMarketParams.spot")?;
        validate_f64_positive(self.strike, "OptionMarketParams.strike")?;
        validate_f64_finite(self.time_to_expiry, "OptionMarketParams.time_to_expiry")?;
        validate_f64_non_negative(self.time_to_expiry, "OptionMarketParams.time_to_expiry")?;
        validate_f64_finite(self.rate, "OptionMarketParams.rate")?;
        validate_f64_finite(self.dividend_yield, "OptionMarketParams.dividend_yield")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_and_typed_builders_set_all_fields() {
        let plain = OptionMarketParams::new(100.0, 95.0, 0.03, 0.25, 1.5, 0.01, OptionType::Put);
        assert_eq!(plain.spot, 100.0);
        assert_eq!(plain.strike, 95.0);
        assert_eq!(plain.rate, 0.03);
        assert_eq!(plain.volatility, 0.25);
        assert_eq!(plain.time_to_expiry, 1.5);
        assert_eq!(plain.dividend_yield, 0.01);
        assert_eq!(plain.option_type, OptionType::Put);
        assert!(plain.validate().is_ok());

        let typed = OptionMarketParams::new_typed(
            100.0,
            105.0,
            Rate::from_percent(4.0),
            Percentage::new(20.0),
            2.0,
            Percentage::new(1.5),
            OptionType::Call,
        );
        assert!((typed.rate - 0.04).abs() < 1e-12);
        assert!((typed.volatility - 0.20).abs() < 1e-12);
        assert!((typed.dividend_yield - 0.015).abs() < 1e-12);
        assert_eq!(typed.option_type, OptionType::Call);
        assert!(typed.validate().is_ok());
    }

    #[test]
    fn call_and_put_helpers_default_dividend_yield_to_zero() {
        let call = OptionMarketParams::call(100.0, 100.0, 0.05, 0.30, 1.0);
        let put = OptionMarketParams::put(100.0, 100.0, 0.05, 0.30, 1.0);
        let typed_call = OptionMarketParams::call_typed(
            100.0,
            100.0,
            Rate::from_bps(500),
            Percentage::new(30.0),
            1.0,
        );
        let typed_put = OptionMarketParams::put_typed(
            100.0,
            100.0,
            Rate::from_bps(500),
            Percentage::new(30.0),
            1.0,
        );

        for params in [call, put, typed_call, typed_put] {
            assert_eq!(params.dividend_yield, 0.0);
        }
    }

    #[test]
    fn dividend_yield_setters_override_existing_value() {
        let plain =
            OptionMarketParams::call(100.0, 100.0, 0.05, 0.30, 1.0).with_dividend_yield(0.02);
        let typed = OptionMarketParams::put(100.0, 100.0, 0.05, 0.30, 1.0)
            .with_dividend_yield_pct(Percentage::new(2.5));

        assert!((plain.dividend_yield - 0.02).abs() < 1e-12);
        assert!((typed.dividend_yield - 0.025).abs() < 1e-12);
    }

    #[test]
    fn validate_rejects_non_positive_volatility() {
        // Failure mode: a non-positive volatility makes the Black/Bachelier
        // pricing formulas degenerate; it was previously unvalidated.
        let zero_vol = OptionMarketParams::new(100.0, 100.0, 0.05, 0.0, 1.0, 0.0, OptionType::Call);
        let err = zero_vol
            .validate()
            .expect_err("zero volatility must be rejected by validate");
        assert!(
            err.to_string().contains("volatility"),
            "error should name the volatility: {err}"
        );
        assert!(
            OptionMarketParams::call(100.0, 100.0, 0.05, -0.20, 1.0)
                .validate()
                .is_err(),
            "negative volatility must be rejected"
        );
        // A well-formed parameter bundle passes.
        assert!(OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0)
            .validate()
            .is_ok());
    }

    #[test]
    fn validate_rejects_non_positive_spot_and_strike() {
        assert!(
            OptionMarketParams::new(0.0, 100.0, 0.05, 0.20, 1.0, 0.0, OptionType::Call)
                .validate()
                .is_err(),
            "zero spot must be rejected"
        );
        assert!(
            OptionMarketParams::new(100.0, -10.0, 0.05, 0.20, 1.0, 0.0, OptionType::Put)
                .validate()
                .is_err(),
            "negative strike must be rejected"
        );
    }

    #[test]
    fn validate_rejects_negative_or_non_finite_time_to_expiry() {
        assert!(
            OptionMarketParams::new(100.0, 100.0, 0.05, 0.20, -0.5, 0.0, OptionType::Call)
                .validate()
                .is_err(),
            "negative time-to-expiry must be rejected"
        );
        assert!(
            OptionMarketParams::new(100.0, 100.0, 0.05, 0.20, f64::NAN, 0.0, OptionType::Call)
                .validate()
                .is_err(),
            "non-finite time-to-expiry must be rejected"
        );
        // Zero time-to-expiry is permitted (at-expiry intrinsic-value limit).
        assert!(
            OptionMarketParams::new(100.0, 100.0, 0.05, 0.20, 0.0, 0.0, OptionType::Call)
                .validate()
                .is_ok(),
            "zero time-to-expiry must be accepted"
        );
    }

    #[test]
    fn validate_rejects_non_finite_rate_and_dividend_yield() {
        assert!(
            OptionMarketParams::new(
                100.0,
                100.0,
                f64::INFINITY,
                0.20,
                1.0,
                0.0,
                OptionType::Call
            )
            .validate()
            .is_err(),
            "non-finite rate must be rejected"
        );
        assert!(
            OptionMarketParams::new(100.0, 100.0, 0.05, 0.20, 1.0, f64::NAN, OptionType::Call)
                .validate()
                .is_err(),
            "non-finite dividend yield must be rejected"
        );
    }

    #[test]
    fn validate_catches_bad_params_on_struct_literal() {
        // A struct-literal bundle that bypassed the constructors must still be
        // checkable via `validate`.
        let bad = OptionMarketParams {
            spot: 100.0,
            strike: 100.0,
            rate: 0.05,
            volatility: -0.10,
            time_to_expiry: 1.0,
            dividend_yield: 0.0,
            option_type: OptionType::Call,
        };
        assert!(bad.validate().is_err());

        let good = OptionMarketParams {
            volatility: 0.20,
            ..bad
        };
        assert!(good.validate().is_ok());
    }
}
