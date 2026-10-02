//! Canonical example payloads for the portfolio component and result artifacts.
//!
//! Analytics outputs are computed by running the analytic on the neighbouring
//! input example, so each input/output pair stays consistent. Engine outputs
//! that need a calibrated market to produce are literal JSON, round-tripped
//! through the contract type so a payload the type rejects cannot be published;
//! they describe the one-deposit portfolio of the materialization example.

use finstack_quant_core::schema::{example, example_from_json};
use finstack_quant_core::Result;
use serde_json::{json, Value};

fn usd(amount: &str) -> Value {
    json!({"amount": amount, "currency": "USD"})
}

/// An empty application report, as produced by a scenario with one operation.
fn application_report() -> Value {
    json!({
        "operations_applied": 1,
        "user_operations": 1,
        "expanded_operations": 1,
        "changes": {
            "market_targets": [],
            "changed_instrument_indices": [],
            "as_of_changed": false,
            "portfolio_shape_changed": false,
            "all_dirty": true
        },
        "warnings": []
    })
}

/// The valuation of the example portfolio's single deposit position.
fn valuation() -> Value {
    json!({
        "as_of": "2024-01-01",
        "position_values": {
            "POS_001": {
                "position_id": "POS_001",
                "entity_id": "ACME_CORP",
                "value_native": usd("1000125.50"),
                "value_base": usd("1000125.50"),
                "metric_scale": 1.0,
                "risk_metrics_complete": true
            }
        },
        "total_base_currency": usd("1000125.50"),
        "by_entity": {"ACME_CORP": usd("1000125.50")},
        "fx_collapse_policy": "cashflow_date"
    })
}

/// A two-factor variance decomposition of a two-position book.
fn risk_decomposition(scale: f64) -> Value {
    json!({
        "total_risk": 0.0004 * scale,
        "measure": "variance",
        "factor_contributions": [
            {
                "factor_id": "USD-RATES",
                "absolute_risk": 0.00025 * scale,
                "relative_risk": 0.625,
                "marginal_risk": 0.0125 * scale
            },
            {
                "factor_id": "IG-CREDIT",
                "absolute_risk": 0.0001 * scale,
                "relative_risk": 0.25,
                "marginal_risk": 0.005 * scale
            }
        ],
        "residual_risk": 0.00005 * scale,
        "position_factor_contributions": [],
        "position_residual_contributions": []
    })
}

pub(super) fn materialization_report() -> Result<Vec<Value>> {
    example_from_json::<crate::materialization::MaterializationReport>(json!({
        "report": {"diagnostics": [], "truncated": false},
        "unique_instruments": 1,
        "positions": 1,
        "dependencies": 1,
        "cache_hits": 0,
        "input_bytes": 1024,
        "timing_available": false,
        "phase_nanos": {
            "parse": 0,
            "validate_versions": 0,
            "decode_instruments": 0,
            "build_positions": 0,
            "index_build": 0
        }
    }))
}

// ---------------------------------------------------------------------------
// Brinson-Fachler
// ---------------------------------------------------------------------------

fn sector_periods(shift: f64) -> Vec<crate::SectorPeriod> {
    vec![
        crate::SectorPeriod {
            sector: "Technology".to_string(),
            portfolio_weight: 0.60,
            benchmark_weight: 0.50,
            portfolio_return: 0.040 + shift,
            benchmark_return: 0.030 + shift,
        },
        crate::SectorPeriod {
            sector: "Utilities".to_string(),
            portfolio_weight: 0.40,
            benchmark_weight: 0.50,
            portfolio_return: 0.010,
            benchmark_return: 0.015,
        },
    ]
}

pub(super) fn sector_period() -> Result<Vec<Value>> {
    example(&sector_periods(0.0)[0])
}

/// Two periods of [`sector_period`]-style inputs, attributed and linked.
pub(super) fn carino_linked_attribution() -> Result<Vec<Value>> {
    let periods = [
        crate::brinson_fachler(&sector_periods(0.0))?,
        crate::brinson_fachler(&sector_periods(-0.02))?,
    ];
    example(&crate::carino_link(&periods)?)
}

// ---------------------------------------------------------------------------
// Duration-matched excess returns
// ---------------------------------------------------------------------------

fn cell_config_value() -> crate::CellConfig {
    crate::CellConfig { width: 1.0 }
}

fn reference_returns() -> Vec<crate::ReferenceReturn> {
    [(2.0, 0.004), (5.0, 0.009), (10.0, 0.015)]
        .into_iter()
        .map(|(duration, total_return)| crate::ReferenceReturn {
            duration,
            total_return,
        })
        .collect()
}

fn duration_cell_table_value() -> Result<crate::DurationCellTable> {
    Ok(crate::cell_returns_from_reference(
        &reference_returns(),
        "UST",
        &cell_config_value(),
    )?)
}

fn excess_return_positions() -> Vec<crate::ExcessReturnPosition> {
    vec![
        crate::ExcessReturnPosition {
            id: "ACME 4.25 2030".to_string(),
            weight: 0.6,
            duration: 4.6,
            total_return: 0.0112,
        },
        crate::ExcessReturnPosition {
            id: "GLOBEX 5.5 2034".to_string(),
            weight: 0.4,
            duration: 7.4,
            total_return: 0.0131,
        },
    ]
}

pub(super) fn cell_config() -> Result<Vec<Value>> {
    example(&cell_config_value())
}

pub(super) fn reference_return() -> Result<Vec<Value>> {
    example(&reference_returns()[1])
}

pub(super) fn duration_cell_table() -> Result<Vec<Value>> {
    example(&duration_cell_table_value()?)
}

pub(super) fn excess_return_position() -> Result<Vec<Value>> {
    example(&excess_return_positions()[0])
}

pub(super) fn excess_return_result() -> Result<Vec<Value>> {
    example(&crate::excess_returns(
        &excess_return_positions(),
        &duration_cell_table_value()?,
    )?)
}

// ---------------------------------------------------------------------------
// Fixed-income (Campisi) attribution
// ---------------------------------------------------------------------------

fn fi_snapshot(
    sector: &str,
    weight: f64,
    spread: f64,
    modified_duration: f64,
    delta_spread: f64,
) -> crate::FiPositionSnapshot {
    let yield_annual = 0.04 + spread;
    let spread_duration = modified_duration - 0.2;
    let delta_treasury_yield = -0.0010;
    crate::FiPositionSnapshot {
        sector: sector.to_string(),
        weight,
        // Carry plus the first-order treasury and spread price effects.
        total_return: yield_annual / 12.0
            - modified_duration * delta_treasury_yield
            - spread_duration * delta_spread,
        yield_annual,
        modified_duration,
        spread_duration,
        spread,
        delta_treasury_yield,
        delta_spread,
    }
}

/// A portfolio overweight Industrials and longer in duration than its benchmark.
fn fi_period_input_value(delta_spread: f64) -> crate::FiPeriodInput {
    crate::FiPeriodInput {
        portfolio: vec![
            fi_snapshot("Industrials", 0.6, 0.012, 5.6, delta_spread),
            fi_snapshot("Financials", 0.4, 0.015, 4.4, 0.0),
        ],
        benchmark: vec![
            fi_snapshot("Industrials", 0.5, 0.011, 5.0, delta_spread),
            fi_snapshot("Financials", 0.5, 0.014, 4.0, 0.0),
        ],
    }
}

fn fi_config_value() -> crate::FiAttributionConfig {
    crate::FiAttributionConfig {
        period_years: 1.0 / 12.0,
    }
}

pub(super) fn fi_attribution_config() -> Result<Vec<Value>> {
    example(&fi_config_value())
}

pub(super) fn fi_period_input() -> Result<Vec<Value>> {
    example(&fi_period_input_value(-0.0005))
}

pub(super) fn fi_carino_linked_result() -> Result<Vec<Value>> {
    example(&crate::campisi_carino_link_from_snapshots(
        &[
            fi_period_input_value(-0.0005),
            fi_period_input_value(0.0003),
        ],
        &fi_config_value(),
    )?)
}

pub(super) fn fi_reconciliation_report() -> Result<Vec<Value>> {
    let period = fi_period_input_value(-0.0005);
    let result =
        crate::campisi_attribution(&period.portfolio, &period.benchmark, &fi_config_value())?;
    example(&result.reconciliation_check(1e-9))
}

// ---------------------------------------------------------------------------
// Curve x sector grid attribution
// ---------------------------------------------------------------------------

fn grid_positions(tilt: f64, alpha: f64) -> Vec<crate::GridPosition> {
    let position = |cell: &str, sector: &str, weight: f64, total_return: f64| crate::GridPosition {
        cell: cell.to_string(),
        sector: sector.to_string(),
        weight,
        total_return,
    };
    vec![
        position("0-5y", "Industrials", 0.5 + tilt, 0.008 + alpha),
        position("5-10y", "Industrials", 0.2, 0.013),
        position("5-10y", "Financials", 0.3 - tilt, 0.015),
    ]
}

pub(super) fn grid_position() -> Result<Vec<Value>> {
    example(&grid_positions(0.0, 0.0)[0])
}

pub(super) fn grid_carino_linked_result() -> Result<Vec<Value>> {
    let period = crate::grid_attribution(&grid_positions(0.1, 0.001), &grid_positions(0.0, 0.0))?;
    example(&crate::grid_carino_link(&[period.clone(), period])?)
}

// ---------------------------------------------------------------------------
// Factor Brinson
// ---------------------------------------------------------------------------

fn factor_brinson_input_value() -> crate::FactorBrinsonInput {
    crate::FactorBrinsonInput {
        asset_ids: vec!["AAPL".to_string(), "XOM".to_string()],
        asset_returns: vec![0.030, 0.010],
        // Row-major assets x factors.
        exposures: vec![1.2, 0.0, 0.8, 1.0],
        factor_names: vec!["market".to_string(), "energy".to_string()],
        portfolio_weights: vec![0.7, 0.3],
        benchmark_weights: vec![0.5, 0.5],
    }
}

pub(super) fn factor_brinson_input() -> Result<Vec<Value>> {
    example(&factor_brinson_input_value())
}

/// The attribution of [`factor_brinson_input`].
///
/// The factor returns leave the benchmark no residual, which the Brinson
/// identification of allocation and selection requires.
pub(super) fn factor_brinson_result() -> Result<Vec<Value>> {
    example(&crate::factor_brinson_attribution(
        &factor_brinson_input_value(),
        &[0.0225, -0.005],
    )?)
}

// ---------------------------------------------------------------------------
// Performance measurement
// ---------------------------------------------------------------------------

pub(super) fn twrr_period() -> Result<Vec<Value>> {
    example_from_json::<crate::TwrrPeriod>(json!({
        "beginning_market_value": 1_000_000.0,
        "ending_market_value": 1_062_000.0,
        "cashflows": [{"amount": 50_000.0, "fraction_of_period_remaining": 0.5}]
    }))
}

/// Three annual returns linked over a three-year horizon.
pub(super) fn linked_return() -> Result<Vec<Value>> {
    example(&crate::twrr_linked(&[0.08, -0.03, 0.11], 3.0)?)
}

pub(super) fn dated_cashflow() -> Result<Vec<Value>> {
    example_from_json::<crate::DatedCashflow>(json!({
        "date": "2024-01-01",
        "amount": -1_000_000.0
    }))
}

// ---------------------------------------------------------------------------
// Strategy weight allocation
// ---------------------------------------------------------------------------

fn weight_allocation_spec_value() -> Result<crate::WeightAllocationSpec> {
    serde_json::from_value(json!({
        "scheme": "inverse_volatility",
        "total_capital": 10_000_000.0,
        "strategies": [
            {"id": "carry", "fixed_weight": null, "returns": [0.010, -0.004, 0.006, 0.002], "risk_budget": null},
            {"id": "trend", "fixed_weight": null, "returns": [0.020, -0.012, 0.015, -0.006], "risk_budget": null}
        ],
        "covariance": null
    }))
    .map_err(|error| {
        finstack_quant_core::Error::Internal(format!("parse weight allocation example: {error}"))
    })
}

pub(super) fn weight_allocation_spec() -> Result<Vec<Value>> {
    example(&weight_allocation_spec_value()?)
}

pub(super) fn weight_allocation_result() -> Result<Vec<Value>> {
    example(&crate::factor_model::allocate_weights(
        &weight_allocation_spec_value()?,
    )?)
}

// ---------------------------------------------------------------------------
// Valuation, cashflows, margin and scenarios of the example portfolio
// ---------------------------------------------------------------------------

pub(super) fn portfolio_result() -> Result<Vec<Value>> {
    let meta = serde_json::to_value(finstack_quant_core::config::results_meta(
        &finstack_quant_core::config::FinstackConfig::default(),
    ))
    .map_err(|error| {
        finstack_quant_core::Error::Internal(format!("serialize example meta: {error}"))
    })?;
    example_from_json::<crate::PortfolioResult>(json!({
        "schema_version": 1,
        "valuation": valuation(),
        "metrics": {
            "aggregated": {
                "dv01": {"metric_id": "dv01", "total": -8.5, "by_entity": {"ACME_CORP": -8.5}}
            },
            "by_position": {
                "POS_001": {"currency": "USD", "metrics": {"dv01": -8.5}}
            }
        },
        "meta": meta
    }))
}

pub(super) fn cashflow_aggregation_options() -> Result<Vec<Value>> {
    example(&crate::cashflows::CashflowAggregationOptions::default())
}

pub(super) fn portfolio_cashflows() -> Result<Vec<Value>> {
    let event = json!({
        "position_id": "POS_001",
        "instrument_id": "DEP_1M",
        "instrument_type": "deposit",
        "date": "2024-02-01",
        "amount": usd("1004305.56"),
        "kind": "notional",
        "reset_date": null,
        "accrual_factor": 0.0,
        "rate": null
    });
    example_from_json::<crate::cashflows::PortfolioCashflows>(json!({
        "events": [event],
        "by_position": {"POS_001": [event]},
        "by_date": {"2024-02-01": {"USD": {"notional": usd("1004305.56")}}},
        "position_summaries": {
            "POS_001": {
                "position_id": "POS_001",
                "instrument_id": "DEP_1M",
                "instrument_type": "deposit",
                "representation": "contractual",
                "event_count": 1
            }
        },
        "issues": [],
        "fx_collapse_policy": "cip_forward"
    }))
}

pub(super) fn portfolio_margin_result() -> Result<Vec<Value>> {
    example_from_json::<crate::PortfolioMarginResult>(json!({
        "as_of": "2024-01-01",
        "base_currency": "USD",
        "total_initial_margin": usd("2500000"),
        "by_csa": {},
        "total_required_im_collateral": usd("0"),
        "total_im_transfer": usd("0"),
        "total_segregated_im": usd("0"),
        "total_variation_margin": usd("1250000"),
        "total_margin": usd("3750000"),
        "netting_sets": [{
            "netting_set_id": {
                "kind": "bilateral",
                "counterparty_id": "ACME-BANK",
                "csa_id": "CSA-ACME-2024"
            },
            "csa_id": "CSA-ACME-2024",
            "as_of": "2024-01-01",
            "initial_margin": usd("2500000"),
            "variation_margin": usd("1250000"),
            "total_margin": usd("3750000"),
            "position_count": 1,
            "im_methodology": "simm",
            "is_approximate": false,
            "im_breakdown": {}
        }],
        "total_positions": 1,
        "positions_without_margin": 0,
        "degraded_positions": []
    }))
}

/// Maximize value-weighted PV over the example portfolio, fully invested.
pub(super) fn portfolio_optimization_spec() -> Result<Vec<Value>> {
    let portfolio =
        serde_json::to_value(super::example_portfolio()?.to_spec()).map_err(|error| {
            finstack_quant_core::Error::Internal(format!("serialize example portfolio: {error}"))
        })?;
    example_from_json::<crate::optimization::PortfolioOptimizationSpec>(json!({
        "portfolio": portfolio,
        "objective": {"maximize": {"weighted_sum": {"metric": "pv_base"}}},
        "constraints": [{"budget": {"rhs": 1.0}}],
        "weighting": "value_weight"
    }))
}

pub(super) fn portfolio_primitive_exposure_report() -> Result<Vec<Value>> {
    example_from_json::<crate::primitive::PortfolioPrimitiveExposureReport>(json!({
        "base_currency": "USD",
        "paths": [{
            "position_id": "POS_001",
            "path": ["DEP_1M"],
            "instrument_id": "DEP_1M",
            "instrument_type": "deposit",
            "quantity": 1.0,
            "value": usd("1000125.50"),
            "measures": {"dv01": -8.5}
        }],
        "aggregates": [{
            "instrument_id": "DEP_1M",
            "instrument_type": "deposit",
            "net_quantity": 1.0,
            "gross_quantity": 1.0,
            "net_value": usd("1000125.50"),
            "gross_value": usd("1000125.50"),
            "net_measures": {"dv01": -8.5},
            "gross_measures": {"dv01": 8.5}
        }]
    }))
}

pub(super) fn market_factor_key() -> Result<Vec<Value>> {
    example(&crate::MarketFactorKey::Fx {
        base: finstack_quant_core::currency::Currency::EUR,
        quote: finstack_quant_core::currency::Currency::USD,
    })
}

fn scenario_pnl() -> Value {
    json!({"total": usd("-425.25"), "by_position": {"POS_001": usd("-425.25")}})
}

pub(super) fn scenario_pnl_view() -> Result<Vec<Value>> {
    example_from_json::<crate::scenarios::ScenarioPnlView>(json!({
        "pnl": scenario_pnl(),
        "report": application_report()
    }))
}

pub(super) fn scenario_pnl_batch_item() -> Result<Vec<Value>> {
    example_from_json::<crate::scenarios::ScenarioPnlBatchItem>(json!({
        "scenario_id": "usd_rates_stress",
        "pnl": scenario_pnl(),
        "report": application_report()
    }))
}

pub(super) fn scenario_revalue_view() -> Result<Vec<Value>> {
    example_from_json::<crate::scenarios::ScenarioRevalueView>(json!({
        "valuation": valuation(),
        "report": application_report()
    }))
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

pub(super) fn replay_config() -> Result<Vec<Value>> {
    example_from_json::<crate::replay::ReplayConfig>(json!({"mode": "pv_and_pnl"}))
}

/// A one-step replay: the first date carries no P&L.
pub(super) fn replay_result() -> Result<Vec<Value>> {
    example_from_json::<crate::replay::ReplayResult>(json!({
        "steps": [{
            "date": "2024-01-01",
            "valuation": valuation(),
            "daily_mtm_pnl": null,
            "cumulative_mtm_pnl": null,
            "attribution": null
        }],
        "summary": {
            "start_date": "2024-01-01",
            "end_date": "2024-01-01",
            "num_steps": 1,
            "start_value": usd("1000125.50"),
            "end_value": usd("1000125.50"),
            "total_mtm_pnl": usd("0"),
            "max_mtm_drawdown": usd("0"),
            "max_mtm_drawdown_pct": 0.0,
            "max_mtm_drawdown_peak_date": "2024-01-01",
            "max_mtm_drawdown_trough_date": "2024-01-01"
        }
    }))
}

pub(super) fn reconciliation_report() -> Result<Vec<Value>> {
    example(&crate::attribution::ReconciliationReport {
        total_residual: 0.0,
        is_reconciled: true,
        tolerance: 0.01,
    })
}

// ---------------------------------------------------------------------------
// Factor model
// ---------------------------------------------------------------------------

pub(super) fn sensitivity_matrix_json() -> Result<Vec<Value>> {
    example_from_json::<crate::sensitivity::SensitivityMatrixJson>(json!({
        "base_currency": "USD",
        "position_ids": ["POS_001", "POS_002"],
        "factor_ids": ["USD-RATES", "IG-CREDIT"],
        "data": [[-4_600.0, 0.0], [-7_400.0, -6_900.0]]
    }))
}

pub(super) fn factor_pnl_profile() -> Result<Vec<Value>> {
    example_from_json::<crate::sensitivity::FactorPnlProfile>(json!({
        "base_currency": "USD",
        "factor_id": "USD-RATES",
        "position_ids": ["POS_001", "POS_002"],
        "shifts": [-1.0, 0.0, 1.0],
        "position_pnls": [[4_612.0, 0.0, -4_588.0], [7_431.0, 0.0, -7_369.0]]
    }))
}

pub(super) fn factor_assignment_report() -> Result<Vec<Value>> {
    example_from_json::<crate::factor_model::FactorAssignmentReport>(json!({
        "assignments": [{
            "position_id": "POS_001",
            "mappings": [[
                {"curve": {"id": "USD-OIS", "curve_type": "discount"}},
                "USD-RATES",
                1.0
            ]]
        }],
        "unmatched": [{
            "position_id": "POS_002",
            "dependency": {"spot": {"id": "AAPL"}}
        }]
    }))
}

pub(super) fn credit_vol_report() -> Result<Vec<Value>> {
    example_from_json::<crate::factor_model::CreditVolReport>(json!({
        "total": 0.0185,
        "measure": "volatility",
        "generic": 0.0120,
        "by_level": [{
            "level_name": "sector",
            "total": 0.0090,
            "by_bucket": {"Financials": 0.0055, "Industrials": 0.0035}
        }],
        "idiosyncratic_total": 0.0108,
        "by_position_optional": null
    }))
}

pub(super) fn position_change() -> Result<Vec<Value>> {
    example(&crate::factor_model::PositionChange::Resize {
        position_id: "POS_001".into(),
        new_quantity: 0.5,
    })
}

pub(super) fn stress_pnl() -> Result<Vec<Value>> {
    example_from_json::<crate::factor_model::StressPnl>(json!({
        "total_pnl": -12_000.0,
        "position_pnl": [["POS_001", -4_600.0], ["POS_002", -7_400.0]]
    }))
}

pub(super) fn stress_result() -> Result<Vec<Value>> {
    example_from_json::<crate::factor_model::StressResult>(json!({
        "total_pnl": -12_000.0,
        "position_pnl": [["POS_001", -4_600.0], ["POS_002", -7_400.0]],
        "stressed_decomposition": risk_decomposition(1.5)
    }))
}

/// Halving one position: every factor's variance contribution falls.
pub(super) fn what_if_result() -> Result<Vec<Value>> {
    example_from_json::<crate::factor_model::WhatIfResult>(json!({
        "before": risk_decomposition(1.0),
        "after": risk_decomposition(0.5),
        "delta": [
            {"factor_id": "USD-RATES", "absolute_change": -0.000125, "relative_change": -0.5},
            {"factor_id": "IG-CREDIT", "absolute_change": -0.00005, "relative_change": -0.5}
        ]
    }))
}
