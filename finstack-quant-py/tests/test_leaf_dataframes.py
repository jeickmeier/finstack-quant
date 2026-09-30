"""Tests for the pandas ``DataFrame`` accessors on leaf result types.

Covers the newly added exports across five namespaces:

- ``analytics``: ``BetaResult.to_dataframe``, ``GreeksResult.to_dataframe``,
  ``MultiFactorResult.to_dataframe``.
- ``models.credit``: ``pd.MasterScaleResult.to_dataframe``,
  ``recovery_waterfall.RecoveryWaterfallResult.to_dataframe``,
  ``liability_management.ExchangeOfferAnalysis.to_dataframe`` and
  ``LmeAnalysis.to_dataframe``.
- ``core.market_data``: ``fx.FxRateResult.to_dataframe``,
  ``scalars.ScalarTimeSeries.to_dataframe``.
- ``models.factor.credit``: ``PeriodDecomposition.to_level_dataframe`` and
  ``to_adder_dataframe``.

Everything is built through public constructors and calculators, so the tests
stay self-contained.
"""

from __future__ import annotations

from calendar import monthrange
import datetime
from datetime import date as dt_date, timedelta
import json
import math

import pandas as pd
import pytest

from finstack_quant.analytics import MultiFactorResult, Performance
from finstack_quant.core.market_data import FxMatrix, ScalarTimeSeries
from finstack_quant.models.credit import liability_management, pd as credit_pd, recovery_waterfall
from finstack_quant.models.factor.credit import (
    CreditCalibrator,
    PeriodDecomposition,
    decompose_levels,
    decompose_period,
)

# analytics


def _benchmarked_performance() -> Performance:
    """A noisy 2x beta, so every regression statistic takes a different value.

    A perfectly collinear pair (``fund == 2 * benchmark``) collapses the frame:
    ``beta == ci_lower == ci_upper == 2.0``, ``r_squared == adjusted_r_squared
    == 1.0`` and ``std_err == alpha == 0``. A column-swapped export would then
    compare equal. Adding a small idiosyncratic residual separates all six.
    """
    dates = [dt_date(2024, 1, 1) + timedelta(days=i) for i in range(8)]
    benchmark = [-0.02, -0.01, 0.0, 0.01, 0.02, 0.03, 0.015, -0.005]
    residual = [0.004, -0.003, 0.006, -0.002, 0.001, -0.005, 0.003, -0.004]
    fund = [2.0 * value + noise for value, noise in zip(benchmark, residual, strict=True)]
    return Performance.from_returns_arrays(
        dates,
        [fund, benchmark],
        ["FUND", "BENCH"],
        benchmark_ticker="BENCH",
        frequency="monthly",
    )


def test_beta_result_to_dataframe_is_one_row() -> None:
    result = _benchmarked_performance().beta()[0]
    df = result.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == ["beta", "std_err", "ci_lower", "ci_upper"]
    row = df.iloc[0]
    # Each cell against its own accessor, not against a shared constant.
    assert row["beta"] == pytest.approx(result.beta)
    assert row["std_err"] == pytest.approx(result.std_err)
    assert row["ci_lower"] == pytest.approx(result.ci_lower)
    assert row["ci_upper"] == pytest.approx(result.ci_upper)
    # ... and the four accessors are themselves four different numbers.
    assert len({row["beta"], row["std_err"], row["ci_lower"], row["ci_upper"]}) == 4
    assert row["ci_lower"] < row["beta"] < row["ci_upper"]
    assert row["std_err"] > 0.0


def test_greeks_result_to_dataframe_is_one_row() -> None:
    result = _benchmarked_performance().greeks()[0]
    df = result.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == ["alpha", "beta", "r_squared", "adjusted_r_squared"]
    row = df.iloc[0]
    assert row["alpha"] == pytest.approx(result.alpha)
    assert row["beta"] == pytest.approx(result.beta)
    assert row["r_squared"] == pytest.approx(result.r_squared)
    assert row["adjusted_r_squared"] == pytest.approx(result.adjusted_r_squared)
    assert len({row["alpha"], row["beta"], row["r_squared"], row["adjusted_r_squared"]}) == 4
    # An imperfect fit: adjusted R^2 is strictly penalised below R^2.
    assert row["adjusted_r_squared"] < row["r_squared"] < 1.0
    assert row["alpha"] != 0.0


_FACTOR_A = [-0.02, -0.01, 0.0, 0.01, 0.02, 0.03, 0.015, -0.005]
_FACTOR_B = [0.01, 0.0, -0.01, 0.02, 0.0, 0.01, -0.02, 0.005]


def _multi_factor_result(swap_factors: bool = False) -> MultiFactorResult:
    """Two-factor regression with betas 2.0 and 0.5, optionally supplied swapped."""
    dates = [dt_date(2024, 1, 1) + timedelta(days=i) for i in range(8)]
    returns = [2.0 * a + 0.5 * b for a, b in zip(_FACTOR_A, _FACTOR_B, strict=True)]
    perf = Performance.from_returns_arrays(dates, [returns], ["FUND"], frequency="monthly")
    factors = [_FACTOR_B, _FACTOR_A] if swap_factors else [_FACTOR_A, _FACTOR_B]
    return perf.multi_factor_greeks(0, factors)


def test_multi_factor_result_to_dataframe_is_one_row_per_factor() -> None:
    result = _multi_factor_result()
    df = result.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == len(result.betas)
    assert list(df.columns) == [
        "factor",
        "beta",
        "alpha",
        "r_squared",
        "adjusted_r_squared",
        "residual_vol",
    ]
    assert list(df["factor"]) == ["factor_0", "factor_1"]
    assert list(df["beta"]) == pytest.approx(list(result.betas))
    # The two loadings are the two different numbers the regression was built
    # from, so a frame that repeated one row would fail here.
    assert list(df["beta"]) == pytest.approx([2.0, 0.5])
    # Regression-level statistics repeat on every row.
    assert list(df["r_squared"]) == pytest.approx([result.r_squared] * len(df))
    assert list(df["alpha"]) == pytest.approx([result.alpha] * len(df))


def test_multi_factor_result_to_dataframe_accepts_factor_names() -> None:
    result = _multi_factor_result()
    df = result.to_dataframe(["value", "momentum"])
    assert list(df["factor"]) == ["value", "momentum"]


def test_multi_factor_result_to_dataframe_rejects_mismatched_names() -> None:
    result = _multi_factor_result()
    with pytest.raises(ValueError, match="factor_names"):
        result.to_dataframe(["only_one"])


def test_multi_factor_result_rows_follow_the_supplied_factor_order() -> None:
    """Row order is the caller's factor order, and each beta travels with it.

    This replaces an ``assert_frame_equal(f(x), f(x))`` on a frozen object,
    which could only have failed on intra-process nondeterminism. Supplying the
    same two factors in the opposite order is the assertion with content: the
    rows permute and the loadings follow, rather than the frame silently
    re-sorting or pinning the label to the wrong row.
    """
    forward = _multi_factor_result().to_dataframe()
    swapped = _multi_factor_result(swap_factors=True).to_dataframe()

    assert list(forward["factor"]) == list(swapped["factor"]) == ["factor_0", "factor_1"]
    assert list(forward["beta"]) == pytest.approx([2.0, 0.5])
    assert list(swapped["beta"]) == pytest.approx([0.5, 2.0])
    # The regression itself is unchanged: only the row order moved.
    assert forward["r_squared"].iloc[0] == pytest.approx(swapped["r_squared"].iloc[0])
    assert forward["alpha"].iloc[0] == pytest.approx(swapped["alpha"].iloc[0])


# models.credit.pd


def test_master_scale_result_to_dataframe_is_one_row() -> None:
    result = credit_pd.MasterScale.sp_assumptions().map_pd(0.003)
    df = result.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == ["grade", "grade_index", "input_pd", "central_pd"]
    assert df["grade"].iloc[0] == result.grade
    assert df["grade_index"].iloc[0] == result.grade_index
    assert df["input_pd"].iloc[0] == pytest.approx(0.003)
    assert df["central_pd"].iloc[0] == pytest.approx(result.central_pd)


def test_master_scale_results_concat_into_a_grading_table() -> None:
    scale = credit_pd.MasterScale.sp_assumptions()
    pds = [0.0005, 0.003, 0.05]
    table = pd.concat([scale.map_pd(value).to_dataframe() for value in pds], ignore_index=True)
    assert len(table) == len(pds)
    assert list(table["input_pd"]) == pytest.approx(pds)


# models.credit.recovery_waterfall

_ALLOCATION_COLUMNS = [
    "id",
    "seniority",
    "priority",
    "total_claim",
    "collateral_recovery",
    "general_recovery",
    "total_recovery",
    "recovery_rate",
    "deficiency",
]


def _claims() -> list[recovery_waterfall.RecoveryClaim]:
    """Three claims of different size, supplied out of priority order."""
    return [
        recovery_waterfall.RecoveryClaim("SUB", "subordinated", 3, 100.0),
        recovery_waterfall.RecoveryClaim("SEN", "senior_secured", 1, 100.0),
        recovery_waterfall.RecoveryClaim("MEZZ", "senior_unsecured", 2, 50.0),
    ]


def _waterfall() -> recovery_waterfall.RecoveryWaterfallResult:
    return recovery_waterfall.allocate_recovery(120.0, _claims())


def test_recovery_waterfall_to_dataframe_is_one_row_per_claim() -> None:
    result = _waterfall()
    df = result.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == len(result.allocations) == 3
    assert set(_ALLOCATION_COLUMNS) <= set(df.columns)


def test_recovery_waterfall_to_dataframe_is_ordered_by_priority() -> None:
    df = _waterfall().to_dataframe()
    assert list(df["priority"]) == sorted(df["priority"]) == [1, 2, 3]
    assert list(df["id"]) == ["SEN", "MEZZ", "SUB"]
    # 120 of value against 250 of claims: senior is made whole, mezz takes the
    # remainder, sub recovers nothing. Distinct per-row values, so a frame that
    # replicated one allocation across all three rows fails here.
    assert list(df["total_recovery"]) == pytest.approx([100.0, 20.0, 0.0])
    assert list(df["recovery_rate"]) == pytest.approx([1.0, 0.4, 0.0])
    assert list(df["deficiency"]) == pytest.approx([0.0, 30.0, 100.0])


def test_recovery_waterfall_to_dataframe_keeps_schema_when_empty() -> None:
    result = recovery_waterfall.allocate_recovery(100.0, [])
    df = result.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 0
    assert list(df.columns) == _ALLOCATION_COLUMNS


def test_recovery_waterfall_ordering_ignores_the_input_order() -> None:
    """The same claims in a different order produce the identical frame.

    That is what the priority sort actually guarantees. Re-exporting one frozen
    result twice, which is what this test used to do, can only fail on
    intra-process nondeterminism.
    """
    forward = recovery_waterfall.allocate_recovery(120.0, _claims()).to_dataframe()
    reversed_input = recovery_waterfall.allocate_recovery(120.0, list(reversed(_claims()))).to_dataframe()
    pd.testing.assert_frame_equal(forward, reversed_input)
    assert list(forward["id"]) == ["SEN", "MEZZ", "SUB"]


# models.credit.liability_management


def test_exchange_offer_analysis_to_dataframe_is_one_row() -> None:
    analysis = liability_management.analyze_exchange_offer(60.0, 75.0, 2.0, 0.0, "par_for_par")
    df = analysis.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == [
        "exchange_type",
        "old_npv",
        "new_npv",
        "consent_fee",
        "equity_sweetener_value",
        "tender_total",
        "delta_npv",
        "breakeven_recovery",
        "tender_recommended",
    ]
    row = df.iloc[0]
    assert row["exchange_type"] == analysis.exchange_type
    # Distinct per-column values, so a shuffled schema cannot slip through.
    assert row["old_npv"] == pytest.approx(60.0)
    assert row["new_npv"] == pytest.approx(75.0)
    assert row["consent_fee"] == pytest.approx(2.0)
    assert row["tender_total"] == pytest.approx(analysis.tender_total) == pytest.approx(77.0)
    assert row["delta_npv"] == pytest.approx(analysis.delta_npv) == pytest.approx(17.0)
    assert bool(row["tender_recommended"]) is analysis.tender_recommended


_LME_COLUMNS = [
    "lme_type",
    "cost",
    "notional_reduction",
    "discount_capture",
    "discount_capture_pct",
    "remaining_holder_impact_pct",
    "pre_total_debt",
    "post_total_debt",
    "pre_leverage",
    "post_leverage",
    "leverage_reduction",
]


def test_lme_analysis_to_dataframe_flattens_leverage_impact() -> None:
    analysis = liability_management.analyze_lme("open_market_repurchase", 100.0, 0.70, 0.50, ebitda=12.5)
    df = analysis.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == _LME_COLUMNS
    impact = analysis.leverage_impact
    assert impact is not None
    assert df["pre_leverage"].iloc[0] == pytest.approx(impact.pre_leverage)
    assert df["leverage_reduction"].iloc[0] == pytest.approx(impact.leverage_reduction)
    assert df["notional_reduction"].iloc[0] == pytest.approx(analysis.notional_reduction)


def test_lme_analysis_to_dataframe_nulls_leverage_without_ebitda() -> None:
    analysis = liability_management.analyze_lme("open_market_repurchase", 100.0, 0.70, 0.50)
    df = analysis.to_dataframe()
    assert len(df) == 1
    assert list(df.columns) == _LME_COLUMNS
    assert analysis.leverage_impact is None
    assert df["pre_leverage"].isna().all()
    assert df["leverage_reduction"].isna().all()


# core.market_data.fx


def test_fx_rate_result_to_dataframe_is_one_row() -> None:
    matrix = FxMatrix()
    matrix.set_quote("EUR", "USD", 1.1)
    result = matrix.rate("EUR", "USD", datetime.date(2025, 1, 1))
    df = result.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == ["rate", "triangulated"]
    assert df["rate"].iloc[0] == pytest.approx(result.rate)
    assert bool(df["triangulated"].iloc[0]) is result.triangulated


# core.market_data.scalars


def test_scalar_time_series_to_dataframe_is_date_indexed() -> None:
    observations = [
        (datetime.date(2025, 1, 3), 0.04),
        (datetime.date(2025, 1, 1), 0.03),
        (datetime.date(2025, 1, 2), 0.035),
    ]
    series = ScalarTimeSeries("SOFR", observations)
    df = series.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["value"]
    assert len(df) == len(series.observations)
    assert isinstance(df.index, pd.DatetimeIndex)
    # Rows are chronological, not insertion order.
    assert list(df.index) == [pd.Timestamp(d) for d, _ in sorted(observations)]
    assert list(df["value"]) == pytest.approx([0.03, 0.035, 0.04])


def test_scalar_time_series_ordering_ignores_the_insertion_order() -> None:
    """Two insertion orders of the same observations give the identical frame.

    That is the content behind the chronological guarantee. Exporting one
    frozen series twice — the previous assertion — could only fail on
    intra-process nondeterminism.
    """
    observations = [
        (datetime.date(2025, 1, 1), 0.03),
        (datetime.date(2025, 1, 2), 0.035),
        (datetime.date(2025, 1, 3), 0.04),
    ]
    chronological = ScalarTimeSeries("SOFR", observations).to_dataframe()
    shuffled = ScalarTimeSeries("SOFR", list(reversed(observations))).to_dataframe()
    pd.testing.assert_frame_equal(chronological, shuffled)
    assert list(chronological["value"]) == pytest.approx([0.03, 0.035, 0.04])


# models.factor.credit

_LEVEL_DELTA_COLUMNS = [
    "from_date",
    "to_date",
    "level_index",
    "dimension",
    "bucket",
    "delta",
]
_ADDER_DELTA_COLUMNS = ["from_date", "to_date", "issuer_id", "d_adder"]


def _add_months_eom(origin: dt_date, months: int) -> dt_date:
    year = origin.year + (origin.month - 1 + months) // 12
    month = (origin.month - 1 + months) % 12 + 1
    last = monthrange(year, month)[1]
    origin_eom = origin.day == monthrange(origin.year, origin.month)[1]
    day = last if origin_eom else min(origin.day, last)
    return dt_date(year, month, day)


def _monthly_dates(n: int, end: dt_date) -> list[str]:
    origin = _add_months_eom(end, -(n - 1))
    return [_add_months_eom(origin, i).isoformat() for i in range(n)]


def _hierarchy_calibration_inputs() -> dict:
    """Synthetic 24-month panel with 6 issuers over a rating x region hierarchy.

    Mirrors the fixture in ``test_credit_factor_model_bindings.py`` so the
    calibration is known to converge for a 2-level hierarchy.
    """
    n = 24
    dates = _monthly_dates(n, dt_date(2024, 3, 31))
    generic_values = [0.0100 + 0.00005 * math.sin(i) for i in range(n)]

    issuer_specs = [
        ("ISSUER-A", "IG", "EU"),
        ("ISSUER-B", "IG", "NA"),
        ("ISSUER-C", "IG", "APAC"),
        ("ISSUER-D", "HY", "EU"),
        ("ISSUER-E", "HY", "NA"),
        ("ISSUER-F", "HY", "APAC"),
    ]

    spreads: dict[str, list[float]] = {}
    tags: dict[str, dict[str, str]] = {}
    as_of_spreads: dict[str, float] = {}
    for idx, (issuer_id, rating, region) in enumerate(issuer_specs):
        base = 0.0100 + idx * 0.0025
        beta_pc = 0.7 + 0.05 * idx
        series = [base + beta_pc * (generic_values[i] - 0.0100) + 0.00001 * math.cos(idx + i * 0.5) for i in range(n)]
        spreads[issuer_id] = series
        tags[issuer_id] = {"rating": rating, "region": region}
        as_of_spreads[issuer_id] = float(series[-1])

    return {
        "history_panel": {"dates": dates, "spreads": spreads},
        "issuer_tags": {"tags": tags},
        "generic_factor": {
            "spec": {"name": "CDX IG 5Y", "series_id": "cdx.ig.5y"},
            "values": generic_values,
        },
        "as_of": "2024-03-31",
        "as_of_spreads": as_of_spreads,
        "idiosyncratic_overrides": {},
    }


def _hierarchy_period_decomposition() -> PeriodDecomposition:
    config = {
        "policy": "globally_off",
        "hierarchy": {"levels": ["rating", "region"]},
        # Level-1 threshold of 1 keeps the singleton rating x region buckets:
        # fold-up now counts every bucket member (it was previously inert
        # under globally_off), and this fixture wants all 6 level-1 buckets
        # to survive so the long dataframe shape stays interesting.
        "min_bucket_size_per_level": {"per_level": [3, 1]},
        "vol_model": "sample",
        "covariance_strategy": "diagonal",
        "beta_shrinkage": "none",
        "use_returns_or_levels": "returns",
        "panel_frequency": "monthly",
        "bucket_weighting": "equal",
    }
    inputs = _hierarchy_calibration_inputs()
    model = CreditCalibrator(json.dumps(config)).calibrate(json.dumps(inputs))
    spreads_json = json.dumps(inputs["as_of_spreads"])
    start = decompose_levels(model, spreads_json, 0.0100, "2024-02-29")
    end = decompose_levels(model, spreads_json, 0.01015, "2024-03-31")
    return decompose_period(start, end)


def _flat_period_decomposition(issuers: tuple[str, str] = ("ZEBRA", "ALPHA")) -> PeriodDecomposition:
    """A level-free hierarchy, so ``to_level_dataframe`` has nothing to emit.

    ``issuers`` fixes the *declaration* order of every issuer-keyed map. The
    default declares ``ZEBRA`` first so the alphabetical export order is not
    simply the input order.
    """
    first, second = issuers
    series = {"ZEBRA": [0.0100, 0.0101], "ALPHA": [0.0090, 0.0091]}
    as_of = {"ZEBRA": 0.0101, "ALPHA": 0.0091}
    start_spreads = {"ZEBRA": 0.0105, "ALPHA": 0.0095}
    end_spreads = {"ZEBRA": 0.01065, "ALPHA": 0.0096}

    def ordered(values: dict[str, object]) -> dict[str, object]:
        return {first: values[first], second: values[second]}

    config = {
        "policy": "globally_off",
        "hierarchy": {"levels": []},
        "min_bucket_size_per_level": {"per_level": []},
        "vol_model": "sample",
        "covariance_strategy": "diagonal",
        "beta_shrinkage": "none",
        "use_returns_or_levels": "returns",
        "panel_frequency": "monthly",
        "bucket_weighting": "equal",
    }
    inputs = {
        "history_panel": {
            "dates": ["2024-01-01", "2024-02-01"],
            "spreads": ordered(series),
        },
        "issuer_tags": {"tags": ordered({"ZEBRA": {}, "ALPHA": {}})},
        "generic_factor": {
            "spec": {"name": "G", "series_id": "G"},
            "values": [0.0100, 0.0101],
        },
        "as_of": "2024-02-01",
        "as_of_spreads": ordered(as_of),
        "idiosyncratic_overrides": {},
    }
    model = CreditCalibrator(json.dumps(config)).calibrate(json.dumps(inputs))
    start = decompose_levels(model, json.dumps(ordered(start_spreads)), 0.0100, "2024-03-01")
    end = decompose_levels(model, json.dumps(ordered(end_spreads)), 0.01015, "2024-03-02")
    return decompose_period(start, end)


def test_period_decomposition_level_dataframe_is_long_and_sorted() -> None:
    period = _hierarchy_period_decomposition()
    df = period.to_level_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert set(_LEVEL_DELTA_COLUMNS) <= set(df.columns)

    expected_rows = sum(len(period.level_deltas(k)) for k in range(period.n_levels))
    assert len(df) == expected_rows
    # 2 rating buckets + 6 rating x region buckets. Asserted rather than
    # skipped on: a regression to zero rows would otherwise pass silently.
    assert expected_rows == 8

    assert list(df["level_index"]) == sorted(df["level_index"]) == [0, 0, 1, 1, 1, 1, 1, 1]
    assert list(df["dimension"]) == ["Rating"] * 2 + ["Region"] * 6
    assert list(df["bucket"]) == [
        "HY",
        "IG",
        "HY.APAC",
        "HY.EU",
        "HY.NA",
        "IG.APAC",
        "IG.EU",
        "IG.NA",
    ]
    for level_index, group in df.groupby("level_index"):
        deltas = period.level_deltas(int(level_index))
        # Both the labels and their values, in the map's own key order.
        assert list(group["bucket"]) == list(deltas)
        assert list(group["delta"]) == pytest.approx([deltas[bucket] for bucket in group["bucket"]])

    assert set(df["from_date"]) == {period.from_date}
    assert set(df["to_date"]) == {period.to_date}


def test_period_decomposition_level_dataframe_keeps_schema_when_no_levels() -> None:
    period = _flat_period_decomposition()
    assert period.n_levels == 0
    df = period.to_level_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 0
    assert list(df.columns) == _LEVEL_DELTA_COLUMNS


def test_period_decomposition_adder_dataframe_is_sorted_by_issuer() -> None:
    period = _flat_period_decomposition()
    adders = period.d_adder()
    df = period.to_adder_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert set(_ADDER_DELTA_COLUMNS) <= set(df.columns)
    assert len(df) == len(adders) == 2
    # "ALPHA" was declared after "ZEBRA"; the export is key-sorted, so the
    # literal below is not the input order.
    assert list(df["issuer_id"]) == ["ALPHA", "ZEBRA"]
    assert list(df["d_adder"]) == pytest.approx([adders["ALPHA"], adders["ZEBRA"]])
    # The two adders differ, so a frame that mislabelled them would fail.
    assert adders["ALPHA"] != adders["ZEBRA"]
    assert set(df["from_date"]) == {period.from_date}


def test_period_decomposition_adder_order_ignores_the_declaration_order() -> None:
    """Declaring the issuers the other way round yields the identical frame.

    That is what the key sort guarantees. Exporting one frozen decomposition
    twice — the previous assertion — could only fail on intra-process
    nondeterminism.
    """
    zebra_first = _flat_period_decomposition(("ZEBRA", "ALPHA")).to_adder_dataframe()
    alpha_first = _flat_period_decomposition(("ALPHA", "ZEBRA")).to_adder_dataframe()
    pd.testing.assert_frame_equal(zebra_first, alpha_first)
    assert list(zebra_first["issuer_id"]) == ["ALPHA", "ZEBRA"]
