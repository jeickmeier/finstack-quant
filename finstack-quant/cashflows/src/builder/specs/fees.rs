//! Fee specification types for fixed and periodic fees.

use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;

/// Fee specification for fixed-fee and periodic-basis-point programs.
///
/// Sign policy: any non-zero fee amount is emitted. Negative fixed amounts and
/// negative `bp` quotes (rebates) flow through as negative fee cashflows for
/// both variants.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum FeeSpec {
    /// Fixed fee paid once on a specified date.
    Fixed {
        /// Payment date of the fixed fee.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        date: Date,
        /// Fee amount in currency units.
        amount: Money,
    },
    /// Periodic fee quoted in basis points and accrued over generated periods.
    PeriodicBp {
        /// Economic balance used as the fee base.
        base: FeeBase,
        /// Fee quote in basis points per annum, stored as `Decimal` to preserve
        /// the quoted value exactly.
        #[serde(with = "finstack_quant_core::wire::decimal")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DecimalWire")
        )]
        bp: Decimal,
        /// Accrual and payment frequency for the fee schedule.
        frequency: Tenor,
        /// Day-count convention used to annualize the fee accrual.
        day_count: DayCount,
        /// Business-day convention applied to generated fee dates.
        business_day_convention: BusinessDayConvention,
        /// Holiday calendar identifier used with `business_day_convention`.
        ///
        /// Use `"weekends_only"` when only weekend adjustment is required.
        calendar_id: String,
        /// Stub-handling rule for irregular first or last fee periods
        /// (`ShortFront` when omitted from the wire form).
        #[serde(default = "crate::serde_defaults::stub_short_front")]
        stub: StubKind,
        /// How the outstanding balance is sampled for fee calculation.
        #[serde(default, skip_serializing_if = "FeeAccrualBasis::is_default")]
        accrual_basis: FeeAccrualBasis,
    },
}

/// Controls how the outstanding balance is sampled during fee accrual.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FeeAccrualBasis {
    /// Use the outstanding balance at the period's accrual start, matching the
    /// coupon convention (same-date amortization/PIK on the payment date does
    /// not affect the fee base).
    #[default]
    PointInTime,
    /// Time-weighted average outstanding over the accrual period.
    TimeWeightedAverage,
}

impl FeeAccrualBasis {
    /// Returns true if this is the default variant (for serde skip_serializing_if).
    pub fn is_default(&self) -> bool {
        matches!(self, Self::PointInTime)
    }
}

/// Fee base for periodic bp fees.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum FeeBase {
    /// Fee base is the drawn outstanding after amortization and PIK updates.
    Drawn,
    /// Base on undrawn = max(limit - outstanding, 0).
    Undrawn {
        /// Total facility commitment used to compute the undrawn amount.
        facility_limit: Money,
    },
}

/// Fee tier for utilization-based fee structures.
///
/// Tiers are evaluated in order: the first tier where utilization >= threshold applies.
/// Tiers must be sorted by threshold (ascending); [`evaluate_fee_tiers`]
/// rejects unordered tiers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FeeTier {
    /// Utilization threshold (0.0 to 1.0). Fee applies when utilization >= this threshold.
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub threshold: Decimal,
    /// Fee rate in basis points for this tier.
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub bp: Decimal,
}

/// Evaluate fee tiers to find the applicable rate for a given utilization.
///
/// Returns the fee rate from the highest tier where utilization >= threshold.
/// If no tiers match or tiers are empty, returns 0.0.
///
/// # Arguments
///
/// * `tiers` - Slice of fee tiers, sorted by threshold strictly ascending
/// * `utilization` - Current utilization rate (0.0 to 1.0) as Decimal
///
/// # Returns
///
/// The fee rate in basis points for the applicable tier, or 0.0 if no tier matches
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when tier thresholds are not
/// strictly ascending (the tier-selection logic relies on this ordering).
pub fn evaluate_fee_tiers(
    tiers: &[FeeTier],
    utilization: Decimal,
) -> finstack_quant_core::Result<Decimal> {
    if let Some(w) = tiers.windows(2).find(|w| w[0].threshold >= w[1].threshold) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "evaluate_fee_tiers: tier thresholds must be strictly ascending, \
             found {} followed by {}",
            w[0].threshold, w[1].threshold
        )));
    }
    Ok(tiers
        .iter()
        .rev()
        .find(|tier| utilization >= tier.threshold)
        .map(|tier| tier.bp)
        .unwrap_or(Decimal::ZERO))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn fee_spec_rejects_unknown_fields() {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::Date;
        use time::Month;

        // Round-trip a valid spec, then inject a typo'd field: strict serde
        // must reject it instead of silently ignoring the typo.
        let spec = FeeSpec::PeriodicBp {
            base: FeeBase::Drawn,
            bp: dec!(50),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            accrual_basis: FeeAccrualBasis::TimeWeightedAverage,
        };
        let mut value = serde_json::to_value(&spec).expect("serializable spec");
        let inner = value
            .get_mut("periodic_bp")
            .expect("externally tagged variant")
            .as_object_mut()
            .expect("struct variant body");
        inner.insert(
            "acrual_basis".to_string(),
            serde_json::json!("point_in_time"),
        );
        let result: Result<FeeSpec, _> = serde_json::from_value(value);
        assert!(
            result.is_err(),
            "FeeSpec::PeriodicBp must reject typo'd field, got {result:?}"
        );

        let spec = FeeSpec::Fixed {
            date: Date::from_calendar_date(2025, Month::January, 15).expect("valid date"),
            amount: Money::from((100_i64, Currency::USD)),
        };
        let mut value = serde_json::to_value(&spec).expect("serializable spec");
        let inner = value
            .get_mut("fixed")
            .expect("externally tagged variant")
            .as_object_mut()
            .expect("struct variant body");
        inner.insert("ammount".to_string(), serde_json::json!(1.0));
        let result: Result<FeeSpec, _> = serde_json::from_value(value);
        assert!(
            result.is_err(),
            "FeeSpec::Fixed must reject typo'd field, got {result:?}"
        );
    }

    #[test]
    fn fee_tier_rejects_unknown_fields() {
        let tier = FeeTier {
            threshold: dec!(0.5),
            bp: dec!(25),
        };
        let mut value = serde_json::to_value(&tier).expect("serializable tier");
        value
            .as_object_mut()
            .expect("struct body")
            .insert("extra".to_string(), serde_json::json!(1));
        let result: Result<FeeTier, _> = serde_json::from_value(value);
        assert!(result.is_err(), "FeeTier must reject unknown fields");
    }

    #[test]
    fn evaluate_fee_tiers_rejects_unordered_thresholds() {
        let tiers = [
            FeeTier {
                threshold: dec!(0.5),
                bp: dec!(50),
            },
            FeeTier {
                threshold: dec!(0.25),
                bp: dec!(25),
            },
        ];
        let result = evaluate_fee_tiers(&tiers, dec!(0.6));
        assert!(result.is_err(), "descending thresholds must be rejected");

        let ordered = [
            FeeTier {
                threshold: dec!(0.25),
                bp: dec!(25),
            },
            FeeTier {
                threshold: dec!(0.5),
                bp: dec!(50),
            },
        ];
        assert_eq!(
            evaluate_fee_tiers(&ordered, dec!(0.6)).expect("ascending tiers"),
            dec!(50)
        );
        assert_eq!(
            evaluate_fee_tiers(&ordered, dec!(0.1)).expect("ascending tiers"),
            Decimal::ZERO
        );
    }
}
