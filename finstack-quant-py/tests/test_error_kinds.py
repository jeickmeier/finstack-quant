"""The Python exception class follows the Rust error kind.

Every Rust error exposes ``kind()`` (``NotFound``, ``Validation`` or
``Computation``), and the bindings map it once: ``NotFound`` raises
``KeyError``, ``Validation`` raises ``ValueError`` and ``Computation`` raises
``RuntimeError``. The WASM facade reports the same kind as ``error.kind``
(``tests/facade/error_kinds.test.mjs`` drives the same failures), so a caller
sees one classification in both hosts.

Each case below is a genuine library failure whose kind was previously decided
by a per-binding table, and at least one of those tables disagreed with Rust.
"""

from __future__ import annotations

import json

import pytest

from finstack_quant import analytics, models, portfolio, statements, statements_analytics
from finstack_quant.core.market_data import invert_fx_rate
from finstack_quant.models.factor.credit import CreditCalibrator, decompose_levels


def _evaluate(*formulas: tuple[str, str]) -> None:
    builder = statements.ModelBuilder("kinds")
    builder.periods("2024Q1..Q2", None)
    builder.value("revenue", [("2024Q1", 1.0), ("2024Q2", 1.0)])
    for node, formula in formulas:
        builder.compute(node, formula)
    statements.Evaluator().evaluate(builder.build())


def _credit_model() -> object:
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
        "history_panel": {"dates": ["2024-01-01", "2024-02-01"], "spreads": {"A": [0.010, 0.0101]}},
        "issuer_tags": {"tags": {"A": {}}},
        "generic_factor": {"spec": {"name": "G", "series_id": "G"}, "values": [0.010, 0.0101]},
        "as_of": "2024-02-01",
        "as_of_spreads": {"A": 0.0101},
        "idiosyncratic_overrides": {},
    }
    return CreditCalibrator(json.dumps(config)).calibrate(json.dumps(inputs))


def _performance() -> analytics.Performance:
    dates = [f"2024-01-{day:02d}" for day in range(1, 11)]
    returns = [0.01, -0.01, 0.02, 0.0, 0.01, -0.02, 0.01, 0.0, 0.01, 0.01]
    return analytics.Performance.from_returns_arrays(dates, [returns], ["A"])


class TestStatements:
    def test_a_dependency_cycle_is_a_computation_failure(self) -> None:
        with pytest.raises(RuntimeError, match=r"(?i)circular"):
            _evaluate(("a", "b + 1"), ("b", "a + 1"))

    def test_an_unknown_identifier_is_a_validation_failure(self) -> None:
        with pytest.raises(ValueError, match="nope"):
            _evaluate(("x", "revenue + nope"))

    def test_explaining_a_missing_period_is_not_found(self) -> None:
        builder = statements.ModelBuilder("kinds")
        builder.periods("2024Q1..Q2", None)
        builder.value("revenue", [("2024Q1", 1.0), ("2024Q2", 1.0)])
        builder.compute("x", "revenue * 2")
        model = builder.build()
        result = statements.Evaluator().evaluate(model)
        with pytest.raises(KeyError):
            statements_analytics.explain_formula(model, result, "x", "2030Q1")


class TestPortfolio:
    def test_an_unknown_entity_is_not_found(self) -> None:
        spec = {
            "id": "P",
            "as_of": "2025-01-15",
            "base_currency": "USD",
            "entities": {"FUND": {"id": "FUND"}},
            "positions": [
                {
                    "position_id": "X",
                    "entity_id": "NOPE",
                    "instrument_id": "D",
                    "instrument_spec": {
                        "type": "deposit",
                        "spec": {
                            "id": "D",
                            "notional": {"amount": "1", "currency": "USD"},
                            "start_date": "2025-01-15",
                            "maturity": "2025-07-15",
                            "day_count": "act_360",
                            "fixed_rate": "0.04",
                            "discount_curve_id": "USD-OIS",
                            "attributes": {},
                        },
                    },
                    "quantity": 1.0,
                    "unit": "units",
                }
            ],
        }
        with pytest.raises(KeyError, match="NOPE"):
            portfolio.Portfolio.from_spec(json.dumps(spec))


class TestModels:
    def test_an_issuer_outside_the_factor_model_is_not_found(self) -> None:
        with pytest.raises(KeyError, match="ZZZ"):
            decompose_levels(_credit_model(), '{"ZZZ": 0.0125}', 0.012, "2025-06-30")

    def test_nearest_correlation_that_does_not_converge_is_a_computation_failure(self) -> None:
        matrix = [1.0, 0.95, -0.95, 0.95, 1.0, 0.95, -0.95, 0.95, 1.0]
        with pytest.raises(RuntimeError):
            models.correlation.nearest_correlation(matrix, 3, 1, 1e-16)

    def test_cos_rejects_invalid_inputs_as_validation_failures(self) -> None:
        with pytest.raises(ValueError, match="num_terms"):
            models.bs_cos_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True, 0)
        with pytest.raises(ValueError, match="volatility must be finite and strictly positive"):
            models.bs_cos_price(100.0, 100.0, 0.05, 0.0, -0.2, 1.0, True)

    def test_a_degenerate_cos_range_is_a_computation_failure(self) -> None:
        with pytest.raises(RuntimeError):
            models.bs_cos_price(100.0, 100.0, 0.05, 0.0, 0.2, 0.0, True)


class TestCoreAndAnalytics:
    def test_inverting_a_zero_fx_rate_is_a_validation_failure(self) -> None:
        with pytest.raises(ValueError, match="Invalid FX rate"):
            invert_fx_rate(0.0)

    def test_an_unknown_calendar_is_not_found(self) -> None:
        with pytest.raises(KeyError, match="nope_cal"):
            _performance().cagr("bus_252", "nope_cal")

    def test_a_calendar_union_resolves(self) -> None:
        assert _performance().cagr("bus_252", "nyse+gblo")["A"] > 0.0
