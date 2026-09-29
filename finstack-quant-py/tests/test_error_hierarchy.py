"""Tests for the shared ``FinstackError`` exception base and its compatibility guarantees.

``FinstackError`` lets a caller write ``except FinstackError`` instead of
enumerating every named exception the library defines. Because it was inserted
*above* pre-existing classes rather than replacing their bases, the tests below
also pin the backward-compatibility invariant that matters most: everything that
``except ValueError`` used to catch, it still catches.

``pyo3::create_exception!`` accepts a single base type, so one named exception
stays outside the tree: ``CalibrationEnvelopeError`` derives from
``RuntimeError``, which cannot also be a ``ValueError`` without multiple
inheritance. ``CholeskyError`` *has* been folded in, and the tests below assert
that rather than only its older ``ValueError`` ancestry.

The ``issubclass`` assertions pin the class *tree*; they cannot pin the error
*mapping*, because an exception the test raises itself proves nothing about what
the extension raises. ``TestRealFailuresRaiseTheMappedClass`` therefore drives
each family to a genuine library failure and asserts the class that comes back,
plus the ``except ValueError`` invariant on that same real failure.
"""

from __future__ import annotations

from datetime import date
import json
from typing import Any

import pytest

from finstack_quant.analytics import AnalyticsError, constrained_least_squares
from finstack_quant.calibration import CalibrationEnvelopeError
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.math import linalg
from finstack_quant.core.money import Money
from finstack_quant.portfolio import (
    ContractLimitExceededError,
    ContractValidationError,
    FinstackError,
    MalformedContractSchemaError,
    MissingContractVersionError,
    Portfolio,
    PortfolioError,
    PortfolioResult,
    UnsupportedContractVersionError,
    value_portfolio,
)

REPARENTED_ERRORS = [
    AnalyticsError,
    PortfolioError,
    ContractValidationError,
    UnsupportedContractVersionError,
    MissingContractVersionError,
    MalformedContractSchemaError,
    ContractLimitExceededError,
    linalg.CholeskyError,
]

# A distinctive, non-round total so an accessor that returns a hard-coded or
# defaulted zero is visible. `"0"` would make every equality below `0.0 == 0.0`.
TOTAL_BASE_AMOUNT = 12_345.67

AS_OF = "2025-01-15"


def _result_json(metrics: dict[str, Any] | None = None) -> str:
    """Build a minimal canonical ``PortfolioResult`` envelope as JSON."""
    aggregated = metrics if metrics is not None else {}
    return json.dumps({
        "schema_version": 1,
        "valuation": {
            "as_of": "2025-01-01",
            "position_values": {},
            "total_base_currency": {"amount": "12345.67", "currency": "USD"},
            "by_entity": {},
        },
        "metrics": {"aggregated": aggregated, "by_position": {}},
        "meta": {
            "numeric_mode": "f64",
            "rounding": {
                "mode": "bankers",
                "ingest_scale_by_currency": {},
                "output_scale_by_currency": {},
                "tolerances": {"rate_epsilon": 1e-12, "generic_epsilon": 1e-10},
                "version": 1,
            },
        },
    })


def _portfolio_json(base_currency: str = "USD", discount_curve_id: str = "USD-OIS") -> str:
    """Single USD deposit position, parameterised on the two failure levers."""
    return json.dumps({
        "id": "ERR-HIERARCHY",
        "as_of": AS_OF,
        "base_currency": base_currency,
        "entities": {"FUND": {"id": "FUND"}},
        "positions": [
            {
                "position_id": "USD-POS",
                "entity_id": "FUND",
                "instrument_id": "USD-DEP",
                "instrument_spec": {
                    "type": "deposit",
                    "spec": {
                        "id": "USD-DEP",
                        "notional": {"amount": "1000000", "currency": "USD"},
                        "start_date": AS_OF,
                        "maturity": "2025-07-15",
                        "day_count": "act_360",
                        "fixed_rate": "0.04",
                        "discount_curve_id": discount_curve_id,
                        "attributes": {},
                    },
                },
                "quantity": 1.0,
                "unit": "units",
            }
        ],
    })


def _usd_market() -> MarketContext:
    market = MarketContext()
    market.insert(
        DiscountCurve(
            "USD-OIS",
            date.fromisoformat(AS_OF),
            [(0.0, 1.0), (0.5, 0.98), (1.0, 0.95)],
            day_count="act_365f",
        )
    )
    return market


def _materialization_bundle(schema: str) -> str:
    """Materialization bundle whose only variable is its contract marker."""
    return json.dumps({
        "schema": schema,
        "portfolio": {
            "id": "materialized",
            "base_currency": "USD",
            "as_of": "2025-01-01",
            "entities": {"entity": {"id": "entity", "name": None}},
        },
        "instruments": [],
        "positions": [],
    })


class TestFinstackErrorBase:
    """``FinstackError`` exists and sits above the named exception families."""

    def test_is_exception_subclass(self) -> None:
        """The base is a real exception type, usable in an ``except`` clause."""
        assert isinstance(FinstackError, type)
        assert issubclass(FinstackError, Exception)

    def test_is_raisable_and_catchable(self) -> None:
        """A bare ``FinstackError`` can be raised and caught."""
        with pytest.raises(FinstackError, match="boom"):
            raise FinstackError("boom")

    def test_derives_from_value_error(self) -> None:
        """The base derives from ``ValueError`` so reparenting broke no caller.

        ``create_exception!`` allows only one base class, so ``FinstackError``
        could not be spliced in as a second base. Deriving it from ``ValueError``
        instead keeps ``ValueError`` in every subclass's MRO.
        """
        assert issubclass(FinstackError, ValueError)

    @pytest.mark.parametrize("error_type", REPARENTED_ERRORS, ids=lambda t: t.__name__)
    def test_named_errors_are_finstack_errors(self, error_type: type[Exception]) -> None:
        """Every reparented exception is caught by ``except FinstackError``."""
        assert issubclass(error_type, FinstackError)
        with pytest.raises(FinstackError):
            raise error_type("failure")

    def test_single_clause_catches_across_domains(self) -> None:
        """One ``except FinstackError`` spans the analytics and portfolio families."""
        caught: list[str] = []
        for error_type in (AnalyticsError, PortfolioError, ContractValidationError):
            try:
                raise error_type("failure")
            except FinstackError as exc:
                caught.append(type(exc).__name__)
        assert caught == ["AnalyticsError", "PortfolioError", "ContractValidationError"]


class TestRealFailuresRaiseTheMappedClass:
    """Real library failures, not hand-raised stand-ins.

    Every assertion above this class is structural: it would keep passing if the
    binding's error *mapping* were reverted to bare ``ValueError``, because the
    exceptions are raised by the test itself. The tests here call into the
    extension until it fails on its own, so they pin the mapping rather than the
    class tree.
    """

    def test_analytics_dimension_mismatch_raises_analytics_error(self) -> None:
        """Analytics input validation surfaces as ``AnalyticsError``."""
        with pytest.raises(AnalyticsError, match="Input dimensions do not match"):
            constrained_least_squares([0.0, 1.0, 1.0], 2, [0.05, 0.02, 0.01], [0.6, 0.3, 0.1])

    def test_analytics_failure_is_still_caught_by_value_error(self) -> None:
        """The same real failure keeps satisfying a pre-existing ``except ValueError``."""
        with pytest.raises(ValueError, match="Input dimensions do not match") as excinfo:
            constrained_least_squares([0.0, 1.0, 1.0], 2, [0.05, 0.02, 0.01], [0.6, 0.3, 0.1])
        assert isinstance(excinfo.value, AnalyticsError)
        assert isinstance(excinfo.value, FinstackError)

    def test_missing_curve_raises_key_error(self) -> None:
        """A position priced off an absent curve is a lookup miss: ``KeyError``.

        The portfolio error carries the kind of the pricing failure it wraps,
        so the class matches ``price_instrument`` on the same position (and the
        WASM error ``kind`` of ``not_found``).
        """
        portfolio = Portfolio.from_spec(_portfolio_json(discount_curve_id="MISSING-CURVE"))
        with pytest.raises(KeyError, match="MISSING-CURVE") as excinfo:
            value_portfolio(portfolio, MarketContext())
        assert not isinstance(excinfo.value, PortfolioError)

    def test_missing_fx_matrix_raises_key_error(self) -> None:
        """Collapsing a USD position into an EUR base without FX is a missing FX rate.

        Same portfolio and market as the control, only the base currency
        differs, so the trigger is the absent rate.
        """
        portfolio = Portfolio.from_spec(_portfolio_json(base_currency="EUR"))
        with pytest.raises(KeyError, match=r"(?i)fx"):
            value_portfolio(portfolio, _usd_market())

    def test_the_same_market_prices_cleanly_in_the_native_base(self) -> None:
        """Control for the two portfolio triggers: neither fixture is simply broken."""
        valuation = value_portfolio(Portfolio.from_spec(_portfolio_json()), _usd_market())
        assert len(valuation) == 1

    def test_bad_contract_marker_raises_contract_validation_error(self) -> None:
        """An unknown persisted-contract version is a ``ContractValidationError``."""
        bundle = _materialization_bundle("finstack_quant.portfolio_materialization/99")
        with pytest.raises(ContractValidationError, match="validation failed with") as excinfo:
            Portfolio.from_materialization(bundle)
        # The structured report is the reason this class exists at all.
        report = excinfo.value.report
        assert len(report) == 1
        assert report[0]["severity"] == "error"
        assert "finstack_quant.portfolio_materialization/99" in report[0]["message"]

    def test_contract_failure_is_still_caught_by_value_error(self) -> None:
        """The contract family stayed inside ``ValueError`` too."""
        bundle = _materialization_bundle("not-a-contract-marker")
        with pytest.raises(ValueError, match="validation failed with") as excinfo:
            Portfolio.from_materialization(bundle)
        assert isinstance(excinfo.value, ContractValidationError)
        assert isinstance(excinfo.value, FinstackError)

    def test_non_positive_definite_matrix_raises_cholesky_error(self) -> None:
        """``cholesky_decomposition`` maps a factorisation failure to ``CholeskyError``."""
        with pytest.raises(linalg.CholeskyError, match=r"(?i)positive semi-definite"):
            linalg.cholesky_decomposition([[1.0, 2.0], [2.0, 1.0]])

    def test_cholesky_failure_is_still_caught_by_value_error(self) -> None:
        """``CholeskyError`` sits under ``ValueError`` but outside ``FinstackError``."""
        with pytest.raises(ValueError, match=r"(?i)positive semi-definite") as excinfo:
            linalg.cholesky_decomposition([[1.0, 2.0], [2.0, 1.0]])
        assert isinstance(excinfo.value, linalg.CholeskyError)
        assert isinstance(excinfo.value, FinstackError)

    def test_a_positive_definite_matrix_still_factorises(self) -> None:
        """Control: the trigger above fails on the matrix, not on the call shape."""
        lower = linalg.cholesky_decomposition([[4.0, 2.0], [2.0, 5.0]])
        assert lower[0][0] == pytest.approx(2.0)

    def test_one_clause_catches_every_real_finstack_failure(self) -> None:
        """``except FinstackError`` spans real analytics and contract failures."""
        triggers = [
            lambda: constrained_least_squares([0.0, 1.0, 1.0], 2, [0.05, 0.02, 0.01], [0.6, 0.3, 0.1]),
            lambda: Portfolio.from_materialization(_materialization_bundle("bad-marker")),
        ]
        caught: list[str] = []
        for trigger in triggers:
            try:
                trigger()
            except FinstackError as exc:
                caught.append(type(exc).__name__)
        assert caught == [
            "AnalyticsError",
            "ContractValidationError",
        ]


class TestBackwardCompatibility:
    """Pre-existing ``except`` clauses must keep behaving identically."""

    def test_analytics_error_still_caught_by_value_error(self) -> None:
        """``AnalyticsError`` documented itself as a ``ValueError``; it still is."""
        assert issubclass(AnalyticsError, ValueError)
        with pytest.raises(ValueError, match="invalid analytics input"):
            raise AnalyticsError("invalid analytics input")

    @pytest.mark.parametrize("error_type", REPARENTED_ERRORS, ids=lambda t: t.__name__)
    def test_reparented_errors_still_caught_by_value_error(self, error_type: type[Exception]) -> None:
        """Inserting ``FinstackError`` kept ``ValueError`` in every MRO."""
        assert issubclass(error_type, ValueError)
        with pytest.raises(ValueError, match="failure"):
            raise error_type("failure")

    def test_contract_subclasses_keep_their_intermediate_base(self) -> None:
        """The contract family kept its own two-level shape."""
        for error_type in (
            UnsupportedContractVersionError,
            MissingContractVersionError,
            MalformedContractSchemaError,
            ContractLimitExceededError,
        ):
            assert issubclass(error_type, ContractValidationError)

    def test_currency_mismatch_still_raises_value_error(self) -> None:
        """Bare ``ValueError`` from core mapping helpers was not reclassified."""
        with pytest.raises(ValueError, match=r"(?i)currency|mismatch"):
            Money(100.0, Currency("USD")) + Money(50.0, Currency("EUR"))

    def test_unclassified_core_errors_stay_outside_the_tree(self) -> None:
        """Core still raises bare ``ValueError``, not a ``FinstackError`` subclass.

        Reclassifying these would silently change the exception type at every
        existing call site, so they were deliberately left alone.
        """
        with pytest.raises(ValueError, match=r"(?i)currency|mismatch") as exc_info:
            Money(100.0, Currency("USD")) + Money(50.0, Currency("EUR"))
        assert not isinstance(exc_info.value, FinstackError)


class TestErrorsOutsideTheTree:
    """Exceptions that could not join the tree keep their documented ancestry."""

    def test_calibration_envelope_error_is_runtime_error(self) -> None:
        """``CalibrationEnvelopeError`` derives from ``RuntimeError``.

        It cannot also derive from ``FinstackError`` (a ``ValueError``) without
        multiple inheritance, which ``create_exception!`` cannot express, so it
        is the one named exception that stays outside the tree.
        """
        assert issubclass(CalibrationEnvelopeError, RuntimeError)
        assert not issubclass(CalibrationEnvelopeError, FinstackError)

    def test_cholesky_error_was_folded_into_the_tree(self) -> None:
        """``CholeskyError`` is a ``FinstackError``, not merely a ``ValueError``.

        Asserting only ``ValueError`` here would keep passing if the reparenting
        were reverted, which is exactly the regression this file exists to catch.
        """
        assert issubclass(linalg.CholeskyError, FinstackError)
        assert issubclass(linalg.CholeskyError, ValueError)


class TestResultAccessors:
    """Scalar accessors live on ``PortfolioResult`` itself.

    The free ``portfolio_result_*`` functions were removed as duplicates.
    """

    def test_total_value_property(self) -> None:
        result = PortfolioResult.from_json(_result_json())
        assert result.total_value == pytest.approx(TOTAL_BASE_AMOUNT)

    def test_get_metric_method(self) -> None:
        metrics = {"dv01": {"metric_id": "dv01", "total": 12.5, "by_entity": {}}}
        result = PortfolioResult.from_json(_result_json(metrics))
        assert result.get_metric("dv01") == pytest.approx(12.5)
        assert result.get_metric("absent") is None

    def test_require_metric_raises_key_error(self) -> None:
        result = PortfolioResult.from_json(_result_json())
        with pytest.raises(KeyError):
            result.require_metric("absent")

    def test_typed_components_round_trip(self) -> None:
        result = PortfolioResult.from_json(_result_json())
        rebuilt = PortfolioResult(result.valuation, result.metrics)
        assert rebuilt.total_value == pytest.approx(result.total_value)
        assert "rounding" in rebuilt.meta
