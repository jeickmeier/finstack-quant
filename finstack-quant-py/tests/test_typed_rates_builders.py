import pytest

from finstack_quant.valuations.instruments import (
    BasisSwap,
    BermudanSwaption,
    CmsOption,
    CmsSpreadOption,
    CmsSwap,
    InflationCapFloor,
    InflationSwap,
    XccySwap,
    YoYInflationSwap,
)


@pytest.mark.parametrize(
    "instrument_type",
    [
        BasisSwap,
        BermudanSwaption,
        CmsOption,
        CmsSpreadOption,
        CmsSwap,
        InflationCapFloor,
        InflationSwap,
        XccySwap,
        YoYInflationSwap,
    ],
)
def test_rates_builder_changes_only_requested_fields(
    instrument_type: type[
        BasisSwap
        | BermudanSwaption
        | CmsOption
        | CmsSpreadOption
        | CmsSwap
        | InflationCapFloor
        | InflationSwap
        | XccySwap
        | YoYInflationSwap
    ],
) -> None:
    instrument = instrument_type.example()
    new_id = '{"literal":"trade id"}'
    copied = instrument.to_builder().id(new_id).build()
    expected = instrument.to_dict()
    expected["id"] = new_id

    assert copied.id == new_id
    assert copied.to_dict() == expected
    assert instrument.id != new_id
