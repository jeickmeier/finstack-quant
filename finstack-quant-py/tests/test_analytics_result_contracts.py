"""Result metadata and strict JSON contracts survive every Python round trip."""

import json
import pickle

import pytest

from finstack_quant import analytics

FIXTURES = {
    "BetaResult": {
        "beta": "nan",
        "std_err": "nan",
        "ci_lower": "nan",
        "ci_upper": "nan",
        "n_obs": 2,
        "confidence_level": 0.95,
    },
    "GreeksResult": {"alpha": "nan", "beta": "nan", "r_squared": "nan", "adjusted_r_squared": "nan", "n_obs": 1},
    "MultiFactorResult": {
        "alpha": 0.0,
        "betas": [1.0],
        "r_squared": "nan",
        "adjusted_r_squared": "nan",
        "residual_vol": 0.0,
        "n_obs": 4,
    },
    "DrawdownEpisode": {
        "start": "2025-01-01",
        "valley": "2025-01-02",
        "end": None,
        "duration_days": 2,
        "max_drawdown": -0.1,
    },
    "PeriodStats": {
        "n_obs": 2,
        "wins": 1,
        "losses": 1,
        "best": 0.01,
        "worst": -0.01,
        "consecutive_wins": 1,
        "consecutive_losses": 1,
        "win_rate": 0.5,
        "avg_return": 0.0,
        "avg_win": 0.01,
        "avg_loss": -0.01,
        "payoff_ratio": 1.0,
        "profit_factor": 1.0,
        "cpc_index": 0.5,
        "kelly_risk_fraction": 0.0,
    },
    "LookbackSample": {"n_obs": 1, "holding_start": "2025-01-01", "holding_end": "2025-01-02"},
    "CorrelationResult": {
        "tickers": ["FUND"],
        "matrix": [[1.0]],
        "n_obs": 2,
        "start_date": "2025-01-01",
        "end_date": "2025-01-02",
    },
    "RollingGreeks": {"dates": ["2025-01-02"], "alphas": [0.0], "betas": [1.0]},
    "LookbackReturns": {
        "fiscal_config": {"start_month": 1, "start_day": 1},
        "ref_date": "2025-01-02",
        "ticker_names": ["FUND"],
        "mtd": [0.1],
        "qtd": [0.1],
        "ytd": [0.1],
        "fytd": [0.1],
        "mtd_samples": [{"n_obs": 1, "holding_start": "2025-01-01", "holding_end": "2025-01-02"}],
        "qtd_samples": [{"n_obs": 1, "holding_start": "2025-01-01", "holding_end": "2025-01-02"}],
        "ytd_samples": [{"n_obs": 1, "holding_start": "2025-01-01", "holding_end": "2025-01-02"}],
        "fytd_samples": [{"n_obs": 1, "holding_start": "2025-01-01", "holding_end": "2025-01-02"}],
    },
}
REQUIRED = {
    "BetaResult": ["n_obs", "confidence_level"],
    "GreeksResult": ["n_obs"],
    "MultiFactorResult": ["n_obs"],
    "DrawdownEpisode": ["duration_days"],
    "PeriodStats": ["n_obs", "wins", "losses"],
    "LookbackReturns": ["fiscal_config", "mtd_samples"],
}


@pytest.mark.parametrize("name", FIXTURES)
def test_result_json_and_pickle_round_trip(name: str) -> None:
    cls = getattr(analytics, name)
    value = cls.from_json(json.dumps(FIXTURES[name]))
    assert json.loads(value.to_json()) == FIXTURES[name]
    restored = pickle.loads(pickle.dumps(value))  # noqa: S301 - trusted in-process fixture
    assert json.loads(restored.to_json()) == FIXTURES[name]


@pytest.mark.parametrize("name", FIXTURES)
def test_result_json_rejects_unknown_fields(name: str) -> None:
    wire = dict(FIXTURES[name], unknown_result_field=True)
    with pytest.raises(ValueError, match="unknown field"):
        getattr(analytics, name).from_json(json.dumps(wire))


@pytest.mark.parametrize(("name", "field"), [(name, field) for name, fields in REQUIRED.items() for field in fields])
def test_result_json_requires_metadata(name: str, field: str) -> None:
    wire = FIXTURES[name].copy()
    del wire[field]
    with pytest.raises(ValueError, match="missing field"):
        getattr(analytics, name).from_json(json.dumps(wire))


def test_drawdown_recovery_remains_optional() -> None:
    wire = FIXTURES["DrawdownEpisode"].copy()
    del wire["end"]
    assert analytics.DrawdownEpisode.from_json(json.dumps(wire)).end is None
