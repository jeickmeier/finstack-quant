"""Cross-host goldens for the ``finstack_quant.core`` surface bound in WASM.

Every case below is computed by Rust and exposed through both hosts. The same
case names, inputs and expected values are asserted by the WASM facade test
``finstack-quant-wasm/tests/facade/core_parity.test.mjs`` against the shared
golden file ``tests/data/core_wasm_parity.json``. Dates are compared as ISO
strings: Python returns ``datetime.date`` and WASM returns epoch days, which
each test converts.

Regenerate the golden (after an intentional Rust change) with::

    UPDATE_CORE_WASM_PARITY=1 uv run --no-sync pytest finstack-quant-py/tests/test_core_wasm_parity.py
"""

from __future__ import annotations

from collections.abc import Callable
import datetime
import json
import math
import os
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.core import config, currency, dates, rating_scales, types
from finstack_quant.core.market_data import (
    BaseCorrelationCurve,
    CreditIndexData,
    DiscountCurve,
    FxMatrix,
    HazardCurve,
    InflationCurve,
    InflationIndex,
    MarketContext,
    PriceCurve,
    ScalarTimeSeries,
    VolSurface,
)
from finstack_quant.core.math import linalg, special_functions, stats
from finstack_quant.core.money import Money

GOLDEN = Path(__file__).parent / "data" / "core_wasm_parity.json"


def _iso(value: datetime.date | None) -> str | None:
    return None if value is None else value.isoformat()


def _isos(values: list[datetime.date]) -> list[str]:
    return [value.isoformat() for value in values]


def _discount() -> DiscountCurve:
    return DiscountCurve.from_zero_rates(
        "USD-OIS", "2025-01-02", [(0.5, 0.04), (1.0, 0.042), (5.0, 0.045)], compounding="semi_annual"
    )


def _inflation() -> InflationCurve:
    return InflationCurve("US-CPI", "2025-01-02", 300.0, [(0.0, 300.0), (5.0, 331.2)], indexation_lag_months=3)


def _price() -> PriceCurve:
    return PriceCurve("WTI", "2025-01-02", [(0.0, 72.0), (1.0, 70.5)], spot_price=72.0)


def _base_correlation() -> BaseCorrelationCurve:
    return BaseCorrelationCurve("CDX-IG", [(3.0, 0.25), (7.0, 0.45)])


def _surface() -> VolSurface:
    return VolSurface("SPX-VOL", [0.5, 1.0], [90.0, 100.0, 110.0], [0.24, 0.2, 0.22, 0.23, 0.21, 0.22])


def _series() -> ScalarTimeSeries:
    return ScalarTimeSeries("SOFR-FIXINGS", [("2025-01-02", 0.0431), ("2025-01-06", 0.0433)], interpolation="linear")


def _index(lag: str = "3M") -> InflationIndex:
    return InflationIndex(
        "US-CPI-U",
        [("2024-10-01", 315.664), ("2024-11-01", 315.493), ("2024-12-01", 315.605)],
        "USD",
        interpolation="linear",
        lag=lag,
    )


def _fx() -> FxMatrix:
    return FxMatrix.from_dict({"EUR/USD": 1.1, "GBPUSD": 1.27})


def _market() -> MarketContext:
    market = MarketContext()
    market.insert(DiscountCurve.flat("USD-OIS", "2025-01-02", 0.04))
    market.insert(_inflation())
    market.insert(_price())
    market.insert(_base_correlation())
    market.insert(_surface())
    market.insert_fx(_fx())
    market.insert_price("SPX", 5900.0)
    market.insert_price("ACME", 12.5, "USD")
    market.insert_series(_series())
    market.insert_inflation_index(_index())
    # A credit index is persisted by reference, so its curves are stored too.
    hazard = HazardCurve.flat("CDX-IG-HZD", "2025-01-02", 0.01, 0.4)
    market.insert(hazard)
    market.insert_credit_index("CDX-IG-43", CreditIndexData(125, 0.4, hazard, _base_correlation()))
    market.map_collateral("USD-CSA", "USD-OIS")
    return market


def _schedule() -> dates.Schedule:
    return (
        dates.Schedule
        .builder("2025-01-15", "2026-01-15")
        .frequency("3M")
        .adjust_with("modified_following", "nyse")
        .payment_lag_days(2)
        .build()
    )


def _config() -> config.FinstackConfig:
    cfg = config.FinstackConfig("away_from_zero")
    cfg.set_output_scale("JPY", 2)
    cfg.set_extension("core.example.v1", {"flag": True, "levels": [1, 2]})
    return cfg


def _attributes() -> types.Attributes:
    attributes = types.Attributes()
    attributes.add_tag("energy")
    attributes.set_meta("sector", "utilities")
    return attributes


def _eigen() -> list[float]:
    values, _vectors = linalg.symmetric_eigen([[2.0, 0.0], [0.0, 5.0]])
    return sorted(values)


def _ledoit_wolf() -> dict[str, Any]:
    covariance, shrinkage = linalg.ledoit_wolf_shrinkage([[1.0, 1.0], [-1.0, -1.0], [2.0, -2.0], [-2.0, 2.0]])
    return {"covariance": [x for row in covariance for x in row], "shrinkage": shrinkage}


US_FEDERAL = dates.FiscalConfig.us_federal()
NYSE = dates.HolidayCalendar("nyse")
REGISTRY = rating_scales.embedded_registry()
SERIES = [100.0, 101.5, 99.75, 102.25, 103.0]

CASES: dict[str, Callable[[], Any]] = {
    # --- dates: IMM, CDS and option rules ---------------------------------
    "third_wednesday": lambda: _iso(dates.third_wednesday(3, 2025)),
    "third_friday": lambda: _iso(dates.third_friday(3, 2025)),
    "next_imm": lambda: _iso(dates.next_imm("2025-03-20")),
    "is_imm_date": lambda: [dates.is_imm_date("2025-03-19"), dates.is_imm_date("2025-03-20")],
    "is_cds_date": lambda: [dates.is_cds_date("2025-03-20"), dates.is_cds_date("2025-03-19")],
    "next_cds_date": lambda: _iso(dates.next_cds_date("2025-03-20")),
    "prev_cds_date": lambda: _iso(dates.prev_cds_date("2025-03-20")),
    "prev_cds_semiannual_roll": lambda: _iso(dates.prev_cds_semiannual_roll("2024-02-14")),
    "next_semiannual_cds_maturity": lambda: _iso(dates.next_semiannual_cds_maturity("2028-09-20")),
    "imm_option_expiry": lambda: _iso(dates.imm_option_expiry(3, 2025)),
    "next_imm_option_expiry": lambda: _iso(dates.next_imm_option_expiry("2025-03-14")),
    "next_equity_option_expiry": lambda: _iso(dates.next_equity_option_expiry("2025-03-21")),
    # --- dates: SIFMA ------------------------------------------------------
    "sifma_settlement_date": lambda: _iso(dates.sifma_settlement_date(3, 2027)),
    "sifma_settlement_date_uncovered": lambda: _iso(dates.sifma_settlement_date(3, 1990)),
    "sifma_settlement_date_for_class": lambda: _iso(
        dates.sifma_settlement_date_for_class(1, 2026, dates.SifmaSettlementClass.B)
    ),
    "estimated_sifma_settlement_date_for_class": lambda: _iso(
        dates.estimated_sifma_settlement_date_for_class(
            3, 2031, dates.SifmaSettlementClass.from_agency_term("GNMA", 30)
        )
    ),
    "next_sifma_settlement": lambda: _iso(dates.next_sifma_settlement("2027-03-16")),
    # --- dates: DateExt, calendars, day counts ---------------------------------
    "add_business_days": lambda: _iso(dates.add_business_days("2025-06-27", 3, "target2")),
    "add_weekdays": lambda: _iso(dates.add_weekdays("2025-06-27", -3)),
    "add_months": lambda: _iso(dates.add_months("2024-01-31", 1)),
    "end_of_month": lambda: _iso(dates.end_of_month("2024-02-15")),
    "is_weekend": lambda: [dates.is_weekend("2025-01-04"), dates.is_weekend("2025-01-06")],
    "quarter": lambda: dates.quarter("2025-08-15"),
    "fiscal_year": lambda: [dates.fiscal_year("2024-11-15", US_FEDERAL), dates.fiscal_year("2024-09-30", US_FEDERAL)],
    "months_until": lambda: dates.months_until("2020-01-15", "2022-03-10"),
    "days_since_epoch": lambda: dates.days_since_epoch("2025-01-02"),
    "days_30_360": lambda: [
        dates.days_30_360("2025-01-31", "2025-03-31", name) for name in ("us_sia", "isda", "european", "italian")
    ],
    "days_30e_360_isda": lambda: [dates.days_30e_360_isda("2025-01-31", "2025-02-28", flag) for flag in (False, True)],
    "holiday_calendar": lambda: {
        "code": dates.HolidayCalendar("NYSE + gblo").code,
        "is_holiday": NYSE.is_holiday("2025-01-01"),
        "is_business_day": NYSE.is_business_day("2025-01-06"),
        "count_business_days": NYSE.count_business_days("2025-01-01", "2025-02-01"),
        "metadata_id": NYSE.metadata.id,
    },
    # --- dates: periods and schedules -------------------------------------------
    "period_id": lambda: [
        dates.PeriodId.parse("2025M12").next().code,
        dates.PeriodId.quarter(2025, 1).prev().code,
        dates.PeriodId.parse("FY2025Q4").next_fiscal(US_FEDERAL).code,
        dates.PeriodId.week(2025, 5).kind.periods_per_year,
    ],
    "period_kind_prior_observation_date": lambda: _iso(dates.PeriodKind.QUARTERLY.prior_observation_date("2025-03-31")),
    "build_periods": lambda: json.loads(dates.build_periods("2025Q1..Q4", "2025Q2").to_json()),
    "build_fiscal_periods": lambda: json.loads(dates.build_fiscal_periods("FY2025Q1..Q2", US_FEDERAL).to_json()),
    "schedule": lambda: json.loads(_schedule().to_json()),
    "schedule_dates": lambda: _isos(_schedule().payment_dates),
    "schedule_cds_imm": lambda: _isos(
        dates.Schedule.builder("2025-03-20", "2026-03-20").frequency("3M").cds_imm().build().dates
    ),
    # --- math ---------------------------------------------------------------------
    "symmetric_eigen": _eigen,
    "ledoit_wolf_shrinkage": _ledoit_wolf,
    "linalg_constants": lambda: [linalg.SINGULAR_THRESHOLD, linalg.DIAGONAL_TOLERANCE, linalg.SYMMETRY_TOLERANCE],
    "mean_var": lambda: list(stats.mean_var(SERIES)),
    "stats_or_nan": lambda: [
        stats.mean_or_nan(SERIES),
        stats.sample_variance_or_nan(SERIES),
        stats.sample_std_or_nan(SERIES),
        stats.median_or_nan(SERIES),
        stats.quantile_linear_or_nan(SERIES, 0.3),
        stats.finite_min_or_nan(SERIES),
        stats.finite_max_or_nan(SERIES),
    ],
    "stats_empty_is_nan": lambda: [
        math.isnan(stats.mean_or_nan([])),
        math.isnan(stats.sample_variance_or_nan([1.0])),
        math.isnan(stats.finite_min_or_nan([float("nan")])),
    ],
    "finite_count": lambda: stats.finite_count([1.0, float("nan"), float("inf"), 2.0]),
    "log_returns": lambda: stats.log_returns(SERIES),
    "norm_with_params": lambda: [
        special_functions.norm_cdf_with_params(1.5, 1.0, 2.0),
        special_functions.norm_pdf_with_params(1.5, 1.0, 2.0),
    ],
    "student_t": lambda: [
        special_functions.student_t_cdf(1.2, 5.0),
        special_functions.student_t_inv_cdf(0.975, 5.0),
    ],
    # --- config, rating scales, types, money ----------------------------------------
    "finstack_config": lambda: json.loads(_config().to_json()),
    "finstack_config_scales": lambda: [
        _config().output_scale("JPY"),
        _config().output_scale("USD"),
        _config().ingest_scale("USD"),
        _config().rounding_mode.name,
        _config().extension_keys(),
    ],
    "rating_registry": lambda: {
        "default_scale_id": REGISTRY.default_scale_id(),
        "default_score": REGISTRY.default_scorecard_score(),
        "scale_ids": REGISTRY.scale_ids(),
        "policy": REGISTRY.unknown_scale_policy().name,
        "known": REGISTRY.is_known_rating_scale(REGISTRY.default_scale_id()),
        "scale": json.loads(REGISTRY.rating_scale(REGISTRY.default_scale_id()).to_json()),
        "extension_key": rating_scales.RATING_SCALES_EXTENSION_KEY,
        "from_config": rating_scales.registry_from_config(config.FinstackConfig()).default_scale_id(),
    },
    "credit_rating": lambda: [
        types.CreditRating("Baa3").name,
        types.CreditRating.BBB.notches_to("BB"),
        types.CreditRating.AA_PLUS.to_moodys_string(),
        types.CreditRating.BB_PLUS.is_investment_grade(),
        types.CreditRating.NR.is_speculative_grade(),
        types.CreditRating.D.is_default(),
        types.CreditRating.BBB_MINUS.to_json(),
    ],
    "identifiers": lambda: [
        types.CurveId("USD-OIS").to_json(),
        types.InstrumentId("BOND_A").as_str(),
        types.CurveId("").is_empty(),
    ],
    "attributes": lambda: {
        "json": json.loads(_attributes().to_json()),
        "matches": [_attributes().matches_selector("tag:energy"), _attributes().matches_selector("meta:sector=tech")],
        "items": [list(item) for item in _attributes().items()],
    },
    "rate_zero": lambda: [types.Rate.ZERO.as_decimal, types.Bps.ZERO.as_bp, types.Percentage.ZERO.as_percent],
    "money": lambda: [
        json.loads(Money.zero("USD").to_json()),
        list(Money.from_tuple((12.5, "EUR")).to_tuple()),
    ],
    "currency": lambda: [
        currency.Currency.from_numeric(840).code,
        currency.JPY.decimals,
        currency.TRY.numeric,
    ],
    # --- market data ------------------------------------------------------------------
    "discount_curve": lambda: json.loads(_discount().to_json()),
    "discount_curve_queries": lambda: [
        _discount().zero_annual(2.0),
        _discount().zero_rate(2.0, "semi_annual"),
        _discount().zero_rate_on_date("2027-01-02", "quarterly"),
        _discount().df_on_date_curve("2027-01-02"),
        _discount().df_between_dates("2026-01-02", "2027-01-02"),
        _discount().to_forward_curve("USD-FWD", 0.25).rate(1.0),
        _discount().day_count,
        _discount().interp_style,
        _discount().extrapolation,
        list(_discount().knots),
        list(_discount().dfs),
    ],
    "discount_curve_from_dates": lambda: json.loads(
        DiscountCurve.from_dates("USD-OIS", "2025-01-02", [("2026-01-02", 0.96), ("2030-01-02", 0.8)]).to_json()
    ),
    "inflation_curve": lambda: json.loads(_inflation().to_json()),
    "inflation_curve_queries": lambda: [
        _inflation().cpi(2.5),
        _inflation().cpi_on_date("2027-07-02"),
        _inflation().cpi_with_lag(2.5),
        _inflation().index_ratio(2.5),
        _inflation().inflation_rate(0.0, 5.0),
        _inflation().indexation_lag_months,
    ],
    "price_curve": lambda: json.loads(_price().to_json()),
    "price_curve_queries": lambda: [_price().price(0.5), _price().price_on_date("2025-07-02"), _price().kind],
    "base_correlation_curve": lambda: json.loads(_base_correlation().to_json()),
    "base_correlation": lambda: _base_correlation().correlation(5.0),
    "vol_surface": lambda: json.loads(_surface().to_json()),
    "vol_surface_queries": lambda: [
        _surface().vol(0.75, 100.0),
        list(_surface().grid_shape),
        _surface().secondary_axis,
        _surface().quote_type,
        _surface().interpolation_mode,
    ],
    "scalar_time_series": lambda: json.loads(_series().to_json()),
    "scalar_time_series_queries": lambda: [
        _series().value_on("2025-01-04"),
        _series().value_on_exact("2025-01-06"),
        _iso(_series().first_date),
        _iso(_series().last_date),
    ],
    "inflation_index": lambda: json.loads(_index().to_json()),
    "inflation_index_queries": lambda: [
        _index("none").ratio("2024-10-01", "2024-12-01"),
        _index("none").value_on("2024-11-16"),
        _index().ref_cpi_months_lag("2025-02-15", 3),
        _isos(list(_index().date_range())),
        _index().lag,
    ],
    "fx_matrix": lambda: [
        _fx().rate("EUR", "USD", "2025-01-02").rate,
        _fx().rate("USD", "GBP", "2025-01-02").rate,
    ],
    "market_context": lambda: json.loads(_market().to_json()),
    "market_context_queries": lambda: {
        "curve_ids": _market().curve_ids(),
        "stats": _market().stats(),
        "contains": [_market().contains("USD-OIS"), _market().contains("missing")],
        "is_empty": [_market().is_empty(), MarketContext().is_empty()],
        "df": _market().get_discount("USD-OIS").df(1.0),
        "vol": _market().get_surface("SPX-VOL").vol(0.75, 100.0),
        "series": _market().get_series("SOFR-FIXINGS").value_on("2025-01-06"),
        "index_lag": _market().get_inflation_index("US-CPI-U").lag,
        "credit_index": _market().get_credit_index("CDX-IG-43").num_constituents,
        "fx_rate": _market().fx_required().rate("EUR", "USD", "2025-01-02").rate,
        "converted": list(_market().convert_money(Money(100.0, "EUR"), "USD", "2025-01-02").to_tuple()),
        "rolled_base_date": _iso(_market().roll_forward(30).get_discount("USD-OIS").base_date),
        "round_trip": json.loads(MarketContext.from_json(_market().to_json()).to_json())
        == json.loads(_market().to_json()),
    },
}


def _close(actual: Any, expected: Any, path: str) -> None:
    if isinstance(expected, float) or (isinstance(actual, float) and isinstance(expected, int)):
        assert actual == pytest.approx(expected, rel=1e-12, abs=1e-14), path
    elif isinstance(expected, dict):
        assert isinstance(actual, dict), path
        assert sorted(actual) == sorted(expected), path
        for key, value in expected.items():
            _close(actual[key], value, f"{path}.{key}")
    elif isinstance(expected, list):
        assert isinstance(actual, (list, tuple)), path
        assert len(actual) == len(expected), path
        for index, value in enumerate(expected):
            _close(actual[index], value, f"{path}[{index}]")
    else:
        assert actual == expected, path


def test_golden_lists_exactly_these_cases() -> None:
    if os.environ.get("UPDATE_CORE_WASM_PARITY"):
        GOLDEN.write_text(json.dumps({name: case() for name, case in CASES.items()}, indent=2, sort_keys=True) + "\n")
    assert sorted(json.loads(GOLDEN.read_text())) == sorted(CASES)


@pytest.mark.parametrize("name", sorted(CASES))
def test_python_matches_the_cross_host_golden(name: str) -> None:
    _close(CASES[name](), json.loads(GOLDEN.read_text())[name], name)


def test_fx_pair_keys_are_parsed_by_rust() -> None:
    """F350: a multi-byte pair key is a ``ValueError``, never a panic."""
    with pytest.raises(ValueError, match="invalid FX pair"):
        FxMatrix.from_dict({"é€x": 1.0})
    with pytest.raises(ValueError, match="Invalid currency code"):
        FxMatrix.from_dict({"EUR/XYZ": 1.0})


def test_unknown_names_raise_the_rust_error_kind() -> None:
    with pytest.raises(KeyError):
        dates.HolidayCalendar("not-a-calendar")
    with pytest.raises(KeyError):
        _market().get_discount("missing")
    with pytest.raises(ValueError, match="Invalid price curve kind"):
        PriceCurve("X", "2025-01-02", [(0.0, 1.0), (1.0, 2.0)], kind="Price")
    with pytest.raises(ValueError, match="monthly index must be in 1"):
        dates.PeriodId.month(2025, 13)
    with pytest.raises(ValueError, match="std_dev must be finite and positive"):
        special_functions.norm_cdf_with_params(0.0, 0.0, -1.0)


@pytest.mark.parametrize(
    "selector",
    [
        "period_plan.schema.json",
        "schedule_spec.schema.json",
        "schedule.schema.json",
        "scorecard_scale.schema.json",
    ],
)
def test_core_schemas_behind_the_wasm_types_publish_a_valid_example(selector: str) -> None:
    """The schemas that generate the TypeScript twins carry an example that validates."""
    from finstack_quant.core import schema

    examples = json.loads(schema.get(selector))["examples"]
    assert examples
    for example in examples:
        assert json.loads(schema.validate(selector, json.dumps(example))) == []


def test_period_plan_and_schedule_json_validate_against_their_schemas() -> None:
    from finstack_quant.core import schema

    plan = dates.build_periods("2025Q1..Q4", "2025Q2").to_json()
    assert json.loads(schema.validate("period_plan.schema.json", plan)) == []
    assert json.loads(schema.validate("schedule.schema.json", _schedule().to_json())) == []
    spec = json.dumps(dates.Schedule.builder("2025-01-15", "2026-01-15").frequency("3M").to_spec())
    assert json.loads(schema.validate("schedule_spec.schema.json", spec)) == []
    scale = REGISTRY.rating_scale("moodys").to_json()
    assert json.loads(schema.validate("scorecard_scale.schema.json", scale)) == []
