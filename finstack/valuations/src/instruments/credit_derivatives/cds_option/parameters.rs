//! Construction parameters for [`CDSOption`](super::CDSOption).
//!
//! Validated at the point of construction so the resulting `CDSOption` is
//! guaranteed to satisfy the Bloomberg CDSO model's preconditions.

use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::credit_derivatives::cds_option::ProtectionStartConvention;
use finstack_core::{dates::Date, money::Money};
use rust_decimal::Decimal;

/// Strike upper bound — `0.10` decimal = 1000 bp = 10% spread, matching the
/// Bloomberg CDSO calibration guard for distressed credit.
pub(crate) const MAX_STRIKE: f64 = 0.10;

pub(crate) fn validate_common_terms(
    strike: Decimal,
    expiry: Date,
    cds_maturity: Date,
    index_factor: Option<f64>,
) -> finstack_core::Result<()> {
    let strike_f64 = decimal_to_f64(strike, "strike")?;
    if strike_f64 <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "strike must be positive, got {}",
            strike
        )));
    }
    if strike_f64 > MAX_STRIKE {
        return Err(finstack_core::Error::Validation(format!(
            "strike {} exceeds maximum {}",
            strike, MAX_STRIKE
        )));
    }
    if expiry >= cds_maturity {
        return Err(finstack_core::Error::Validation(format!(
            "option expiry ({}) must be before CDS maturity ({})",
            expiry, cds_maturity
        )));
    }
    if let Some(factor) = index_factor {
        if factor <= 0.0 || factor > 1.0 {
            return Err(finstack_core::Error::Validation(format!(
                "index_factor must be in (0, 1], got {}",
                factor
            )));
        }
    }
    Ok(())
}

/// Construction-time inputs for a CDS option.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CDSOptionParams {
    /// Strike spread as a decimal rate (e.g., `0.01` = 100 bp).
    pub strike: Decimal,
    /// Option expiry date. Must precede `cds_maturity`.
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Underlying CDS maturity date.
    #[schemars(with = "String")]
    pub cds_maturity: Date,
    /// Notional amount.
    pub notional: Money,
    /// Option type (Call = payer, Put = receiver).
    pub option_type: OptionType,
    /// Whether the underlying is a CDS index (vs single-name CDS). The
    /// Bloomberg CDSO model treats the two cases differently in the
    /// no-knockout calibration.
    #[serde(default)]
    pub underlying_is_index: bool,
    /// Optional index-factor scaling for re-versioned indices. Must be in
    /// `(0, 1]`.
    pub index_factor: Option<f64>,
    /// Contractual coupon `c` of the underlying CDS as a decimal rate
    /// (e.g., `0.01` for 100 bp standard CDX). When `None`, the synthetic
    /// underlying CDS uses `strike` as its running coupon (single-name
    /// SNAC default). Required for CDX/iTraxx index options.
    #[serde(default)]
    pub underlying_cds_coupon: Option<Decimal>,
    /// Convention for selecting the synthetic underlying CDS accrual start
    /// when the option does not provide an explicit effective date.
    #[serde(default)]
    pub protection_start_convention: ProtectionStartConvention,
}

impl CDSOptionParams {
    fn validate(&self) -> finstack_core::Result<()> {
        validate_common_terms(
            self.strike,
            self.expiry,
            self.cds_maturity,
            self.index_factor,
        )
    }

    /// Construct a validated set of parameters.
    pub fn new(
        strike: Decimal,
        expiry: Date,
        cds_maturity: Date,
        notional: Money,
        option_type: OptionType,
    ) -> finstack_core::Result<Self> {
        let params = Self {
            strike,
            expiry,
            cds_maturity,
            notional,
            option_type,
            underlying_is_index: false,
            index_factor: None,
            underlying_cds_coupon: None,
            protection_start_convention: ProtectionStartConvention::default(),
        };
        params.validate()?;
        Ok(params)
    }

    /// Convenience constructor for a payer (call on spread) option.
    pub fn call(
        strike: Decimal,
        expiry: Date,
        cds_maturity: Date,
        notional: Money,
    ) -> finstack_core::Result<Self> {
        Self::new(strike, expiry, cds_maturity, notional, OptionType::Call)
    }

    /// Convenience constructor for a receiver (put on spread) option.
    pub fn put(
        strike: Decimal,
        expiry: Date,
        cds_maturity: Date,
        notional: Money,
    ) -> finstack_core::Result<Self> {
        Self::new(strike, expiry, cds_maturity, notional, OptionType::Put)
    }

    /// Mark this option as referencing a CDS index and set its index factor.
    pub fn as_index(mut self, index_factor: f64) -> finstack_core::Result<Self> {
        self.underlying_is_index = true;
        self.index_factor = Some(index_factor);
        self.validate()?;
        Ok(self)
    }

    /// Set the contractual coupon `c` of the underlying CDS as a decimal
    /// rate. Required for CDX/iTraxx index options.
    #[must_use]
    pub fn with_underlying_cds_coupon(mut self, coupon: Decimal) -> Self {
        self.underlying_cds_coupon = Some(coupon);
        self
    }

    /// Set the accrual-start convention for the synthetic underlying CDS.
    #[must_use]
    pub fn with_protection_start_convention(
        mut self,
        convention: ProtectionStartConvention,
    ) -> Self {
        self.protection_start_convention = convention;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use time::macros::date;

    #[test]
    fn valid_params_construct() {
        CDSOptionParams::call(
            Decimal::new(1, 2),
            date!(2025 - 06 - 20),
            date!(2030 - 06 - 20),
            Money::new(10_000_000.0, Currency::USD),
        )
        .unwrap();
    }

    #[test]
    fn zero_strike_rejected() {
        let err = CDSOptionParams::call(
            Decimal::ZERO,
            date!(2025 - 06 - 20),
            date!(2030 - 06 - 20),
            Money::new(10_000_000.0, Currency::USD),
        )
        .unwrap_err();
        assert!(err.to_string().contains("strike must be positive"));
    }

    #[test]
    fn negative_strike_rejected() {
        assert!(CDSOptionParams::call(
            Decimal::new(-5, 3),
            date!(2025 - 06 - 20),
            date!(2030 - 06 - 20),
            Money::new(10_000_000.0, Currency::USD),
        )
        .is_err());
    }

    #[test]
    fn expiry_after_maturity_rejected() {
        let err = CDSOptionParams::call(
            Decimal::new(1, 2),
            date!(2030 - 06 - 21),
            date!(2030 - 06 - 20),
            Money::new(10_000_000.0, Currency::USD),
        )
        .unwrap_err();
        assert!(err.to_string().contains("must be before CDS maturity"));
    }

    #[test]
    fn index_factor_bounds_enforced() {
        let params = CDSOptionParams::call(
            Decimal::new(1, 2),
            date!(2025 - 06 - 20),
            date!(2030 - 06 - 20),
            Money::new(10_000_000.0, Currency::USD),
        )
        .unwrap();
        assert!(params.clone().as_index(1.5).is_err());
        let indexed = params.as_index(0.85).unwrap();
        assert!(indexed.underlying_is_index);
        assert_eq!(indexed.index_factor, Some(0.85));
    }
}
