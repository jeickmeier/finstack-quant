import pytest

from finstack_quant.core.money import Money
from finstack_quant.margin import (
    CsaSpec,
    DrcPosition,
    FrtbSbaEngine,
    FrtbSensitivities,
    OtcMarginSpec,
    SimmCreditClassification,
)
from finstack_quant.valuations.instruments import InterestRateSwap


def test_csa_explicit_terms_and_independent_edit() -> None:
    regulatory = CsaSpec.usd_regulatory()
    updated = regulatory.to_builder().vm_threshold(Money(250_000, "USD"), Money(50_000, "USD")).im_params(None).build()
    assert updated.vm_threshold == 250_000
    assert not updated.requires_im
    assert regulatory.vm_threshold == 0
    assert regulatory.requires_im

    with pytest.raises(ValueError, match="calendar_id"):
        CsaSpec.builder("CSA-EXPLICIT", "USD", "USD-OIS").build()
    with pytest.raises(ValueError, match="denominated in USD"):
        regulatory.to_builder().vm_threshold(Money(1000, "EUR"), Money(100, "USD")).build()


def test_threshold_edit_preserves_custom_vm_terms() -> None:
    original = CsaSpec.usd_regulatory().with_vm_threshold(1_000.0, 500.0, rounding=250.0, independent_amount=50_000.0)
    modified = original.to_builder().vm_threshold(Money(2_000, "USD"), Money(750, "USD")).build()
    assert (modified.vm_threshold, modified.vm_mta) == (2_000.0, 750.0)
    assert modified.vm_rounding == original.vm_rounding == 250.0
    assert modified.vm_independent_amount == original.vm_independent_amount == 50_000.0


def test_typed_otc_spec_attaches_to_existing_rates_trade() -> None:
    csa = CsaSpec.usd_regulatory()
    spec = OtcMarginSpec.bilateral_simm(csa).with_simm_credit_classification(SimmCreditClassification.non_qualifying())
    # Exercise the same margin metadata route as an authored IRS.
    trade = InterestRateSwap.example()
    builder = InterestRateSwap.builder()
    doc = trade.to_dict()
    result = (
        builder
        .id("IRS-TYPED-CSA")
        .notional(trade.notional)
        .fixed_leg(trade.fixed_leg)
        .float_leg(trade.float_leg)
        .side(doc["side"])
        .margin_spec(spec)
        .build()
    )
    assert result.margin_spec == spec.to_dict()


def test_native_drc_position_validates_before_aggregation() -> None:
    kwargs = {
        "issuer": "ACME",
        "jtd_amount": 1_000_000.0,
        "rating_bucket": 3,
        "sector": "corporate",
        "seniority": "senior_unsecured",
        "maturity_years": 1.0,
    }
    sens = FrtbSensitivities("USD")
    sens.add_drc_position(DrcPosition(**kwargs))
    assert FrtbSbaEngine().calculate(sens).drc > 0.0
    with pytest.raises(ValueError, match="maturity"):
        DrcPosition(**(kwargs | {"maturity_years": -1.0}))
