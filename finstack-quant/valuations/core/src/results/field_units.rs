//! Display units of the scalar fields of structured result types.
//!
//! Several results (tranche metrics, OAS solves, scenario tables, pool
//! reports, bond quote sets, simulations, equity metrics) document their units
//! in prose and do not carry them on the wire. These tables give a host the
//! unit text for each field, in the order the struct declares them. Values are
//! never rescaled.

use crate::metrics::{MetricId, MetricUnit};
use finstack_quant_core::currency::Currency;

/// The display unit of one scalar field of a result struct.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MetricFieldUnit {
    /// Wire name of the field.
    pub field: String,
    /// Unit text as the field documents it, such as `USD per bp` or `years`.
    pub label: String,
    /// Canonical Rust field the unit is documented on.
    pub source: String,
}

/// Units of the scalar `TrancheMetrics` fields, in the order the struct declares them.
///
/// `TrancheMetrics` documents these units in prose and does not state them on
/// the wire, so a host reads them here. Values are never rescaled.
///
/// # Arguments
///
/// * `currency` - The `TrancheMetrics::currency` of the result being labelled;
///   it names the unit of `pv`, `accrued` and `cs01`.
///
/// # Returns
///
/// One record per scalar field. `tranche_id`, `currency` and the
/// registry-keyed `metrics` map have no field unit and are not listed; registry
/// metrics keep the units their metric metadata reports.
pub fn tranche_metric_field_units(currency: Currency) -> Vec<MetricFieldUnit> {
    const CURRENT_BALANCE: &str = "% of current balance";
    [
        ("pv", currency.to_string()),
        ("clean_price_pct", CURRENT_BALANCE.to_string()),
        ("dirty_price_pct", CURRENT_BALANCE.to_string()),
        ("accrued", currency.to_string()),
        ("factor", "current / original balance".to_string()),
        ("wal", "years".to_string()),
        ("ytm", "decimal".to_string()),
        ("z_spread_bp", "bp".to_string()),
        ("cs01", format!("{currency} per bp")),
        ("spread_duration", "years".to_string()),
        ("spread_convexity", "years²".to_string()),
        ("effective_duration", "years".to_string()),
        ("effective_convexity", "years²".to_string()),
        ("target_price_pct", CURRENT_BALANCE.to_string()),
        ("wal_to_call", "years".to_string()),
        ("z_spread_to_call_bp", "bp".to_string()),
        ("dm_to_call_bp", "bp".to_string()),
        ("dm_bp", "bp".to_string()),
    ]
    .into_iter()
    .map(|(field, label)| MetricFieldUnit {
        field: field.to_string(),
        label,
        source: format!("valuations::TrancheMetrics::{field}"),
    })
    .collect()
}

/// Units of the tranche solvers' results, which do not state them on the wire.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TrancheSolveFieldUnits {
    /// Unit of the value `tranche_breakeven_cdr` returns.
    pub breakeven_cdr: MetricFieldUnit,
    /// Unit of the value `tranche_discount_margin` returns.
    pub discount_margin: MetricFieldUnit,
    /// Unit of every `OasResult` field, in the order the struct declares them.
    pub oas: Vec<MetricFieldUnit>,
}

/// Units of the tranche break-even CDR, discount margin and OAS results.
///
/// Those solvers document their units in prose: the two scalars are decimals
/// (`0.0125` = 125 bp) and `OasResult` prices are percent of the tranche's current
/// balance. Values are never rescaled.
pub fn tranche_solve_field_units() -> TrancheSolveFieldUnits {
    const CURRENT_BALANCE: &str = "% of current balance";
    let scalar = |field: &str, source: &str| MetricFieldUnit {
        field: field.to_string(),
        label: "decimal".to_string(),
        source: format!("valuations::{source}"),
    };
    TrancheSolveFieldUnits {
        breakeven_cdr: scalar("breakeven_cdr", "tranche_breakeven_cdr"),
        discount_margin: scalar("discount_margin", "tranche_discount_margin"),
        oas: [
            ("oas", "decimal"),
            ("model_price", CURRENT_BALANCE),
            ("market_price", CURRENT_BALANCE),
            ("num_paths", "paths"),
            ("price_std_error", CURRENT_BALANCE),
        ]
        .into_iter()
        .map(|(field, label)| MetricFieldUnit {
            field: field.to_string(),
            label: label.to_string(),
            source: format!("valuations::OasResult::{field}"),
        })
        .collect(),
    }
}

/// Units of a `ScenarioTable`'s cells, which do not appear on the wire.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScenarioTableFieldUnits {
    /// Unit of each swept `ScenarioCell` input, in the order the struct declares them.
    pub axes: Vec<MetricFieldUnit>,
    /// Unit of each measure `ScenarioCell` reports, in the order the struct declares them.
    pub measures: Vec<MetricFieldUnit>,
}

/// Units of the `ScenarioCell` fields, the one place a host reads them from.
///
/// Rates are decimals, the price is percent of the tranche's current balance and
/// the writedown is in currency units. Values are never rescaled.
///
/// # Arguments
///
/// * `currency` - The `ScenarioTable::currency` of the table being labelled; it
///   names the unit of `writedown`.
///
/// # Returns
///
/// One record per numeric cell field, split into the swept axes and the reported
/// measures so a host can offer the measures without naming them.
pub fn scenario_table_field_units(currency: Currency) -> ScenarioTableFieldUnits {
    let unit = |&(field, label): &(&str, &str)| MetricFieldUnit {
        field: field.to_string(),
        label: label.to_string(),
        source: format!("valuations::ScenarioCell::{field}"),
    };
    let currency = currency.to_string();
    ScenarioTableFieldUnits {
        axes: [
            ("cpr", "annual decimal"),
            ("cdr", "annual decimal"),
            ("severity", "decimal"),
        ]
        .iter()
        .map(unit)
        .collect(),
        measures: [
            ("price", "clean, % of current balance"),
            ("wal", "years"),
            ("writedown", currency.as_str()),
        ]
        .iter()
        .map(unit)
        .collect(),
    }
}

/// Units of a `PoolReport`, which do not all appear on the wire.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PoolReportFieldUnits {
    /// Unit of `PoolReport::total_balance`.
    pub total_balance: MetricFieldUnit,
    /// Unit of `PoolReport::warf`.
    pub warf: MetricFieldUnit,
    /// Unit of every `PoolReport::stats` field, in the order `PoolStats`
    /// declares them.
    pub stats: Vec<MetricFieldUnit>,
    /// Unit of the numeric fields of each `by_obligor`, `by_industry` and
    /// `by_rating` row, in the order `PoolConcentrationEntry` declares them.
    pub concentration: Vec<MetricFieldUnit>,
}

/// Units of the `PoolReport` fields, the one place a host reads them from.
///
/// `PoolStats` mixes a decimal coupon, a basis-point spread and a percent-point
/// defaulted share. Values are never rescaled.
///
/// # Arguments
///
/// * `currency` - The `AssetPool::currency` of the report being labelled; it
///   names the unit of `total_balance`, `undrawn_commitment` and each row's
///   `balance`.
///
/// # Returns
///
/// One record per numeric field, grouped by the struct holding it. Each row's
/// string `key` has no unit and is not listed.
pub fn pool_report_field_units(currency: Currency) -> PoolReportFieldUnits {
    const POOL_SHARE: &str = "% of current pool balance";
    let unit = |owner: &str, field: &str, label: String| MetricFieldUnit {
        field: field.to_string(),
        label,
        source: format!("valuations::{owner}::{field}"),
    };
    let currency = currency.to_string();
    PoolReportFieldUnits {
        total_balance: unit("PoolReport", "total_balance", currency.clone()),
        warf: unit("PoolReport", "warf", "Moody's rating factor".to_string()),
        stats: [
            ("wac", "annual decimal, fixed-rate only"),
            ("weighted_avg_spread_bp", "bp"),
            ("weighted_avg_maturity", "years"),
            ("diversity_score", "score"),
            ("num_obligors", "named obligors"),
            ("num_industries", "named industries"),
            ("defaulted_balance_pct", POOL_SHARE),
            ("undrawn_commitment", currency.as_str()),
        ]
        .into_iter()
        .map(|(field, label)| unit("PoolStats", field, label.to_string()))
        .collect(),
        concentration: [
            ("balance", currency.as_str()),
            ("balance_pct", POOL_SHARE),
            ("asset_count", "assets"),
        ]
        .into_iter()
        .map(|(field, label)| unit("PoolConcentrationEntry", field, label.to_string()))
        .collect(),
    }
}

/// Units of the `BondQuoteSet` fields, in the order the struct declares them, the
/// one place a host reads them from.
///
/// The set mixes currency prices, a percent-of-par price and decimal yields and
/// spreads. Each field with a metric states that metric's [`MetricId::unit`], and
/// `clean_price_pct`, which has none, states its quote convention. Values are never
/// rescaled.
///
/// # Arguments
///
/// * `currency` - The currency of the bond whose set is being labelled; it names
///   the unit of `clean_price_currency` and `dirty_price_currency`.
///
/// # Returns
///
/// One record per field of `BondQuoteSet`.
pub fn bond_quote_set_field_units(currency: Currency) -> Vec<MetricFieldUnit> {
    let label = |metric: Option<MetricId>| match metric.map(|metric| metric.unit()) {
        None => "% of par".to_string(),
        Some(MetricUnit::Currency) => currency.to_string(),
        Some(unit) => unit.as_str().to_string(),
    };
    [
        ("clean_price_currency", Some(MetricId::CleanPrice)),
        ("clean_price_pct", None),
        ("dirty_price_currency", Some(MetricId::DirtyPrice)),
        ("ytm", Some(MetricId::Ytm)),
        ("ytw", Some(MetricId::Ytw)),
        ("z_spread", Some(MetricId::ZSpread)),
        ("discount_margin", Some(MetricId::DiscountMargin)),
        ("oas", Some(MetricId::Oas)),
        ("asw_par", Some(MetricId::ASWPar)),
        ("asw_market", Some(MetricId::ASWMarket)),
        ("i_spread", Some(MetricId::ISpread)),
        ("japanese_simple_yield", Some(MetricId::JapaneseSimpleYield)),
        ("moosmuller_ytm", Some(MetricId::MoosmullerYtm)),
    ]
    .into_iter()
    .map(|(field, metric)| MetricFieldUnit {
        field: field.to_string(),
        label: label(metric),
        source: format!("valuations::BondQuoteSet::{field}"),
    })
    .collect()
}

/// Units of the charted `SimulationDiagnostics` series, which do not appear on the wire.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SimulationFieldUnits {
    /// Unit of each numeric `CoverageTestDiagnostic` field, in the order the
    /// struct declares them.
    pub coverage_tests: Vec<MetricFieldUnit>,
    /// Unit of `WaterfallDistribution::diverted_cash` and of each diversion's amount.
    pub diverted_cash: MetricFieldUnit,
    /// Unit of `TrancheCreditEnhancement::credit_enhancement`.
    pub credit_enhancement: MetricFieldUnit,
    /// Unit of `PeriodDiagnostics::excess_spread`.
    pub excess_spread: MetricFieldUnit,
}

/// Units of the coverage, diversion, enhancement and excess-spread series of a
/// structured-credit simulation, the one place a host reads them from.
///
/// Coverage ratios, triggers and cushions are in times covered, credit enhancement
/// is a decimal share of the collateral and excess spread an annual decimal of the opening pool
/// balance. Values are never rescaled.
///
/// # Arguments
///
/// * `currency` - The deal currency of the simulation being labelled, as its
///   `Money` amounts state it; it names the unit of `diverted_cash`.
///
/// # Returns
///
/// One record per charted field, grouped by the struct holding it.
pub fn simulation_field_units(currency: Currency) -> SimulationFieldUnits {
    let unit = |owner: &str, field: &str, label: String| MetricFieldUnit {
        field: field.to_string(),
        label,
        source: format!("valuations::{owner}::{field}"),
    };
    SimulationFieldUnits {
        coverage_tests: ["ratio", "trigger_level", "cushion"]
            .into_iter()
            .map(|field| unit("CoverageTestDiagnostic", field, "times covered".to_string()))
            .collect(),
        diverted_cash: unit(
            "WaterfallDistribution",
            "diverted_cash",
            currency.to_string(),
        ),
        credit_enhancement: unit(
            "TrancheCreditEnhancement",
            "credit_enhancement",
            "decimal share of collateral".to_string(),
        ),
        excess_spread: unit(
            "PeriodDiagnostics",
            "excess_spread",
            "annual decimal of opening pool balance".to_string(),
        ),
    }
}

/// Units of an `EquityMetrics`, which do not all appear on the wire.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EquityMetricsFieldUnits {
    /// Unit of `EquityMetrics::invested`.
    pub invested: MetricFieldUnit,
    /// Unit of `EquityMetrics::irr`.
    pub irr: MetricFieldUnit,
    /// Unit of `EquityMetrics::moic`.
    pub moic: MetricFieldUnit,
    /// Unit of `EquityMetrics::nav_pct`.
    pub nav_pct: MetricFieldUnit,
    /// Unit of each `EquityMetrics::cash_on_cash` value.
    pub cash_on_cash: MetricFieldUnit,
    /// Unit of the `purchase_price_pct` argument of `calculate_equity_metrics`,
    /// and what omitting it means.
    pub purchase_price: MetricFieldUnit,
}

/// Units of the `EquityMetrics` fields, the one place a host reads them from.
///
/// The IRR is an annual decimal, the MOIC a multiple of the invested amount, the NAV
/// a percent of the equity's current balance and cash-on-cash a decimal share of the
/// invested amount. Values are never rescaled.
///
/// # Arguments
///
/// * `currency` - The `EquityMetrics::currency` of the result being labelled; it
///   names the unit of `invested`.
///
/// # Returns
///
/// One record per numeric field, then the purchase price argument the amounts
/// depend on. `tranche_id` and `currency` have no unit and are not listed.
pub fn equity_metrics_field_units(currency: Currency) -> EquityMetricsFieldUnits {
    let unit = |field: &str, label: String| MetricFieldUnit {
        field: field.to_string(),
        label,
        source: format!("valuations::EquityMetrics::{field}"),
    };
    EquityMetricsFieldUnits {
        invested: unit("invested", currency.to_string()),
        irr: unit("irr", "annual decimal, ACT/365F".to_string()),
        moic: unit("moic", "multiple of invested".to_string()),
        nav_pct: unit("nav_pct", "% of current balance".to_string()),
        cash_on_cash: unit("cash_on_cash", "decimal share of invested".to_string()),
        purchase_price: MetricFieldUnit {
            field: "purchase_price_pct".to_string(),
            label: "% of current balance; omitted means par".to_string(),
            source: "valuations::calculate_equity_metrics::purchase_price_pct".to_string(),
        },
    }
}
