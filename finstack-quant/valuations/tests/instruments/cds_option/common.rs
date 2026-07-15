//! Common test fixtures and utilities for CDS Option tests.
//!
//! Provides reusable market setups, option builders, and assertion helpers
//! to maintain DRY principles across the test suite.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::DateExt;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, DiscountCurveRateCalibration, DiscountCurveRateQuote,
    DiscountCurveRateQuoteType, HazardCurve,
};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::constants::isda::STANDARD_RECOVERY_SENIOR;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::{
    CDSOption, CDSOptionParams, ProtectionStartConvention,
};
use finstack_quant_valuations::instruments::CreditParams;
use finstack_quant_valuations::instruments::OptionType;
use rust_decimal::Decimal;

/// Standard flat discount curve for testing
pub fn flat_discount(id: &str, base: Date, rate: f64) -> DiscountCurve {
    let df1 = (-rate).exp();
    let df5 = (-rate * 5.0).exp();
    let df10 = (-rate * 10.0).exp();

    DiscountCurve::builder(id)
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, df1), (5.0, df5), (10.0, df10)])
        .rate_calibration(DiscountCurveRateCalibration {
            index_id: "USD-SOFR-OIS".to_string(),
            currency: Currency::USD,
            quotes: vec![
                DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Deposit,
                    tenor: "1Y".to_string(),
                    rate,
                },
                DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Swap,
                    tenor: "5Y".to_string(),
                    rate,
                },
                DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Swap,
                    tenor: "10Y".to_string(),
                    rate,
                },
            ],
        })
        .build()
        .unwrap()
}

/// Standard flat hazard curve for testing
pub fn flat_hazard(id: &str, base: Date, recovery: f64, hazard_rate: f64) -> HazardCurve {
    let par = hazard_rate * 10000.0 * (1.0 - recovery);
    HazardCurve::builder(id)
        .base_date(base)
        .recovery_rate(recovery)
        .knots([(1.0, hazard_rate), (5.0, hazard_rate), (10.0, hazard_rate)])
        .par_spreads([(1.0, par), (5.0, par), (10.0, par)])
        .build()
        .unwrap()
}

/// Standard market context with typical curves
pub fn standard_market(as_of: Date) -> MarketContext {
    let disc = flat_discount("USD-OIS", as_of, 0.03);
    let credit = flat_hazard("HZ-SN", as_of, STANDARD_RECOVERY_SENIOR, 0.02);

    MarketContext::new().insert(disc).insert(credit)
}

/// Clone `market` with the named hazard curve's `recovery_rate` metadata
/// overridden, keeping its λ knots unchanged.
///
/// Used to build a "frozen-curve" market for a trade whose recovery has been
/// bumped: the hazard λ stays fixed (the frozen-curve / LGD-only semantics)
/// while the curve's recovery metadata is realigned with the bumped trade
/// recovery so the ISDA recovery-consistency guard accepts the pair.
pub fn frozen_market_with_recovery(
    market: &MarketContext,
    hazard_id: &str,
    recovery: f64,
) -> MarketContext {
    let hazard = market.get_hazard(hazard_id).unwrap();
    let realigned = hazard.with_recovery_rate(recovery).unwrap();
    market.clone().insert(realigned)
}

/// Convert basis points (f64) to a Decimal rate.
/// e.g., 100.0 bp -> Decimal 0.01
fn bp_to_decimal(bp: f64) -> Decimal {
    Decimal::try_from(bp / 10000.0).expect("valid decimal from bp")
}

/// Test builder for `CDSOption`. Strikes are accepted in basis points for
/// readability and converted to decimal internally.
pub struct CDSOptionBuilder {
    id: String,
    strike_bp: f64,
    option_type: OptionType,
    expiry_months: i32,
    cds_maturity_months: i32,
    notional: Money,
    implied_vol: Option<f64>,
    is_index: bool,
    index_factor: Option<f64>,
    underlying_cds_coupon_bp: Option<f64>,
    protection_start_convention: ProtectionStartConvention,
    knockout: Option<bool>,
}

impl CDSOptionBuilder {
    pub fn new() -> Self {
        Self {
            id: "CDSOPT-TEST".to_string(),
            strike_bp: 100.0,
            option_type: OptionType::Call,
            expiry_months: 12,
            cds_maturity_months: 60,
            notional: Money::new(10_000_000.0, Currency::USD),
            implied_vol: Some(0.30),
            is_index: false,
            index_factor: None,
            underlying_cds_coupon_bp: None,
            protection_start_convention: ProtectionStartConvention::Forward,
            knockout: None,
        }
    }

    #[allow(dead_code)]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn strike(mut self, bp: f64) -> Self {
        self.strike_bp = bp;
        self
    }

    pub fn call(mut self) -> Self {
        self.option_type = OptionType::Call;
        self
    }

    pub fn put(mut self) -> Self {
        self.option_type = OptionType::Put;
        self
    }

    pub fn expiry_months(mut self, months: i32) -> Self {
        self.expiry_months = months;
        self
    }

    pub fn cds_maturity_months(mut self, months: i32) -> Self {
        self.cds_maturity_months = months;
        self
    }

    pub fn notional(mut self, amount: f64, currency: Currency) -> Self {
        self.notional = Money::new(amount, currency);
        self
    }

    pub fn implied_vol(mut self, vol: f64) -> Self {
        self.implied_vol = Some(vol);
        self
    }

    #[allow(dead_code)]
    pub fn no_vol_override(mut self) -> Self {
        self.implied_vol = None;
        self
    }

    pub fn with_index(mut self, factor: f64) -> Self {
        self.is_index = true;
        self.index_factor = Some(factor);
        self.knockout = Some(false);
        self
    }

    /// Set the contractual coupon `c` of the underlying CDS in basis
    /// points. Required for CDX-style index options (e.g. 100 bp) where
    /// the running coupon differs from the option strike.
    #[allow(dead_code)]
    pub fn underlying_cds_coupon_bp(mut self, bp: f64) -> Self {
        self.underlying_cds_coupon_bp = Some(bp);
        self
    }

    #[allow(dead_code)]
    pub fn protection_start_convention(mut self, convention: ProtectionStartConvention) -> Self {
        self.protection_start_convention = convention;
        self
    }

    pub fn knockout(mut self, knockout: bool) -> Self {
        self.knockout = Some(knockout);
        self
    }

    pub fn build(self, as_of: Date) -> CDSOption {
        let expiry = as_of.add_months(self.expiry_months);
        let cds_maturity = as_of.add_months(self.cds_maturity_months);

        let strike = bp_to_decimal(self.strike_bp);
        let mut option_params = CDSOptionParams::new(
            strike,
            expiry,
            cds_maturity,
            self.notional,
            self.option_type,
        )
        .expect("valid option params");
        option_params =
            option_params.with_protection_start_convention(self.protection_start_convention);

        if self.is_index {
            option_params = option_params
                .as_index(self.index_factor.unwrap_or(1.0))
                .expect("valid index factor");
        }
        if let Some(bp) = self.underlying_cds_coupon_bp {
            option_params = option_params.with_underlying_cds_coupon(bp_to_decimal(bp));
        }

        let credit_params = CreditParams::corporate_standard("SN", "HZ-SN");
        let mut option = CDSOption::new(
            self.id,
            &option_params,
            &credit_params,
            "USD-OIS",
            "CDS-OPT-VOL",
        )
        .expect("valid CDS option");

        if let Some(vol) = self.implied_vol {
            option
                .instrument_pricing_overrides
                .market_quotes
                .implied_volatility = Some(vol);
        }
        option.knockout = self.knockout.unwrap_or(!self.is_index);

        option
    }
}

impl Default for CDSOptionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Assert that a value is finite and non-NaN
pub fn assert_finite(value: f64, msg: &str) {
    assert!(
        value.is_finite(),
        "{}: value is not finite ({})",
        msg,
        value
    );
}

/// Assert that a value is within expected range
pub fn assert_in_range(value: f64, min: f64, max: f64, msg: &str) {
    assert!(
        value >= min && value <= max,
        "{}: value {} not in range [{}, {}]",
        msg,
        value,
        min,
        max
    );
}

/// Assert that a value is positive
pub fn assert_positive(value: f64, msg: &str) {
    assert!(value > 0.0, "{}: value {} should be positive", msg, value);
}

/// Assert that a value is non-negative
pub fn assert_non_negative(value: f64, msg: &str) {
    assert!(
        value >= 0.0,
        "{}: value {} should be non-negative",
        msg,
        value
    );
}

/// Assert relative tolerance between two values
pub fn assert_approx_eq(actual: f64, expected: f64, rel_tol: f64, msg: &str) {
    let diff = (actual - expected).abs();
    let threshold = expected.abs() * rel_tol;
    assert!(
        diff <= threshold,
        "{}: actual={}, expected={}, diff={}, threshold={}",
        msg,
        actual,
        expected,
        diff,
        threshold
    );
}

/// Assert monotonic increasing property
pub fn assert_increasing(values: &[(f64, f64)], x_label: &str, y_label: &str) {
    for i in 1..values.len() {
        assert!(
            values[i].1 > values[i - 1].1,
            "{} should increase with {}: {}={} gives {}={}, but {}={} gives {}={}",
            y_label,
            x_label,
            x_label,
            values[i - 1].0,
            y_label,
            values[i - 1].1,
            x_label,
            values[i].0,
            y_label,
            values[i].1
        );
    }
}

/// Assert monotonic decreasing property
pub fn assert_decreasing(values: &[(f64, f64)], x_label: &str, y_label: &str) {
    for i in 1..values.len() {
        assert!(
            values[i].1 < values[i - 1].1,
            "{} should decrease with {}: {}={} gives {}={}, but {}={} gives {}={}",
            y_label,
            x_label,
            x_label,
            values[i - 1].0,
            y_label,
            values[i - 1].1,
            x_label,
            values[i].0,
            y_label,
            values[i].1
        );
    }
}
