//! Conversion vocabulary of convertible bonds: conversion policy and events,
//! share-price triggers, anti-dilution and dividend protection, and [`ConversionSpec`].

use finstack_quant_core::dates::Date;

/// Share-price trigger of a convertible bond: the last sale price must be at
/// least `threshold_pct` of the conversion price on `required_days_above` of
/// `observation_days` consecutive trading days.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, remote = "Self")]
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

impl serde::Serialize for PriceTrigger {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Self::serialize(self, serializer)
    }
}

impl<'de> serde::Deserialize<'de> for PriceTrigger {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Self::deserialize(deserializer)?;
        value
            .validate("price_trigger")
            .map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}

/// Defines how and when conversion can occur.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ConversionPolicy {
    /// Holder may convert at any time (subject to window, if any).
    Voluntary,
    /// Bond will mandatorily convert on the specified date.
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AntiDilutionPolicy {
    /// No anti-dilution protection.
    None,
    /// Full ratchet: conversion price is reduced to the new issue price
    /// regardless of how many shares were issued.
    FullRatchet,
    /// Broad-based weighted average: conversion price is adjusted based on the
    /// weighted average of the old and new share prices, factoring in shares.
    WeightedAverage,
}

/// How dividends affect conversion terms (dividend protection).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations_fixed_income::{
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
    pub fn is_protected(&self) -> bool {
        !matches!(self, DividendAdjustment::None)
    }
}

/// A dilutive event that triggers anti-dilution adjustment.
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
#[serde(deny_unknown_fields, remote = "Self")]
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
    /// Historical dilution events recorded in chronological order.
    #[serde(default)]
    pub dilution_events: Vec<DilutionEvent>,
}

impl ConversionSpec {
    /// Validate invariants of a conversion specification.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        use finstack_quant_core::validation;

        if self.ratio.is_none() && self.price.is_none() {
            return Err(finstack_quant_core::Error::Validation(
                "conversion specification requires conversion.ratio or conversion.price"
                    .to_string(),
            ));
        }
        if let Some(ratio) = self.ratio {
            validation::validate_f64_positive(ratio, "conversion ratio")?;
        }
        if let Some(price) = self.price {
            validation::validate_f64_positive(price, "conversion price")?;
        }
        match &self.policy {
            ConversionPolicy::Window { start, end } => {
                validation::validate_date_range_non_strict(
                    *start,
                    *end,
                    "convertible conversion window",
                )?;
            }
            ConversionPolicy::UponEvent(ConversionEvent::PriceTrigger(trigger)) => {
                trigger.validate("conversion.policy.upon_event.price_trigger")?;
            }
            ConversionPolicy::MandatoryVariable {
                upper_conversion_price,
                lower_conversion_price,
                ..
            } => {
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
                        "mandatory-variable bounds inverted: lower ({lower_conversion_price}) \
                         must not exceed upper ({upper_conversion_price})"
                    )));
                }
            }
            ConversionPolicy::Voluntary
            | ConversionPolicy::MandatoryOn(_)
            | ConversionPolicy::UponEvent(_) => {}
        }
        if self
            .dilution_events
            .windows(2)
            .any(|w| w[1].date < w[0].date)
        {
            return Err(finstack_quant_core::Error::Validation(
                "conversion dilution events must be chronologically ordered".to_string(),
            ));
        }
        for event in &self.dilution_events {
            validation::validate_f64_positive(event.new_issue_price, "dilution new_issue_price")?;
            validation::validate_f64_positive(
                event.new_shares_issued,
                "dilution new_shares_issued",
            )?;
            validation::validate_f64_positive(
                event.shares_outstanding_before,
                "dilution shares_outstanding_before",
            )?;
        }
        Ok(())
    }
}

impl serde::Serialize for ConversionSpec {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Self::serialize(self, serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ConversionSpec {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Self::deserialize(deserializer)?;
        value.validate().map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // schema-rejection-test
    fn conversion_policy_rejects_unknown_fields() {
        assert!(
            serde_json::from_str::<ConversionPolicy>(
                r#"{"window":{"start":"2025-01-01","end":"2026-01-01","extra":1}}"#
            )
            .is_err()
        );
    }
}
