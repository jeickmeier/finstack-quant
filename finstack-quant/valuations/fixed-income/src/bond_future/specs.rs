//! Contract specifications for exchange-traded bond futures.

use finstack_quant_core::dates::DayCount;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
enum RepoDayCountWire {
    #[serde(rename = "act_360")]
    Act360,
    #[serde(rename = "act_365f")]
    Act365F,
}

impl From<RepoDayCountWire> for DayCount {
    fn from(value: RepoDayCountWire) -> Self {
        match value {
            RepoDayCountWire::Act360 => Self::Act360,
            RepoDayCountWire::Act365F => Self::Act365F,
        }
    }
}

impl TryFrom<DayCount> for RepoDayCountWire {
    type Error = finstack_quant_core::Error;

    fn try_from(value: DayCount) -> std::result::Result<Self, Self::Error> {
        match value {
            DayCount::Act360 => Ok(Self::Act360),
            DayCount::Act365F => Ok(Self::Act365F),
            unsupported => Err(finstack_quant_core::Error::Validation(format!(
                "bond-future repo_day_count must be act_360 or act_365f, got {unsupported:?}"
            ))),
        }
    }
}

mod repo_day_count_wire {
    use super::{DayCount, RepoDayCountWire};
    use serde::{Deserialize, Serialize};

    pub(super) fn serialize<S>(
        value: &DayCount,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        RepoDayCountWire::try_from(*value)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> std::result::Result<DayCount, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RepoDayCountWire::deserialize(deserializer).map(Into::into)
    }
}

/// Contract specifications for bond futures.
///
/// Defines the notional bond parameters used for conversion factor
/// calculations and the implied-repo day count. Contract size lives on the
/// future's `terms`: `terms.multiplier` is the currency value of one full
/// price point, i.e. one hundredth of the per-contract face (1,000 for a
/// $100,000 UST 10Y contract).
///
/// Delivery timing is carried by the future's explicit `delivery_start` /
/// `terms.settlement_date`, so the spec holds no settlement lag or holiday
/// calendar.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations_fixed_income::bond_future::BondFutureSpecs;
///
/// let specs = BondFutureSpecs::default();
/// assert_eq!(specs.standard_coupon, 0.06);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BondFutureSpecs {
    /// Standard coupon rate for conversion factor calculation (e.g., 0.06 for 6%).
    pub standard_coupon: f64,
    /// Standard maturity in years for conversion factor calculation.
    pub standard_maturity_years: f64,
    /// Day-count convention for implied repo rate annualization (`act_360` or `act_365f`).
    #[serde(default = "default_repo_day_count", with = "repo_day_count_wire")]
    #[cfg_attr(feature = "json-schema", schemars(with = "RepoDayCountWire"))]
    pub repo_day_count: DayCount,
}

fn default_repo_day_count() -> DayCount {
    DayCount::Act360
}

/// Return the implied-repo annualization denominator for a supported bond-future repo day count.
///
/// # Arguments
///
/// * `day_count` - Supported repo day count (`DayCount::Act360` or `DayCount::Act365F`).
///
/// # Errors
///
/// Returns `Error::Validation` for any unsupported day-count convention.
pub fn repo_annualization_denominator(day_count: DayCount) -> finstack_quant_core::Result<f64> {
    match day_count {
        DayCount::Act360 => Ok(360.0),
        DayCount::Act365F => Ok(365.0),
        unsupported => Err(finstack_quant_core::Error::Validation(format!(
            "bond-future repo_day_count must be act_360 or act_365f, got {unsupported:?}"
        ))),
    }
}

impl Default for BondFutureSpecs {
    /// Default specifications for UST 10-year futures.
    fn default() -> Self {
        Self::ust_10y()
    }
}

impl BondFutureSpecs {
    /// CME 10-Year U.S. Treasury Note futures (6% standard coupon, 10Y maturity, ACT/360 repo).
    #[must_use]
    pub const fn ust_10y() -> Self {
        Self {
            standard_coupon: 0.06,
            standard_maturity_years: 10.0,
            repo_day_count: DayCount::Act360,
        }
    }

    /// CME 5-Year U.S. Treasury Note futures (6% standard coupon, 5Y maturity, ACT/360 repo).
    #[must_use]
    pub const fn ust_5y() -> Self {
        Self {
            standard_coupon: 0.06,
            standard_maturity_years: 5.0,
            repo_day_count: DayCount::Act360,
        }
    }

    /// CME 2-Year U.S. Treasury Note futures (6% standard coupon, 2Y maturity, ACT/360 repo).
    #[must_use]
    pub const fn ust_2y() -> Self {
        Self {
            standard_coupon: 0.06,
            standard_maturity_years: 2.0,
            repo_day_count: DayCount::Act360,
        }
    }

    /// Eurex Euro-Bund futures (6% standard coupon, 10Y maturity, ACT/360 repo).
    #[must_use]
    pub const fn bund() -> Self {
        Self {
            standard_coupon: 0.06,
            standard_maturity_years: 10.0,
            repo_day_count: DayCount::Act360,
        }
    }

    /// ICE Long Gilt futures (4% standard coupon, 10Y maturity, ACT/365F repo).
    #[must_use]
    pub const fn gilt() -> Self {
        Self {
            standard_coupon: 0.04,
            standard_maturity_years: 10.0,
            repo_day_count: DayCount::Act365F,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_bond_future_specs_match_exchange_conventions() {
        let ust_10y = BondFutureSpecs::ust_10y();
        assert_eq!(ust_10y.standard_coupon, 0.06);
        assert_eq!(ust_10y.standard_maturity_years, 10.0);
        assert_eq!(ust_10y.repo_day_count, DayCount::Act360);

        let gilt = BondFutureSpecs::gilt();
        assert_eq!(gilt.standard_coupon, 0.04);
        assert_eq!(gilt.standard_maturity_years, 10.0);
        assert_eq!(gilt.repo_day_count, DayCount::Act365F);
    }

    #[test]
    fn repo_day_count_wire_accepts_only_supported_canonical_values() {
        for value in ["act_360", "act_365f"] {
            let mut json = serde_json::to_value(BondFutureSpecs::default())
                .expect("serialize bond future specs");
            json["repo_day_count"] = serde_json::json!(value);
            assert!(
                serde_json::from_value::<BondFutureSpecs>(json).is_ok(),
                "{value} must be accepted"
            );
        }

        for value in ["act360", "act365", "30_360", "act_act"] {
            let mut json = serde_json::to_value(BondFutureSpecs::default())
                .expect("serialize bond future specs");
            json["repo_day_count"] = serde_json::json!(value);
            assert!(
                serde_json::from_value::<BondFutureSpecs>(json).is_err(),
                "{value} must be rejected"
            );
        }
    }
}
