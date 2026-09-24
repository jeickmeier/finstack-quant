//! Convertible bond instrument types and implementation.
//!
//! Data model for `ConvertibleBond` and related enums used by pricing and
//! metrics modules. Pricing logic is intentionally kept out of this file.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Error;

use crate::cashflow::builder::specs::{FixedCouponSpec, FloatingCouponSpec};
use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::fixed_income::bond::CallPutSchedule;
use crate::instruments::model_params::ModelParamsSnapshot;

use super::pricing;
use crate::impl_instrument_base;

/// Soft-call trigger condition for convertible bonds.
///
/// A soft call allows the issuer to call the bond only if the underlying stock
/// price has been trading above a threshold (typically 130% of the conversion
/// price) for a sustained period. This protects holders from having their
/// conversion option terminated when the stock is only marginally above parity.
///
/// # Industry Practice
///
/// The standard soft-call trigger is:
/// - **Threshold**: 130% of conversion price (most common)
/// - **Observation period**: 20 of 30 consecutive trading days
///
/// Some issuances use 120% or 150% thresholds.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SoftCallTrigger {
    /// Threshold as a percentage of conversion price (e.g., 130.0 = 130%).
    ///
    /// The issuer can only exercise the call if the stock price exceeds
    /// `threshold_pct / 100 * conversion_price` for the required number of days.
    pub threshold_pct: f64,
    /// Number of trading days in the observation window (e.g., 30).
    pub observation_days: u32,
    /// Minimum number of days within the window that the stock must exceed
    /// the threshold (e.g., 20 out of 30 days).
    pub required_days_above: u32,
}

impl Default for SoftCallTrigger {
    /// Standard market convention: 130% trigger, 20 of 30 days.
    fn default() -> Self {
        Self {
            threshold_pct: 130.0,
            observation_days: 30,
            required_days_above: 20,
        }
    }
}

impl SoftCallTrigger {
    /// Validate soft-call trigger parameters.
    ///
    /// - `threshold_pct` must exceed 100% (otherwise the trigger is trivially satisfied).
    /// - `observation_days` and `required_days_above` must be non-zero.
    /// - `required_days_above` cannot exceed `observation_days`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.threshold_pct.is_finite() || self.threshold_pct <= 100.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "soft-call threshold_pct ({:.1}%) must be finite and exceed 100%",
                self.threshold_pct,
            )));
        }
        if self.observation_days == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "soft-call observation_days must be greater than zero".to_string(),
            ));
        }
        if self.required_days_above == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "soft-call required_days_above must be greater than zero".to_string(),
            ));
        }
        if self.required_days_above > self.observation_days {
            return Err(finstack_quant_core::Error::Validation(format!(
                "soft-call required_days_above ({}) cannot exceed observation_days ({})",
                self.required_days_above, self.observation_days,
            )));
        }
        Ok(())
    }
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
    /// Credit curve identifier for risky discounting (bond floor).
    /// If not provided, falls back to discount_curve_id (implies no credit spread).
    ///
    /// **Convention**: this curve must represent ZERO-RECOVERY (pure hazard)
    /// risky discounting, i.e. `risky_df = rf_df * survival_probability`. The
    /// pricer converts it to recovery-adjusted discounting via the blend
    /// `risky * (1 - R) + rf * R` using [`Self::recovery_rate`]. Supplying a
    /// market recovery-adjusted spread curve here double-counts `(1 - R)` and
    /// overstates the credit discount.
    #[builder(optional)]
    pub credit_curve_id: Option<CurveId>,
    /// Conversion terms for equity conversion.
    pub conversion: ConversionSpec,
    /// Optional underlying equity identifier (ticker or instrument id).
    #[builder(optional)]
    pub underlying_equity_id: Option<String>,
    /// Optional call/put schedule (issuer/holder redemption before maturity).
    #[builder(optional)]
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
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_call_trigger: Option<SoftCallTrigger>,
    /// Number of business days from trade date to settlement date.
    ///
    /// When set, accrued interest and clean price are computed relative to the
    /// settlement date (trade date + settlement_days business days) rather than
    /// the valuation date. Standard values:
    /// - **US corporate convertibles**: 2 (T+2)
    /// - **US Treasury**: 1 (T+1)
    ///
    /// If `None`, settlement is assumed same-day (as_of = settlement date).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_days: Option<u32>,
    /// Assumed recovery rate on default, as a fraction (e.g., 0.40 = 40%).
    ///
    /// Used in the Tsiveriotis-Zhang credit model to blend risky and risk-free
    /// discounting on the cash component. A recovery rate of 0 reduces to the
    /// standard zero-recovery TZ model. Typical values:
    /// - **Investment grade**: 0.40 (ISDA standard assumption)
    /// - **High yield**: 0.25-0.35
    /// - **Distressed**: 0.10-0.20
    ///
    /// Required when `credit_curve_id` is set; otherwise absence means that no
    /// credit adjustment is requested.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_rate: Option<f64>,
    /// Fixed coupon specification (if applicable).
    #[builder(optional)]
    pub fixed_coupon: Option<FixedCouponSpec>,
    /// Floating coupon specification (if applicable).
    #[builder(optional)]
    pub floating_coupon: Option<FloatingCouponSpec>,
    /// Attributes for selection and tagging.
    #[builder(default)]
    /// Instrument-owned pricing inputs.
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

impl From<finstack_quant_models::TreeGreeks> for ConvertibleGreeks {
    fn from(g: finstack_quant_models::TreeGreeks) -> Self {
        Self {
            price: g.price,
            delta: g.delta,
            gamma: g.gamma,
            vega: g.vega,
            theta: g.theta,
            rho: g.rho,
        }
    }
}

/// Defines how and when conversion can occur.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConversionPolicy {
    /// Holder may convert at any time (subject to window, if any).
    Voluntary,
    /// Bond will mandatorily convert on the specified date.
    ///
    /// **Modeling scope**: the tree pricer models conversion only at the
    /// single step mapped from this date; early voluntary conversion before
    /// the mandatory date is not modeled.
    MandatoryOn(
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        Date,
    ),
    /// Holder may convert within a window.
    Window {
        /// Start.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        start: Date,
        /// End.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        end: Date,
    },
    /// Conversion tied to an external event or condition.
    UponEvent(ConversionEvent),
    /// Mandatory conversion with variable delivery ratio (PERCS / DECS / ACES).
    ///
    /// At `conversion_date`, the delivery ratio depends on the stock price:
    /// - If `spot <= lower_conversion_price`: ratio = face / lower_price (max shares, loss)
    /// - If `lower < spot <= upper`: ratio = face / spot (variable, delivers face value)
    /// - If `spot > upper_conversion_price`: ratio = face / upper_price (min shares, capped)
    ///
    /// # Industry Practice
    ///
    /// PERCS (Preference Equity Redemption Cumulative Stock) cap the upside.
    /// DECS (Dividend Enhanced Convertible Stock) have a dead zone between prices.
    /// ACES (Automatically Convertible Equity Securities) are similar to DECS.
    ///
    /// **Modeling scope**: the tree pricer models conversion only at the
    /// single step mapped from `conversion_date`; early voluntary conversion
    /// before that date (offered by some mandatory structures) is not modeled.
    MandatoryVariable {
        /// Date of mandatory conversion.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        conversion_date: Date,
        /// Upper conversion price (above this, holder receives min shares).
        upper_conversion_price: f64,
        /// Lower conversion price (below this, holder receives max shares).
        lower_conversion_price: f64,
    },
}

/// Events that may trigger conversion.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConversionEvent {
    /// Qualified Ipo variant.
    QualifiedIpo,
    /// Change Of Control variant.
    ChangeOfControl,
    /// Forced conversion if share price meets threshold for a lookback period.
    PriceTrigger {
        /// Threshold.
        threshold: f64,
        /// Lookback days.
        lookback_days: u32,
    },
}

/// Anti-dilution protection applied to conversion terms.
///
/// When dilutive events occur (stock splits, below-market issuances, special
/// dividends), the conversion ratio is adjusted to protect bondholders from
/// value erosion.
///
/// # Industry Practice
///
/// Most convertible bonds use **Weighted Average** anti-dilution, which is
/// less protective but more issuer-friendly. **Full Ratchet** is mainly seen
/// in private placements and venture-style convertibles.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AntiDilutionPolicy {
    /// No anti-dilution protection.
    None,
    /// Full ratchet: conversion price is reduced to the new issue price
    /// regardless of how many shares were issued. Most protective for holders.
    ///
    /// Formula: `new_conversion_price = min(current_conversion_price, new_issue_price)`
    FullRatchet,
    /// Broad-based weighted average: conversion price is adjusted based on the
    /// weighted average of the old and new share prices, factoring in the number
    /// of shares. Less dilutive to existing shareholders than full ratchet.
    ///
    /// Formula:
    /// ```text
    /// new_cp = old_cp × (shares_outstanding + new_money / old_cp)
    ///                  / (shares_outstanding + new_shares_issued)
    /// ```
    WeightedAverage,
}

/// How dividends affect conversion terms (dividend protection).
///
/// Dividend protection compensates the holder for dividends paid on the
/// underlying, which otherwise leak value from the conversion option (the
/// stock drifts at `r - q` under the risk-neutral measure). The variants
/// carry no threshold parameter, so protection is **full**: the entire
/// continuous dividend yield `q` is protected.
///
/// # Pricing model (continuous-yield accretion)
///
/// Under the continuous dividend yield `q` used by the tree pricer, each
/// protected dividend payment bumps the conversion ratio by the dividend
/// fraction; the continuous limit is exponential accretion from the
/// valuation date:
///
/// ```text
/// AdjustRatio:  ratio(t)  = ratio_0 * exp(q * t)
/// AdjustPrice:  K_conv(t) = K_0 * exp(-q * t)
/// ```
///
/// Since `ratio = notional / K_conv`, the price decay is mathematically the
/// same ratio accretion — the pricer implements both variants through the
/// single ratio-accretion mechanism. Accretion composes multiplicatively on
/// top of any anti-dilution adjustments from
/// [`ConvertibleBond::effective_conversion_ratio`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::convertible::{
///     AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
/// };
///
/// let conversion = ConversionSpec {
///     ratio: Some(25.0),
///     price: None,
///     policy: ConversionPolicy::Voluntary,
///     anti_dilution: AntiDilutionPolicy::None,
///     dividend_adjustment: DividendAdjustment::AdjustRatio,
///     dilution_events: Vec::new(),
/// };
/// assert!(conversion.dividend_adjustment.is_protected());
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DividendAdjustment {
    /// No dividend adjustment.
    None,
    /// Adjust conversion price downward by the dividend amount.
    AdjustPrice,
    /// Adjust conversion ratio upward to compensate for dividends.
    AdjustRatio,
}

impl DividendAdjustment {
    /// Whether this adjustment provides dividend protection.
    ///
    /// `AdjustPrice` and `AdjustRatio` are economically equivalent under the
    /// continuous-yield model (see the enum docs) and both return `true`.
    pub fn is_protected(&self) -> bool {
        !matches!(self, DividendAdjustment::None)
    }
}

/// A dilutive event that triggers anti-dilution adjustment.
///
/// Records details of an equity issuance or corporate action that may
/// affect the conversion ratio under the bond's anti-dilution provisions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DilutionEvent {
    /// Date of the dilutive event.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// New issue price per share (for below-market issuances).
    pub new_issue_price: f64,
    /// Number of new shares issued.
    pub new_shares_issued: f64,
    /// Number of shares outstanding before the event.
    pub shares_outstanding_before: f64,
}

/// Conversion specification for the instrument.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ConversionSpec {
    /// Conversion ratio (shares per bond). If not provided, derive from price.
    pub ratio: Option<f64>,
    /// Conversion price (price per share). If not provided, derive from ratio.
    pub price: Option<f64>,
    /// Policy governing conversion timing/conditions.
    pub policy: ConversionPolicy,
    /// Anti-dilution protection policy.
    pub anti_dilution: AntiDilutionPolicy,
    /// Dividend adjustment mechanism.
    pub dividend_adjustment: DividendAdjustment,
    /// Historical dilution events that affect the conversion ratio, recorded
    /// in chronological (non-decreasing date) order and applied in that
    /// order. Validation rejects out-of-order events.
    #[serde(default)]
    pub dilution_events: Vec<DilutionEvent>,
}

impl ConvertibleBond {
    fn validate(&self) -> finstack_quant_core::Result<()> {
        use crate::instruments::common_impl::validation;

        validation::validate_date_range_strict(self.issue_date, self.maturity, "convertible bond")?;
        validation::validate_money_finite(self.notional, "convertible bond notional")?;
        validation::validate_money_gt(self.notional, 0.0, "convertible bond notional")?;

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
            ConversionPolicy::UponEvent(ConversionEvent::PriceTrigger {
                threshold,
                lookback_days,
            }) => {
                validation::validate_f64_positive(
                    *threshold,
                    "convertible price-trigger threshold",
                )?;
                if *lookback_days == 0 {
                    return Err(finstack_quant_core::Error::Validation(
                        "convertible price-trigger lookback_days must be greater than zero"
                            .to_string(),
                    ));
                }
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

        let underlying = self
            .underlying_equity_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "convertible bond requires a non-empty underlying_equity_id".to_string(),
                )
            })?;
        let _ = underlying;

        if self.fixed_coupon.is_some() && self.floating_coupon.is_some() {
            return Err(finstack_quant_core::Error::Validation(
                "convertible bond cannot have simultaneous fixed and floating coupon schedules"
                    .to_string(),
            ));
        }
        if let Some(fixed_coupon) = &self.fixed_coupon {
            if fixed_coupon.rate.is_sign_negative() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "convertible bond fixed coupon rate must be non-negative, got {}",
                    fixed_coupon.rate
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
            trigger.validate()?;
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
        use crate::cashflow::builder::specs::FixedCouponSpec;
        use crate::cashflow::builder::CouponType;
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
            .underlying_equity_id_opt(Some("TECH".to_string()))
            .call_put_opt(None)
            .fixed_coupon_opt(Some(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: coupon_rate,
                schedule: finstack_quant_cashflows::builder::ScheduleParams {
                    frequency: Tenor::semi_annual(),

                    day_count: DayCount::Thirty360,

                    business_day_convention: BusinessDayConvention::Following,

                    calendar_id: "weekends_only".to_string(),

                    stub: StubKind::None,

                    end_of_month: false,

                    payment_lag_days: 0,

                    adjust_accrual_dates: false,
                    roll_rule: crate::cashflow::builder::specs::RollRule::None,
                },
            }))
            .floating_coupon_opt(None)
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
        use crate::cashflow::builder::specs::FixedCouponSpec;
        use crate::cashflow::builder::CouponType;
        use crate::instruments::fixed_income::bond::CallPut;
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
            .underlying_equity_id_opt(Some("INDU".to_string()))
            .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides {
                market_quotes: crate::instruments::MarketQuoteOverrides {
                    implied_volatility: Some(0.01),
                    ..Default::default()
                },
                ..Default::default()
            })
            .call_put_opt(Some(CallPutSchedule {
                calls: vec![CallPut {
                    start_date: date!(2026 - 03 - 15),
                    end_date: maturity,
                    price_pct_of_par: 101.0,
                    make_whole: None,
                }],
                puts: vec![CallPut {
                    start_date: date!(2025 - 03 - 15),
                    end_date: date!(2025 - 03 - 15),
                    price_pct_of_par: 100.0,
                    make_whole: None,
                }],
            }))
            .soft_call_trigger_opt(Some(SoftCallTrigger {
                threshold_pct: 130.0,
                observation_days: 30,
                required_days_above: 20,
            }))
            .recovery_rate_opt(Some(0.35))
            .fixed_coupon_opt(Some(FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: coupon_rate,
                schedule: finstack_quant_cashflows::builder::ScheduleParams {
                    frequency: Tenor::semi_annual(),

                    day_count: DayCount::Thirty360,

                    business_day_convention: BusinessDayConvention::Following,

                    calendar_id: "weekends_only".to_string(),

                    stub: StubKind::None,

                    end_of_month: false,

                    payment_lag_days: 0,

                    adjust_accrual_dates: false,
                    roll_rule: crate::cashflow::builder::specs::RollRule::None,
                },
            }))
            .floating_coupon_opt(None)
            .attributes(Attributes::new())
            .build()
    }

    /// Calculate policy-specific equity conversion value divided by notional.
    ///
    /// # Arguments
    ///
    /// * `curves` - Market context containing `underlying_equity_id` as the
    ///   current share price in the bond's currency. Mandatory-variable terms
    ///   determine share delivery from the lower and upper conversion prices.
    pub fn parity(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::traits::Instrument::validate_for_pricing(self)?;
        let underlying_id = self.underlying_equity_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::internal("convertible parity requires underlying_equity_id")
        })?;

        let spot_price = curves.get_price(underlying_id)?;
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
    /// * `curves` - Market context containing the current underlying share price
    ///   in the bond's currency; missing data and invalid conversion terms fail.
    /// * `bond_price` - Finite nonnegative total bond value in notional-currency
    ///   units, not percent of par. Returns `bond_price / conversion_value - 1`;
    ///   a nonpositive or non-finite conversion value fails.
    pub fn conversion_premium(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        bond_price: f64,
    ) -> finstack_quant_core::Result<f64> {
        crate::instruments::common_impl::traits::Instrument::validate_for_pricing(self)?;
        let underlying_id = self.underlying_equity_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::internal(
                "convertible conversion premium requires underlying_equity_id",
            )
        })?;

        let spot_price = curves.get_price(underlying_id)?;
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

    /// Calculate Greeks by repricing the selected convertible lattice.
    ///
    /// # Arguments
    ///
    /// * `curves` - Discount and risky discount curves, equity price and
    ///   volatility, plus forward curves and realized fixings for floating
    ///   coupons. An instrument volatility override takes precedence; surface
    ///   lookup otherwise uses the contractual conversion strike.
    /// * `tree_type` - Optional binomial/trinomial grid and step count; `None`
    ///   uses the canonical 200-step binomial tree for every repricing.
    /// * `bump_size` - Optional finite positive spot-relative bump for delta and
    ///   gamma; `None` uses 1%. Vega is per volatility point and rho per bp.
    /// * `as_of` - Valuation date and origin of the ACT/365F equity model clock.
    pub fn greeks(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        tree_type: Option<pricing::ConvertibleTreeType>,
        bump_size: Option<f64>,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<ConvertibleGreeks> {
        let greeks = pricing::calculate_convertible_greeks(
            self,
            curves,
            tree_type.unwrap_or_default(),
            bump_size,
            as_of,
        )?;
        Ok(greeks.into())
    }

    /// Calculate delta of this convertible bond
    pub fn delta(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let greeks = self.greeks(curves, None, None, as_of)?;
        Ok(greeks.delta)
    }

    /// Calculate gamma of this convertible bond
    pub fn gamma(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let greeks = self.greeks(curves, None, None, as_of)?;
        Ok(greeks.gamma)
    }

    /// Calculate vega of this convertible bond
    pub fn vega(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let greeks = self.greeks(curves, None, None, as_of)?;
        Ok(greeks.vega)
    }

    /// Calculate rho of this convertible bond
    pub fn rho(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let greeks = self.greeks(curves, None, None, as_of)?;
        Ok(greeks.rho)
    }

    /// Calculate theta of this convertible bond
    pub fn theta(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        let greeks = self.greeks(curves, None, None, as_of)?;
        Ok(greeks.theta)
    }
}

impl crate::instruments::common_impl::traits::Instrument for ConvertibleBond {
    impl_instrument_base!(crate::pricer::InstrumentType::Convertible);

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
        use finstack_quant_core::types::PriceId;

        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(credit_curve_id) = &self.credit_curve_id {
            deps.add_discount_curve(credit_curve_id.clone());
            deps.add_credit_curve(credit_curve_id.clone());
        }
        if let Some(floating_coupon) = &self.floating_coupon {
            deps.add_forward_curve(floating_coupon.rate_spec.index_id.clone());
            deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
                floating_coupon.rate_spec.index_id.as_str(),
            ));
        }
        if let Some(call_put) = &self.call_put {
            for option in call_put.calls.iter().chain(&call_put.puts) {
                if let Some(make_whole) = &option.make_whole {
                    deps.add_discount_curve(make_whole.reference_curve_id.clone());
                }
            }
        }
        if let Some(underlying_id) = &self.underlying_equity_id {
            deps.add_market_scalar_id(underlying_id.as_str());
            let price_id = PriceId::new(underlying_id);
            let reference_strike = self
                .effective_conversion_ratio()
                .filter(|ratio| *ratio > 0.0)
                .map(|ratio| self.notional.amount() / ratio);
            for dividend_yield_id in super::market_inputs::dividend_yield_candidate_ids(self)? {
                deps.add_market_scalar_id(dividend_yield_id);
            }
            for vol_surface_id in super::market_inputs::volatility_candidate_ids(self)? {
                // Convertible volatility may be supplied either as a unitless
                // MarketScalar or as a full surface under the same candidate ID.
                deps.add_market_scalar_id(vol_surface_id.clone());
                deps.add_volatility_dependency(VolatilityDependency::new(
                    vol_surface_id,
                    Some(price_id.clone()),
                    reference_strike,
                ));
            }
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        if let Some(ref trigger) = self.soft_call_trigger {
            trigger.validate()?;
        }
        pricing::price_convertible_bond(
            self,
            curves,
            pricing::ConvertibleTreeType::default(),
            as_of,
        )
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.issue_date)
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

    crate::impl_focused_pricing_overrides!();
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
        _as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let schedule = pricing::build_convertible_schedule(self, curves)?;
        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Contractual))
    }
}

// Declare canonical market dependencies for the DV01 calculator.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use crate::instruments::common_impl::traits::Instrument;

    #[test]
    fn market_dependencies_cover_convertible_equity_input_fallbacks() {
        let mut bond = ConvertibleBond::example().expect("example");
        bond.attributes
            .meta
            .insert("vol_surface_id".to_string(), "TECH-CUSTOM-VOL".to_string());

        let dividend_ids = super::super::market_inputs::dividend_yield_candidate_ids(&bond)
            .expect("dividend candidates");
        let volatility_ids = super::super::market_inputs::volatility_candidate_ids(&bond)
            .expect("volatility candidates");
        let deps =
            crate::instruments::Instrument::market_dependencies(&bond).expect("dependencies");

        assert!(deps
            .market_scalar_ids
            .contains(bond.underlying_equity_id.as_ref().expect("underlying")));
        assert!(dividend_ids
            .iter()
            .all(|id| deps.market_scalar_ids.contains(id)));
        assert!(volatility_ids
            .iter()
            .all(|id| deps.market_scalar_ids.contains(id)));
        assert_eq!(
            deps.volatility_dependencies
                .iter()
                .map(|dependency| dependency.vol_surface_id.as_str())
                .collect::<Vec<_>>(),
            volatility_ids
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn floating_convertible_uses_the_canonical_fixing_series_id() {
        let mut bond = ConvertibleBond::example().expect("example");
        let floating_bond = crate::instruments::fixed_income::bond::Bond::example_floating()
            .expect("floating bond example");
        let crate::instruments::fixed_income::bond::CashflowSpec::Floating(floating_coupon) =
            floating_bond.cashflow_spec
        else {
            unreachable!("floating example must have a floating coupon")
        };
        let expected = finstack_quant_core::market_data::fixings::fixing_series_id(
            floating_coupon.rate_spec.index_id.as_str(),
        );
        bond.fixed_coupon = None;
        bond.floating_coupon = Some(floating_coupon);

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
        bond.soft_call_trigger = Some(SoftCallTrigger {
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
    fn validation_rejects_double_coupon_and_inconsistent_conversion_quotes() {
        let mut bond = ConvertibleBond::example().expect("example");
        bond.floating_coupon = Some(
            match crate::instruments::fixed_income::bond::Bond::example_floating()
                .expect("floating example")
                .cashflow_spec
            {
                crate::instruments::fixed_income::bond::CashflowSpec::Floating(spec) => spec,
                _ => unreachable!("floating example"),
            },
        );
        assert!(bond
            .validate_for_pricing()
            .expect_err("simultaneous coupon schedules must fail")
            .to_string()
            .contains("simultaneous"));

        bond.floating_coupon = None;
        bond.conversion.price = Some(100.0);
        assert!(bond
            .validate_for_pricing()
            .expect_err("inconsistent ratio and price must fail")
            .to_string()
            .contains("inconsistent"));
    }
}
