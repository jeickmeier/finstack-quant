"""Cross-cutting ergonomics guarantees for the Python bindings.

These lock in the quant-facing contracts that are easy to regress because no
single domain owns them: the package advertises its version, builders chain,
and date-valued parameters accept both ISO strings and `datetime.date`.
"""

from __future__ import annotations

import datetime
import json
import re
from typing import Any

import pandas as pd
import pytest

import finstack_quant as fq
from finstack_quant.core.dates import StubKind
from finstack_quant.statements import Evaluator, ModelBuilder


class TestVersion:
    """`finstack_quant.__version__` lets a notebook record what it ran against."""

    def test_version_is_exposed(self) -> None:
        assert isinstance(fq.__version__, str)
        assert fq.__version__

    def test_version_is_pep440_release(self) -> None:
        """A parseable version is what makes it useful in a provenance stamp."""
        assert re.match(r"^\d+\.\d+\.\d+", fq.__version__), fq.__version__

    def test_version_matches_extension(self) -> None:
        """The pure-Python re-export must not drift from the compiled module."""
        from finstack_quant.finstack_quant import __version__ as ext_version

        assert fq.__version__ == ext_version

    def test_version_is_not_a_lazy_submodule(self) -> None:
        """`__version__` is in `__all__` but must never route through `__getattr__`."""
        assert "__version__" in fq.__all__
        # A lazily-imported name would be a module, not a string.
        assert not hasattr(fq.__version__, "__name__")


class TestModelBuilderChaining:
    """Configuration calls return the builder, so they compose."""

    def test_chained_construction_builds_a_model(self) -> None:
        model = (
            ModelBuilder("chained")
            .periods("2025Q1..Q3")
            .value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0), ("2025Q3", 121.0)])
            .compute("cogs", "revenue * 0.4")
            .compute("gross_profit", "revenue - cogs")
            .build()
        )
        assert set(model.node_ids()) == {"revenue", "cogs", "gross_profit"}
        # Node ids alone are value-blind: evaluate so the chained `value` and
        # `compute` calls are pinned to the numbers they configured.
        result = Evaluator().evaluate(model)
        assert result.get_scalar("revenue", "2025Q2") == pytest.approx(110.0)
        assert result.get_scalar("cogs", "2025Q2") == pytest.approx(44.0)
        assert result.get_scalar("gross_profit", "2025Q2") == pytest.approx(66.0)

    def test_chaining_returns_the_same_object(self) -> None:
        """Chaining must mutate in place rather than fork a copy.

        If it forked, the statement-per-line style would silently diverge from
        the chained one.
        """
        builder = ModelBuilder("identity")
        assert builder.periods("2025Q1..Q1") is builder
        assert builder.value("revenue", [("2025Q1", 1.0)]) is builder

    def test_statement_style_still_works(self) -> None:
        """The pre-existing call style must keep working unchanged."""
        builder = ModelBuilder("statements")
        builder.periods("2025Q1..Q1")
        builder.value("revenue", [("2025Q1", 100.0)])
        builder.compute("half", "revenue * 0.5")
        assert builder.build().node_count == 2

    def test_chained_and_statement_styles_agree(self) -> None:
        """Agreement means the same *values*, not merely the same node names.

        ``node_ids()`` is value-blind: two models carrying 10.0 and 999.0 for
        the same node compare equal on ids alone. Both spellings are evaluated
        so the configured numbers are what is compared.
        """
        chained = (
            ModelBuilder("m")
            .periods("2025Q1..Q2")
            .value("revenue", [("2025Q1", 10.0), ("2025Q2", 20.0)])
            .compute("cogs", "revenue * 0.4")
            .build()
        )
        builder = ModelBuilder("m")
        builder.periods("2025Q1..Q2")
        builder.value("revenue", [("2025Q1", 10.0), ("2025Q2", 20.0)])
        builder.compute("cogs", "revenue * 0.4")
        stepwise = builder.build()

        assert chained.node_ids() == stepwise.node_ids()

        chained_frame = Evaluator().evaluate(chained).to_dataframe(orient="long")
        stepwise_frame = Evaluator().evaluate(stepwise).to_dataframe(orient="long")
        pd.testing.assert_frame_equal(chained_frame, stepwise_frame)
        values = chained_frame.set_index(["node_id", "period"])["value"].to_dict()
        assert values == {
            ("revenue", "2025Q1"): 10.0,
            ("revenue", "2025Q2"): 20.0,
            ("cogs", "2025Q1"): 4.0,
            ("cogs", "2025Q2"): 8.0,
        }

    def test_mixed_node_builder_chains(self) -> None:
        builder = ModelBuilder("mixed")
        builder.periods("2025Q1..Q1")
        builder.value("revenue", [("2025Q1", 100.0)])
        model = builder.mixed("margin").formula("revenue * 0.5").name("Margin").build().build()
        assert "margin" in model.node_ids()
        # The chained `formula` has to reach the evaluator, not just the id list.
        assert Evaluator().evaluate(model).get_scalar("margin", "2025Q1") == pytest.approx(50.0)

    def test_build_is_terminal(self) -> None:
        """`build()` consumes the builder; a later call must say so clearly."""
        builder = ModelBuilder("terminal")
        builder.periods("2025Q1..Q1")
        builder.build()
        with pytest.raises(ValueError, match="no longer usable"):
            builder.value("revenue", [("2025Q1", 1.0)])

    def test_failed_call_does_not_brick_the_builder(self) -> None:
        """A rejected formula must leave the builder usable.

        The pre-validation that guarantees this is easy to lose when the
        method signatures change.
        """
        builder = ModelBuilder("robust")
        builder.periods("2025Q1..Q1")
        builder.value("revenue", [("2025Q1", 100.0)])
        with pytest.raises(ValueError, match="parse error"):
            builder.compute("bad", "revenue -* 2")
        assert builder.compute("good", "revenue * 2") is builder

    def test_chained_model_evaluates(self) -> None:
        """End-to-end: the chained builder feeds the evaluator and reaches pandas."""
        model = (
            ModelBuilder("e2e")
            .periods("2025Q1..Q2")
            .value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
            .compute("cogs", "revenue * 0.4")
            .build()
        )
        result = Evaluator().evaluate(model)
        frame = result.to_dataframe(orient="long")
        assert set(frame["node_id"]) == {"revenue", "cogs"}
        # The formula the chain configured, evaluated: 100 * 0.4 and 110 * 0.4.
        values = frame.set_index(["node_id", "period"])["value"].to_dict()
        assert values[("cogs", "2025Q1")] == pytest.approx(40.0)
        assert values[("cogs", "2025Q2")] == pytest.approx(44.0)
        assert values[("revenue", "2025Q1")] == pytest.approx(100.0)


class TestDateArguments:
    """Date-valued parameters accept an ISO string or a `datetime.date`."""

    @staticmethod
    def _market() -> str:
        import json

        return json.dumps({
            "schema_version": 1,
            "curves": [
                {
                    "type": "discount",
                    "id": "USD-OIS",
                    "base": "2024-01-01",
                    "day_count": "act_360",
                    "knot_points": [[0.0, 1.0], [5.0, 0.90], [10.0, 0.80]],
                    "interp_style": "monotone_convex",
                    "extrapolation": "flat_forward",
                    "min_forward_rate": None,
                    "allow_non_monotonic": False,
                    "min_forward_tenor": 1e-6,
                }
            ],
            "fx": None,
            "surfaces": [],
            "prices": {},
            "series": [],
            "inflation_indices": [],
            "dividends": [],
            "credit_indices": [],
            "fx_delta_vol_surfaces": [],
            "vol_cubes": [],
            "collateral": {},
            "hierarchy": None,
        })

    @staticmethod
    def _bond() -> Any:
        from finstack_quant.core.currency import Currency
        from finstack_quant.core.money import Money
        from finstack_quant.core.types import Rate
        from finstack_quant.valuations.instruments import Bond

        return Bond.fixed(
            "BOND-1",
            Money(1_000_000.0, Currency("USD")),
            Rate(0.05),
            datetime.date(2024, 1, 1),
            datetime.date(2034, 1, 1),
            StubKind.SHORT_FRONT,
            "USD-OIS",
        )

    def test_as_of_accepts_string_and_date_identically(self) -> None:
        """The two spellings must price to the same number, not merely both work."""
        from finstack_quant.valuations.instruments import price_instrument

        bond, market = self._bond(), self._market()
        from_str = price_instrument(bond.to_json(), market, "2024-01-01")
        from_date = price_instrument(bond.to_json(), market, datetime.date(2024, 1, 1))
        assert from_str.price == from_date.price

    def test_as_of_rejects_non_date_with_a_useful_message(self) -> None:
        from finstack_quant.valuations.instruments import price_instrument

        bond, market = self._bond(), self._market()
        with pytest.raises(TypeError, match="expected a date-like object") as excinfo:
            price_instrument(bond.to_json(), market, 20240101)
        # The message has to say what to do next, not merely that it failed.
        message = str(excinfo.value)
        assert "datetime.date" in message
        assert "ISO-8601" in message
        assert "got int" in message

    def test_as_of_rejects_malformed_iso_string(self) -> None:
        from finstack_quant.valuations.instruments import price_instrument

        bond, market = self._bond(), self._market()
        # `(?i)date|parse|input` matched almost any message, including a
        # generic "invalid input"; this pins the actual diagnostic and echoes
        # back the offending text.
        with pytest.raises(ValueError, match=r"Invalid date '01/01/2024'"):
            price_instrument(bond.to_json(), market, "01/01/2024")


class TestDateAcceptanceIsUniform:
    """Every date-taking entry point accepts both spellings, identically.

    The migration is only worth having if the two forms are interchangeable, so
    these assert equal *results* rather than merely that both calls succeed —
    a conversion that silently shifted the date by a day would pass the weaker
    check.
    """

    @staticmethod
    def _schedule() -> str:
        from finstack_quant.cashflows import build_cashflow_schedule_json

        return build_cashflow_schedule_json(
            json.dumps({
                "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
                "issue_date": "2024-08-31",
                "maturity": "2025-08-31",
                "coupon_program": [
                    {
                        "kind": "fixed",
                        "spec": {
                            "coupon_type": "cash",
                            "rate": "0.06",
                            "frequency": {"count": 12, "unit": "months"},
                            "day_count": "30_360",
                            "business_day_convention": "following",
                            "calendar_id": "weekends_only",
                            "stub": "none",
                            "end_of_month": False,
                            "payment_lag_days": 0,
                        },
                    }
                ],
            })
        )

    def test_cashflows_accrued_interest(self) -> None:
        from finstack_quant.cashflows import accrued_interest

        schedule = self._schedule()
        assert accrued_interest(schedule, "2025-02-28") == accrued_interest(schedule, datetime.date(2025, 2, 28))

    def test_covenants_evaluate_engine(self) -> None:
        from finstack_quant import covenants

        specs = json.loads(covenants.lbo_standard_json(6.0, 2.0, 1.1, 50_000_000.0))
        engine = covenants.validate_covenant_engine_json(
            json.dumps({
                "specs": [specs[0]],
                "breach_history": [],
                "windows": [],
                "waivers": [],
            })
        )
        metrics = json.dumps({"debt_to_ebitda": 4.0})

        def as_json(reports: dict[str, object]) -> dict[str, str]:
            return {key: report.to_json() for key, report in reports.items()}

        assert as_json(covenants.evaluate_engine(engine, metrics, "2026-03-31")) == as_json(
            covenants.evaluate_engine(engine, metrics, datetime.date(2026, 3, 31))
        )

    def test_a_pandas_timestamp_is_also_accepted(self) -> None:
        """`pandas.Timestamp` is what a quant actually holds after a `read_csv`."""
        from finstack_quant.cashflows import accrued_interest

        schedule = self._schedule()
        assert accrued_interest(schedule, pd.Timestamp("2025-02-28")) == (accrued_interest(schedule, "2025-02-28"))

    def test_factor_model_decompose_levels(self) -> None:
        """A third, structurally different entry point: takes a `time::Date`.

        `accrued_interest` and `evaluate_engine` forward an ISO string to
        the crate; `decompose_levels` converts to a `time::Date` instead, so it
        exercises the other half of the helper pair. Comparing the stamped
        ``date`` catches an off-by-one in the conversion, which comparing only
        the numeric output would not.
        """
        from finstack_quant.models.factor.credit import CreditCalibrator, decompose_levels

        config_json = json.dumps({
            "policy": "globally_off",
            "hierarchy": {"levels": []},
            "min_bucket_size_per_level": {"per_level": []},
            "vol_model": "sample",
            "covariance_strategy": "diagonal",
            "beta_shrinkage": "none",
            "use_returns_or_levels": "returns",
            "panel_frequency": "monthly",
            "bucket_weighting": "equal",
        })
        inputs_json = json.dumps({
            "history_panel": {
                "dates": ["2024-01-01", "2024-02-01"],
                "spreads": {"A": [0.010, 0.0101]},
            },
            "issuer_tags": {"tags": {"A": {}}},
            "generic_factor": {
                "spec": {"name": "G", "series_id": "G"},
                "values": [0.010, 0.0101],
            },
            "as_of": "2024-02-01",
            "as_of_spreads": {"A": 0.0101},
            "idiosyncratic_overrides": {},
        })
        model = CreditCalibrator(config_json).calibrate(inputs_json)

        from_str = decompose_levels(model, '{"A": 0.0125}', 0.012, "2025-06-30")
        from_date = decompose_levels(model, '{"A": 0.0125}', 0.012, datetime.date(2025, 6, 30))
        assert from_str.date == from_date.date
        assert from_str.generic == from_date.generic

    def test_a_non_date_argument_raises_a_helpful_type_error(self) -> None:
        """The newly-migrated sites must fail the way the original six do.

        An `int` is the realistic mistake (``20260331`` from a CSV column), and
        the message has to name the fix rather than only the failure.
        """
        from finstack_quant import covenants

        specs = json.loads(covenants.lbo_standard_json(6.0, 2.0, 1.1, 50_000_000.0))
        engine = covenants.validate_covenant_engine_json(
            json.dumps({
                "specs": [specs[0]],
                "breach_history": [],
                "windows": [],
                "waivers": [],
            })
        )
        with pytest.raises(TypeError, match="expected a date-like object") as excinfo:
            covenants.evaluate_engine(engine, json.dumps({"debt_to_ebitda": 4.0}), 20260331)
        message = str(excinfo.value)
        assert "datetime.date" in message
        assert "ISO-8601" in message
        assert "got int" in message

    def test_period_taking_functions_still_require_a_string(self) -> None:
        """`credit_assessment` takes a fiscal PERIOD (``"2025Q4"``), not a date.

        It is deliberately excluded from the date migration; pin that so a later
        sweep does not "fix" it into accepting a `datetime.date`, which has no
        meaning for a period identifier.
        """
        import inspect

        from finstack_quant.statements_analytics import credit_assessment

        assert "period" in inspect.signature(credit_assessment).parameters
        assert "as_of" not in inspect.signature(credit_assessment).parameters
