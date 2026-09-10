"""JSON pricing tests for valuation registry routes."""

from __future__ import annotations

import json

import pytest

from finstack_quant.valuations.instruments import (
    instrument_cashflows_json,
    price_instrument,
    structured_credit_tranche_breakeven_cdr,
    structured_credit_tranche_discount_margin,
    structured_credit_tranche_metrics,
    structured_credit_tranche_oas,
    structured_credit_tranche_scenario_table,
    validate_instrument_json,
)


def _money(amount: str) -> dict[str, str]:
    return {"amount": amount, "currency": "USD"}


def _instrument_json(instrument: dict[str, object]) -> str:
    return json.dumps({"schema": "finstack_quant.instrument/1", "instrument": instrument})


def _tranche(
    tranche_id: str,
    attachment: float,
    detachment: float,
    seniority: str,
    balance: str,
    rate: float,
    priority: int,
) -> dict[str, object]:
    return {
        "id": tranche_id,
        "attachment_point": attachment,
        "detachment_point": detachment,
        "behavior_type": "standard",
        "seniority": seniority,
        "rating": None,
        "original_balance": _money(balance),
        "current_balance": _money(balance),
        "target_balance": None,
        "coupon": {"fixed": {"rate": rate}},
        "oc_trigger": None,
        "ic_trigger": None,
        "frequency": {"count": 3, "unit": "months"},
        "day_count": "act_360",
        "deferred_interest": _money("0"),
        "pik_enabled": False,
        "is_revolving": False,
        "can_reinvest": False,
        "maturity": "2026-01-01",
        "payment_priority": priority,
        "attributes": {},
    }


def _structured_credit_json() -> str:
    pool = {
        "id": "POOL",
        "deal_type": "abs",
        "base_currency": "USD",
        "assets": [
            {
                "id": "A1",
                "asset_type": {"type": "high_yield_bond", "industry": None},
                "balance": _money("1000000"),
                "rate": 0.06,
                "spread_bp": None,
                "index_id": None,
                "maturity": "2026-01-01",
                "credit_quality": None,
                "industry": None,
                "obligor_id": None,
                "is_defaulted": False,
                "recovery_amount": None,
                "purchase_price": None,
                "acquisition_date": None,
                "day_count": "30_360",
                "smm_override": None,
                "mdr_override": None,
            }
        ],
        "cumulative_defaults": _money("0"),
        "cumulative_recoveries": _money("0"),
        "cumulative_prepayments": _money("0"),
        "cumulative_scheduled_amortization": _money("0"),
        "reinvestment_period": None,
        "collection_account": _money("0"),
        "reserve_account": _money("0"),
        "excess_spread_account": _money("0"),
        "rep_lines": None,
    }
    spec = {
        "id": "ABS-STOCH-PV",
        "deal_type": "abs",
        "pool": pool,
        "tranches": {
            "tranches": [
                _tranche("SR", 0.0, 80.0, "senior", "800000", 0.05, 1),
                _tranche("EQ", 80.0, 100.0, "equity", "200000", 0.0, 4),
            ],
            "total_size": _money("1000000"),
        },
        "closing_date": "2024-01-01",
        "first_payment_date": "2025-02-01",
        "maturity": "2026-01-01",
        "frequency": {"count": 1, "unit": "months"},
        "payment_calendar_id": "nyse",
        "discount_curve_id": "USD-OIS",
        "instrument_pricing_overrides": {"model_config": {"mc_paths": 1}},
        "attributes": {},
        "prepayment_spec": {"cpr": 0.0, "curve": None},
        "default_spec": {"cdr": 0.0, "curve": None},
        "recovery_spec": {"rate": 0.4, "recovery_lag": 0},
        "stochastic_prepay_spec": {"model": "deterministic", "cpr": 0.0, "curve": None},
        "stochastic_default_spec": {"model": "deterministic", "cdr": 0.0, "curve": None},
        "market_conditions": {
            "refi_rate": 0.04,
        },
        "credit_factors": {},
    }
    return _instrument_json({"type": "structured_credit", "spec": spec})


def _market_json(include_discount: bool = True) -> str:
    curves: list[dict[str, object]] = []
    if include_discount:
        curves.append({
            "type": "discount",
            "id": "USD-OIS",
            "base": "2024-01-01",
            "day_count": "act_360",
            "knot_points": [[0.0, 1.0], [5.0, 0.90]],
            "interp_style": "monotone_convex",
            "extrapolation": "flat_forward",
            "min_forward_rate": None,
            "allow_non_monotonic": False,
            "min_forward_tenor": 1e-6,
        })
    return json.dumps({
        "schema_version": 1,
        "curves": curves,
        "fx": None,
        "surfaces": [],
        "prices": {"SOFR-RATE": {"unitless": 0.03}},
        "series": [],
        "inflation_indices": [],
        "dividends": [],
        "credit_indices": [],
        "fx_delta_vol_surfaces": [],
        "vol_cubes": [],
        "collateral": {},
        "hierarchy": None,
    })


def _tarn_market_json() -> str:
    return json.dumps({
        "schema_version": 1,
        "curves": [
            {
                "type": "discount",
                "id": "USD-OIS",
                "base": "2025-01-01",
                "day_count": "act_365f",
                "knot_points": [[0.0, 1.0], [6.0, 0.8869204367171575]],
                "interp_style": "linear",
                "extrapolation": "flat_forward",
                "min_forward_rate": None,
                "allow_non_monotonic": False,
                "min_forward_tenor": 1e-6,
            },
            {
                "type": "forward",
                "id": "USD-SOFR-6M",
                "base": "2025-01-01",
                "reset_lag": 0,
                "day_count": "act_365f",
                "tenor": 0.5,
                "knot_points": [[0.0, 0.03], [6.0, 0.03]],
                "interp_style": "linear",
                "extrapolation": "flat_forward",
            },
        ],
        "fx": None,
        "surfaces": [],
        "prices": {"SOFR-RATE": {"unitless": 0.03}},
        "series": [],
        "inflation_indices": [],
        "dividends": [],
        "credit_indices": [],
        "fx_delta_vol_surfaces": [],
        "vol_cubes": [],
        "collateral": {},
        "hierarchy": None,
    })


def _sabr_cube_json(cube_id: str, alpha: float, forward: float) -> dict[str, object]:
    params = {"alpha": alpha, "beta": 0.5, "rho": -0.20, "nu": 0.40}
    return {
        "id": cube_id,
        "expiries": [0.25, 1.0, 5.0],
        "tenors": [2.0, 10.0],
        "params": [params] * 6,
        "forwards": [forward] * 6,
        "interpolation_mode": "vol",
    }


def _cms_spread_market_json() -> str:
    return json.dumps({
        "schema_version": 1,
        "curves": [
            {
                "type": "discount",
                "id": "USD-OIS",
                "base": "2025-01-01",
                "day_count": "act_365f",
                "knot_points": [[0.0, 1.0], [30.0, 0.3499377491111553]],
                "interp_style": "linear",
                "extrapolation": "flat_forward",
                "min_forward_rate": None,
                "allow_non_monotonic": False,
                "min_forward_tenor": 1e-6,
            },
            {
                "type": "forward",
                "id": "USD-SOFR-3M",
                "base": "2025-01-01",
                "reset_lag": 0,
                "day_count": "act_365f",
                "tenor": 0.25,
                "knot_points": [
                    [0.0, 0.025],
                    [2.0, 0.030],
                    [10.0, 0.045],
                    [30.0, 0.055],
                ],
                "interp_style": "linear",
                "extrapolation": "flat_forward",
            },
        ],
        "fx": None,
        "surfaces": [],
        "prices": {},
        "series": [],
        "inflation_indices": [],
        "dividends": [],
        "credit_indices": [],
        "fx_delta_vol_surfaces": [],
        "vol_cubes": [
            _sabr_cube_json("USD-SWAPTION-VOL-10Y", 0.035, 0.045),
            _sabr_cube_json("USD-SWAPTION-VOL-2Y", 0.035, 0.030),
        ],
        "collateral": {},
        "hierarchy": None,
    })


def _tarn_json() -> str:
    return _instrument_json({
        "type": "tarn",
        "spec": {
            "id": "TARN-PY-E2E",
            "fixed_rate": 0.06,
            "coupon_floor": 0.0,
            "target_coupon": 1.0,
            "notional": {"amount": "1000000", "currency": "USD"},
            "coupon_dates": [
                "2025-01-01",
                "2025-07-01",
                "2026-01-01",
                "2026-07-01",
            ],
            "floating_tenor": {"count": 6, "unit": "months"},
            "floating_index_id": "USD-SOFR-6M",
            "discount_curve_id": "USD-OIS",
            "day_count": "act_365f",
            "instrument_pricing_overrides": {
                "market_quotes": {"implied_volatility": 1e-12},
                "model_config": {
                    "mc_paths": 32,
                    "hw1f_mean_reversion": 0.05,
                    "hw1f_sigma": 0.01,
                },
            },
            "attributes": {},
        },
    })


def _snowball_json() -> str:
    return _instrument_json({
        "type": "snowball",
        "spec": {
            "id": "SNOWBALL-PY-E2E",
            "variant": "snowball",
            "initial_coupon": 0.03,
            "fixed_rate": 0.05,
            "leverage": 1.0,
            "coupon_floor": 0.0,
            "coupon_cap": None,
            "notional": {"amount": "1000000", "currency": "USD"},
            "coupon_dates": [
                "2025-01-01",
                "2025-07-01",
                "2026-01-01",
                "2026-07-01",
            ],
            "floating_index_id": "USD-SOFR-6M",
            "floating_tenor": {"count": 6, "unit": "months"},
            "discount_curve_id": "USD-OIS",
            "callable": None,
            "day_count": "act_365f",
            "instrument_pricing_overrides": {
                "market_quotes": {"implied_volatility": 1e-12},
                "model_config": {
                    "mc_paths": 32,
                    "hw1f_mean_reversion": 0.05,
                    "hw1f_sigma": 0.01,
                },
            },
            "attributes": {},
        },
    })


def _inverse_floater_json() -> str:
    return _instrument_json({
        "type": "snowball",
        "spec": {
            "id": "INV-FLOATER-PY-E2E",
            "variant": "inverse_floater",
            "initial_coupon": 0.0,
            "fixed_rate": 0.08,
            "leverage": 1.5,
            "coupon_floor": 0.0,
            "coupon_cap": 0.10,
            "notional": {"amount": "500000", "currency": "USD"},
            "coupon_dates": [
                "2025-01-01",
                "2025-07-01",
                "2026-01-01",
                "2026-07-01",
            ],
            "floating_index_id": "USD-SOFR-6M",
            "floating_tenor": {"count": 6, "unit": "months"},
            "discount_curve_id": "USD-OIS",
            "callable": None,
            "day_count": "act_365f",
            "attributes": {},
        },
    })


def _callable_range_accrual_json() -> str:
    return _instrument_json({
        "type": "callable_range_accrual",
        "spec": {
            "id": "CALLABLE-RA-PY-E2E",
            "range_accrual": {
                "id": "RA-PY-E2E",
                "underlying_ticker": "SOFR",
                "observation_dates": [
                    "2025-07-01",
                    "2026-01-01",
                    "2026-07-01",
                ],
                "lower_bound": 0.01,
                "upper_bound": 0.04,
                "bounds_type": "absolute",
                "coupon_rate": 0.06,
                "notional": {"amount": "1000000", "currency": "USD"},
                "day_count": "act_365f",
                "discount_curve_id": "USD-OIS",
                "accrual_start_date": "2025-01-01",
                "rate_index_id": "USD-SOFR",
                "projection_curve_id": "USD-OIS",
                "reference_tenor": {"count": 6, "unit": "months"},
                "spot_id": "SOFR-RATE",
                "vol_surface_id": "SOFR-VOL",
                "div_yield_id": None,
                "attributes": {},
                "quanto": None,
                "payment_date": None,
                "past_fixings_in_range": None,
                "total_past_observations": None,
            },
            "call_provision": {
                "call_dates": ["2025-07-01"],
                "call_price": 1.0,
                "lockout_periods": 0,
            },
            "instrument_pricing_overrides": {
                "market_quotes": {"implied_volatility": 1e-12},
                "model_config": {
                    "mc_paths": 8,
                    "hw1f_mean_reversion": 0.05,
                    "hw1f_sigma": 0.01,
                },
            },
            "attributes": {},
        },
    })


def _bermudan_swaption_json() -> str:
    return _instrument_json({
        "type": "bermudan_swaption",
        "spec": {
            "id": "BERM-10NC2-USD",
            "option_type": "call",
            "notional": {"amount": "10000000", "currency": "USD"},
            "settlement": "physical",
            "vol_surface_id": "USD-SWPNVOL",
            "underlying_fixed_leg": {
                "rate": "0.03",
                "start": "2027-01-17",
                "end": "2037-01-17",
                "frequency": {"count": 6, "unit": "months"},
                "day_count": "30_360",
                "business_day_convention": "modified_following",
                "stub": "none",
                "end_of_month": False,
                "payment_lag_days": 0,
                "calendar_id": None,
                "discount_curve_id": "USD-OIS",
                "compounding_simple": True,
                "par_method": None,
            },
            "underlying_float_leg": {
                "spread_bp": "0",
                "start": "2027-01-17",
                "end": "2037-01-17",
                "frequency": {"count": 3, "unit": "months"},
                "day_count": "act_360",
                "business_day_convention": "modified_following",
                "stub": "none",
                "end_of_month": False,
                "payment_lag_days": 0,
                "reset_lag_days": 0,
                "calendar_id": None,
                "fixing_calendar_id": None,
                "discount_curve_id": "USD-OIS",
                "forward_curve_id": "USD-OIS",
                "compounding": "simple",
            },
            "bermudan_schedule": {
                "exercise_dates": ["2029-01-17", "2030-01-17"],
                "lockout_end": None,
                "notice_days": 0,
            },
            "bermudan_type": "co_terminal",
            "attributes": {},
        },
    })


def _cms_spread_option_json() -> str:
    return _instrument_json({
        "type": "cms_spread_option",
        "spec": {
            "id": "CMS-SPREAD-PY-E2E",
            "long_cms_tenor": {"count": 10, "unit": "years"},
            "short_cms_tenor": {"count": 2, "unit": "years"},
            "strike": 0.005,
            "option_type": "call",
            "notional": {"amount": "10000000", "currency": "USD"},
            "expiry_date": "2026-01-01",
            "payment_date": "2026-01-05",
            "long_vol_surface_id": "USD-SWAPTION-VOL-10Y",
            "short_vol_surface_id": "USD-SWAPTION-VOL-2Y",
            "discount_curve_id": "USD-OIS",
            "forward_curve_id": "USD-SOFR-3M",
            "spread_correlation": 0.5,
            "day_count": "act_365f",
            "attributes": {},
        },
    })


def test_bermudan_swaption_json_validates() -> None:
    canonical = json.loads(validate_instrument_json(_bermudan_swaption_json()))
    assert canonical["instrument"]["type"] == "bermudan_swaption"


@pytest.mark.parametrize(
    "overrides",
    [
        {"unknown_override": 1},
        {"instrument": {"rate_bump_bp": 3.0}},
        {"metrics": {"quoted_clean_price": 99.0}},
        {"scenario": {"mc_seed_scenario": "seed"}},
    ],
)
def test_validate_instrument_json_rejects_invalid_override_ownership(
    overrides: dict[str, object],
) -> None:
    instrument = json.loads(_structured_credit_json())
    instrument["instrument"]["spec"]["pricing_overrides"] = overrides

    with pytest.raises(ValueError, match="unknown field"):
        validate_instrument_json(json.dumps(instrument))


def test_validate_instrument_json_accepts_focused_nested_overrides() -> None:
    instrument = json.loads(_structured_credit_json())
    spec = instrument["instrument"]["spec"]
    spec["instrument_pricing_overrides"] = {"model_config": {"mc_paths": 2}}
    spec["metric_pricing_overrides"] = {"bump_config": {"rate_bump_bp": 3.0}}
    spec["scenario_pricing_overrides"] = {"scenario_price_shock_pct": -0.05}

    canonical = json.loads(validate_instrument_json(json.dumps(instrument)))
    canonical_spec = canonical["instrument"]["spec"]
    assert canonical_spec["instrument_pricing_overrides"]["model_config"]["mc_paths"] == 2
    assert canonical_spec["metric_pricing_overrides"]["bump_config"]["rate_bump_bp"] == 3.0
    assert canonical_spec["scenario_pricing_overrides"]["scenario_price_shock_pct"] == -0.05


def test_tarn_json_prices_with_hull_white_mc() -> None:
    result = price_instrument(
        _tarn_json(),
        _tarn_market_json(),
        "2025-01-01",
        "monte_carlo_hull_white_1f",
    )

    assert result.price > 0
    assert result.get_metric("mc_num_paths") == 32


def test_snowball_json_prices_with_hull_white_mc() -> None:
    result = price_instrument(
        _snowball_json(),
        _tarn_market_json(),
        "2025-01-01",
        "monte_carlo_hull_white_1f",
    )

    assert result.price > 0
    assert result.get_metric("mc_num_paths") == 32


def test_inverse_floater_json_prices_with_discounting() -> None:
    result = price_instrument(
        _inverse_floater_json(),
        _tarn_market_json(),
        "2025-01-01",
        "discounting",
    )

    assert result.price > 0


def test_callable_range_accrual_json_prices_with_hull_white_mc() -> None:
    result = price_instrument(
        _callable_range_accrual_json(),
        _tarn_market_json(),
        "2025-01-01",
        "monte_carlo_hull_white_1f",
    )

    assert result.price > 0
    assert result.get_metric("mc_num_paths") == 8


def test_cms_spread_option_json_prices_with_static_replication() -> None:
    result = price_instrument(
        _cms_spread_option_json(),
        _cms_spread_market_json(),
        "2025-01-01",
        "static_replication",
    )

    assert result.price > 0
    assert result.get_metric("cms_spread_forward") > 0


def test_structured_credit_stochastic_json_details_include_all_tranches() -> None:
    result = price_instrument(
        _structured_credit_json(),
        _market_json(),
        "2024-01-01",
        "structured_credit_stochastic",
    )

    # `details` is model-specific payload with no typed accessor, so this
    # assertion still reads the wire envelope via ``to_json``.
    details = json.loads(result.to_json())["details"]
    assert details["type"] == "structured_credit_stochastic"
    assert len(details["data"]["tranche_results"]) == 2
    assert {row["tranche_id"] for row in details["data"]["tranche_results"]} == {"SR", "EQ"}


def test_structured_credit_stochastic_json_missing_market_data_raises() -> None:
    # Missing discount curves surface as Calibration failures, which the
    # binding layer maps to RuntimeError (see errors.rs valuations_to_py).
    with pytest.raises(RuntimeError, match="Curve not found"):
        price_instrument(
            _structured_credit_json(),
            _market_json(include_discount=False),
            "2024-01-01",
            "structured_credit_stochastic",
        )


def test_python_pricing_routes_validate_instrument_before_other_inputs() -> None:
    payload = json.loads(_structured_credit_json())
    payload["instrument"]["spec"]["cleanup_call_pct"] = -0.5
    invalid = json.dumps(payload)
    market = "not-market-json"

    calls = [
        lambda: price_instrument(invalid, market, "not-a-date", "not-a-model"),
        lambda: price_instrument(
            invalid,
            market,
            "not-a-date",
            model="not-a-model",
            metrics=["not-a-metric"],
        ),
        lambda: instrument_cashflows_json(invalid, market, "not-a-date", "not-a-model"),
        lambda: structured_credit_tranche_discount_margin(invalid, "missing", market, "not-a-date", float("nan")),
        lambda: structured_credit_tranche_breakeven_cdr(invalid, "missing", market, "not-a-date"),
        lambda: structured_credit_tranche_oas(invalid, "missing", float("nan"), market, "not-a-date", "not-json"),
        lambda: structured_credit_tranche_metrics(invalid, "missing", market, "not-a-date", float("nan")),
        lambda: structured_credit_tranche_scenario_table(invalid, "missing", market, "not-a-date", "not-json"),
    ]

    for call in calls:
        with pytest.raises(ValueError, match="cleanup_call_pct"):
            call()


def test_structured_credit_waterfall_rules_prices_through_json() -> None:
    # `waterfall_rules` is an additive serde-default field on the deal; the
    # rebuilt binding must accept and price a deal that configures it (here an
    # available-funds cap on the senior). Before the field existed the
    # deny-unknown-fields deserialization rejected this payload.
    payload = json.loads(_structured_credit_json())
    payload["instrument"]["spec"]["waterfall_rules"] = {"afc": {"capped_tranches": ["SR"]}}
    result = price_instrument(
        json.dumps(payload),
        _market_json(),
        "2024-01-01",
        "structured_credit_stochastic",
    )
    assert result.price > 0
