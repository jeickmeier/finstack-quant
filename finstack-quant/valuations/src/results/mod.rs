//! Valuation result types, output formatting, and display field-unit tables.

pub use finstack_quant_valuations_core::results::*;
pub use finstack_quant_valuations_fixed_income::bond::pricing::quote_conversions::bond_quote_set_field_units;
pub use finstack_quant_valuations_structured_credit::field_units::{
    EquityMetricsFieldUnits, PoolReportFieldUnits, ScenarioTableFieldUnits, SimulationFieldUnits,
    TrancheSolveFieldUnits, equity_metrics_field_units, pool_report_field_units,
    scenario_table_field_units, simulation_field_units, tranche_metric_field_units,
    tranche_solve_field_units,
};
