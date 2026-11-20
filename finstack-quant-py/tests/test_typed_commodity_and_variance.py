from datetime import date

import pytest

from finstack_quant.valuations.instruments import (
    CommodityAsianOption,
    CommodityForward,
    CommodityFuture,
    CommodityFutureOption,
    CommoditySpotOption,
    CommoditySpreadOption,
    CommoditySwap,
    CommoditySwaption,
    EquityTotalReturnFuture,
    EquityTotalReturnSwap,
    FiIndexTotalReturnSwap,
    FxSpot,
    FxVarianceSwap,
    VarianceSwap,
)


@pytest.mark.parametrize(
    "instrument_type",
    [
        CommodityAsianOption,
        CommodityForward,
        CommodityFuture,
        CommodityFutureOption,
        CommoditySpotOption,
        CommoditySpreadOption,
        CommoditySwap,
        CommoditySwaption,
        EquityTotalReturnFuture,
        EquityTotalReturnSwap,
        FiIndexTotalReturnSwap,
        FxSpot,
        FxVarianceSwap,
        VarianceSwap,
    ],
)
def test_native_builder_copies_full_contract_and_wire_data(
    instrument_type: type[
        CommodityAsianOption
        | CommodityForward
        | CommodityFuture
        | CommodityFutureOption
        | CommoditySpotOption
        | CommoditySpreadOption
        | CommoditySwap
        | CommoditySwaption
        | EquityTotalReturnFuture
        | EquityTotalReturnSwap
        | FiIndexTotalReturnSwap
        | FxSpot
        | FxVarianceSwap
        | VarianceSwap
    ],
) -> None:
    existing = instrument_type.example()
    copy = existing.to_builder().id("EDITED-CONTRACT").build()
    assert copy.to_dict() == {**existing.to_dict(), "id": "EDITED-CONTRACT"}
    assert instrument_type.from_json(copy.to_json()).to_dict() == copy.to_dict()


def test_commodity_asian_builder_accepts_datetime_fixings() -> None:
    asian = CommodityAsianOption.example()
    assert all(isinstance(day, date) for day in asian.fixing_dates)
    with pytest.raises(ValueError, match=r"duplicate|strict|order|increasing"):
        asian.to_builder().fixing_dates([asian.fixing_dates[0]] * 2).build()
