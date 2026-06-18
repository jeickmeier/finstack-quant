"""JSON pricing tests for valuation registry routes."""

from __future__ import annotations

import json

import pytest

from finstack_quant.valuations.instruments import price_instrument, validate_instrument_json


def _money(amount: str) -> dict[str, str]:
    return {"amount": amount, "currency": "USD"}


def _credit_enhancement() -> dict[str, object]:
    return {
        "subordination": _money("0"),
        "overcollateralization": _money("0"),
        "reserve_account": _money("0"),
        "excess_spread": 0.0,
        "cash_trap_active": False,
    }


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
        "behavior_type": "Standard",
        "seniority": seniority,
        "rating": None,
        "original_balance": _money(balance),
        "current_balance": _money(balance),
        "target_balance": None,
        "coupon": {"Fixed": {"rate": rate}},
        "oc_trigger": None,
        "ic_trigger": None,
        "credit_enhancement": _credit_enhancement(),
        "frequency": {"count": 3, "unit": "months"},
        "day_count": "Act360",
        "deferred_interest": _money("0"),
        "pik_enabled": False,
        "is_revolving": False,
        "can_reinvest": False,
        "maturity": "2026-01-01",
        "expected_maturity": None,
        "payment_priority": priority,
        "attributes": {},
    }


def _structured_credit_json() -> str:
    pool = {
        "id": "POOL",
        "deal_type": "ABS",
        "base_currency": "USD",
        "assets": [
            {
                "id": "A1",
                "asset_type": {"type": "HighYieldBond", "industry": None},
                "balance": _money("1000000"),
                "rate": 0.06,
                "spread_bps": None,
                "index_id": None,
                "maturity": "2026-01-01",
                "credit_quality": None,
                "industry": None,
                "obligor_id": None,
                "is_defaulted": False,
                "recovery_amount": None,
                "purchase_price": None,
                "acquisition_date": None,
                "day_count": "Thirty360",
                "smm_override": None,
                "mdr_override": None,
            }
        ],
        "cumulative_defaults": _money("0"),
        "cumulative_recoveries": _money("0"),
        "cumulative_prepayments": _money("0"),
        "cumulative_scheduled_amortization": None,
        "reinvestment_period": None,
        "collection_account": _money("0"),
        "reserve_account": _money("0"),
        "excess_spread_account": _money("0"),
        "rep_lines": None,
    }
    spec = {
        "id": "ABS-STOCH-PV",
        "deal_type": "ABS",
        "pool": pool,
        "tranches": {
            "tranches": [
                _tranche("SR", 0.0, 80.0, "Senior", "800000", 0.05, 1),
                _tranche("EQ", 80.0, 100.0, "Equity", "200000", 0.0, 4),
            ],
            "total_size": _money("1000000"),
        },
        "closing_date": "2024-01-01",
        "first_payment_date": "2025-02-01",
        "reinvestment_end_date": None,
        "maturity": "2026-01-01",
        "frequency": {"count": 1, "unit": "months"},
        "payment_calendar_id": "nyse",
        "discount_curve_id": "USD-OIS",
        "pricing_overrides": {"mc_paths": 1},
        "attributes": {},
        "prepayment_spec": {"cpr": 0.0, "curve": None},
        "default_spec": {"cdr": 0.0, "curve": None},
        "recovery_spec": {"rate": 0.4, "recovery_lag": 0},
        "stochastic_prepay_spec": {"model": "Deterministic", "cpr": 0.0, "curve": None},
        "stochastic_default_spec": {"model": "Deterministic", "cdr": 0.0, "curve": None},
        "market_conditions": {
            "refi_rate": 0.04,
            "original_rate": None,
            "hpa": None,
            "unemployment": None,
            "seasonal_factor": 1.0,
            "custom_factors": {},
        },
        "credit_factors": {
            "credit_score": None,
            "dti": None,
            "ltv": None,
            "delinquency_days": 0,
            "unemployment_rate": None,
            "custom_factors": {},
        },
    }
    return json.dumps({"type": "structured_credit", "spec": spec})


def _market_json(include_discount: bool = True) -> str:
    curves: list[dict[str, object]] = []
    if include_discount:
        curves.append({
            "type": "discount",
            "id": "USD-OIS",
            "base": "2024-01-01",
            "day_count": "Act360",
            "knot_points": [[0.0, 1.0], [5.0, 0.90]],
            "interp_style": "monotone_convex",
            "extrapolation": "flat_forward",
            "min_forward_rate": None,
            "allow_non_monotonic": False,
            "min_forward_tenor": 1e-6,
        })
    return json.dumps({
        "version": 2,
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
    })


def _tarn_market_json() -> str:
    return json.dumps({
        "version": 2,
        "curves": [
            {
                "type": "discount",
                "id": "USD-OIS",
                "base": "2025-01-01",
                "day_count": "Act365F",
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
                "day_count": "Act365F",
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
    })


def _sabr_cube_json(cube_id: str, alpha: float, forward: float) -> dict[str, object]:
    params = {"alpha": alpha, "beta": 0.5, "rho": -0.20, "nu": 0.40}
    return {
        "id": cube_id,
        "expiries": [0.25, 1.0, 5.0],
        "tenors": [2.0, 10.0],
        "params": [params] * 6,
        "forwards": [forward] * 6,
    }


def _cms_spread_market_json() -> str:
    return json.dumps({
        "version": 2,
        "curves": [
            {
                "type": "discount",
                "id": "USD-OIS",
                "base": "2025-01-01",
                "day_count": "Act365F",
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
                "day_count": "Act365F",
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
    })


def _tarn_json() -> str:
    return json.dumps({
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
            "day_count": "Act365F",
            "pricing_overrides": {
                "mc_paths": 32,
                "mean_reversion": 0.05,
                "implied_volatility": 1e-12,
            },
            "attributes": {},
        },
    })


def _snowball_json() -> str:
    return json.dumps({
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
            "day_count": "Act365F",
            "pricing_overrides": {
                "mc_paths": 32,
                "mean_reversion": 0.05,
                "implied_volatility": 1e-12,
            },
            "attributes": {},
        },
    })


def _inverse_floater_json() -> str:
    return json.dumps({
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
            "day_count": "Act365F",
            "pricing_overrides": {},
            "attributes": {},
        },
    })


def _callable_range_accrual_json() -> str:
    return json.dumps({
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
                "day_count": "Act365F",
                "discount_curve_id": "USD-OIS",
                "spot_id": "SOFR-RATE",
                "vol_surface_id": "SOFR-VOL",
                "div_yield_id": None,
                "pricing_overrides": {},
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
            "pricing_overrides": {
                "mc_paths": 8,
                "mean_reversion": 0.05,
                "implied_volatility": 1e-12,
            },
            "attributes": {},
        },
    })


def _bermudan_swaption_json() -> str:
    return json.dumps({
        "type": "bermudan_swaption",
        "spec": {
            "id": "BERM-10NC2-USD",
            "option_type": "call",
            "notional": {"amount": "10000000", "currency": "USD"},
            "strike": "0.03",
            "swap_start": "2027-01-17",
            "swap_end": "2037-01-17",
            "fixed_freq": {"count": 6, "unit": "months"},
            "float_freq": {"count": 3, "unit": "months"},
            "day_count": "Thirty360",
            "settlement": "physical",
            "discount_curve_id": "USD-OIS",
            "forward_curve_id": "USD-OIS",
            "vol_surface_id": "USD-SWPNVOL",
            "bermudan_schedule": {
                "exercise_dates": ["2029-01-17", "2030-01-17"],
                "lockout_end": None,
                "notice_days": 0,
            },
            "bermudan_type": "CoTerminal",
        },
    })


def _cms_spread_option_json() -> str:
    return json.dumps({
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
            "day_count": "Act365F",
            "pricing_overrides": {},
            "attributes": {},
        },
    })


def test_bermudan_swaption_json_validates() -> None:
    canonical = json.loads(validate_instrument_json(_bermudan_swaption_json()))
    assert canonical["type"] == "bermudan_swaption"


def test_tarn_json_prices_with_hull_white_mc() -> None:
    result = json.loads(
        price_instrument(
            _tarn_json(),
            _tarn_market_json(),
            "2025-01-01",
            "monte_carlo_hull_white_1f",
        )
    )

    assert float(result["value"]["amount"]) > 0
    assert result["measures"]["mc_num_paths"] == 32


def test_snowball_json_prices_with_hull_white_mc() -> None:
    result = json.loads(
        price_instrument(
            _snowball_json(),
            _tarn_market_json(),
            "2025-01-01",
            "monte_carlo_hull_white_1f",
        )
    )

    assert float(result["value"]["amount"]) > 0
    assert result["measures"]["mc_num_paths"] == 32


def test_inverse_floater_json_prices_with_discounting() -> None:
    result = json.loads(
        price_instrument(
            _inverse_floater_json(),
            _tarn_market_json(),
            "2025-01-01",
            "discounting",
        )
    )

    assert float(result["value"]["amount"]) > 0


def test_callable_range_accrual_json_prices_with_hull_white_mc() -> None:
    result = json.loads(
        price_instrument(
            _callable_range_accrual_json(),
            _tarn_market_json(),
            "2025-01-01",
            "monte_carlo_hull_white_1f",
        )
    )

    assert float(result["value"]["amount"]) > 0
    assert result["measures"]["mc_num_paths"] == 8


def test_cms_spread_option_json_prices_with_static_replication() -> None:
    result = json.loads(
        price_instrument(
            _cms_spread_option_json(),
            _cms_spread_market_json(),
            "2025-01-01",
            "static_replication",
        )
    )

    assert float(result["value"]["amount"]) > 0
    assert result["measures"]["cms_spread_forward"] > 0


def test_structured_credit_stochastic_json_details_include_all_tranches() -> None:
    result = json.loads(
        price_instrument(
            _structured_credit_json(),
            _market_json(),
            "2024-01-01",
            "structured_credit_stochastic",
        )
    )

    details = result["details"]
    assert details["type"] == "structured_credit_stochastic"
    assert len(details["data"]["tranche_results"]) == 2
    assert {row["tranche_id"] for row in details["data"]["tranche_results"]} == {"SR", "EQ"}


def test_structured_credit_stochastic_json_missing_market_data_raises() -> None:
    with pytest.raises(ValueError, match="Curve not found"):
        price_instrument(
            _structured_credit_json(),
            _market_json(include_discount=False),
            "2024-01-01",
            "structured_credit_stochastic",
        )


def test_structured_credit_waterfall_rules_prices_through_json() -> None:
    # `waterfall_rules` is an additive serde-default field on the deal; the
    # rebuilt binding must accept and price a deal that configures it (here an
    # available-funds cap on the senior). Before the field existed the
    # deny-unknown-fields deserialization rejected this payload.
    payload = json.loads(_structured_credit_json())
    payload["spec"]["waterfall_rules"] = {"afc": {"capped_tranches": ["SR"]}}
    result = json.loads(
        price_instrument(
            json.dumps(payload),
            _market_json(),
            "2024-01-01",
            "structured_credit_stochastic",
        )
    )
    assert float(result["value"]["amount"]) > 0


def test_structured_credit_tranche_metric_methods() -> None:
    # The standalone tranche metrics are exposed as methods on the
    # StructuredCredit wrapper, mirroring `price` / `price_with_metrics`.
    from finstack_quant.valuations.instruments.fixed_income import StructuredCredit

    sc = StructuredCredit.from_json(_structured_credit_json())
    market = _market_json()

    oas = json.loads(sc.oas(market, "2024-01-01", "SR", 99.0))
    assert oas["model_price"] > 0
    assert oas["oas"] == oas["oas"]  # finite (not NaN)

    breakeven = sc.breakeven_cdr(market, "2024-01-01", "SR")
    assert breakeven >= 0.0

    grid = json.dumps({"cprs": [0.10, 0.20], "cdrs": [0.02], "severities": [0.40]})
    table = json.loads(sc.scenario_table(market, "2024-01-01", "SR", grid))
    assert len(table["cells"]) == 2

    # Per-tranche metrics bundle: PV/WAL and the credit z-spread/CS01 computed
    # from this tranche's own cashflows (meaningful per note).
    tm = json.loads(sc.tranche_metrics(market, "2024-01-01", "SR"))
    assert tm["tranche_id"] == "SR"
    assert tm["pv"] > 0
    assert tm["wal"] >= 0.0
    # Solved against its own model price -> z-spread ~ 0.
    assert abs(tm["z_spread_bp"]) < 1.0
    # Against a cheaper market price the z-spread must widen (positive).
    tm_cheap = json.loads(sc.tranche_metrics(market, "2024-01-01", "SR", 95.0))
    assert tm_cheap["z_spread_bp"] > tm["z_spread_bp"]
    assert tm_cheap["target_price_pct"] == 95.0

    # Discount margin requires a floating-rate tranche; the fixed-rate senior
    # here must raise, exercising the method dispatch and error mapping.
    with pytest.raises(ValueError, match="floating-rate"):
        sc.discount_margin(market, "2024-01-01", "SR", 800_000.0)
