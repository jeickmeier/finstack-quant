import pytest

from finstack_quant.valuations.instruments import (
    AgencyCmo,
    AgencyMbsPassthrough,
    AgencyTba,
    AsianOption,
    Autocallable,
    BarrierOption,
    Basket,
    CallableRangeAccrual,
    CliquetOption,
    DollarRoll,
    LeveredRealEstateEquity,
    LookbackOption,
    PrivateMarketsFund,
    RangeAccrual,
    RealEstateAsset,
    Snowball,
    Tarn,
)

_Exotic = (
    AgencyCmo
    | AgencyMbsPassthrough
    | AgencyTba
    | AsianOption
    | Autocallable
    | BarrierOption
    | Basket
    | CallableRangeAccrual
    | CliquetOption
    | DollarRoll
    | LeveredRealEstateEquity
    | LookbackOption
    | PrivateMarketsFund
    | RangeAccrual
    | RealEstateAsset
    | Snowball
    | Tarn
)


@pytest.mark.parametrize(
    "instrument_type",
    [
        AgencyCmo,
        AgencyMbsPassthrough,
        AgencyTba,
        AsianOption,
        Autocallable,
        BarrierOption,
        Basket,
        CallableRangeAccrual,
        CliquetOption,
        DollarRoll,
        LeveredRealEstateEquity,
        LookbackOption,
        PrivateMarketsFund,
        RangeAccrual,
        RealEstateAsset,
        Snowball,
        Tarn,
    ],
)
def test_builder_copies_all_contract_terms(instrument_type: type[_Exotic]) -> None:
    contract = instrument_type.example()
    copy = contract.to_builder().id("EX.01").build()
    expected = contract.to_dict()
    expected["id"] = "EX.01"

    assert copy.to_dict() == expected
    assert contract.id != copy.id


def test_real_estate_builder_preserves_explicit_costs_and_optional_schedule() -> None:
    from datetime import date
    from decimal import Decimal

    from finstack_quant.core.money import Money

    amount = Money.from_decimal(Decimal("1500.00000000003"), "USD")
    date_and_cost = [(date(2027, 1, 1), 9_000.25)]
    asset = (
        RealEstateAsset
        .example()
        .to_builder()
        .capex_schedule(date_and_cost)
        .acquisition_costs([amount])
        .disposition_costs([amount])
        .build()
    )

    assert asset.capex_schedule == date_and_cost
    assert asset.acquisition_costs[0].amount_decimal == Decimal("1500.00000000003")
    assert asset.disposition_costs == [amount]
    assert asset.to_builder().capex_schedule(None).build().capex_schedule is None
