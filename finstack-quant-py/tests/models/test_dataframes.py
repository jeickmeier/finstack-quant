"""Tests for model-owned pandas ``DataFrame`` accessors.

- ``models.monte_carlo``: ``PathSummary.to_dataframe``.
- ``models.correlation``: ``PortfolioLossResult.to_distribution_dataframe``
  / ``to_summary_dataframe``, ``TrancheLossStatistics.to_dataframe``.

Everything is built through public constructors and calculators, so the tests
stay self-contained.
"""

from __future__ import annotations

import pandas as pd
import pytest

from finstack_quant.models.correlation import (
    CopulaSpec,
    CreditExposure,
    PortfolioLossConfig,
    PortfolioLossResult,
    simulate_portfolio_loss,
)
from finstack_quant.models.monte_carlo import PathSummary, simulate_paths

# monte_carlo


def _gbm_spec(seed: int) -> dict[str, object]:
    return {
        "process": {"type": "gbm", "r": 0.05, "q": 0.0, "sigma": 0.2},
        "initial_state": [100.0],
        "time_grid": {"type": "uniform", "expiry": 1.0, "num_steps": 2},
        "num_paths": 3,
        "seed": seed,
    }


def test_path_summary_to_dataframe_is_path_and_time_by_factor() -> None:
    summary = simulate_paths(_gbm_spec(7))
    df = summary.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert list(df.index.names) == ["path", "time"]
    assert list(df.columns) == summary.factor_names == ["spot"]
    assert len(df) == summary.num_simulated_paths * len(summary.times)
    assert list(df["spot"]) == summary.values
    # One factor unstacks to the time-by-path table.
    wide = df["spot"].unstack("path")
    assert list(wide.index) == summary.times
    assert list(wide.columns) == [0, 1, 2]
    assert list(wide[1]) == summary.values[3:6]
    assert summary._repr_html_() == df._repr_html_()


def test_path_summary_to_dataframe_has_one_column_per_factor() -> None:
    summary = simulate_paths({
        **_gbm_spec(7),
        "process": {
            "type": "heston",
            "r": 0.04,
            "q": 0.01,
            "kappa": 2.0,
            "theta": 0.05,
            "sigma_v": 0.3,
            "rho": -0.7,
            "v0": 0.03,
        },
        "initial_state": [100.0, 0.03],
        "antithetic": True,
    })
    df = summary.to_dataframe()
    assert list(df.columns) == ["spot", "variance"]
    assert df.shape == (6 * 3, 2)
    assert list(df["spot"]) == summary.values[0::2]
    assert list(df["variance"]) == summary.values[1::2]
    assert df.loc[(5, 0.0)].tolist() == [100.0, 0.03]


def test_path_summary_to_dataframe_rejects_an_inconsistent_shape() -> None:
    inconsistent = PathSummary.from_json(
        '{"num_paths":1,"num_simulated_paths":1,"dim":1,"times":[0.0,1.0],"factor_names":["spot"],"values":[1.0]}'
    )
    with pytest.raises(ValueError, match="1 values"):
        inconsistent.to_dataframe()
    assert inconsistent._repr_html_() is None


def test_path_summary_dataframe_is_reproducible_from_the_seed() -> None:
    """Two independent simulations on one seed export the identical frame.

    Re-exporting a single frozen summary — the previous assertion — proved
    nothing: the paths were already fixed. Re-simulating is what pins the
    determinism claim the seed argument exists for. A different seed must move
    the paths, otherwise the seed is being ignored.
    """
    first = simulate_paths(_gbm_spec(7)).to_dataframe()
    second = simulate_paths(_gbm_spec(7)).to_dataframe()
    pd.testing.assert_frame_equal(first, second)

    other_seed = simulate_paths(_gbm_spec(8)).to_dataframe()
    assert list(other_seed.columns) == list(first.columns)
    assert not first.equals(other_seed), "a different seed must produce different paths"


# models.correlation


_PORTFOLIO_NOTIONAL = 2_000.0


def _portfolio_loss_result() -> PortfolioLossResult:
    """Twenty names over 5,000 paths, so the loss distribution has interior mass.

    The previous two-name / 200-path fixture put the whole distribution above
    every tranche's detachment, which collapsed the tranche export: ``VaR ==
    ES == tranche_notional`` and ``prob_full_writedown ==
    prob_attachment_breached == expected_loss_fraction``. Eight of the eleven
    columns then shared four values and a shuffled schema went unnoticed.
    """
    exposures = [CreditExposure(f"N{index}", 100.0, 0.05, 0.6, [0.4]) for index in range(20)]
    config = PortfolioLossConfig(5_000, 42, 0.99, CopulaSpec.gaussian())
    return simulate_portfolio_loss(exposures, config)


def test_portfolio_loss_distribution_dataframe_is_one_row_per_path() -> None:
    result = _portfolio_loss_result()
    df = result.to_distribution_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["loss"]
    assert len(df) == len(result.losses)
    assert list(df["loss"]) == pytest.approx(list(result.losses))


def test_portfolio_loss_summary_dataframe_is_one_row() -> None:
    result = _portfolio_loss_result()
    df = result.to_summary_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == [
        "expected_loss",
        "var",
        "expected_shortfall",
        "confidence",
        "num_paths",
    ]
    row = df.iloc[0]
    assert row["expected_loss"] == pytest.approx(result.expected_loss)
    assert row["var"] == pytest.approx(result.var)
    assert row["expected_shortfall"] == pytest.approx(result.expected_shortfall)
    assert row["confidence"] == pytest.approx(0.99)
    assert row["num_paths"] == len(result.losses) == 5_000
    # The three loss statistics are ordered, and therefore distinct: a frame
    # that rendered one of them into all three columns fails here.
    assert row["expected_loss"] < row["var"] < row["expected_shortfall"]


_TRANCHE_COLUMNS = [
    "attachment",
    "detachment",
    "tranche_notional",
    "expected_loss_fraction",
    "expected_loss_amount",
    "var_fraction",
    "var_amount",
    "expected_shortfall_fraction",
    "expected_shortfall_amount",
    "prob_attachment_breached",
    "prob_full_writedown",
]


def test_tranche_loss_statistics_to_dataframe_is_one_row() -> None:
    """A mezzanine tranche the loss distribution straddles, so no two cells tie."""
    stats = _portfolio_loss_result().tranche_loss_statistics(0.05, 0.20, _PORTFOLIO_NOTIONAL)
    df = stats.to_dataframe()
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == _TRANCHE_COLUMNS

    row = df.iloc[0]
    assert row["attachment"] == pytest.approx(0.05)
    assert row["detachment"] == pytest.approx(0.20)
    assert row["tranche_notional"] == pytest.approx(stats.tranche_notional) == pytest.approx(300.0)
    # Every one of the eleven columns takes a different value.
    assert len({float(row[column]) for column in df.columns}) == len(_TRANCHE_COLUMNS)


def test_tranche_loss_statistics_respect_the_domain_ordering() -> None:
    """ES >= VaR >= EL, and a full write-down is rarer than a breach.

    These are the invariants that make the eleven columns meaningful, and they
    only bite on a tranche that is neither wiped out nor untouched.
    """
    df = _portfolio_loss_result().tranche_loss_statistics(0.05, 0.20, _PORTFOLIO_NOTIONAL).to_dataframe()
    row = df.iloc[0]

    for scale in ("fraction", "amount"):
        expected_loss = float(row[f"expected_loss_{scale}"])
        value_at_risk = float(row[f"var_{scale}"])
        shortfall = float(row[f"expected_shortfall_{scale}"])
        assert shortfall >= value_at_risk >= expected_loss > 0.0, scale

    # Fractions and amounts describe the same quantity at two scales.
    notional = float(row["tranche_notional"])
    for statistic in ("expected_loss", "var", "expected_shortfall"):
        assert row[f"{statistic}_amount"] == pytest.approx(row[f"{statistic}_fraction"] * notional)

    breached = float(row["prob_attachment_breached"])
    written_down = float(row["prob_full_writedown"])
    assert 0.0 <= written_down <= breached <= 1.0
    assert written_down < breached, "a straddled tranche must breach more often than it wipes out"


def test_tranche_loss_statistics_concat_into_a_capital_structure() -> None:
    result = _portfolio_loss_result()
    tranches = [(0.0, 0.03), (0.03, 0.07), (0.07, 1.0)]
    table = pd.concat(
        [result.tranche_loss_statistics(a, d, _PORTFOLIO_NOTIONAL).to_dataframe() for a, d in tranches],
        ignore_index=True,
    )
    assert len(table) == len(tranches)
    assert list(table["attachment"]) == pytest.approx([a for a, _ in tranches])
    assert list(table["detachment"]) == pytest.approx([d for _, d in tranches])
    # Subordination: junior tranches lose a strictly larger fraction.
    assert list(table["expected_loss_fraction"]) == sorted(table["expected_loss_fraction"], reverse=True)
    assert table["expected_loss_amount"].sum() == pytest.approx(result.expected_loss)
