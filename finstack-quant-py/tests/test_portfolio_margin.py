"""Portfolio margin aggregation binding.

Covers the typed surface over the Rust ``PortfolioMarginAggregator``:
``from_portfolio`` / ``calculate``, the ``PortfolioMarginResult`` and
``NettingSetMargin`` getters, and their JSON, pickle and DataFrame exits.
"""

from __future__ import annotations

from datetime import date
import json
import pickle

import pandas as pd
import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.margin import ImMethodology, NettingSetId
from finstack_quant.portfolio import (
    NettingSetMargin,
    Portfolio,
    PortfolioMarginAggregator,
    PortfolioMarginResult,
)

AS_OF = "2025-01-15"

NETTING_SET_COLUMNS = [
    "netting_set_id",
    "csa_id",
    "is_cleared",
    "initial_margin",
    "variation_margin",
    "total_margin",
    "currency",
    "position_count",
    "im_methodology",
    "is_approximate",
]


def _usd(amount: str) -> dict[str, str]:
    return {"amount": amount, "currency": "USD"}


def _netting_set_doc(
    netting_set_id: dict[str, str],
    csa_id: str | None,
    initial: str,
    variation: str,
    total: str,
    positions: int,
    methodology: str,
) -> dict[str, object]:
    return {
        "netting_set_id": netting_set_id,
        "csa_id": csa_id,
        "as_of": AS_OF,
        "initial_margin": _usd(initial),
        "variation_margin": _usd(variation),
        "total_margin": _usd(total),
        "position_count": positions,
        "im_methodology": methodology,
        "is_approximate": False,
        "sensitivities": None,
        "im_breakdown": {},
    }


def _result_doc() -> dict[str, object]:
    """Two netting sets; the cleared one collects VM, so it adds no VM to its total."""
    bilateral = _netting_set_doc(
        {"kind": "bilateral", "counterparty_id": "BANK_A", "csa_id": "CSA_01"},
        None,
        "100",
        "25",
        "125",
        2,
        "schedule",
    )
    cleared = _netting_set_doc({"kind": "cleared", "ccp_id": "LCH"}, None, "40", "-10", "40", 3, "clearing_house")
    return {
        "as_of": AS_OF,
        "base_currency": "USD",
        "total_initial_margin": _usd("140"),
        "by_csa": {},
        "total_required_im_collateral": _usd("0"),
        "total_im_transfer": _usd("0"),
        "total_segregated_im": _usd("0"),
        "total_variation_margin": _usd("15"),
        "total_margin": _usd("165"),
        "netting_sets": [cleared, bilateral],
        "total_positions": 5,
        "positions_without_margin": 1,
        "degraded_positions": [{"position_id": "POS-X", "message": "no sensitivities"}],
    }


def _unmargined_portfolio() -> Portfolio:
    """One deposit; deposits carry no margin specification."""
    return Portfolio.from_spec(
        json.dumps({
            "id": "MARGIN-BOOK",
            "as_of": AS_OF,
            "base_currency": "USD",
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
                            "discount_curve_id": "USD-OIS",
                            "attributes": {},
                        },
                    },
                    "quantity": 1.0,
                    "unit": "units",
                }
            ],
        })
    )


def _market() -> MarketContext:
    context = MarketContext()
    context.insert(
        DiscountCurve(
            "USD-OIS", date.fromisoformat(AS_OF), [(0.0, 1.0), (0.5, 0.98), (1.0, 0.95)], day_count="act_365f"
        )
    )
    return context


def test_calculate_on_an_unmargined_portfolio_reports_no_netting_sets() -> None:
    portfolio = _unmargined_portfolio()
    aggregator = PortfolioMarginAggregator.from_portfolio(portfolio)
    result = aggregator.calculate(portfolio, _market(), AS_OF)

    assert isinstance(result, PortfolioMarginResult)
    assert result.as_of == date.fromisoformat(AS_OF)
    assert result.base_currency == "USD"
    assert result.by_netting_set == {}
    assert result.by_csa == {}
    assert result.total_positions == 0
    assert result.positions_without_margin == 1
    assert result.degraded_positions == []
    for total in (
        result.total_initial_margin,
        result.total_variation_margin,
        result.total_margin,
        result.total_required_im_collateral,
        result.total_im_transfer,
        result.total_segregated_im,
    ):
        assert isinstance(total, Money)
        assert total.amount == 0.0

    frame = result.to_dataframe()
    assert isinstance(frame, pd.DataFrame)
    assert list(frame.columns) == NETTING_SET_COLUMNS
    assert len(frame) == 0

    # The aggregator is reusable, and accepts a date object and JSON inputs.
    again = aggregator.calculate(portfolio, _market().to_json(), date.fromisoformat(AS_OF), current_im_collateral={})
    assert again.to_json() == result.to_json()


def test_calculate_rejects_collateral_for_an_unknown_csa() -> None:
    portfolio = _unmargined_portfolio()
    aggregator = PortfolioMarginAggregator.from_portfolio(portfolio)
    with pytest.raises(ValueError, match="Unknown IM collateral CSA"):
        aggregator.calculate(portfolio, _market(), AS_OF, current_im_collateral={"NO-SUCH-CSA": Money("1", "USD")})
    with pytest.raises(TypeError):
        aggregator.calculate(portfolio, _market(), 20250115)


def test_result_getters_reconcile_with_netting_set_rows() -> None:
    result = PortfolioMarginResult.from_json(json.dumps(_result_doc()))

    by_netting_set = result.by_netting_set
    bilateral_id = NettingSetId.bilateral("BANK_A", "CSA_01")
    cleared_id = NettingSetId.cleared("LCH")
    # Ascending identifier-string order, independent of the input order.
    assert list(by_netting_set) == [bilateral_id, cleared_id]
    bilateral = by_netting_set[bilateral_id]
    cleared = by_netting_set[cleared_id]
    assert isinstance(bilateral, NettingSetMargin)
    assert bilateral.netting_set_id == bilateral_id
    assert not bilateral.is_cleared
    assert cleared.is_cleared
    assert bilateral.im_methodology == ImMethodology.schedule()
    assert cleared.im_methodology == ImMethodology.clearing_house()
    assert bilateral.as_of == date.fromisoformat(AS_OF)
    assert bilateral.csa_id is None
    assert bilateral.sensitivities is None
    assert bilateral.im_breakdown == {}
    assert bilateral.is_approximate is False
    assert (bilateral.position_count, cleared.position_count) == (2, 3)

    # total_margin = IM + max(VM, 0) per netting set.
    for margin in (bilateral, cleared):
        expected = margin.initial_margin.amount + max(margin.variation_margin.amount, 0.0)
        assert margin.total_margin.amount == pytest.approx(expected)

    # Portfolio totals are the netting-set sums.
    margins = list(by_netting_set.values())
    assert result.total_initial_margin.amount == pytest.approx(sum(m.initial_margin.amount for m in margins))
    assert result.total_variation_margin.amount == pytest.approx(sum(m.variation_margin.amount for m in margins))
    assert result.total_margin.amount == pytest.approx(sum(m.total_margin.amount for m in margins))
    assert result.total_positions == sum(m.position_count for m in margins)
    assert result.positions_without_margin == 1
    assert result.degraded_positions == [("POS-X", "no sensitivities")]

    frame = result.to_dataframe()
    assert list(frame.columns) == NETTING_SET_COLUMNS
    assert list(frame["netting_set_id"]) == ["BANK_A:CSA_01", "CCP:LCH"]
    assert list(frame["is_cleared"]) == [False, True]
    assert list(frame["im_methodology"]) == ["schedule", "clearing_house"]
    assert list(frame["currency"]) == ["USD", "USD"]
    assert frame["initial_margin"].sum() == pytest.approx(result.total_initial_margin.amount)
    assert frame["variation_margin"].sum() == pytest.approx(result.total_variation_margin.amount)
    assert frame["total_margin"].sum() == pytest.approx(result.total_margin.amount)
    assert int(frame["position_count"].sum()) == result.total_positions

    breakdown = bilateral.to_dataframe()
    assert list(breakdown.columns) == ["netting_set_id", "risk_class", "initial_margin", "currency"]
    assert len(breakdown) == 0


def test_results_round_trip_through_json_and_pickle() -> None:
    result = PortfolioMarginResult.from_json(json.dumps(_result_doc()))
    assert PortfolioMarginResult.from_json(result.to_json()).to_json() == result.to_json()
    assert pickle.loads(pickle.dumps(result)).to_json() == result.to_json()  # noqa: S301
    # Canonical order on the wire is ascending identifier, whatever the input order.
    assert [ns["netting_set_id"]["kind"] for ns in json.loads(result.to_json())["netting_sets"]] == [
        "bilateral",
        "cleared",
    ]

    margin = next(iter(result.by_netting_set.values()))
    assert NettingSetMargin.from_json(margin.to_json()).to_json() == margin.to_json()
    assert pickle.loads(pickle.dumps(margin)).to_json() == margin.to_json()  # noqa: S301
    assert "BANK_A:CSA_01" in repr(margin)
    assert "netting_sets=2" in repr(result)


def test_inconsistent_totals_are_rejected() -> None:
    doc = _result_doc()
    doc["total_initial_margin"] = _usd("999")
    with pytest.raises(ValueError, match="totals do not equal netting-set sums"):
        PortfolioMarginResult.from_json(json.dumps(doc))
