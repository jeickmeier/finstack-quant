from __future__ import annotations

from datetime import date
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import DiscountCurve, FxMatrix, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.portfolio import Portfolio, PortfolioAttribution, attribute_portfolio_pnl

AS_OF_T0 = "2025-01-15"
AS_OF_T1 = "2025-01-16"


def _portfolio_json(currency: str = "USD", base_ccy: str = "USD") -> str:
    curve_id = f"{currency}-OIS"
    return json.dumps({
        "id": f"{currency}-ATTR",
        "as_of": AS_OF_T0,
        "base_ccy": base_ccy,
        "entities": {"FUND": {"id": "FUND"}},
        "positions": [
            {
                "position_id": f"{currency}-POS",
                "entity_id": "FUND",
                "instrument_id": f"{currency}-DEP",
                "instrument_spec": {
                    "type": "deposit",
                    "spec": {
                        "id": f"{currency}-DEP",
                        "notional": {"amount": 1_000_000.0, "currency": currency},
                        "start_date": AS_OF_T0,
                        "maturity": "2025-07-15",
                        "day_count": "Act360",
                        "quote_rate": 0.04,
                        "discount_curve_id": curve_id,
                        "attributes": {},
                    },
                },
                "quantity": 1.0,
                "unit": "units",
            }
        ],
    })


def _market(currency: str, as_of: str, shift: float = 0.0, fx: float | None = None) -> MarketContext:
    market = MarketContext()
    market.insert(
        DiscountCurve(
            f"{currency}-OIS",
            date.fromisoformat(as_of),
            [(0.0, 1.0), (0.5, 0.98 - shift), (1.0, 0.95 - shift)],
            day_count="act_365f",
        )
    )
    if fx is not None:
        matrix = FxMatrix()
        matrix.set_quote(Currency("EUR"), Currency("USD"), fx)
        market.insert_fx(matrix)
    return market


def _ordered_two_position_portfolio_json() -> str:
    payload = json.loads(_portfolio_json())
    first = payload["positions"][0]
    second = json.loads(json.dumps(first))
    first["position_id"] = "USD-POS-Z"
    second["position_id"] = "USD-POS-A"
    second["instrument_id"] = "USD-DEP-2"
    second["instrument_spec"]["spec"]["id"] = "USD-DEP-2"
    payload["positions"] = [first, second]
    return json.dumps(payload)


def test_attribute_portfolio_pnl_returns_typed_aggregate_money_and_nested_json() -> None:
    portfolio = Portfolio.from_spec(_portfolio_json())
    market_t0 = _market("USD", AS_OF_T0)
    market_t1 = _market("USD", AS_OF_T1, shift=0.002)

    result = attribute_portfolio_pnl(
        portfolio,
        market_t0,
        market_t1,
        AS_OF_T0,
        AS_OF_T1,
        "Parallel",
    )

    assert isinstance(result, PortfolioAttribution)
    assert isinstance(result.total_pnl, Money)
    assert isinstance(result.rates_curves_pnl, Money)
    assert result.total_pnl.currency == Currency("USD")
    assert result.rates_curves_pnl.amount != 0.0
    assert result.result_invalid is False

    by_position = json.loads(result.by_position_json())
    assert list(by_position) == ["USD-POS"]
    assert by_position["USD-POS"]["total_pnl"]["currency"] == "USD"

    report = result.reconciliation_check(1.0e-8)
    assert report["is_reconciled"] is True
    assert report["tolerance"] == pytest.approx(1.0e-8)
    assert abs(float(report["total_residual"])) <= 1.0e-8

    restored = json.loads(result.to_json())
    assert restored["by_position"] == by_position
    assert restored["total_pnl"]["currency"] == "USD"


def test_nested_attribution_json_preserves_portfolio_insertion_order() -> None:
    result = attribute_portfolio_pnl(
        _ordered_two_position_portfolio_json(),
        _market("USD", AS_OF_T0),
        _market("USD", AS_OF_T1, shift=0.002),
        AS_OF_T0,
        AS_OF_T1,
        "Parallel",
    )

    assert list(json.loads(result.by_position_json())) == ["USD-POS-Z", "USD-POS-A"]


def test_attribute_portfolio_pnl_accepts_json_extractors_and_tracks_fx_translation() -> None:
    market_t0 = _market("EUR", AS_OF_T0, fx=1.10)
    market_t1 = _market("EUR", AS_OF_T1, fx=1.12)

    typed = attribute_portfolio_pnl(
        Portfolio.from_spec(_portfolio_json("EUR")),
        market_t0,
        market_t1,
        AS_OF_T0,
        AS_OF_T1,
        "Parallel",
    )
    from_json = attribute_portfolio_pnl(
        _portfolio_json("EUR"),
        market_t0.to_json(),
        market_t1.to_json(),
        AS_OF_T0,
        AS_OF_T1,
        "Parallel",
    )

    assert typed.fx_translation_pnl.currency == Currency("USD")
    assert typed.fx_translation_pnl.amount > 0.0
    assert json.loads(typed.to_json()) == json.loads(from_json.to_json())
