//! Canonical example payloads for the result and component artifacts.
//!
//! Conventions, catalog rows and metric metadata are read from the registries
//! compiled into the library; instrument payloads reuse each instrument's own
//! `example()`. Results a pricer would have to run to produce are literal JSON,
//! round-tripped through the contract type so a payload the type rejects cannot
//! be published.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::schema::{example, example_from_json};
use finstack_quant_core::Result;
use serde_json::{json, Value};

use crate::instruments::fixed_income::structured_credit;
use crate::market::conventions::ConventionRegistry;

fn date(year: i32, month: time::Month, day: u8) -> Result<Date> {
    Date::from_calendar_date(year, month, day).map_err(|error| {
        finstack_quant_core::Error::Internal(format!("build example date: {error}"))
    })
}

/// The instrument union's bond branch.
pub(super) fn instrument_json() -> Result<Vec<Value>> {
    example(&crate::instruments::InstrumentJson::Bond(
        crate::instruments::Bond::example()?,
    ))
}

/// One fixed coupon of a USD bond, discounted on the OIS curve.
pub(super) fn instrument_cashflow() -> Result<Vec<Value>> {
    example_from_json::<crate::instruments::cashflow_export::InstrumentCashflowEnvelope>(json!({
        "instrument_id": "US912828XG33",
        "currency": "USD",
        "model": "discounting",
        "as_of": "2025-01-15",
        "discount_curve_id": "USD-OIS",
        "flows": [{
            "date": "2025-07-15",
            "amount": 21_250.0,
            "currency": "USD",
            "kind": "fixed",
            "accrual_factor": 0.5,
            "year_fraction": 0.4958904109589041,
            "rate": 0.0425,
            "discount_factor": 0.9789,
            "discount_curve_id": "USD-OIS",
            "pv": 20_801.625
        }],
        "total_pv": 20_801.625,
        "reconciles_with_base_value": true
    }))
}

/// The canonical composite with one rebalancing trade.
pub(super) fn composite_rebalance_result() -> Result<Vec<Value>> {
    example(&crate::instruments::CompositeRebalanceResult {
        instrument: crate::instruments::CompositeInstrument::example()?,
        trades: vec![crate::instruments::CompositeTrade {
            instrument_id: "AAPL".into(),
            instrument_type: "equity".to_string(),
            quantity_delta: 25.0,
        }],
    })
}

/// One dated composite state with no look-through exposures.
pub(super) fn composite_history_row() -> Result<Vec<Value>> {
    example_from_json::<crate::instruments::CompositeHistoryRow>(json!({
        "date": "2025-01-31",
        "value": {"amount": "1012500", "currency": "USD"},
        "cashflows": {"amount": "0", "currency": "USD"},
        "pnl": {"amount": "12500", "currency": "USD"},
        "period_return": 0.0125,
        "return_index": 1.0125,
        "held_state_effective_date": "2025-01-02",
        "next_state_effective_date": null,
        "exposures": {"reporting_currency": "USD", "paths": [], "aggregates": []},
        "rebalance_trades": []
    }))
}

/// A 10,000-path revolver valuation with per-path detail omitted.
pub(super) fn enhanced_monte_carlo_result() -> Result<Vec<Value>> {
    example_from_json::<crate::instruments::fixed_income::revolving_credit::EnhancedMonteCarloResult>(
        json!({
            "mc_result": {
                "estimate": {
                    "mean": {"amount": "9875000", "currency": "USD"},
                    "stderr": 4_200.0,
                    "ci_95": [
                        {"amount": "9866768", "currency": "USD"},
                        {"amount": "9883232", "currency": "USD"}
                    ],
                    "num_paths": 10_000,
                    "num_simulated_paths": 10_000,
                    "std_dev": 420_000.0,
                    "median": null,
                    "percentile_25": null,
                    "percentile_75": null,
                    "min": null,
                    "max": null
                },
                "paths": null,
                "run": {
                    "seed": 42,
                    "use_parallel": true,
                    "antithetic": false,
                    "chunk_size": 1_000,
                    "num_steps": 20
                }
            },
            "path_results": [],
            "draw_option_cost": {
                "mean": {"amount": "31500", "currency": "USD"},
                "stderr": 650.0,
                "ci_95": [
                    {"amount": "30226", "currency": "USD"},
                    {"amount": "32774", "currency": "USD"}
                ],
                "num_paths": 10_000,
                "num_simulated_paths": 10_000,
                "std_dev": 65_000.0,
                "median": null,
                "percentile_25": null,
                "percentile_75": null,
                "min": null,
                "max": null
            }
        }),
    )
}

pub(super) fn oas_result() -> Result<Vec<Value>> {
    example(&structured_credit::OasResult {
        oas: 0.0145,
        model_price: 99.25,
        market_price: 99.25,
        num_paths: 2_000,
        price_std_error: 0.04,
    })
}

/// A two-cell prepayment/default grid for one mezzanine tranche.
pub(super) fn scenario_table() -> Result<Vec<Value>> {
    example_from_json::<structured_credit::ScenarioTable>(json!({
        "tranche_id": "B",
        "cells": [
            {"cpr": 0.15, "cdr": 0.02, "severity": 0.4, "price": 99.1, "wal": 5.8, "writedown": 0.0},
            {"cpr": 0.15, "cdr": 0.06, "severity": 0.4, "price": 93.4, "wal": 6.3, "writedown": 0.035}
        ]
    }))
}

pub(super) fn tranche_metrics() -> Result<Vec<Value>> {
    example_from_json::<structured_credit::TrancheMetrics>(json!({
        "tranche_id": "A",
        "currency": "USD",
        "pv": 248_750_000.0,
        "price_pct": 99.5,
        "factor": 1.0,
        "wal": 4.6,
        "z_spread_bp": 142.0,
        "cs01": 108_500.0,
        "spread_duration": 4.36,
        "spread_convexity": 0.24,
        "modified_duration": 0.24,
        "convexity": 0.001,
        "target_price_pct": 99.5,
        "dm_bp": 139.0
    }))
}

pub(super) fn borrowing_base_report() -> Result<Vec<Value>> {
    example(&structured_credit::BorrowingBaseReport {
        eligible_collateral: Money::from((120_000_000_i64, Currency::USD)),
        concentration_excess: Money::from((5_000_000_i64, Currency::USD)),
        borrowing_base: Money::from((97_750_000_i64, Currency::USD)),
    })
}

/// The first CME row of the listed-product catalog compiled into the library.
pub(super) fn listed_product_coverage() -> Result<Vec<Value>> {
    let rows = crate::market::listed::listed_product_catalog(Some(
        crate::market::listed::ListedExchange::Cme,
    ))?;
    let row = rows.first().ok_or_else(|| {
        finstack_quant_core::Error::Internal("listed product catalog has no CME row".to_string())
    })?;
    example(row)
}

/// The interpretation of a bucketed DV01 key.
pub(super) fn metric_metadata() -> Result<Vec<Value>> {
    let rows = crate::pricer::metric_metadata(&["bucketed_dv01::USD-OIS::10y".to_string()])?;
    let row = rows.first().ok_or_else(|| {
        finstack_quant_core::Error::Internal("metric metadata returned no row".to_string())
    })?;
    example(row)
}

pub(super) fn equity_metrics() -> Result<Vec<Value>> {
    example(&structured_credit::EquityMetrics {
        tranche_id: "EQ".to_string(),
        currency: Currency::USD,
        invested: 40_000_000.0,
        irr: Some(0.142),
        moic: 1.85,
        nav_pct: 62.5,
        cash_on_cash: vec![
            (date(2025, time::Month::April, 15)?, 0.045),
            (date(2025, time::Month::July, 15)?, 0.047),
        ],
    })
}

/// One quarterly interest payment on a senior floating-rate tranche.
fn tranche_cashflows_value(tranche_id: &str) -> Value {
    let usd = |amount: &str| json!({"amount": amount, "currency": "USD"});
    json!({
        "tranche_id": tranche_id,
        "cashflows": [["2025-04-15", usd("3750000")]],
        "detailed_flows": [{
            "date": "2025-04-15",
            "reset_date": null,
            "amount": usd("3750000"),
            "kind": "float_reset",
            "accrual_factor": 0.25,
            "rate": 0.06
        }],
        "accrual_periods": [{
            "start": "2025-01-15",
            "end": "2025-04-15",
            "payment_date": "2025-04-15",
            "opening_balance": usd("250000000"),
            "coupon_rate": 0.06,
            "day_count": "act_360"
        }],
        "interest_flows": [["2025-04-15", usd("3750000")]],
        "principal_flows": [],
        "pik_flows": [],
        "deferred_flows": [],
        "writedown_flows": [],
        "final_balance": usd("250000000"),
        "total_interest": usd("3750000"),
        "total_principal": usd("0"),
        "total_pik": usd("0"),
        "total_deferred": usd("0"),
        "total_writedown": usd("0")
    })
}

pub(super) fn tranche_cashflows() -> Result<Vec<Value>> {
    example_from_json::<structured_credit::TrancheCashflows>(tranche_cashflows_value("A"))
}

/// Deal-level accounting with no reserve activity.
fn simulation_diagnostics_value() -> Value {
    let zero = json!({"amount": "0", "currency": "USD"});
    json!({
        "periods": [],
        "reserve_balance_path": [],
        "reserve_interest_paid": [],
        "draws_from_reserve": zero,
        "draws_from_principal": zero,
        "unfunded_draws": zero,
        "reserve_replenished": zero,
        "early_amortization_date": null,
        "tranche_draws": []
    })
}

pub(super) fn simulation_diagnostics() -> Result<Vec<Value>> {
    example_from_json::<structured_credit::SimulationDiagnostics>(simulation_diagnostics_value())
}

/// The market data the canonical bond needs to price.
pub(super) fn market_dependencies() -> Result<Vec<Value>> {
    use crate::instruments::Instrument;
    example(&crate::instruments::Bond::example()?.market_dependencies()?)
}

pub(super) fn convertible_greeks() -> Result<Vec<Value>> {
    example(
        &crate::instruments::fixed_income::convertible::ConvertibleGreeks {
            price: 1_087.5,
            delta: 6.2,
            gamma: 0.045,
            vega: 3.1,
            theta: -0.08,
            rho: 1.9,
        },
    )
}

/// A facility and its residual, each with one quarterly payment.
pub(super) fn facility_projection() -> Result<Vec<Value>> {
    example_from_json::<crate::instruments::fixed_income::asset_backed_facility::FacilityProjection>(
        json!({
            "facility": tranche_cashflows_value("FACILITY"),
            "residual": tranche_cashflows_value("RESIDUAL"),
            "commitment_fees": [["2025-04-15", {"amount": "62500", "currency": "USD"}]],
            "draws": [],
            "diagnostics": simulation_diagnostics_value()
        }),
    )
}

pub(super) fn merton_mc_result() -> Result<Vec<Value>> {
    example_from_json::<
        crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcResult,
    >(json!({
        "clean_price_pct": 94.8,
        "dirty_price_pct": 96.1,
        "expected_loss": 0.031,
        "unexpected_loss": 0.094,
        "expected_shortfall_95": 0.41,
        "average_pik_fraction": 0.22,
        "effective_spread_bp": 485.0,
        "path_statistics": {
            "default_rate": 0.058,
            "avg_default_time": 3.4,
            "avg_terminal_notional": 108.5,
            "avg_recovery_rate": 0.42,
            "pik_exercise_rate": 0.22
        },
        "num_paths": 10_000,
        "standard_error": 0.12
    }))
}

pub(super) fn cds_index_params() -> Result<Vec<Value>> {
    example(
        &crate::instruments::credit_derivatives::cds_index::CdsIndexParams::cdx_na_ig(42, 1, 100.0),
    )
}

pub(super) fn cds_tranche_params() -> Result<Vec<Value>> {
    example(
        &crate::instruments::credit_derivatives::cds_tranche::CdsTrancheParams::mezzanine_tranche(
            "CDX.NA.IG",
            42,
            Money::from((10_000_000_i64, Currency::USD)),
            date(2029, time::Month::June, 20)?,
            100.0,
        ),
    )
}

/// Two daily scenarios shifting one rate and one equity factor.
pub(super) fn market_history() -> Result<Vec<Value>> {
    use crate::metrics::risk::{MarketHistory, MarketScenario, RiskFactorShift, RiskFactorType};

    let shifts = |rate: f64, equity: f64| {
        vec![
            RiskFactorShift {
                factor: RiskFactorType::DiscountRate {
                    curve_id: "USD-OIS".into(),
                    tenor_years: 5.0,
                },
                shift: rate,
            },
            RiskFactorShift {
                factor: RiskFactorType::EquitySpot {
                    ticker: "SPX".to_string(),
                },
                shift: equity,
            },
        ]
    };
    example(&MarketHistory {
        base_date: date(2025, time::Month::January, 15)?,
        window_days: 2,
        scenarios: vec![
            MarketScenario {
                date: date(2025, time::Month::January, 14)?,
                shifts: shifts(0.0004, -0.012),
            },
            MarketScenario {
                date: date(2025, time::Month::January, 13)?,
                shifts: shifts(-0.0002, 0.006),
            },
        ],
    })
}

pub(super) fn rate_index_conventions() -> Result<Vec<Value>> {
    example(
        ConventionRegistry::try_global()?
            .require_rate_index(&finstack_quant_core::types::IndexId::new("USD-SOFR"))?,
    )
}

pub(super) fn cds_convention_spec() -> Result<Vec<Value>> {
    use crate::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    example(
        ConventionRegistry::try_global()?.resolve_cds(&CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        })?,
    )
}

pub(super) fn swaption_conventions() -> Result<Vec<Value>> {
    example(ConventionRegistry::try_global()?.require_swaption(
        &crate::market::conventions::ids::SwaptionConventionId::new("USD"),
    )?)
}

pub(super) fn inflation_swap_conventions() -> Result<Vec<Value>> {
    example(ConventionRegistry::try_global()?.require_inflation_swap(
        &crate::market::conventions::ids::InflationSwapConventionId::new("USD-CPI"),
    )?)
}

pub(super) fn xccy_conventions() -> Result<Vec<Value>> {
    example(ConventionRegistry::try_global()?.require_xccy(
        &crate::market::conventions::ids::XccyConventionId::new("EUR/USD-XCCY"),
    )?)
}

pub(super) fn ir_future_conventions() -> Result<Vec<Value>> {
    example(ConventionRegistry::try_global()?.require_ir_future(
        &crate::market::conventions::ids::IrFutureContractId::new("CME:SR3"),
    )?)
}
