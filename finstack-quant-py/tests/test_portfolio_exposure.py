from datetime import date

import pytest

from finstack_quant.core.config import FinstackConfig
from finstack_quant.core.market_data import MarketContext
from finstack_quant.margin import NettingSetId
from finstack_quant.portfolio import (
    ExposureSimulationConfig,
    Portfolio,
    instrument_exposure_profile,
    portfolio_exposure_profile,
)
from finstack_quant.valuations.instruments import Equity


def test_dated_paths_reprice_signed_positions_without_mutating_the_market() -> None:
    dates = [date(2025, 1, 2), date(2026, 1, 2)]
    market = MarketContext().insert_price("AAPL-SPOT", 100, "USD")
    snapshot = market.to_json()
    equity = Equity.example()
    book = Portfolio.builder("short-book", "USD", dates[0]).position("short", equity, -2).build()

    def scope(position_ids: list[str]) -> ExposureSimulationConfig:
        return ExposureSimulationConfig(
            netting_set=NettingSetId.bilateral("dealer", "agreement"),
            netting_source_id="trading-book",
            position_ids=position_ids,
            dates=dates,
            factors=[{"kind": "equity", "state": 0, "price_id": "AAPL-SPOT"}],
            simulation={
                "process": {"type": "gbm", "r": 0.08, "q": 0.01, "sigma": 0.0},
                "initial_state": [100.0],
                "time_grid": {"type": "times", "times": [0.0, 1.0]},
                "num_paths": 5,
                "seed": 42,
                "antithetic": True,
            },
        )

    standalone = instrument_exposure_profile(equity, "USD", market, FinstackConfig(), scope([equity.id]))
    book_result = portfolio_exposure_profile(book, market, FinstackConfig(), scope(["short"]))
    assert standalone.dates == dates == book_result.dates
    assert standalone.epe[0] == 10_000.0
    assert book_result.ene == pytest.approx([2 * value for value in standalone.epe])
    assert book_result.mtm_values[1] < book_result.mtm_values[0]
    assert book_result.basis["path_count"] == 10
    assert market.to_json() == snapshot

    with pytest.raises(ValueError, match="exact position IDs"):
        portfolio_exposure_profile(book, market, FinstackConfig(), scope(["wrong"]))
