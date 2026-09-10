"""Tests for the `margin` pandas ``DataFrame`` accessors.

Covers the newly added exports: `VmResult.to_dataframe`,
`ImResult.to_dataframe` / `to_breakdown_dataframe`, the three metric types,
and the two long-format sensitivity exports
(`FrtbSensitivities` / `SimmSensitivities`).

Everything is built through public constructors and calculators, so the tests
stay self-contained.
"""

from __future__ import annotations

import datetime as dt
import json

import pandas as pd
import pytest

from finstack_quant.margin import (
    CsaSpec,
    ExcessCollateral,
    FrtbSensitivities,
    ImResult,
    MarginFundingCost,
    MarginUtilization,
    ScheduleImCalculator,
    SimmCalculator,
    SimmCurvatureSensitivity,
    SimmSensitivities,
    VmCalculator,
    VmResult,
)

SENSITIVITY_COLUMNS = ["risk_class", "bucket", "tenor", "issuer", "kind", "amount"]


def _sort_keys(df: pd.DataFrame) -> list[tuple[object, ...]]:
    """Reproduce the Rust row ordering key for the long-format exports.

    Rust sorts on ``(risk_class, kind, issuer, bucket, tenor)`` where the last
    three are ``Option<String>`` and ``None`` sorts before any value. Missing
    values are therefore mapped to a ``(0, "")`` prefix rather than compared as
    the literal string ``"None"``.
    """

    def optional(value: object) -> tuple[int, str]:
        return (1, str(value)) if pd.notna(value) else (0, "")

    return [
        (
            str(row["risk_class"]),
            str(row["kind"]),
            optional(row["issuer"]),
            optional(row["bucket"]),
            optional(row["tenor"]),
        )
        for _, row in df.iterrows()
    ]


# VmResult


VM_COLUMNS = [
    "date",
    "settlement_date",
    "gross_exposure",
    "net_exposure",
    "post_amount",
    "collect_amount",
    "net_margin",
    "requires_call",
    "currency",
]


def _threshold_csa() -> CsaSpec:
    """A CSA with a real threshold and independent amount.

    ``CsaSpec.usd_regulatory`` has a zero threshold and zero IA, which makes
    ``net_exposure`` identical to ``gross_exposure``. Two columns holding the
    same number cannot detect a swap between them, so this fixture separates
    them: net = max(gross - threshold, 0) + IA.
    """
    spec = json.loads(CsaSpec.usd_regulatory().to_json())
    spec["id"] = "USD-THRESHOLD-CSA"
    spec["vm_params"]["threshold"]["amount"] = "300000"
    spec["vm_params"]["independent_amount"]["amount"] = "100000"
    spec["vm_params"]["mta"]["amount"] = "50000"
    return CsaSpec.from_json(json.dumps(spec))


def _vm_result() -> VmResult:
    return VmCalculator(_threshold_csa()).calculate(1_000_000.0, 250_000.0, "USD", "2025-06-30")


def test_vm_result_to_dataframe_is_one_row() -> None:
    result = _vm_result()
    df = result.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == VM_COLUMNS
    row = df.iloc[0]
    assert row["currency"] == "USD"
    assert row["date"] == "2025-06-30"
    assert result.date == dt.date(2025, 6, 30)
    assert row["settlement_date"] == result.settlement_date.isoformat()
    assert result.settlement_date > result.date
    assert row["gross_exposure"] == pytest.approx(result.gross_exposure) == pytest.approx(1_000_000.0)
    # 1,000,000 - 300,000 threshold + 100,000 IA: distinct from the gross.
    assert row["net_exposure"] == pytest.approx(result.net_exposure) == pytest.approx(800_000.0)
    assert row["net_exposure"] != row["gross_exposure"]
    # 800,000 required against 250,000 posted.
    assert row["collect_amount"] == pytest.approx(result.collect_amount) == pytest.approx(550_000.0)
    assert row["post_amount"] == pytest.approx(result.post_amount) == pytest.approx(0.0)
    assert row["net_margin"] == pytest.approx(result.net_margin) == pytest.approx(-550_000.0)
    assert bool(row["requires_call"]) == result.requires_call is True


def test_vm_result_to_dataframe_renders_a_collateral_return() -> None:
    """The mirror case, so ``collect_amount`` and ``post_amount`` differ.

    A single call direction leaves one of the two at zero and makes
    ``net_margin`` equal the other, so the two legs are only distinguishable
    across a pair of results.
    """
    result = VmCalculator(_threshold_csa()).calculate(1_000_000.0, 1_200_000.0, "USD", dt.date(2025, 6, 30))
    row = result.to_dataframe().iloc[0]

    assert row["gross_exposure"] == pytest.approx(1_000_000.0)
    assert row["net_exposure"] == pytest.approx(800_000.0)
    assert row["collect_amount"] == pytest.approx(0.0)
    assert row["post_amount"] == pytest.approx(400_000.0)
    assert row["net_margin"] == pytest.approx(400_000.0)
    assert row["collect_amount"] != row["post_amount"]


# ImResult


def _im_result() -> ImResult:
    sensitivities = SimmSensitivities("USD")
    sensitivities.add_ir_delta("USD", "5Y", 100_000.0)
    sensitivities.add_equity_delta("AAPL", 250_000.0)
    sensitivities.add_fx_delta("EUR", 75_000.0)
    calculator = SimmCalculator("v2_6")
    return calculator.calculate_from_sensitivities(sensitivities, "USD", "2025-06-30")


def test_im_result_to_dataframe_is_one_row() -> None:
    result = _im_result()
    df = result.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == [
        "amount",
        "currency",
        "methodology",
        "mpor_days",
        "as_of",
        "approximation",
    ]
    row = df.iloc[0]
    assert row["amount"] == pytest.approx(result.amount)
    assert row["currency"] == "USD"
    assert row["as_of"] == "2025-06-30"
    assert row["mpor_days"] == result.mpor_days
    assert bool(row["approximation"]) == result.approximation


def test_im_result_breakdown_dataframe_is_sorted_by_risk_class() -> None:
    result = _im_result()
    df = result.to_breakdown_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["risk_class", "amount", "currency"]
    assert len(df) == len(result.breakdown_keys()) == 3
    risk_classes = list(df["risk_class"])
    # The canonical BTreeMap contract makes both access paths deterministic.
    assert result.breakdown_keys() == ["Equity_Delta", "FX_Delta", "IR_Delta"]
    assert risk_classes == ["Equity_Delta", "FX_Delta", "IR_Delta"]
    assert risk_classes == sorted(risk_classes), "breakdown must be deterministic"
    assert set(risk_classes) == set(result.breakdown_keys())
    # Every row against its own key, and the three amounts are three different
    # numbers, so a frame that repeated one amount would fail here.
    amounts = [result.breakdown_amount(name) for name in risk_classes]
    assert list(df["amount"]) == pytest.approx(amounts)
    assert len(set(amounts)) == 3
    assert set(df["currency"]) == {"USD"}


def test_im_result_breakdown_dataframe_keeps_schema_when_empty() -> None:
    """A result with no risk classes at all yields zero rows and every column.

    This used to run on ``ScheduleImCalculator``, which always publishes
    exactly one breakdown key — so ``len(df) == len(result.breakdown_keys())``
    was ``1 == 1`` and the empty path was never taken. An unpopulated SIMM
    sensitivity set is the genuinely empty case.
    """
    result = SimmCalculator("v2_6").calculate_from_sensitivities(SimmSensitivities("USD"), "USD", "2025-06-30")

    assert result.breakdown_keys() == []
    assert result.amount == pytest.approx(0.0)

    df = result.to_breakdown_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 0
    assert list(df.columns) == ["risk_class", "amount", "currency"]


def test_schedule_im_breakdown_dataframe_has_one_row_per_asset_class() -> None:
    """The schedule methodology publishes exactly its one asset class."""
    result = ScheduleImCalculator.bcbs_standard().calculate_for_notional(
        10_000_000.0, "USD", "interest_rate", 7.0, "2025-06-30"
    )
    df = result.to_breakdown_dataframe()

    assert list(df.columns) == ["risk_class", "amount", "currency"]
    assert list(df["risk_class"]) == result.breakdown_keys() == ["interest_rate"]
    assert df.iloc[0]["amount"] == pytest.approx(result.breakdown_amount("interest_rate"))
    assert df.iloc[0]["amount"] == pytest.approx(result.amount)
    assert df.iloc[0]["currency"] == "USD"


# Metrics


def test_margin_utilization_to_dataframe() -> None:
    metric = MarginUtilization(800_000.0, 1_000_000.0, "USD")
    df = metric.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == [
        "posted",
        "required",
        "ratio",
        "shortfall",
        "is_adequate",
        "currency",
    ]
    row = df.iloc[0]
    assert row["ratio"] == pytest.approx(0.8)
    assert row["currency"] == "USD"
    assert bool(row["is_adequate"]) is False


def test_excess_collateral_to_dataframe() -> None:
    metric = ExcessCollateral(1_200_000.0, 1_000_000.0, "USD")
    df = metric.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == [
        "collateral_value",
        "required_value",
        "excess",
        "excess_percentage",
        "has_excess",
        "has_shortfall",
        "currency",
    ]
    row = df.iloc[0]
    assert row["excess"] == pytest.approx(200_000.0)
    assert row["excess_percentage"] == pytest.approx(0.2)
    assert bool(row["has_excess"]) is True
    assert bool(row["has_shortfall"]) is False


def test_margin_funding_cost_to_dataframe() -> None:
    metric = MarginFundingCost(1_000_000.0, 0.05, 0.03, "USD")
    df = metric.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == [
        "margin_posted",
        "funding_rate",
        "collateral_rate",
        "spread",
        "annual_cost",
        "currency",
    ]
    row = df.iloc[0]
    assert row["spread"] == pytest.approx(0.02)
    assert row["annual_cost"] == pytest.approx(20_000.0)
    assert row["currency"] == "USD"


# FrtbSensitivities long format


def test_frtb_sensitivities_to_dataframe_keeps_schema_when_empty() -> None:
    df = FrtbSensitivities("USD").to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 0
    assert list(df.columns) == SENSITIVITY_COLUMNS


def test_frtb_sensitivities_to_dataframe_is_long_and_sorted() -> None:
    sens = FrtbSensitivities("USD")
    sens.add_girr_delta("5Y", 12_000.0)
    sens.add_girr_delta("2Y", 8_000.0)
    sens.add_csr_nonsec_delta("ACME", 3, "5Y", "bond", 4_000.0)
    sens.add_equity_delta("AAPL", 1, 25_000.0)
    sens.add_fx_delta("EUR", "USD", 9_000.0)
    sens.add_girr_curvature(500.0, -400.0)
    sens.add_rrao_position("EXOTIC-1", 5_000_000.0, True)

    df = sens.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == SENSITIVITY_COLUMNS
    # 2 GIRR deltas + 1 CSR + 1 equity + 1 FX + 2 curvature halves + 1 RRAO.
    assert len(df) == 8

    keys = _sort_keys(df)
    assert keys == sorted(keys), "rows must be sorted by their key columns"

    girr_delta = df[(df["risk_class"] == "girr") & (df["kind"] == "delta")]
    assert len(girr_delta) == 2
    assert set(girr_delta["tenor"]) == {"2Y", "5Y"}
    assert set(girr_delta["issuer"]) == {"USD"}

    curvature = df[df["kind"].isin(["curvature_up", "curvature_down"])]
    assert len(curvature) == 2
    assert set(curvature["kind"]) == {"curvature_up", "curvature_down"}

    fx = df[df["risk_class"] == "fx"]
    assert list(fx["issuer"]) == ["EUR/USD"]

    csr = df[df["risk_class"] == "csr_non_sec"]
    assert list(csr["bucket"]) == ["3"], "bucket is exported as a string label"

    rrao = df[df["risk_class"] == "rrao"]
    assert list(rrao["kind"]) == ["exotic_notional"]
    assert rrao.iloc[0]["amount"] == pytest.approx(5_000_000.0)


def test_frtb_sensitivities_row_order_ignores_the_insertion_order() -> None:
    """Two insertion orders of the same book give the identical frame.

    That is what the ``(risk_class, kind, issuer, bucket, tenor)`` sort
    guarantees. Exporting one frozen object twice — the previous assertion —
    could only have failed on intra-process nondeterminism.
    """
    # Amounts key off the tenor, never the insertion index, so reversing the
    # loop reorders the same book rather than building a different one.
    amounts = {"1Y": 1_100.0, "2Y": 2_200.0, "5Y": 5_500.0, "10Y": 10_100.0, "30Y": 30_300.0}
    tenors = tuple(amounts)

    def build(order: tuple[str, ...]) -> FrtbSensitivities:
        sens = FrtbSensitivities("USD")
        for tenor in order:
            sens.add_girr_delta(tenor, amounts[tenor])
            sens.add_equity_delta(f"NAME_{tenor}", 1, 2.0 * amounts[tenor])
        return sens

    forward = build(tenors).to_dataframe()
    backward = build(tuple(reversed(tenors))).to_dataframe()

    assert len(forward) == 10
    pd.testing.assert_frame_equal(forward, backward)


# SimmSensitivities long format


def test_simm_sensitivities_to_dataframe_keeps_schema_when_empty() -> None:
    df = SimmSensitivities("USD").to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 0
    assert list(df.columns) == [*SENSITIVITY_COLUMNS, "expiry_tenor"]


def test_simm_sensitivities_to_dataframe_is_long_and_sorted() -> None:
    sens = SimmSensitivities("USD")
    sens.add_ir_delta("USD", "5Y", 100_000.0)
    sens.add_ir_delta("EUR", "2Y", 50_000.0)
    sens.add_ir_vega("USD", "5Y", 10_000.0)
    sens.add_credit_qualifying_delta("financial", "ACME", "5Y", 20_000.0)
    sens.add_credit_non_qualifying_delta("RMBS-1", "5Y", 15_000.0)
    sens.add_equity_delta("AAPL", 250_000.0)
    sens.add_fx_delta("EUR", 75_000.0)
    sens.add_commodity_delta("crude", 30_000.0)
    sens.add_curvature(SimmCurvatureSensitivity("interest_rate", "USD", "OIS", "1Y", 5_000.0, "5Y"))

    df = sens.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == [*SENSITIVITY_COLUMNS, "expiry_tenor"]
    assert len(df) == 9

    keys = _sort_keys(df)
    assert keys == sorted(keys), "rows must be sorted by their key columns"

    ir = df[df["risk_class"] == "interest_rate"]
    assert set(ir["kind"]) == {"delta", "vega", "curvature"}

    ir_delta = ir[ir["kind"] == "delta"]
    assert set(ir_delta["issuer"]) == {"USD", "EUR"}
    assert set(ir_delta["tenor"]) == {"5Y", "2Y"}

    commodity = df[df["risk_class"] == "commodity"]
    assert list(commodity["bucket"]) == ["crude"]
    assert commodity["issuer"].isna().all(), "commodity has no name axis"

    credit = df[df["risk_class"] == "credit_qualifying"]
    assert list(credit["issuer"]) == ["ACME"]
    assert list(credit["bucket"]) == ["financial"]

    non_qualifying = df[df["risk_class"] == "credit_non_qualifying"]
    assert list(non_qualifying["issuer"]) == ["RMBS-1"]
    assert non_qualifying["bucket"].isna().all()


def test_simm_sensitivities_credit_qualifying_carries_sector_label() -> None:
    sens = SimmSensitivities("USD")
    sens.add_credit_qualifying_delta("sovereign", "GOVT_A", "5Y", 40_000.0)
    df = sens.to_dataframe()

    assert len(df) == 1
    row = df.iloc[0]
    assert row["risk_class"] == "credit_qualifying"
    assert row["bucket"] == "sovereign"
    assert row["issuer"] == "GOVT_A"
    assert row["tenor"] == "5Y"
    assert row["amount"] == pytest.approx(40_000.0)


def test_simm_sensitivities_row_order_ignores_the_insertion_order() -> None:
    """Two insertion orders of the same book give the identical frame.

    Same reasoning as the FRTB case: re-exporting a frozen object proves
    nothing, whereas re-inserting in reverse is exactly what the row sort is
    for.
    """
    amounts = {"1Y": 1_100.0, "2Y": 2_200.0, "5Y": 5_500.0, "10Y": 10_100.0, "30Y": 30_300.0}
    tenors = tuple(amounts)

    def build(order: tuple[str, ...]) -> SimmSensitivities:
        sens = SimmSensitivities("USD")
        for tenor in order:
            sens.add_ir_delta("USD", tenor, amounts[tenor])
            sens.add_equity_delta(f"NAME_{tenor}", 2.0 * amounts[tenor])
        return sens

    forward = build(tenors).to_dataframe()
    backward = build(tuple(reversed(tenors))).to_dataframe()

    assert len(forward) == 10
    pd.testing.assert_frame_equal(forward, backward)


def test_margin_utilization_zero_requirement_pickle_roundtrip() -> None:
    import pickle

    ratio = MarginUtilization(10.0, 0.0, "USD")
    restored = pickle.loads(pickle.dumps(ratio))  # noqa: S301
    assert restored.to_json() == ratio.to_json()
