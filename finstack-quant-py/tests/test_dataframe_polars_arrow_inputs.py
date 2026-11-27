"""Tests for Slice 1 Pythonic DataFrame / Polars / Arrow / date input upgrades."""

from __future__ import annotations

import datetime as dt
import json
import math

import numpy as np
import pandas as pd
import polars as pl
import pyarrow as pa
import pytest

from finstack_quant.analytics import Performance
from finstack_quant.attribution import attribute_return_contribution
from finstack_quant.cashflows.builder import CashFlowSchedule, Notional
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import DiscountCurve, MarketContext, VolSurface
from finstack_quant.core.math import linalg, stats, summation
from finstack_quant.covenants import (
    Covenant,
    CovenantEngine,
    CovenantForecastConfig,
    CovenantSpec,
    CovenantTemplate,
    CovenantType,
    covenant_template,
    forecast_breaches,
    forecast_covenant,
)
from finstack_quant.features import (
    dataframe as feat_df,
    neutralize,
    transform_cross_sectional,
    transform_panel,
    transform_timeseries,
)
from finstack_quant.margin import (
    CsaSpec,
    ExposureDiagnostics,
    ExposureProfile,
    FrtbSensitivities,
    MarginFundingSpread,
    SaCcrTrade,
    SimmSensitivities,
    VmCalculator,
)
from finstack_quant.models.correlation import (
    CopulaSpec,
    PortfolioLossConfig,
    simulate_portfolio_loss,
)
from finstack_quant.models.credit.migration import RatingScale, TransitionMatrix
from finstack_quant.models.factor import FactorCovarianceMatrix, FactorModelConfig
from finstack_quant.models.factor.credit import CreditCalibrator, decompose_levels
from finstack_quant.models.factor.risk import build_stress_attribution
from finstack_quant.models.rates.dtsm import YieldPanel
from finstack_quant.models.volatility import get_surface_vol
from finstack_quant.portfolio import (
    FactorModel,
    Portfolio,
    StrategyAllocationInput,
    WeightAllocationSpec,
    allocate_weights,
    brinson_fachler,
    brinson_linked,
    carino_link,
    cell_returns_from_reference,
    excess_returns,
    factor_brinson_attribution,
    grid_attribution,
    grid_carino_link,
    mwr_xirr,
    scenario_pnl,
    twrr_linked,
)
from finstack_quant.scenarios import ScenarioSpec
from finstack_quant.statements import ModelBuilder
from finstack_quant.statements_analytics import PeerSet

# ---------------------------------------------------------------------------
# 1. Date conversion edge cases (date_utils.rs)
# ---------------------------------------------------------------------------


def test_date_utils_handles_numpy_datetime64_nat_and_iso_datetimes() -> None:
    engine = CovenantEngine.from_specs(covenant_template(CovenantTemplate.cov_lite(7.0, 4.5)))
    metrics = {"total_leverage": 5.0, "senior_leverage": 3.0}

    assert engine.evaluate(metrics, np.datetime64("2026-03-31"))["max_total_leverage"].passed is True
    assert engine.evaluate(metrics, "2026-03-31T14:30:00Z")["max_total_leverage"].passed is True
    assert engine.evaluate(metrics, "2026-03-31 14:30:00")["max_total_leverage"].passed is True
    assert engine.evaluate(metrics, pd.Timestamp("2026-03-31"))["max_total_leverage"].passed is True
    assert engine.evaluate(metrics, dt.date(2026, 3, 31))["max_total_leverage"].passed is True

    with pytest.raises(ValueError, match="NaT"):
        engine.evaluate(metrics, np.datetime64("NaT", "D"))
    with pytest.raises(ValueError, match="NaT"):
        engine.evaluate(metrics, pd.NaT)


def test_mwr_xirr_accepts_numpy_datetime64_pairs_and_rejects_nat() -> None:
    irr = mwr_xirr([(np.datetime64("2025-01-01"), -100.0), (np.datetime64("2026-01-01"), 110.0)])
    assert irr == pytest.approx(0.1, rel=1e-6)

    with pytest.raises(ValueError, match="NaT"):
        mwr_xirr([(np.datetime64("NaT", "D"), -100.0), (np.datetime64("2026-01-01"), 110.0)])


# ---------------------------------------------------------------------------
# 2. Fixed bugs across portfolio, math, surfaces, and margin
# ---------------------------------------------------------------------------


def test_scenario_pnl_empty_dataframe_has_float64_schema() -> None:
    empty_book = Portfolio.from_spec({
        "id": "EMPTY",
        "as_of": "2025-01-15",
        "base_currency": "USD",
        "entities": {},
        "positions": [],
    })
    frame = scenario_pnl(empty_book, ScenarioSpec("flat", []), MarketContext()).pnl.to_pandas()
    assert frame.empty
    assert list(frame.columns) == ["position_id", "pnl"]
    assert str(frame["pnl"].dtype) == "float64"


def test_twrr_linked_accepts_series_dataframe_polars_and_arrow() -> None:
    expected = twrr_linked([0.05, 0.03], horizon_years=1.0).cumulative
    pa_tbl = pa.table({"r": [0.05, 0.03]})
    pl_df = pl.DataFrame({"r": [0.05, 0.03]})
    pd_df = pd.DataFrame({"r": [0.05, 0.03]})
    pd_ser = pd.Series([0.05, 0.03])
    pl_ser = pl.Series("r", [0.05, 0.03])
    arr = np.array([0.05, 0.03], dtype=np.float64)

    for candidate in ([0.05, 0.03], arr, pd_ser, pl_ser, pd_df, pl_df, pa_tbl):
        assert twrr_linked(candidate, horizon_years=1.0).cumulative == pytest.approx(expected)


def test_mwr_xirr_accepts_pandas_series_dataframe_polars_and_arrow() -> None:
    dates = ["2025-01-01", "2026-01-01"]
    amounts = [-100.0, 110.0]

    pd_series = pd.Series(amounts, index=pd.to_datetime(dates))
    pd_idx_df = pd.DataFrame({"amount": amounts}, index=pd.to_datetime(dates))
    pd_col_df = pd.DataFrame({"date": dates, "amount": amounts})
    pl_df = pl.DataFrame({"date": [dt.date(2025, 1, 1), dt.date(2026, 1, 1)], "amount": amounts})
    pa_tbl = pa.table({"date": [dt.date(2025, 1, 1), dt.date(2026, 1, 1)], "amount": amounts})

    for candidate in (pd_series, pd_idx_df, pd_col_df, pl_df, pa_tbl):
        assert mwr_xirr(candidate) == pytest.approx(0.1, rel=1e-6)


def test_brinson_linked_tidy_frame_list_of_frames_and_carino_link_typed() -> None:
    p1 = [
        {
            "sector": "Tech",
            "portfolio_weight": 0.7,
            "benchmark_weight": 0.5,
            "portfolio_return": 0.10,
            "benchmark_return": 0.06,
        },
        {
            "sector": "Energy",
            "portfolio_weight": 0.3,
            "benchmark_weight": 0.5,
            "portfolio_return": 0.04,
            "benchmark_return": 0.05,
        },
    ]
    p2 = [
        {
            "sector": "Tech",
            "portfolio_weight": 0.6,
            "benchmark_weight": 0.5,
            "portfolio_return": 0.02,
            "benchmark_return": 0.03,
        },
        {
            "sector": "Energy",
            "portfolio_weight": 0.4,
            "benchmark_weight": 0.5,
            "portfolio_return": -0.01,
            "benchmark_return": 0.00,
        },
    ]
    baseline = brinson_linked([p1, p2])

    # carino_link fast-path with typed BrinsonPeriodResult objects
    typed_linked = carino_link([brinson_fachler(p1), brinson_fachler(p2)])
    assert typed_linked.to_json() == baseline.to_json()

    # list of DataFrames (pandas, Polars, Arrow)
    pd_list = [pd.DataFrame(p1), pd.DataFrame(p2)]
    pl_list = [pl.DataFrame(p1), pl.DataFrame(p2)]
    pa_list = [pa.Table.from_pylist(p1), pa.Table.from_pylist(p2)]
    for candidate in (pd_list, pl_list, pa_list):
        assert brinson_linked(candidate).linked_allocation == pytest.approx(baseline.linked_allocation)

    # Single tidy DataFrame with "period" column
    tidy_rows = [dict(row, period="2025Q1") for row in p1] + [dict(row, period="2025Q2") for row in p2]
    for tidy in (pd.DataFrame(tidy_rows), pl.DataFrame(tidy_rows), pa.Table.from_pylist(tidy_rows)):
        assert brinson_linked(tidy).linked_selection == pytest.approx(baseline.linked_selection)


def test_sa_ccr_trade_from_dataframe_handles_nat_nan_polars_and_arrow() -> None:
    pd_tape = pd.DataFrame([
        {
            "trade_id": "IRS-1",
            "asset_class": "interest_rate",
            "notional": 1_000_000.0,
            "start_date": pd.Timestamp("2025-01-15"),
            "end_date": pd.Timestamp("2030-01-15"),
            "underlier": "USD-SOFR",
            "hedging_set": "USD-IR",
            "direction": 1.0,
            "supervisory_delta": 1.0,
            "mtm": 10_000.0,
            "is_option": False,
            "option_type": None,
            "option_maturity_date": pd.NaT,
        },
        {
            "trade_id": "IRS-2",
            "asset_class": "interest_rate",
            "notional": 500_000.0,
            "start_date": "2025-01-15",
            "end_date": "2028-01-15",
            "underlier": "USD-SOFR",
            "hedging_set": "USD-IR",
            "direction": 1.0,
            "supervisory_delta": 1.0,
            "mtm": 0.0,
            "is_option": False,
            "option_type": None,
            "option_maturity_date": float("nan"),
        },
    ])
    trades_pd = SaCcrTrade.from_dataframe(pd_tape)
    assert [t.trade_id for t in trades_pd] == ["IRS-1", "IRS-2"]

    pl_tape = pl.DataFrame({
        "trade_id": ["IRS-1"],
        "asset_class": ["interest_rate"],
        "notional": [1_000_000.0],
        "start_date": [dt.date(2025, 1, 15)],
        "end_date": [dt.date(2030, 1, 15)],
        "underlier": ["USD-SOFR"],
        "hedging_set": ["USD-IR"],
        "direction": [1.0],
        "supervisory_delta": [1.0],
        "mtm": [10_000.0],
        "is_option": [False],
        "option_type": [None],
        "option_maturity_date": pl.Series([None], dtype=pl.Date),
    })
    assert [t.trade_id for t in SaCcrTrade.from_dataframe(pl_tape)] == ["IRS-1"]
    assert [t.trade_id for t in SaCcrTrade.from_dataframe(pl_tape.to_arrow())] == ["IRS-1"]


def test_core_math_linalg_stats_summation_accept_series_dataframes_arrow_and_ints() -> None:
    chol_pd = pd.DataFrame([[4.0, 2.0], [2.0, 3.0]])
    chol_pl = pl.DataFrame({"a": [4.0, 2.0], "b": [2.0, 3.0]})
    chol_pa = pa.table({"a": [4.0, 2.0], "b": [2.0, 3.0]})
    expected_chol = linalg.cholesky_decomposition([[4.0, 2.0], [2.0, 3.0]])

    for mat in (chol_pd, chol_pl, chol_pa):
        np.testing.assert_allclose(linalg.cholesky_decomposition(mat), expected_chol)

    x_pd = pd.DataFrame({"x1": [1.0, 2.0, 3.0]})
    x_pl = pl.DataFrame({"x1": [1.0, 2.0, 3.0]})
    x_pa = pa.table({"x1": [1.0, 2.0, 3.0]})
    y_pd = pd.Series([2.0, 4.0, 6.0])
    y_pl = pl.Series("y", [2.0, 4.0, 6.0])
    y_int = np.array([2, 4, 6], dtype=np.int64)

    for x_in, y_in in ((x_pd, y_pd), (x_pl, y_pl), (x_pa, y_int)):
        fit = stats.ols(x_in, y_in, add_intercept=False)
        assert fit.coefficients == pytest.approx([2.0])
        assert fit.r_squared == pytest.approx(1.0)

    assert stats.mean(pd.Series([1, 2, 3], dtype="int64")) == pytest.approx(2.0)
    assert stats.mean(pl.Series("v", [1, 2, 3])) == pytest.approx(2.0)
    assert summation.kahan_sum(pd.Series([1.0, 2.0, 3.0])) == pytest.approx(6.0)
    assert summation.neumaier_sum(pl.Series("v", [1, -2, 3])) == pytest.approx(2.0)


def test_vol_surface_orientation_not_transposed_for_pandas_polars_or_arrow() -> None:
    expiries = [1.0, 2.0]  # 2 rows
    strikes = [90.0, 100.0, 110.0]  # 3 columns
    grid = [[0.22, 0.20, 0.21], [0.25, 0.23, 0.24]]

    ref = VolSurface(
        "REF", expiries, strikes, grid, secondary_axis="strike", quote_type="black_lognormal", interpolation_mode="vol"
    )
    pd_grid = pd.DataFrame(grid, columns=["90", "100", "110"])
    pl_grid = pl.DataFrame({"90": [0.22, 0.25], "100": [0.20, 0.23], "110": [0.21, 0.24]})
    pa_grid = pl_grid.to_arrow()

    for label, vols in (("pd", pd_grid), ("pl", pl_grid), ("pa", pa_grid)):
        surf = VolSurface(
            label,
            expiries,
            strikes,
            vols,
            secondary_axis="strike",
            quote_type="black_lognormal",
            interpolation_mode="vol",
        )
        assert surf.grid_shape == (2, 3)
        assert get_surface_vol(surf, 1.0, 90.0) == pytest.approx(get_surface_vol(ref, 1.0, 90.0))
        assert get_surface_vol(surf, 2.0, 110.0) == pytest.approx(get_surface_vol(ref, 2.0, 110.0))


# ---------------------------------------------------------------------------
# 3. features.dataframe (Polars in -> Polars out) & compiled features
# ---------------------------------------------------------------------------


def test_features_dataframe_polars_preserves_host_library_across_all_helpers() -> None:
    dates_str = ["2026-01-01", "2026-01-01", "2026-01-02", "2026-01-02"]
    dates_obj = [dt.date(2026, 1, 1), dt.date(2026, 1, 1), dt.date(2026, 1, 2), dt.date(2026, 1, 2)]
    assets = ["A", "B", "A", "B"]
    signals = [1.0, 3.0, 2.0, 5.0]
    others = [1.0, 2.0, 2.0, 4.0]
    vols = [1.0, 2.0, 1.0, 2.0]
    factors = [0.0, 1.0, 0.0, 1.0]
    groups = ["g1", "g1", "g2", "g2"]

    pd_frame = pd.DataFrame({
        "date": dates_str,
        "asset": assets,
        "group": groups,
        "signal": signals,
        "other": others,
        "vol": vols,
        "factor": factors,
    })
    pl_frame = pl.DataFrame({
        "date": dates_obj,
        "asset": assets,
        "group": groups,
        "signal": signals,
        "other": others,
        "vol": vols,
        "factor": factors,
    })

    # 1. cross_sectional (including unique pl.Date auto-detection)
    cs_pd = feat_df.cross_sectional(pd_frame, "signal", "date", op="rank")
    cs_pl = feat_df.cross_sectional(pl_frame, "signal", op="rank")
    assert isinstance(cs_pl, pl.Series)
    assert cs_pl.to_list() == pytest.approx(cs_pd.tolist())

    # 2. timeseries (unique pl.Date auto-detection)
    ts_pd = feat_df.timeseries(pd_frame, "signal", "asset", "date", op="diff")
    ts_pl = feat_df.timeseries(pl_frame, "signal", "asset", op="diff")
    assert isinstance(ts_pl, pl.Series)
    for got, exp in zip(ts_pl.to_list(), ts_pd.tolist(), strict=True):
        assert (got is None and pd.isna(exp)) or got == pytest.approx(exp)

    # 3. panel
    ops = [{"name": "r", "family": "cross_sectional", "op": "rank"}]
    pan_pd = feat_df.panel(pd_frame, "signal", ops, time_key="date")
    pan_pl = feat_df.panel(pl_frame, "signal", ops)
    assert isinstance(pan_pl, pl.DataFrame)
    assert pan_pl["r"].to_list() == pytest.approx(pan_pd["r"].tolist())

    # 4. grouped
    grp_pd = feat_df.grouped(pd_frame, "signal", "date", groups="group", op="zscore")
    grp_pl = feat_df.grouped(pl_frame, "signal", groups="group", op="zscore")
    assert isinstance(grp_pl, pl.Series)
    assert grp_pl.to_list() == pytest.approx(grp_pd.tolist())

    # 5. neutralize
    neu_pd = feat_df.neutralize(pd_frame, "signal", "date", exposures=["factor"])
    neu_pl = feat_df.neutralize(pl_frame, "signal", exposures=["factor"])
    assert isinstance(neu_pl, pl.Series)
    assert neu_pl.to_list() == pytest.approx(neu_pd.tolist())

    # 6. pairwise
    pw_frame_pl = pl.DataFrame({
        "date": [dt.date(2026, 1, 1), dt.date(2026, 1, 2), dt.date(2026, 1, 3)],
        "asset": ["A", "A", "A"],
        "signal": [1.0, 2.0, 3.0],
        "other": [1.0, 2.0, 4.0],
    })
    pw_frame_pd = pw_frame_pl.to_pandas()
    pw_pd = feat_df.pairwise(
        pw_frame_pd, "signal", "other", "asset", "date", op="rolling_beta", params={"window": 3, "min_periods": 3}
    )
    pw_pl = feat_df.pairwise(
        pw_frame_pl, "signal", "other", "asset", op="rolling_beta", params={"window": 3, "min_periods": 3}
    )
    assert isinstance(pw_pl, pl.Series)
    assert pw_pl.to_list()[-1] == pytest.approx(pw_pd.iloc[-1])

    # 7. rolling_regression_residual
    rr_pd = feat_df.rolling_regression_residual(
        pw_frame_pd, "signal", ["other"], "asset", "date", {"window": 3, "min_periods": 3}
    )
    rr_pl = feat_df.rolling_regression_residual(
        pw_frame_pl, "signal", ["other"], "asset", params={"window": 3, "min_periods": 3}
    )
    assert isinstance(rr_pl, pl.Series)
    assert rr_pl.to_list()[-1] == pytest.approx(rr_pd.iloc[-1])

    # 8. risk_scaled_weights
    rsw_pd = feat_df.risk_scaled_weights(pd_frame, "signal", "date", volatility="vol")
    rsw_pl = feat_df.risk_scaled_weights(pl_frame, "signal", volatility="vol")
    assert isinstance(rsw_pl, pl.Series)
    assert rsw_pl.to_list() == pytest.approx(rsw_pd.tolist())

    # 9. rank_to_weights
    rtw_pd = feat_df.rank_to_weights(pd_frame, "signal", "date")
    rtw_pl = feat_df.rank_to_weights(pl_frame, "signal")
    assert isinstance(rtw_pl, pl.Series)
    assert rtw_pl.to_list() == pytest.approx(rtw_pd.tolist())

    # 10. neutralize_and_zscore
    nz_pd = feat_df.neutralize_and_zscore(pd_frame, "signal", "date", exposures=["factor"])
    nz_pl = feat_df.neutralize_and_zscore(pl_frame, "signal", exposures=["factor"])
    assert isinstance(nz_pl, pl.Series)
    assert nz_pl.to_list() == pytest.approx(nz_pd.tolist())


def test_compiled_features_accept_series_dataframes_arrow_and_datetime64() -> None:
    vals_pl = pl.Series("v", [1.0, 2.0, 2.0, 4.0])
    keys = np.array(["2026-01-01"] * 4, dtype="datetime64[D]")
    exp_pl = pl.DataFrame({"f": [0.0, 1.0, 0.0, 1.0]})
    exp_pa = exp_pl.to_arrow()

    expected = neutralize([1.0, 2.0, 2.0, 4.0], ["2026-01-01"] * 4, [[0.0, 1.0, 0.0, 1.0]])
    assert neutralize(vals_pl, keys, exp_pl) == pytest.approx(expected)
    assert neutralize(pd.Series([1.0, 2.0, 2.0, 4.0]), keys, exp_pa) == pytest.approx(expected)
    assert transform_cross_sectional(
        pl.Series("v", [1.0, 3.0]), np.array(["2026-01-01", "2026-01-01"], dtype="datetime64[D]"), "rank"
    ) == [0.0, 1.0]
    assert transform_timeseries(
        pl.Series("v", [100.0, 102.0]),
        ["A", "A"],
        np.array(["2026-01-01", "2026-01-02"], dtype="datetime64[D]"),
        "returns",
    ) == [
        None,
        pytest.approx(0.02),
    ]
    res = transform_panel({
        "values": pl.Series("v", [1.0, 3.0]),
        "time_key": ["d", "d"],
        "operations": [{"name": "r", "family": "cross_sectional", "op": "rank"}],
    })
    assert res.get_column("r") == [0.0, 1.0]

    with pytest.raises(ValueError, match="NaT"):
        transform_cross_sectional([1.0], np.array(["NaT"], dtype="datetime64[D]"), "rank")


# ---------------------------------------------------------------------------
# 4. Polars + pyarrow.Table + ArrowTable across domains
# ---------------------------------------------------------------------------


def test_analytics_accepts_pyarrow_and_finstack_arrow_table() -> None:
    prices_pl = pl.DataFrame({
        "date": [dt.date(2024, 1, 1), dt.date(2024, 1, 2), dt.date(2024, 1, 3)],
        "FUND": [100.0, 102.0, 105.0],
        "BENCH": [100.0, 101.0, 102.5],
    })
    pa_prices = prices_pl.to_arrow()
    perf_from_pa = Performance(pa_prices, benchmark_ticker="BENCH", frequency="daily")
    assert perf_from_pa.get_ticker_names() == ["FUND", "BENCH"]

    arrow_tbl = perf_from_pa.to_arrow(0.0, 0.0, 0.0, 0.95)
    assert arrow_tbl.num_rows == 2
    assert arrow_tbl.to_polars()["ticker"].to_list() == ["FUND", "BENCH"]

    returns_pa = pa.table({
        "date": [dt.date(2024, 1, 2), dt.date(2024, 1, 3)],
        "FUND": [0.02, 105.0 / 102.0 - 1.0],
        "BENCH": [0.01, 102.5 / 101.0 - 1.0],
    })
    perf_ret = Performance.from_returns(returns_pa, initial_date="2024-01-01")
    assert perf_ret.cumulative_returns()[0][-1] == pytest.approx(0.05)

    # multi_factor_greeks with Arrow factor table + frequency declared
    returns_long = pa.table({
        "date": [dt.date(2024, 1, 2), dt.date(2024, 1, 3), dt.date(2024, 1, 4), dt.date(2024, 1, 5)],
        "FUND": [0.01, 0.02, -0.01, 0.03],
    })
    factors_pa = pa.table({
        "dt": [dt.date(2024, 1, 2), dt.date(2024, 1, 3), dt.date(2024, 1, 4), dt.date(2024, 1, 5)],
        "MKT": [0.005, 0.015, -0.008, 0.022],
    })
    perf_mfg = Performance.from_returns(returns_long, initial_date="2024-01-01", frequency="daily")
    greeks = perf_mfg.multi_factor_greeks("FUND", factors_pa, risk_free_rate=0.0, date_col="dt")
    assert len(greeks.betas) == 1


def test_portfolio_polars_arrow_and_series_paths() -> None:
    # allocate_weights with covariance DataFrame / Arrow
    s1 = StrategyAllocationInput("A", risk_budget=0.5)
    s2 = StrategyAllocationInput("B", risk_budget=0.5)
    cov_list = [[0.04, 0.01], [0.01, 0.09]]
    base_alloc = allocate_weights(WeightAllocationSpec("risk_budget", 1000.0, [s1, s2], covariance=cov_list)).to_json()
    for cov in (
        pd.DataFrame(cov_list),
        pl.DataFrame({"a": [0.04, 0.01], "b": [0.01, 0.09]}),
        pa.table({"a": [0.04, 0.01], "b": [0.01, 0.09]}),
    ):
        assert (
            allocate_weights(WeightAllocationSpec("risk_budget", 1000.0, [s1, s2], covariance=cov)).to_json()
            == base_alloc
        )

    # excess_returns with typed DurationCellTable & Polars/Arrow positions
    ref = [{"duration": 1.0, "total_return": 0.02}]
    tbl = cell_returns_from_reference(ref, "UST", {"width": 2.0})
    pos = [{"id": "B1", "weight": 1.0, "duration": 1.0, "total_return": 0.03}]
    er_base = excess_returns(pos, tbl).portfolio_excess_return
    assert excess_returns(pl.DataFrame(pos), tbl).portfolio_excess_return == pytest.approx(er_base)
    assert excess_returns(pa.Table.from_pylist(pos), tbl).portfolio_excess_return == pytest.approx(er_base)

    # factor_brinson_attribution with pl.Series factor_returns
    fb_in = {
        "asset_ids": ["A"],
        "asset_returns": [0.02],
        "exposures": [1.0],
        "factor_names": ["Market"],
        "portfolio_weights": [1.0],
        "benchmark_weights": [1.0],
    }
    assert factor_brinson_attribution(fb_in, pl.Series("fr", [0.02])).active_return == pytest.approx(0.0)

    # grid_carino_link fast-path
    port = pl.DataFrame([{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}])
    bench = pa.Table.from_pylist([{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}])
    g1 = grid_attribution(port, bench)
    assert grid_carino_link([g1, g1]).linked_selection == pytest.approx(0.0203, abs=1e-4)

    # FactorModel factor_stress & factor_stress_pnl with dict, Series, Polars, Arrow
    cfg = json.dumps({
        "factors": [
            {
                "id": "usd_rates",
                "factor_type": "rates",
                "market_mapping": {"curve_parallel": {"curve_ids": ["USD-OIS"], "units": "rate_bp"}},
            }
        ],
        "covariance": {"factor_ids": ["usd_rates"], "n": 1, "data": [0.0001]},
        "matching": {"mapping_table": [{"dependency_filter": {}, "attribute_filter": {}, "factor_id": "usd_rates"}]},
        "risk_measure": "variance",
    })
    deposit = {
        "type": "deposit",
        "spec": {
            "id": "DEP",
            "notional": {"amount": "1000000", "currency": "USD"},
            "start_date": "2025-01-15",
            "maturity": "2025-07-15",
            "day_count": "act_360",
            "fixed_rate": "0.04",
            "discount_curve_id": "USD-OIS",
            "attributes": {},
        },
    }
    book = Portfolio.from_spec({
        "id": "BOOK",
        "as_of": "2025-01-15",
        "base_currency": "USD",
        "entities": {"FUND": {"id": "FUND"}},
        "positions": [
            {
                "position_id": "POS",
                "entity_id": "FUND",
                "instrument_id": "DEP",
                "instrument_spec": deposit,
                "quantity": 1.0,
                "unit": "units",
            }
        ],
    })
    curve = DiscountCurve(
        "USD-OIS",
        "2025-01-15",
        [(t, math.exp(-0.04 * t)) for t in (0.0, 0.5, 1.0, 2.0)],
        interp="monotone_convex",
        extrapolation="extend",
        day_count="act_365f",
    )
    market = MarketContext().insert(curve)
    fm = FactorModel.from_config(FactorModelConfig.from_json(cfg))
    base_pnl = fm.factor_stress_pnl(book, market, "2025-01-15", [("usd_rates", 1.0)]).total_pnl
    for stresses in (
        {"usd_rates": 1.0},
        pd.Series({"usd_rates": 1.0}),
        pl.DataFrame({"factor_id": ["usd_rates"], "shift": [1.0]}),
        pa.table({"factor_id": ["usd_rates"], "shift": [1.0]}),
    ):
        assert fm.factor_stress_pnl(book, market, "2025-01-15", stresses).total_pnl == pytest.approx(base_pnl)
        assert fm.factor_stress(book, market, "2025-01-15", stresses).stressed_decomposition.total_risk >= 0.0


def test_margin_polars_and_arrow_inputs() -> None:
    # ExposureProfile.from_dataframe
    ExposureProfile(
        as_of="2025-01-01",
        currency="USD",
        basis={"kind": "external", "source_id": "example", "policy_id": "unsecured"},
        dates=["2025-01-01"],
        times=[0.0],
        mtm_values=[10.0],
        epe=[10.0],
        ene=[0.0],
        diagnostics=ExposureDiagnostics(total_time_points=1),
    )
    pl_exp = pl.DataFrame({
        "t": [0.0],
        "dates": [dt.date(2025, 1, 1)],
        "mtm_values": [10.0],
        "epe": [10.0],
        "ene": [0.0],
    })
    diag = ExposureDiagnostics(total_time_points=1)
    basis = {"kind": "external", "source_id": "example", "policy_id": "unsecured"}
    for tbl in (pl_exp, pl_exp.to_arrow()):
        loaded = ExposureProfile.from_dataframe(
            tbl, as_of="2025-01-01", currency="USD", basis=basis, diagnostics=diag, time_col="t"
        )
        assert loaded.mtm_values == [10.0]

    # SimmSensitivities & FrtbSensitivities
    simm = SimmSensitivities("USD")
    simm.add_ir_delta("USD", "5Y", 1_000.0)
    simm_pl = simm.to_polars()
    assert SimmSensitivities.from_dataframe(simm_pl, "USD").total_ir_delta() == pytest.approx(1_000.0)
    assert SimmSensitivities.from_dataframe(simm.to_pyarrow(), "USD").total_ir_delta() == pytest.approx(1_000.0)

    frtb = FrtbSensitivities("USD")
    frtb.add_girr_delta("USD", "USD-OIS", "5Y", 100_000.0)
    frtb_pl = frtb.to_polars()
    assert FrtbSensitivities.from_dataframe(frtb_pl, "USD").to_json() == frtb.to_json()
    assert FrtbSensitivities.from_dataframe(frtb.to_pyarrow(), "USD").to_json() == frtb.to_json()

    # VmCalculator.generate_margin_calls & MarginFundingSpread.dated with 2-col frames
    calc = VmCalculator(CsaSpec.usd_regulatory())
    calls_pl = pl.DataFrame({"date": ["2024-06-17", "2024-06-18"], "exposure": [1e6, 4e5]})
    assert calc.generate_margin_calls(calls_pl, 0.0).to_dict()["call_type"] == [
        "variation_margin_collect",
        "variation_margin_post",
    ]
    assert isinstance(
        MarginFundingSpread.dated(pa.table({"date": ["2024-06-17", "2025-06-17"], "spread_bp": [40.0, 50.0]})),
        MarginFundingSpread,
    )


def test_covenants_series_polars_and_arrow_inputs() -> None:
    engine = CovenantEngine.from_specs(covenant_template(CovenantTemplate.cov_lite(7.0, 4.5)))
    assert engine.evaluate(pd.Series({"total_leverage": 5.0, "senior_leverage": 3.0}), "2026-03-31")[
        "max_senior_leverage"
    ].passed

    pl_metrics = pl.DataFrame({
        "test_date": [dt.date(2026, 3, 31), dt.date(2026, 6, 30)],
        "total_leverage": [6.0, 7.5],
        "senior_leverage": [3.0, 3.5],
    })
    for tbl in (pl_metrics, pl_metrics.to_arrow()):
        out_tbl = engine.evaluate_series(tbl)
        out = out_tbl.to_pandas()
        assert out.loc[out["covenant"] == "max_total_leverage", "passed"].tolist() == [True, False]
        assert out_tbl.to_polars().filter(pl.col("covenant") == "max_total_leverage")["passed"].to_list() == [
            True,
            False,
        ]
        out_explicit = engine.evaluate_series(tbl, date_col="test_date").to_pandas()
        assert out_explicit["passed"].tolist() == out["passed"].tolist()

    spec = CovenantSpec(Covenant(CovenantType.max_debt_to_ebitda(4.5), "3M", "max_leverage"), "debt_to_ebitda")
    fc_pl = pl.DataFrame({"d": [dt.date(2026, 3, 31), dt.date(2026, 6, 30)], "debt_to_ebitda": [4.0, 5.0]})
    assert forecast_covenant(spec, fc_pl).breach_probability == pytest.approx([0.0, 1.0])
    assert forecast_covenant(spec, fc_pl.to_arrow(), date_col="d").breach_probability == pytest.approx([0.0, 1.0])
    assert (
        len(
            forecast_breaches(
                engine, pl_metrics.to_arrow(), CovenantForecastConfig().with_scope("incurrence"), date_col="test_date"
            )
        )
        == 1
    )


def test_models_polars_and_arrow_inputs() -> None:
    # CreditCalibrator.from_dataframe + decompose_levels
    spreads_pl = pl.DataFrame({
        "dt": [dt.date(2024, 1, 1), dt.date(2024, 2, 1)],
        "A": [0.010, 0.0101],
    })
    tags_pl = pl.DataFrame({"issuer": ["A"], "rating": ["IG"]})
    cfg = {
        "hierarchy": {"levels": ["rating"]},
        "min_bucket_size_per_level": {"per_level": [1]},
        "covariance_strategy": "diagonal",
        "bucket_weighting": "equal",
    }
    model_pl = CreditCalibrator.from_dataframe(
        spreads_pl,
        tags_pl,
        pl.Series("g", [0.010, 0.0101]),
        {"name": "G", "series_id": "G"},
        config=cfg,
        date_col="dt",
    )
    assert model_pl.n_issuers == 1

    model_pa = CreditCalibrator.from_dataframe(
        spreads_pl.to_arrow(),
        tags_pl.to_arrow(),
        [0.010, 0.0101],
        {"name": "G", "series_id": "G"},
        config=cfg,
    )
    assert model_pa.n_issuers == 1

    lvl = decompose_levels(model_pl, pl.DataFrame({"issuer": ["A"], "spread": [0.0125]}), 0.0120, "2024-03-01")
    assert lvl.generic == pytest.approx(120.0)

    # FactorCovarianceMatrix.from_dataframe
    cov_pl = pl.DataFrame({"equity": [0.04, 0.01], "rates": [0.01, 0.02]})
    assert FactorCovarianceMatrix.from_dataframe(cov_pl).variance("equity") == pytest.approx(0.04)
    assert FactorCovarianceMatrix.from_dataframe(cov_pl.to_arrow()).variance("rates") == pytest.approx(0.02)

    # YieldPanel.from_dataframe (dated and undated across pandas/Polars/Arrow)
    yp_pl = pl.DataFrame({
        "obs_date": [dt.date(2025, 1, 1), dt.date(2025, 1, 2)],
        "1Y": [0.01, 0.011],
        "2Y": [0.02, 0.021],
    })
    yp_undated_pl = pl.DataFrame({"1Y": [0.01, 0.011], "2Y": [0.02, 0.021]})
    assert YieldPanel.from_dataframe(yp_pl).num_dates == 2
    assert YieldPanel.from_dataframe(yp_pl.to_arrow(), date_col="obs_date").num_tenors == 2
    assert YieldPanel.from_dataframe(yp_undated_pl).dates is None
    assert YieldPanel.from_dataframe(yp_undated_pl.to_arrow()).tenors == pytest.approx([1.0, 2.0])

    # TransitionMatrix.from_dataframe
    tm_pl = pl.DataFrame({"A": [0.9, 0.0], "D": [0.1, 1.0]})
    assert TransitionMatrix.from_dataframe(
        tm_pl, 1.0, scale=RatingScale.custom_with_default(["A", "D"], "D")
    ).probability("A", "D") == pytest.approx(0.1)
    assert TransitionMatrix.from_dataframe(tm_pl.to_arrow(), 1.0).probability("A", "D") == pytest.approx(0.1)

    # simulate_portfolio_loss
    exp_pa = pa.Table.from_pylist([
        {"id": "A", "notional": 100.0, "default_probability": 0.05, "lgd": 0.6, "factor_loadings": [0.3]}
    ])
    loss_res = simulate_portfolio_loss(exp_pa, PortfolioLossConfig(200, 42, 0.99, CopulaSpec.gaussian()))
    assert len(loss_res.losses) == 200

    # build_stress_attribution
    pnls_pl = pl.DataFrame({"A": [-8.0, -2.0] + [0.5] * 38, "B": [-2.0, -4.0] + [0.5] * 38})
    assert build_stress_attribution(None, pnls_pl).var_threshold == pytest.approx(-6.0)
    assert build_stress_attribution(None, pnls_pl.to_arrow()).var_threshold == pytest.approx(-6.0)


def test_statements_attribution_and_cashflows_polars_and_arrow_inputs() -> None:
    # ModelBuilder.from_dataframe
    stmt_pl = pl.DataFrame({"node_id": ["rev"], "2025Q1": [100.0]})
    spec_pl = ModelBuilder("demo").periods("2025Q1..Q1").from_dataframe(stmt_pl, node_id_column="node_id").build()
    spec_pa = ModelBuilder("demo").periods("2025Q1..Q1").from_dataframe(stmt_pl.to_arrow()).build()
    assert spec_pl.to_json() == spec_pa.to_json()

    # PeerSet.from_dataframe
    peer_pl = pl.DataFrame({"company": ["SUBJ", "PEER"], "leverage": [3.0, 2.0]})
    assert PeerSet.from_dataframe(peer_pl, "SUBJ", "ltm", id_column="company").peer_count == 1
    assert PeerSet.from_dataframe(peer_pl.to_arrow(), "SUBJ", "ltm").peer_count == 1

    # Return contribution attribution
    pos_pl = pl.DataFrame({"id": ["A", "B"], "market_value": [100.0, -60.0], "return": [0.02, 0.01]})
    fac_pl = pl.DataFrame({"factor": ["mkt"], "exposure": [1.0], "factor_return": [0.01]})
    attr_pl = attribute_return_contribution(pos_pl, as_of="2026-01-02", weighting="net_market_value", factors=fac_pl)
    attr_pa = attribute_return_contribution(
        pos_pl.to_arrow(), as_of="2026-01-02", weighting="net_market_value", factors=fac_pl.to_arrow()
    )
    assert attr_pl.portfolio_return == pytest.approx(0.035)
    assert attr_pa.to_json() == attr_pl.to_json()

    # CashFlowSchedule.from_flows
    cf_pl = pl.DataFrame({
        "date": [dt.date(2025, 6, 15)],
        "amount": [100.0],
        "currency": ["USD"],
        "kind": ["fixed"],
        "accrual_factor": [0.0],
    })
    assert (
        CashFlowSchedule.from_flows(cf_pl, Notional.par(1_000.0, "USD"), DayCount.ACT_360).get_flows()[0].kind.name
        == "fixed"
    )
    assert (
        CashFlowSchedule
        .from_flows(cf_pl.to_arrow(), Notional.par(1_000.0, "USD"), DayCount.ACT_360)
        .get_flows()[0]
        .kind.name
        == "fixed"
    )
