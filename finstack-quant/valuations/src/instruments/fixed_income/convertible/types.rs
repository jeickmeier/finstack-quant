//! Convertible bond instrument types and implementation.
//!
//! Data model for `ConvertibleBond` and related enums used by pricing and
//! metrics modules. Pricing logic is intentionally kept out of this file.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_core::Error;

use crate::instruments::common_impl::parameters::convertible::{
    AntiDilutionPolicy, ConversionEvent, ConversionPolicy, ConversionSpec, DividendAdjustment,
    PriceTrigger,
};
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::fixed_income::bond::{CallPutSchedule, CashflowSpec};
use crate::instruments::model_params::ModelParamsSnapshot;
use finstack_quant_cashflows::builder::CashFlowSchedule;

use super::pricing;
use crate::instruments::common_impl::traits::impl_instrument_base;

/// Resolved exercise claim after debt has been converted, called, put, or redeemed.
///
/// These contractual amounts replace the debt and its future coupons. Any
/// earned but unpaid coupon must be included in `cash`. Shares are delivered
/// ex-dividend on `delivery_date`. Same-day completed exercise needs no record;
/// use this state to retain a delayed delivery without replaying the option.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ConvertibleSettlement {
    /// Date the holder's exercise decision extinguished the debt.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub exercise_date: Date,
    /// Contractual delivery/payment date; not earlier than exercise.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub delivery_date: Date,
    /// Fixed number of shares to deliver, finite and nonnegative.
    pub shares: f64,
    /// Fixed cash consideration including any retained coupon, in bond currency.
    pub cash: Money,
    /// Part of `cash` eligible for principal recovery on default; excludes
    /// uncapitalized coupon accrual. Must be between zero and `cash`.
    pub recovery_basis: Money,
}

/// Convertible bond instrument with embedded equity conversion option.
///
/// This fixed income instrument combines debt characteristics (coupons, principal)
/// with equity optionality (conversion rights). Uses the `CashFlowBuilder` for
/// robust schedule generation and tree-based pricing for the hybrid valuation.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ConvertibleBond {
    /// Unique identifier for the instrument.
    pub id: InstrumentId,
    /// Principal amount.
    pub notional: Money,
    /// Issue date.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub issue_date: Date,
    /// Maturity date.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Discount curve identifier for the debt component (risk-free or funding).
    pub discount_curve_id: CurveId,
    /// Issuer hazard curve identifier. Absence specifies no default risk.
    /// All continuation claims are survival-weighted. Default pays the explicit
    /// recovery fraction of outstanding principal and extinguishes equity;
    /// conditional stock drift includes the hazard compensator.
    pub credit_curve_id: Option<CurveId>,
    /// Conversion terms for equity conversion.
    pub conversion: ConversionSpec,
    /// Market-scalar id (`MarketContext::get_price`) of the underlying share
    /// price, as a price in the bond's currency or a unitless level.
    pub spot_id: PriceId,
    /// Equity volatility id: a volatility surface read at the conversion strike
    /// and maturity, or a unitless scalar holding a flat volatility.
    pub vol_surface_id: CurveId,
    /// Optional unitless continuous dividend-yield scalar id (decimal,
    /// 0.02 = 2%). `None` means a zero dividend yield; a configured id must
    /// resolve.
    pub div_yield_id: Option<PriceId>,
    /// Optional call/put schedule (issuer/holder redemption before maturity).
    pub call_put: Option<CallPutSchedule>,
    /// Optional soft-call trigger condition.
    ///
    /// When set, the issuer can only exercise call provisions if the underlying
    /// stock price satisfies the trigger condition (e.g., above 130% of conversion
    /// price for 20 of 30 trading days).
    ///
    /// **Modeling scope**: a single trigger gates the ENTIRE callable life —
    /// every window in [`Self::call_put`] is subject to it. A soft-call period
    /// followed by an unconditional hard-call period is not representable
    /// (omit the trigger to model an unconditional call). In the tree the
    /// trigger is evaluated on the instantaneous node spot with a
    /// Broadie-Glasserman-Kou-style barrier adjustment approximating the
    /// k-of-n-days observation window; the realized path is not tracked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_call_trigger: Option<PriceTrigger>,
    /// Recorded exercise awaiting delivery. At or after `exercise_date`, only
    /// this claim remains; after delivery its value is zero. Mandatory exercise
    /// without a record settles on its contractual conversion date.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_settlement: Option<ConvertibleSettlement>,
    /// Number of business days from trade date to settlement date.
    ///
    /// When set, accrued interest and clean price are computed relative to the
    /// settlement date (trade date + settlement_days business days) rather than
    /// the valuation date. Standard values:
    /// - **US corporate convertibles**: 2 (T+2)
    /// - **US Treasury**: 1 (T+1)
    ///
    /// If `None`, settlement is assumed same-day (as_of = settlement date).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_days: Option<u32>,
    /// Fraction of outstanding principal paid immediately on default, in
    /// `[0, 1]`; includes capitalized PIK and excludes prior amortization.
    /// Default extinguishes coupons and the conversion right. Required with a
    /// credit curve; recovery is an explicit cashflow, not a discount blend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_rate: Option<f64>,
    /// Coupon leg (fixed, floating, step-up or amortizing), the same type as
    /// `Bond.cashflow_spec`. A zero-coupon convertible is a fixed spec with
    /// rate `0`.
    pub cashflow_spec: CashflowSpec,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

/// Greeks for convertible bonds priced with tree models.
///
/// # Units and Conventions
///
/// - **Delta**: Per unit of spot (dPV/dS)
/// - **Gamma**: Per unit of spot squared (d²PV/dS²)
/// - **Vega**: Per 1% absolute volatility move (dPV for +1 vol point)
/// - **Theta**: Per calendar day (P(t+1d) - P(t), typically negative for long positions)
/// - **Rho**: Per 1 basis point parallel shift of the **risk-free discount
///   curve only** (dPV for +1bp). A configured credit curve is held fixed, so
///   for credit-curve convertibles rho isolates the equity-leg discounting and
///   drift sensitivity while the implied credit spread narrows by the bump;
///   the DV01 metric (parallel, all curves) is the full parallel-rate number.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ConvertibleGreeks {
    /// Instrument price
    pub price: f64,
    /// Delta (spot sensitivity per unit spot move)
    pub delta: f64,
    /// Gamma (curvature, second derivative w.r.t. spot)
    pub gamma: f64,
    /// Vega (volatility sensitivity per 1% vol move)
    pub vega: f64,
    /// Theta (time decay per day)
    pub theta: f64,
    /// Rho (interest rate sensitivity per 1bp rate move)
    pub rho: f64,
}

impl ConvertibleBond {
    fn validate(&self) -> finstack_quant_core::Result<()> {
        use finstack_quant_core::validation;

        validation::validate_date_range_strict(self.issue_date, self.maturity, "convertible bond")?;
        validation::validate_money_finite(self.notional, "convertible bond notional")?;
        validation::validate_money_gt(self.notional, 0.0, "convertible bond notional")?;

        if let Some(settlement) = &self.resolved_settlement {
            validate_conversion_date(
                settlement.exercise_date,
                self.issue_date,
                self.maturity,
                "resolved exercise",
            )?;
            validation::validate_date_range_non_strict(
                settlement.exercise_date,
                settlement.delivery_date,
                "convertible delivery",
            )?;
            if !settlement.shares.is_finite()
                || settlement.shares < 0.0
                || !settlement.cash.amount().is_finite()
                || settlement.cash.amount() < 0.0
            {
                return Err(finstack_quant_core::Error::Validation(
                    "convertible delivery amounts must be finite and nonnegative".into(),
                ));
            }
            if settlement.recovery_basis.amount() < 0.0
                || settlement.recovery_basis.amount() > settlement.cash.amount()
            {
                return Err(finstack_quant_core::Error::Validation("convertible settlement recovery basis must be between zero and cash consideration".into()));
            }
            if settlement.recovery_basis.currency() != self.notional.currency() {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected: self.notional.currency(),
                    actual: settlement.recovery_basis.currency(),
                });
            }
            if settlement.cash.currency() != self.notional.currency() {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected: self.notional.currency(),
                    actual: settlement.cash.currency(),
                });
            }
        }

        match (self.conversion.ratio, self.conversion.price) {
            (None, None) => {
                return Err(finstack_quant_core::Error::Validation(
                    "convertible bond requires conversion.ratio or conversion.price".to_string(),
                ));
            }
            (Some(ratio), price) => {
                validation::validate_f64_positive(ratio, "convertible bond conversion ratio")?;
                if let Some(price) = price {
                    validation::validate_f64_positive(price, "convertible bond conversion price")?;
                    let implied_ratio = self.notional.amount() / price;
                    let tolerance = 1e-10 * ratio.abs().max(implied_ratio.abs()).max(1.0);
                    if (ratio - implied_ratio).abs() > tolerance {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "convertible bond conversion ratio ({ratio}) is inconsistent with \
                             notional / conversion price ({implied_ratio})"
                        )));
                    }
                }
            }
            (None, Some(price)) => {
                validation::validate_f64_positive(price, "convertible bond conversion price")?
            }
        }

        match &self.conversion.policy {
            ConversionPolicy::Voluntary
            | ConversionPolicy::UponEvent(
                ConversionEvent::QualifiedIpo | ConversionEvent::ChangeOfControl,
            ) => {}
            ConversionPolicy::MandatoryOn(date) => {
                validate_conversion_date(*date, self.issue_date, self.maturity, "mandatory")?;
            }
            ConversionPolicy::Window { start, end } => {
                validation::validate_date_range_non_strict(
                    *start,
                    *end,
                    "convertible conversion window",
                )?;
                validate_conversion_date(*start, self.issue_date, self.maturity, "window start")?;
                validate_conversion_date(*end, self.issue_date, self.maturity, "window end")?;
            }
            ConversionPolicy::UponEvent(ConversionEvent::PriceTrigger(trigger)) => {
                trigger.validate("conversion.policy.upon_event.price_trigger")?;
            }
            ConversionPolicy::MandatoryVariable {
                conversion_date,
                upper_conversion_price,
                lower_conversion_price,
            } => {
                validate_conversion_date(
                    *conversion_date,
                    self.issue_date,
                    self.maturity,
                    "mandatory-variable",
                )?;
                validation::validate_f64_positive(
                    *lower_conversion_price,
                    "mandatory-variable lower conversion price",
                )?;
                validation::validate_f64_positive(
                    *upper_conversion_price,
                    "mandatory-variable upper conversion price",
                )?;
                if lower_conversion_price > upper_conversion_price {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "convertible bond '{}' has inverted mandatory-variable bounds: \
                         lower conversion price ({lower_conversion_price}) must not exceed \
                         upper conversion price ({upper_conversion_price})",
                        self.id.as_str()
                    )));
                }
            }
        }

        if let Some(pair) = self
            .conversion
            .dilution_events
            .windows(2)
            .find(|pair| pair[1].date < pair[0].date)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "convertible bond '{}' dilution events must be in chronological order; \
                 {} is recorded after {}",
                self.id.as_str(),
                pair[1].date,
                pair[0].date
            )));
        }
        for event in &self.conversion.dilution_events {
            validate_conversion_date(event.date, self.issue_date, self.maturity, "dilution event")?;
            validation::validate_f64_positive(
                event.new_issue_price,
                "convertible dilution-event issue price",
            )?;
            validation::validate_f64_positive(
                event.new_shares_issued,
                "convertible dilution-event new shares",
            )?;
            validation::validate_f64_positive(
                event.shares_outstanding_before,
                "convertible dilution-event prior shares outstanding",
            )?;
        }

        if self.conversion_ratio().is_some() && self.effective_conversion_ratio().is_none() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "convertible bond '{}' anti-dilution adjustments collapse the conversion \
                 price to numerical zero; check the recorded dilution events",
                self.id.as_str()
            )));
        }

        if self.spot_id.as_str().trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "convertible bond requires a non-empty spot_id".to_string(),
            ));
        }

        if let Some(rate) = self.cashflow_spec.fixed_coupon_rate() {
            if rate < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "convertible bond cashflow_spec fixed coupon rate must be non-negative, got {rate}"
                )));
            }
        }
        if let Some(recovery_rate) = self.recovery_rate {
            if !recovery_rate.is_finite() || !(0.0..=1.0).contains(&recovery_rate) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "convertible bond '{}' recovery_rate must be finite and in [0, 1], got {recovery_rate}",
                    self.id.as_str()
                )));
            }
        }
        if self.credit_curve_id.is_some() && self.recovery_rate.is_none() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "convertible bond '{}' requires an explicit recovery_rate when credit_curve_id is set",
                self.id.as_str()
            )));
        }
        if let Some(trigger) = &self.soft_call_trigger {
            trigger.validate("soft_call_trigger")?;
        }
        if let Some(call_put) = &self.call_put {
            call_put.validate_for_life(self.issue_date, self.maturity, "Convertible bond")?;
        }

        Ok(())
    }

    /// Base conversion ratio (shares per bond) derived from explicit ratio or price.
    ///
    /// Returns `None` if neither `ratio` nor `price` is set on the conversion spec.
    pub fn conversion_ratio(&self) -> Option<f64> {
        if let Some(ratio) = self.conversion.ratio {
            Some(ratio)
        } else {
            self.conversion.price.map(|p| self.notional.amount() / p)
        }
    }

    /// Effective conversion ratio after anti-dilution adjustments.
    ///
    /// Applies all recorded [`DilutionEvent`]s in chronological order using
    /// the bond's [`AntiDilutionPolicy`]:
    ///
    /// - **None**: Returns the base conversion ratio unchanged.
    /// - **FullRatchet**: Conversion price is reduced to the lowest new issue
    ///   price across all dilution events. The ratio is then `notional / adjusted_price`.
    /// - **WeightedAverage**: Conversion price is adjusted using the broad-based
    ///   weighted average formula for each event sequentially.
    ///
    /// # Returns
    ///
    /// The adjusted conversion ratio. Returns `None` when neither ratio nor
    /// price is set, or when the recorded dilution events collapse the
    /// adjusted conversion price to numerical zero (below `1e-10`), which
    /// indicates corrupt event data; `validate` reports this case explicitly.
    // `AntiDilutionPolicy::None` is excluded by the `matches!` early return below.
    #[allow(clippy::unreachable)]
    pub fn effective_conversion_ratio(&self) -> Option<f64> {
        let base_ratio = self.conversion_ratio()?;

        // If no anti-dilution or no events, return base ratio
        if matches!(self.conversion.anti_dilution, AntiDilutionPolicy::None)
            || self.conversion.dilution_events.is_empty()
        {
            return Some(base_ratio);
        }

        // Start with the original conversion price
        let notional = self.notional.amount();
        let mut current_cp = notional / base_ratio;

        // Events are stored in chronological order (enforced by validation)
        // and applied sequentially.
        for event in &self.conversion.dilution_events {
            match &self.conversion.anti_dilution {
                AntiDilutionPolicy::None => unreachable!(
                    "effective_conversion_ratio reached AntiDilutionPolicy::None; the \
                     early return above excludes it"
                ),
                AntiDilutionPolicy::FullRatchet => {
                    // Full ratchet: conversion price drops to the new issue price
                    // if it is below the current conversion price.
                    if event.new_issue_price < current_cp {
                        current_cp = event.new_issue_price;
                    }
                }
                AntiDilutionPolicy::WeightedAverage => {
                    // Broad-based weighted average formula:
                    //   new_cp = old_cp × (O + new_money / old_cp) / (O + N)
                    // where:
                    //   O = shares outstanding before the event
                    //   N = new shares issued
                    //   new_money = N × new_issue_price
                    let o = event.shares_outstanding_before;
                    let n = event.new_shares_issued;
                    let new_money = n * event.new_issue_price;

                    if (o + n) > 0.0 {
                        let numerator = o + new_money / current_cp;
                        let denominator = o + n;
                        current_cp *= numerator / denominator;
                    }
                }
            }
        }

        // A conversion price collapsing to (numerical) zero means the dilution
        // events are corrupt: returning the unadjusted base ratio here would
        // silently discard the anti-dilution adjustments and misprice the bond.
        // Return `None` so pricing fails loudly (validation reports the cause).
        if current_cp < 1e-10 {
            return None;
        }

        Some(notional / current_cp)
    }

    /// Create a canonical example convertible bond for testing and documentation.
    ///
    /// Returns a 5-year convertible with fixed coupon and voluntary conversion.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_cashflows::builder::CouponType;
        use finstack_quant_cashflows::builder::FixedCouponSpec;
        use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
        use time::macros::date;

        let coupon_rate = finstack_quant_core::decimal::f64_to_decimal(0.02)?;

        ConvertibleBond::builder()
            .id(InstrumentId::new("CB-TECH-5Y"))
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .issue_date(date!(2024 - 01 - 15))
            .maturity(date!(2029 - 01 - 15))
            .discount_curve_id(CurveId::new("USD-IG"))
            .credit_curve_id_opt(Some(CurveId::new("USD-CREDIT-BBB")))
            .recovery_rate_opt(Some(0.0))
            .conversion(ConversionSpec {
                ratio: Some(25.0),
                price: None,
                policy: ConversionPolicy::Voluntary,
                anti_dilution: AntiDilutionPolicy::None,
                dividend_adjustment: DividendAdjustment::None,
                dilution_events: Vec::new(),
            })
            .spot_id(PriceId::new("TECH"))
            .vol_surface_id(CurveId::new("TECH-VOL"))
            .call_put_opt(None)
            .cashflow_spec(CashflowSpec::Fixed(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: coupon_rate,
                schedule: finstack_quant_cashflows::builder::ScheduleParams {
                    frequency: Tenor::semi_annual(),

                    day_count: DayCount::Thirty360,

                    business_day_convention: BusinessDayConvention::Following,

                    calendar_id: "weekends_only".into(),

                    stub: StubKind::None,

                    end_of_month: false,

                    payment_lag_days: 0,

                    adjust_accrual_dates: false,
                    roll_rule: finstack_quant_cashflows::builder::RollRule::None,
                },
            }))
            .attributes(Attributes::new())
            .build()
    }

    /// Create a mandatory convertible bond example (PERCS/DECS style).
    ///
    /// Returns a 3-year mandatory convertible with variable delivery ratio,
    /// soft-call trigger, and a call/put schedule:
    /// - **Notional:** $1M USD
    /// - **Coupon:** 5.0% semi-annual (higher coupon compensates for capped upside)
    /// - **Conversion:** Mandatory variable at maturity (DECS-style)
    ///   - Upper conversion price: $60 (above this, min shares delivered)
    ///   - Lower conversion price: $40 (below this, max shares delivered)
    /// - **Soft call:** 130% trigger, 20 of 30 trading days
    /// - **Call schedule:** Issuer can call at 101% after year 2
    /// - **Put schedule:** Holder can put at 100% after year 1
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example_mandatory() -> finstack_quant_core::Result<Self> {
        use crate::instruments::fixed_income::bond::CallPut;
        use finstack_quant_cashflows::builder::CouponType;
        use finstack_quant_cashflows::builder::FixedCouponSpec;
        use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
        use time::macros::date;

        let issue = date!(2024 - 03 - 15);
        let maturity = date!(2027 - 03 - 15);
        let coupon_rate = finstack_quant_core::decimal::f64_to_decimal(0.05)?;

        ConvertibleBond::builder()
            .id(InstrumentId::new("CB-MAND-DECS-3Y"))
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .issue_date(issue)
            .maturity(maturity)
            .discount_curve_id(CurveId::new("USD-IG"))
            .credit_curve_id_opt(Some(CurveId::new("USD-CREDIT-BBB")))
            .recovery_rate_opt(Some(0.0))
            .conversion(ConversionSpec {
                ratio: None,
                price: Some(50.0), // reference price for base conversion ratio
                policy: ConversionPolicy::MandatoryVariable {
                    conversion_date: maturity,
                    upper_conversion_price: 60.0,
                    lower_conversion_price: 40.0,
                },
                anti_dilution: AntiDilutionPolicy::WeightedAverage,
                // Mandatory convertibles (PERCS/DECS) typically compensate the
                // holder through the enhanced coupon rather than dividend
                // protection, so `None` is the economically representative
                // choice here. See the `DividendAdjustment` docs for a
                // protected (AdjustRatio) example.
                dividend_adjustment: DividendAdjustment::None,
                dilution_events: Vec::new(),
            })
            .spot_id(PriceId::new("INDU"))
            .vol_surface_id(CurveId::new("INDU-VOL"))
            .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides {
                market_quotes: crate::instruments::MarketQuoteOverrides {
                    implied_volatility: Some(0.01),
                    ..Default::default()
                },
                ..Default::default()
            })
            .call_put_opt(Some(CallPutSchedule {
                calls: vec![CallPut {
                    start: date!(2026 - 03 - 15),
                    end: maturity,
                    price_pct_of_par: 101.0,
                    make_whole: None,
                }],
                puts: vec![CallPut {
                    start: date!(2025 - 03 - 15),
                    end: date!(2025 - 03 - 15),
                    price_pct_of_par: 100.0,
                    make_whole: None,
                }],
            }))
            .soft_call_trigger_opt(Some(PriceTrigger {
                threshold_pct: 130.0,
                observation_days: 30,
                required_days_above: 20,
            }))
            .recovery_rate_opt(Some(0.35))
            .cashflow_spec(CashflowSpec::Fixed(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: coupon_rate,
                schedule: finstack_quant_cashflows::builder::ScheduleParams {
                    frequency: Tenor::semi_annual(),

                    day_count: DayCount::Thirty360,

                    business_day_convention: BusinessDayConvention::Following,

                    calendar_id: "weekends_only".into(),

                    stub: StubKind::None,

                    end_of_month: false,

                    payment_lag_days: 0,

                    adjust_accrual_dates: false,
                    roll_rule: finstack_quant_cashflows::builder::RollRule::None,
                },
            }))
            .attributes(Attributes::new())
            .build()
    }

    /// Calculate policy-specific equity conversion value divided by notional.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing `spot_id` as the
    ///   current share price in the bond's currency. Mandatory-variable terms
    ///   determine share delivery from the lower and upper conversion prices.
    pub fn parity(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::traits::Instrument::validate_for_pricing(self)?;
        let spot_price = market.get_price(&self.spot_id)?;
        let spot = match spot_price {
            finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => money.amount(),
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
        };

        pricing::calculate_parity(self, spot)
    }

    /// Calculate the decimal premium over policy-specific equity conversion value.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the current underlying share price
    ///   in the bond's currency; missing data and invalid conversion terms fail.
    /// * `bond_price` - Finite nonnegative total bond value in notional-currency
    ///   units, not percent of par. Returns `bond_price / conversion_value - 1`;
    ///   a nonpositive or non-finite conversion value fails.
    pub fn conversion_premium(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        bond_price: f64,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::traits::Instrument::validate_for_pricing(self)?;
        let spot_price = market.get_price(&self.spot_id)?;
        let spot = match spot_price {
            finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => money.amount(),
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
        };

        let conversion_value = pricing::compute_conversion_value(self, spot)?;
        if !bond_price.is_finite()
            || bond_price < 0.0
            || !conversion_value.is_finite()
            || conversion_value <= 0.0
        {
            return Err(finstack_quant_core::Error::Validation(
                "conversion premium requires a finite nonnegative bond price and positive conversion value".into(),
            ));
        }
        Ok(bond_price / conversion_value - 1.0)
    }

    /// Calculate Greeks by repricing the convertible binomial lattice.
    ///
    /// # Arguments
    ///
    /// * `market` - Discount and risky discount curves, equity price and
    ///   volatility, plus forward curves and realized fixings for floating
    ///   coupons. An instrument volatility override takes precedence; surface
    ///   lookup otherwise uses the contractual conversion strike.
    /// * `as_of` - Valuation date and origin of the ACT/365F equity model clock.
    ///
    /// The binomial lattice step count is
    /// `instrument_pricing_overrides.model_config.tree_steps` (default 200).
    ///
    /// Bump sizes come from `metric_pricing_overrides.bump_config`
    /// (`spot_bump_decimal`, `vol_bump_decimal`, `rate_bump_bp`), defaulting to 1%,
    /// 1 vol point and 1bp. Vega is per volatility point and rho per bp.
    pub fn greeks(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<ConvertibleGreeks> {
        let bumps = crate::metrics::sensitivities::config::from_context_or_default(
            &finstack_quant_core::config::FinstackConfig::default(),
            Some(&self.metric_pricing_overrides),
        )?;
        pricing::calculate_convertible_greeks(
            self,
            market,
            crate::instruments::GreekBumps::from(&bumps),
            as_of,
        )
    }
}

impl crate::instruments::common_impl::traits::Instrument for ConvertibleBond {
    impl_instrument_base!(
        crate::pricer::InstrumentType::Convertible,
        crate::instruments::fixed_income::FIXED_INCOME
    );

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Tree
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        use crate::instruments::common_impl::dependencies::VolatilityDependency;

        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(credit_curve_id) = &self.credit_curve_id {
            deps.add_credit_curve(credit_curve_id.clone());
        }
        if let Some(rate_spec) = self.cashflow_spec.floating_rate_spec() {
            deps.add_forward_curve(rate_spec.forward_curve_id.clone());
            deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
                rate_spec.forward_curve_id.as_str(),
            ));
        }
        if let Some(call_put) = &self.call_put {
            for option in call_put.calls.iter().chain(&call_put.puts) {
                if let Some(make_whole) = &option.make_whole {
                    deps.add_discount_curve(make_whole.reference_curve_id.clone());
                }
            }
        }
        deps.add_market_scalar_id(&self.spot_id);
        if let Some(div_yield_id) = &self.div_yield_id {
            deps.add_market_scalar_id(div_yield_id);
        }
        let reference_strike = self
            .effective_conversion_ratio()
            .filter(|ratio| *ratio > 0.0)
            .map(|ratio| self.notional.amount() / ratio);
        // Convertible volatility may be supplied either as a unitless
        // MarketScalar or as a full surface under `vol_surface_id`.
        deps.add_market_scalar_id(&self.vol_surface_id);
        deps.add_volatility_dependency(VolatilityDependency::new(
            self.vol_surface_id.clone(),
            Some(self.spot_id.clone()),
            reference_strike,
        ));
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        if let Some(ref trigger) = self.soft_call_trigger {
            trigger.validate("soft_call_trigger")?;
        }
        pricing::price_convertible_bond(self, curves, as_of)
    }

    fn model_params_snapshot(&self) -> ModelParamsSnapshot {
        ModelParamsSnapshot::Convertible {
            conversion_spec: self.conversion.clone(),
        }
    }

    fn with_model_params(
        &self,
        params: &ModelParamsSnapshot,
    ) -> finstack_quant_core::Result<Box<dyn crate::instruments::common_impl::traits::Instrument>>
    {
        match params {
            ModelParamsSnapshot::Convertible { conversion_spec } => {
                let mut modified = self.clone();
                modified.conversion = conversion_spec.clone();
                Ok(Box::new(modified))
            }
            ModelParamsSnapshot::None => Ok(self.clone_box()),
            ModelParamsSnapshot::StructuredCredit { .. } => Err(Error::Validation(
                "Instrument type mismatch: expected ConvertibleBond model parameters".to_string(),
            )),
        }
    }

    crate::instruments::common_impl::traits::impl_focused_pricing_overrides!();
}

fn validate_conversion_date(
    date: Date,
    issue_date: Date,
    maturity: Date,
    context: &str,
) -> finstack_quant_core::Result<()> {
    if date < issue_date || date > maturity {
        return Err(finstack_quant_core::Error::Validation(format!(
            "convertible {context} date {date} is outside instrument life \
             [{issue_date}, {maturity}]"
        )));
    }
    Ok(())
}

impl finstack_quant_cashflows::CashflowScheduleSource for ConvertibleBond {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        if let Some(claim) = &self.resolved_settlement {
            self.validate()?;
            if as_of < claim.exercise_date {
                return Err(Error::Validation(
                    "resolved convertible settlement cannot be reported before exercise".into(),
                ));
            }
            let flows = if claim.delivery_date > as_of && claim.cash.amount() > 0.0 {
                vec![finstack_quant_core::cashflow::CashFlow::new(
                    claim.delivery_date,
                    None,
                    claim.cash,
                    finstack_quant_core::cashflow::CFKind::Notional,
                    0.0,
                    None,
                )]
            } else {
                Vec::new()
            };
            return CashFlowSchedule::from_flows(
                flows,
                finstack_quant_cashflows::builder::Notional::par(
                    claim.cash.amount(),
                    self.notional.currency(),
                )?,
                self.cashflow_spec.day_count(),
                finstack_quant_cashflows::builder::CashFlowMeta {
                    issue_date: Some(claim.exercise_date),
                    maturity: Some(claim.delivery_date),
                    ..Default::default()
                },
            );
        }
        let mut schedule = pricing::build_convertible_schedule(self, curves)?;
        let mandatory = match self.conversion.policy {
            ConversionPolicy::MandatoryOn(date) => Some(date),
            ConversionPolicy::MandatoryVariable {
                conversion_date, ..
            } => Some(conversion_date),
            _ => None,
        };
        if let Some(date) = mandatory.filter(|date| *date < as_of) {
            schedule.retain_flows(|flow| {
                flow.date > as_of
                    && !flow.kind.is_principal_like()
                    && flow
                        .accrual
                        .as_ref()
                        .is_some_and(|period| period.end <= date)
            });
        }
        Ok(schedule.with_representation(
            finstack_quant_cashflows::builder::CashflowRepresentation::Contractual,
        ))
    }
}

// Declare canonical market dependencies for the DV01 calculator.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::parameters::convertible::DilutionEvent;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_cashflows::CashflowProvider;

    #[test]
    fn market_dependencies_use_typed_equity_ids_only() {
        let mut bond = ConvertibleBond::example().expect("example");
        // Retired `attributes.meta` market keys are ordinary tags now.
        bond.attributes
            .meta
            .insert("vol_surface_id".to_string(), "TECH-CUSTOM-VOL".to_string());
        let deps =
            crate::instruments::Instrument::market_dependencies(&bond).expect("dependencies");

        assert_eq!(deps.market_scalar_ids, vec!["TECH", "TECH-VOL"]);
        assert_eq!(
            deps.volatility_dependencies
                .iter()
                .map(|dependency| dependency.vol_surface_id.as_str())
                .collect::<Vec<_>>(),
            vec!["TECH-VOL"]
        );
    }

    #[test]
    fn floating_convertible_uses_the_canonical_fixing_series_id() {
        let mut bond = ConvertibleBond::example().expect("example");
        let floating_bond = crate::instruments::fixed_income::bond::Bond::example_floating()
            .expect("floating bond example");
        let crate::instruments::fixed_income::bond::CashflowSpec::Floating(floating_coupon) =
            &floating_bond.cashflow_spec
        else {
            unreachable!("floating example must have a floating coupon")
        };
        let expected = finstack_quant_core::market_data::fixings::fixing_series_id(
            floating_coupon.rate_spec.forward_curve_id.as_str(),
        );
        bond.cashflow_spec = floating_bond.cashflow_spec.clone();

        let deps =
            crate::instruments::Instrument::market_dependencies(&bond).expect("dependencies");
        assert!(deps.series_ids.contains(&expected));
    }

    #[test]
    fn test_cashflow_provider_matches_convertible_schedule_builder() {
        let bond = ConvertibleBond::example().expect("example should build");
        let market = finstack_quant_core::market_data::context::MarketContext::new();
        let expected = super::pricing::build_convertible_schedule(&bond, &market)
            .expect("schedule should build");
        let actual = bond
            .cashflow_schedule(&market, bond.issue_date)
            .expect("provider schedule should build");

        assert_eq!(actual.get_flows(), expected.get_flows());
        assert_eq!(
            actual.get_notional().initial,
            expected.get_notional().initial
        );
        assert_eq!(actual.get_day_count(), expected.get_day_count());
    }

    #[test]
    fn validation_rejects_incomplete_conversion_terms_and_invalid_soft_call() {
        let mut bond = ConvertibleBond::example().expect("example");
        bond.conversion.ratio = None;
        bond.conversion.price = None;
        assert!(bond
            .validate_for_pricing()
            .expect_err("missing conversion terms must fail")
            .to_string()
            .contains("conversion.ratio"));

        bond.conversion.ratio = Some(25.0);
        bond.soft_call_trigger = Some(PriceTrigger {
            threshold_pct: f64::NAN,
            observation_days: 30,
            required_days_above: 20,
        });
        assert!(bond
            .validate_for_pricing()
            .expect_err("NaN soft-call trigger must fail")
            .to_string()
            .contains("finite"));
    }

    /// Dividend protection is modeled by the pricer (continuous-yield ratio
    /// accretion), so every `dividend_adjustment` variant must pass validation.
    #[test]
    fn validation_accepts_all_dividend_adjustment_variants() {
        let mut bond = ConvertibleBond::example().expect("example");

        for adjustment in [
            DividendAdjustment::None,
            DividendAdjustment::AdjustPrice,
            DividendAdjustment::AdjustRatio,
        ] {
            bond.conversion.dividend_adjustment = adjustment;
            bond.validate_for_pricing().unwrap_or_else(|err| {
                panic!(
                    "{:?} must pass pricing validation: {err}",
                    bond.conversion.dividend_adjustment
                )
            });
        }

        // The shipped mandatory example must also pass pricing validation.
        let mandatory = ConvertibleBond::example_mandatory().expect("mandatory example");
        mandatory
            .validate_for_pricing()
            .expect("example_mandatory must pass pricing validation");
    }

    /// `AdjustPrice`/`AdjustRatio` must survive a serde round-trip with the
    /// stable variant names used by the wire schema.
    #[test]
    fn dividend_adjustment_serde_round_trip() {
        for (adjustment, expected_json) in [
            (DividendAdjustment::None, "\"none\""),
            (DividendAdjustment::AdjustPrice, "\"adjust_price\""),
            (DividendAdjustment::AdjustRatio, "\"adjust_ratio\""),
        ] {
            let json = serde_json::to_string(&adjustment).expect("serialize");
            assert_eq!(json, expected_json);
            let parsed: DividendAdjustment = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed.is_protected(), adjustment.is_protected());
            assert_eq!(format!("{parsed:?}"), format!("{adjustment:?}"));
        }
    }

    /// Regression: dilution events that collapse the adjusted conversion price
    /// to numerical zero previously fell back silently to the unadjusted base
    /// ratio. `effective_conversion_ratio` now returns `None` and validation
    /// reports the corrupt events explicitly.
    #[test]
    fn validation_rejects_collapsed_anti_dilution_conversion_price() {
        let mut bond = ConvertibleBond::example().expect("example");
        bond.conversion.anti_dilution = AntiDilutionPolicy::FullRatchet;
        bond.conversion.dilution_events = vec![DilutionEvent {
            date: bond.issue_date,
            new_issue_price: 1e-12, // ratchets the conversion price to ~zero
            new_shares_issued: 1.0,
            shares_outstanding_before: 100.0,
        }];

        assert!(
            bond.effective_conversion_ratio().is_none(),
            "collapsed conversion price must not fall back to the base ratio"
        );
        let err = bond
            .validate_for_pricing()
            .expect_err("collapsed conversion price must fail validation");
        assert!(
            err.to_string().contains("collapse"),
            "error must explain the anti-dilution collapse; got: {err}"
        );
    }

    /// Weighted-average adjustments are order dependent, so dilution events
    /// must be recorded chronologically; out-of-order events fail validation.
    #[test]
    fn validation_rejects_out_of_order_dilution_events() {
        let mut bond = ConvertibleBond::example().expect("example");
        bond.conversion.anti_dilution = AntiDilutionPolicy::WeightedAverage;
        let event = |date| DilutionEvent {
            date,
            new_issue_price: 50.0,
            new_shares_issued: 10.0,
            shares_outstanding_before: 100.0,
        };
        let later = bond.issue_date + time::Duration::days(60);
        bond.conversion.dilution_events = vec![event(later), event(bond.issue_date)];
        let err = bond
            .validate_for_pricing()
            .expect_err("out-of-order dilution events must fail");
        assert!(err.to_string().contains("chronological"), "{err}");

        bond.conversion.dilution_events = vec![event(bond.issue_date), event(later)];
        bond.validate_for_pricing().expect("chronological events");
    }

    #[test]
    fn validation_rejects_inconsistent_conversion_quotes() {
        let mut bond = ConvertibleBond::example().expect("example");
        bond.conversion.price = Some(100.0);
        assert!(bond
            .validate_for_pricing()
            .expect_err("inconsistent ratio and price must fail")
            .to_string()
            .contains("inconsistent"));
    }
}
