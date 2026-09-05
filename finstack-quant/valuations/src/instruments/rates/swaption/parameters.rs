//! Swaption-specific parameters.

use crate::instruments::rates::irs::PayReceive;
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};
use rust_decimal::Decimal;

/// Swaption-specific parameters.
///
/// Groups swaption parameters beyond basic option parameters.
#[derive(Debug, Clone)]
pub struct SwaptionParams {
    /// Notional amount
    pub notional: Money,
    /// Strike rate (fixed rate)
    pub strike: Decimal,
    /// Swaption expiry date
    pub expiry: Date,
    /// Underlying swap start date
    pub swap_start: Date,
    /// Underlying swap end date
    pub swap_end: Date,
    /// Payer/receiver side
    pub side: PayReceive,
    /// Optional override: fixed-leg payment frequency.
    pub fixed_frequency: Option<Tenor>,
    /// Optional override: floating-leg payment frequency.
    pub float_frequency: Option<Tenor>,
    /// Optional override: fixed-leg accrual day count.
    pub fixed_day_count: Option<DayCount>,
    /// Optional override: floating-leg accrual day count.
    pub float_day_count: Option<DayCount>,
    /// Optional override: volatility model.
    pub vol_model: Option<crate::instruments::rates::swaption::types::VolatilityModel>,
}

impl SwaptionParams {
    /// Create payer swaption parameters.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `strike` is non-finite or cannot be
    /// represented as a [`Decimal`].
    pub fn payer(
        notional: Money,
        strike: f64,
        expiry: Date,
        swap_start: Date,
        swap_end: Date,
    ) -> Result<Self> {
        Ok(Self {
            notional,
            strike: strike_decimal(strike)?,
            expiry,
            swap_start,
            swap_end,
            side: PayReceive::Pay,
            fixed_frequency: None,
            float_frequency: None,
            fixed_day_count: None,
            float_day_count: None,
            vol_model: None,
        })
    }

    /// Create receiver swaption parameters.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `strike` is non-finite or cannot be
    /// represented as a [`Decimal`].
    pub fn receiver(
        notional: Money,
        strike: f64,
        expiry: Date,
        swap_start: Date,
        swap_end: Date,
    ) -> Result<Self> {
        Ok(Self {
            notional,
            strike: strike_decimal(strike)?,
            expiry,
            swap_start,
            swap_end,
            side: PayReceive::Receive,
            fixed_frequency: None,
            float_frequency: None,
            fixed_day_count: None,
            float_day_count: None,
            vol_model: None,
        })
    }

    /// Override fixed leg payment frequency
    pub fn with_fixed_frequency(mut self, frequency: Tenor) -> Self {
        self.fixed_frequency = Some(frequency);
        self
    }

    /// Override float leg payment frequency
    pub fn with_float_frequency(mut self, frequency: Tenor) -> Self {
        self.float_frequency = Some(frequency);
        self
    }

    /// Override the fixed-leg accrual day count.
    pub fn with_fixed_day_count(mut self, day_count: DayCount) -> Self {
        self.fixed_day_count = Some(day_count);
        self
    }

    /// Override the floating-leg accrual day count.
    pub fn with_float_day_count(mut self, day_count: DayCount) -> Self {
        self.float_day_count = Some(day_count);
        self
    }

    /// Override volatility model
    pub fn with_vol_model(
        mut self,
        model: crate::instruments::rates::swaption::types::VolatilityModel,
    ) -> Self {
        self.vol_model = Some(model);
        self
    }
}

fn strike_decimal(strike: f64) -> Result<Decimal> {
    if !strike.is_finite() {
        return Err(Error::Validation(format!(
            "swaption strike must be finite, got {strike}"
        )));
    }

    Decimal::try_from(strike).map_err(|err| {
        Error::Validation(format!(
            "swaption strike {strike} cannot be represented as Decimal: {err}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::rates::swaption::types::VolatilityModel;
    use crate::instruments::rates::swaption::Swaption;
    use finstack_quant_core::currency::Currency;
    use time::macros::date;

    fn sample_dates() -> (Date, Date, Date) {
        (
            date!(2026 - 01 - 15),
            date!(2026 - 07 - 15),
            date!(2031 - 07 - 15),
        )
    }

    #[test]
    fn payer_and_receiver_constructors_set_expected_defaults() {
        let (expiry, swap_start, swap_end) = sample_dates();
        let notional = Money::from((5_000_000_i64, Currency::USD));

        let payer = SwaptionParams::payer(notional, 0.0325, expiry, swap_start, swap_end)
            .expect("valid payer params");
        let receiver = SwaptionParams::receiver(notional, 0.031, expiry, swap_start, swap_end)
            .expect("valid receiver params");

        assert_eq!(payer.notional, notional);
        assert_eq!(payer.strike, Decimal::new(325, 4));
        assert_eq!(payer.side, PayReceive::Pay);
        assert_eq!(payer.expiry, expiry);
        assert_eq!(payer.swap_start, swap_start);
        assert_eq!(payer.swap_end, swap_end);
        assert_eq!(payer.fixed_frequency, None);
        assert_eq!(payer.float_frequency, None);
        assert_eq!(payer.fixed_day_count, None);
        assert_eq!(payer.float_day_count, None);
        assert_eq!(payer.vol_model, None);

        assert_eq!(receiver.strike, Decimal::new(31, 3));
        assert_eq!(receiver.side, PayReceive::Receive);
    }

    #[test]
    fn fluent_overrides_replace_optional_configuration() {
        let (expiry, swap_start, swap_end) = sample_dates();
        let params = SwaptionParams::payer(
            Money::from((2_000_000_i64, Currency::GBP)),
            0.04,
            expiry,
            swap_start,
            swap_end,
        )
        .expect("valid payer params")
        .with_fixed_frequency(Tenor::semi_annual())
        .with_float_frequency(Tenor::quarterly())
        .with_fixed_day_count(DayCount::Act365F)
        .with_float_day_count(DayCount::Act365F)
        .with_vol_model(VolatilityModel::Normal);

        assert_eq!(params.fixed_frequency, Some(Tenor::semi_annual()));
        assert_eq!(params.float_frequency, Some(Tenor::quarterly()));
        assert_eq!(params.fixed_day_count, Some(DayCount::Act365F));
        assert_eq!(params.float_day_count, Some(DayCount::Act365F));
        assert_eq!(params.vol_model, Some(VolatilityModel::Normal));

        let swaption = Swaption::new(
            "GBP-SONIA-SWAPTION",
            &params,
            "GBP-SONIA-OIS",
            "GBP-SONIA-OIS",
            "GBP-SWAPTION-VOL",
        );
        assert_eq!(swaption.underlying_fixed_leg.day_count, DayCount::Act365F);
        assert_eq!(swaption.underlying_float_leg.day_count, DayCount::Act365F);
    }

    #[test]
    fn constructors_reject_non_finite_strike_inputs_without_panicking() {
        let (expiry, swap_start, swap_end) = sample_dates();
        let notional = Money::from((1_000_000_i64, Currency::USD));

        let payer_result = std::panic::catch_unwind(|| {
            SwaptionParams::payer(notional, f64::NAN, expiry, swap_start, swap_end)
        });
        let receiver_result = std::panic::catch_unwind(|| {
            SwaptionParams::receiver(notional, f64::INFINITY, expiry, swap_start, swap_end)
        });

        assert!(payer_result.is_ok(), "NaN strike should not panic");
        assert!(receiver_result.is_ok(), "infinite strike should not panic");

        let payer = payer_result.expect("payer constructor should not unwind");
        let receiver = receiver_result.expect("receiver constructor should not unwind");

        assert_validation_error(payer, "NaN strike");
        assert_validation_error(receiver, "infinite strike");
    }

    fn assert_validation_error(result: Result<SwaptionParams>, label: &str) {
        match result {
            Err(Error::Validation(message)) => {
                assert!(
                    message.contains("swaption strike must be finite"),
                    "{label} returned unexpected validation message: {message}"
                );
            }
            other => panic!("{label} should return a validation error, got {other:?}"),
        }
    }
}
