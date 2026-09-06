//! Unit classification for standard metrics.
//!
//! `ValuationResult::measures` is heterogeneous by design (`ytm` is a decimal
//! rate, `par_spread` is basis points, `dv01` is currency per basis point).
//! [`MetricUnit`] labels each standard metric so host consumers can attach a
//! unit column or convert between representations without re-reading the
//! per-metric documentation.

use super::MetricId;
use serde::{Deserialize, Serialize};

/// Unit family of a metric value.
///
/// The classification follows the unit documented on each `MetricId`
/// constant. Currency-per-bump sensitivities (`dv01`, `cs01`, `vega`) are
/// reported as [`MetricUnit::Currency`]: the value is a currency amount and
/// the bump is part of the metric's definition, not of its unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MetricUnit {
    /// Amount in the result currency (PV components, currency-per-bump
    /// sensitivities, Greeks scaled by notional, jump-to-default).
    Currency,
    /// Decimal rate, probability, fraction or spread (`0.05` = 5%,
    /// `0.01` = 100bp).
    Decimal,
    /// Basis points (`164.7` = 164.7bp).
    BasisPoints,
    /// Years (durations, WAL/WAM, time to maturity, calendar-day counts
    /// expressed in years).
    Years,
    /// Percent or percentage-point value (`5.0` = 5%), including prices quoted
    /// as a percentage of par and speeds quoted as `% PSA`/`% SDA`.
    Percent,
    /// Pure number: ratios, multiples, counts, discount factors, flags, epoch
    /// days, index levels, and FX rates.
    Dimensionless,
    /// Custom metric or a standard metric whose unit depends on the producing
    /// calculator.
    Unknown,
}

impl MetricUnit {
    /// Serde/wire name of the unit (`"currency"`, `"basis_points"`, …).
    pub const fn as_str(&self) -> &'static str {
        match self {
            MetricUnit::Currency => "currency",
            MetricUnit::Decimal => "decimal",
            MetricUnit::BasisPoints => "basis_points",
            MetricUnit::Years => "years",
            MetricUnit::Percent => "percent",
            MetricUnit::Dimensionless => "dimensionless",
            MetricUnit::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for MetricUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl MetricId {
    /// Base metric of a composite key (`bucketed_dv01::USD-OIS::10y` →
    /// `bucketed_dv01`); a scalar key is returned unchanged.
    pub fn base(&self) -> MetricId {
        match self.as_str().split_once("::") {
            Some((base, _)) => MetricId(std::borrow::Cow::Owned(base.to_string())),
            None => self.clone(),
        }
    }

    /// Unit family of this metric's value.
    ///
    /// Composite keys inherit the unit of their base metric. Custom metrics
    /// and standard metrics whose unit is calculator-dependent (for example
    /// `ir_convexity`, `inflation_convexity`) return [`MetricUnit::Unknown`].
    pub fn unit(&self) -> MetricUnit {
        let base = self.base();
        if base.is_custom() {
            return MetricUnit::Unknown;
        }
        // Everything not listed explicitly is a PV component, a
        // currency-per-bump sensitivity or a notional-scaled Greek, all of
        // which are currency amounts.
        explicit_unit(base.as_str()).unwrap_or(MetricUnit::Currency)
    }
}

/// Explicit overrides for metrics whose name does not encode the unit.
fn explicit_unit(name: &str) -> Option<MetricUnit> {
    use MetricUnit::*;
    Some(match name {
        // Yields, rates, spreads and probabilities as decimals.
        "ytm"
        | "ytw"
        | "japanese_simple_yield"
        | "moosmuller_ytm"
        | "xirr"
        | "xirr_to_worst"
        | "z_spread"
        | "oas"
        | "i_spread"
        | "g_spread"
        | "asw_par"
        | "asw_market"
        | "discount_margin"
        | "par_rate"
        | "deposit_par_rate"
        | "quote_rate"
        | "implied_forward"
        | "convexity_adjustment"
        | "default_probability"
        | "cpr"
        | "smm"
        | "cdr"
        | "loss_severity"
        | "pool_factor"
        | "real_yield"
        | "breakeven_inflation"
        | "lp_irr"
        | "effective_rate"
        | "implied_financing_rate"
        | "implied_collateral_return"
        | "equity_dividend_yield"
        | "implied_vol"
        | "variance_strike_vol"
        | "recovery_01"
        | "clo_recovery_rate"
        | "abs_delinquency"
        | "abs_charge_off"
        | "abs_excess_spread"
        | "abs_ce_level"
        | "cmbs_ce_level"
        | "cmbs_waltv"
        | "rmbs_waltv"
        | "expense_ratio"
        | "tracking_error"
        | "utilization"
        | "premium_discount"
        | "clo_was"
        | "clo_wac"
        | "variance_expected"
        | "variance_realized" => Decimal,
        // Basis points.
        "par_spread" | "basis_par_spread" | "incremental_par_spread" | "roll_specialness" => {
            BasisPoints
        }
        // Years.
        "duration_mac"
        | "duration_mod"
        | "real_duration"
        | "spread_duration"
        | "wal"
        | "wam"
        | "time_to_maturity"
        | "variance_time_to_maturity"
        | "yf" => Years,
        // Percent.
        "rmbs_psa_speed" | "rmbs_sda_speed" => Percent,
        // Pure numbers.
        "convexity"
        | "conversion_factor"
        | "moic"
        | "moic_to_worst"
        | "moic_lp"
        | "dpi_lp"
        | "tvpi_lp"
        | "df_start"
        | "df_end"
        | "df_end_from_quote"
        | "annuity"
        | "annuity_primary"
        | "annuity_reference"
        | "financing_annuity"
        | "risky_annuity"
        | "spot_rate"
        | "inverse_rate"
        | "index_ratio"
        | "constituent_count"
        | "equity_shares"
        | "fixed_leg_payment_count"
        | "floating_leg_payment_count"
        | "fixed_first_payment_date"
        | "fixed_last_payment_date"
        | "floating_first_payment_date"
        | "floating_last_payment_date"
        | "fixed_first_accrual_factor"
        | "floating_first_accrual_factor"
        | "expected_maturity"
        | "clo_warf"
        | "clo_diversity"
        | "clo_oc_ratio"
        | "clo_ic_ratio"
        | "cmbs_dscr"
        | "rmbs_wafico"
        | "collateral_coverage"
        | "variance_notional"
        | "theta_period_days"
        | "carry_decomposition_degenerate"
        | "index_delta" => Dimensionless,
        // Calculator-dependent second-order measures and the breakeven shift
        // (whose unit follows the configured target).
        "ir_convexity" | "inflation_convexity" | "breakeven" => Unknown,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desk_metrics_carry_their_documented_units() {
        assert_eq!(MetricId::Ytm.unit(), MetricUnit::Decimal);
        assert_eq!(MetricId::ParRate.unit(), MetricUnit::Decimal);
        assert_eq!(MetricId::ZSpread.unit(), MetricUnit::Decimal);
        assert_eq!(MetricId::ParSpread.unit(), MetricUnit::BasisPoints);
        assert_eq!(MetricId::Dv01.unit(), MetricUnit::Currency);
        assert_eq!(MetricId::Cs01.unit(), MetricUnit::Currency);
        assert_eq!(MetricId::Vega.unit(), MetricUnit::Currency);
        assert_eq!(MetricId::DurationMod.unit(), MetricUnit::Years);
        assert_eq!(MetricId::Convexity.unit(), MetricUnit::Dimensionless);
        assert_eq!(MetricId::CleanPrice.unit(), MetricUnit::Currency);
        assert_eq!(MetricId::Accrued.unit(), MetricUnit::Currency);
        assert_eq!(MetricId::RmbsPsaSpeed.unit(), MetricUnit::Percent);
        assert_eq!(MetricId::IrConvexity.unit(), MetricUnit::Unknown);
    }

    #[test]
    fn composite_keys_inherit_base_unit_and_custom_is_unknown() {
        let bucketed = MetricId::composite(&MetricId::BucketedDv01, &["USD-OIS", "10y"]);
        assert_eq!(bucketed.base(), MetricId::BucketedDv01);
        assert_eq!(bucketed.unit(), MetricUnit::Currency);
        let pv01 = MetricId::composite(&MetricId::Pv01, &["USD-OIS"]);
        assert_eq!(pv01.unit(), MetricUnit::Currency);
        assert_eq!(MetricId::custom("my_metric").unit(), MetricUnit::Unknown);
        assert_eq!(MetricId::Dv01.base(), MetricId::Dv01);
    }

    #[test]
    fn every_standard_metric_classifies_and_wire_names_round_trip() {
        for metric in MetricId::ALL_STANDARD {
            let unit = metric.unit();
            let json = serde_json::to_string(&unit).expect("serialize");
            assert_eq!(json.trim_matches('"'), unit.as_str());
            let back: MetricUnit = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, unit);
        }
    }
}
