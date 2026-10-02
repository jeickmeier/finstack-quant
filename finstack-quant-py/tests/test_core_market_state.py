"""Credit-index references stay canonical across Python market mutations."""

import json
import math
import pickle

import pytest

from finstack_quant.core.market_data import (
    BaseCorrelationCurve,
    CreditIndexData,
    DiscountCurve,
    HazardCurve,
    MarketContext,
)
from finstack_quant.portfolio import (
    ContractLimitExceededError,
    ContractValidationError,
)


def test_credit_index_resolves_canonical_curves_before_snapshot() -> None:
    canonical = HazardCurve.flat("HAZ", "2025-01-01", 0.02, 0.4)
    stale = HazardCurve.flat("HAZ", "2025-01-01", 0.10, 0.4)
    correlation = BaseCorrelationCurve("CORR", [(3.0, 0.2), (100.0, 0.3)])
    market = MarketContext().insert(canonical).insert(correlation)
    assert market.insert_credit_index("INDEX", CreditIndexData(125, 0.4, stale, correlation)) is market
    restored = MarketContext.from_json(market.to_json())
    market.insert(DiscountCurve("UNRELATED", "2025-01-01", [(0.0, 1.0), (5.0, 0.9)]))
    for context in (market, restored):
        assert context.get_credit_index("INDEX").index_credit_curve.sp(5.0) == pytest.approx(math.exp(-0.1))


def test_credit_index_missing_or_wrong_type_reference_is_atomic() -> None:
    hazard = HazardCurve.flat("HAZ", "2025-01-01", 0.02, 0.4)
    correlation = BaseCorrelationCurve("CORR", [(3.0, 0.2), (100.0, 0.3)])
    index = CreditIndexData(125, 0.4, hazard, correlation)
    market = MarketContext().insert(correlation)
    before = market.to_json()
    with pytest.raises(KeyError, match="HAZ"):
        market.insert_credit_index("INDEX", index)
    assert market.to_json() == before

    market.insert(DiscountCurve("HAZ", "2025-01-01", [(0.0, 1.0), (5.0, 0.9)]))
    before = market.to_json()
    with pytest.raises(ValueError, match="HAZ"):
        market.insert_credit_index("INDEX", index)
    assert market.to_json() == before


def _state() -> dict[str, object]:
    market = MarketContext().insert(DiscountCurve.flat("USD-OIS", "2025-01-02", 0.04))
    return json.loads(market.to_json())


def test_from_json_is_the_strict_persisted_state_load() -> None:
    """Twin of the ``market re-ingestion`` tests in ``tests/facade/calibration_errors.test.mjs``."""
    state = _state()
    assert state["schema_version"] == 1
    assert json.loads(MarketContext.from_json(json.dumps(state)).to_json()) == state
    restored = pickle.loads(pickle.dumps(MarketContext.from_json(json.dumps(state))))  # noqa: S301
    assert json.loads(restored.to_json()) == state


def test_from_json_rejects_nesting_beyond_the_canonical_depth_limit() -> None:
    state = _state()
    node: dict[str, object] = {}
    for level in range(50):
        node = {"children": {f"Level{level}": node}}
    state["hierarchy"] = {"roots": {"Rates": node}}
    with pytest.raises(ContractLimitExceededError, match=r"JSON depth.*96") as excinfo:
        MarketContext.from_json(json.dumps(state))
    assert isinstance(excinfo.value, ContractValidationError)
    assert isinstance(excinfo.value, ValueError)


def test_from_json_requires_an_explicit_supported_schema_version() -> None:
    unversioned = _state()
    del unversioned["schema_version"]
    for state, code in (
        (unversioned, "contract/version-missing"),
        ({**unversioned, "schema_version": 2}, "contract/version-unsupported"),
    ):
        with pytest.raises(ContractValidationError) as excinfo:
            MarketContext.from_json(json.dumps(state))
        assert [(item["code"], item["pointer"]) for item in excinfo.value.report] == [(code, "/schema_version")]


@pytest.mark.parametrize(
    ("payload", "code"),
    [
        ("{ malformed", "contract/parse-error"),
        (json.dumps({"schema_version": 1, "unexpected": 1}), "contract/structure-invalid"),
    ],
)
def test_from_json_reports_malformed_and_unknown_state_as_contract_errors(payload: str, code: str) -> None:
    with pytest.raises(ContractValidationError) as excinfo:
        MarketContext.from_json(payload)
    assert [item["code"] for item in excinfo.value.report] == [code]
