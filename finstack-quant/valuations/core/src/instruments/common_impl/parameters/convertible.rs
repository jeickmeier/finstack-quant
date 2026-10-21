//! Conversion vocabulary of convertible bonds: conversion policy and events,
//! share-price triggers, anti-dilution and dividend protection, and the
//! [`ConversionSpec`] that bundles them.
//!
//! These are plain data types shared by the convertible instrument, its
//! pricers and the model-parameter snapshot; the pricing logic that
//! interprets them lives with the convertible bond.

use finstack_quant_core::dates::Date;

/// Share-price trigger of a convertible bond: the last sale price must be at
/// least `threshold_pct` of the conversion price on `required_days_above` of
/// `observation_days` consecutive trading days.
///
/// Used for the issuer's soft call (`ConvertibleBond::soft_call_trigger`) and
/// the holder's contingent conversion
/// (`ConversionPolicy::UponEvent(ConversionEvent::PriceTrigger(..))`).
///
/// # Industry Practice
///
/// The standard trigger is 130% of the conversion price on 20 of 30
/// consecutive trading days; some issues use 120% or 150%.
///
/// # Modeling scope
///
/// The tree evaluates the trigger on the instantaneous node spot. The soft
/// call applies a Broadie-Glasserman-Kou-style barrier shift scaled by
/// `required_days_above / observation_days`; contingent conversion compares
/// the node spot with the nominal level and does not model the observation
/// window.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PriceTrigger {
    /// Trigger level as a percent of the conversion price (`130.0` = 130%);
    /// must exceed 100.
    pub threshold_pct: f64,
    /// Number of trading days in the observation window (e.g., 30).
    pub observation_days: u32,
    /// Minimum number of days within the window on which the share price
    /// must be at or above the level (e.g., 20 of 30).
    pub required_days_above: u32,
}

impl Default for PriceTrigger {
    /// Standard market convention: 130% trigger, 20 of 30 days.
    fn default() -> Self {
        Self {
            threshold_pct: 130.0,
            observation_days: 30,
            required_days_above: 20,
        }
    }
}

impl PriceTrigger {
    /// Share-price level of the trigger: `conversion_price × threshold_pct / 100`.
    ///
    /// # Arguments
    ///
    /// * `conversion_price` - Conversion price per share in the bond currency
    ///   (face amount divided by the effective conversion ratio).
    pub fn level(&self, conversion_price: f64) -> f64 {
        conversion_price * (self.threshold_pct / 100.0)
    }

    /// Validate the trigger parameters.
    ///
    /// - `threshold_pct` must be finite and exceed 100% (otherwise the trigger
    ///   is trivially satisfied).
    /// - `observation_days` and `required_days_above` must be non-zero.
    /// - `required_days_above` cannot exceed `observation_days`.
    ///
    /// # Arguments
    ///
    /// * `field` - Wire path of the trigger quoted in error messages (e.g.
    ///   `soft_call_trigger`).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` naming the violated rule.
    pub fn validate(&self, field: &str) -> finstack_quant_core::Result<()> {
        if !self.threshold_pct.is_finite() || self.threshold_pct <= 100.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{field}.threshold_pct ({:.1}%) must be finite and exceed 100%",
                self.threshold_pct,
            )));
        }
        if self.observation_days == 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{field}.observation_days must be greater than zero"
            )));
        }
        if self.required_days_above == 0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{field}.required_days_above must be greater than zero"
            )));
        }
        if self.required_days_above > self.observation_days {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{field}.required_days_above ({}) cannot exceed observation_days ({})",
                self.required_days_above, self.observation_days,
            )));
        }
        Ok(())
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
    /// the mandatory date is not modeled. Forced conversion takes precedence
    /// over coincident call/put rights, and the scheduled coupon remains payable.
    MandatoryOn(
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        Date,
    ),
    /// Holder may convert within a window.
    Window {
        /// Start.
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        start: Date,
        /// End.
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
    /// Forced conversion takes precedence over coincident call/put rights,
    /// and the scheduled coupon remains payable.
    MandatoryVariable {
        /// Date of mandatory conversion.
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
    /// Contingent conversion once the share price meets the trigger level
    /// (a percent of the conversion price) over the observation window.
    PriceTrigger(PriceTrigger),
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
/// `ConvertibleBond::effective_conversion_ratio`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations_core::instruments::common_impl::parameters::convertible::{
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
