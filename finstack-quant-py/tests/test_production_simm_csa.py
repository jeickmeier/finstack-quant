"""Independent historical SIMM v2.6 parameter and concentration vectors."""

import json
import math

import pytest

from finstack_quant.margin import FrtbSbaEngine, FrtbSensitivities, SimmCalculator, SimmSensitivities


@pytest.mark.parametrize(("currency", "weight"), [("USD", 60.0), ("JPY", 23.0), ("BRL", 97.0)])
def test_ir_currency_weights(currency: str, weight: float) -> None:
    sensitivities = SimmSensitivities("USD")
    sensitivities.add_ir_delta(currency, "5Y", 100.0)
    result = SimmCalculator().calculate_from_sensitivities(sensitivities, "USD", "2025-01-15")
    assert result.amount == pytest.approx(100.0 * weight)


def test_cq_raw_concentration() -> None:
    sensitivities = SimmSensitivities("USD")
    sensitivities.add_credit_qualifying_delta("financial", "BANK", "5Y", 100_000.0)
    result = SimmCalculator().calculate_from_sensitivities(sensitivities, "USD", "2025-01-15")
    assert result.amount == pytest.approx(9_000_000.0)


def test_fx_percent_risk_and_raw_concentration() -> None:
    sensitivities = SimmSensitivities("USD")
    sensitivities.add_fx_delta("EUR", 10_000_000.0)
    result = SimmCalculator().calculate_from_sensitivities(sensitivities, "USD", "2025-01-15")
    assert result.amount == pytest.approx(74_000_000.0)


def test_girr_vega_underlying_decay() -> None:
    sensitivities = FrtbSensitivities("USD")
    sensitivities.add_girr_vega("5Y", "1Y", 100.0)
    sensitivities.add_girr_vega("5Y", "5Y", 100.0)
    result = FrtbSbaEngine(scenarios=["medium"], risk_classes=["girr"]).calculate(sensitivities)
    assert result.total == pytest.approx(math.sqrt(20_000.0 + 20_000.0 * math.exp(-0.04)))


def test_curvature_scales_expiries_before_netting() -> None:
    # Same equity factor: equal and opposite volatility-weighted vegas do not
    # offset curvature when one expires in 14 days and the other in 365 days.
    inputs = [
        {
            "risk_class": "equity",
            "bucket": "residual",
            "factor": "ACME",
            "risk_tenor": None,
            "expiry_tenor": tenor,
            "volatility_weighted_vega": amount,
        }
        for tenor, amount in [("2W", 1000.0), ("1Y", -1000.0)]
    ]
    sensitivities = SimmSensitivities.from_json(json.dumps({"base_currency": "USD", "curvature": inputs}))
    result = SimmCalculator().calculate_from_sensitivities(sensitivities, "USD", "2025-01-15")
    expected = (0.5 * 1000.0 - 7.0 / 365.0 * 1000.0) * 2.5758293035489004**2
    assert result.amount == pytest.approx(expected)


def test_csa_im_collateral_terms_preserve_gross_and_apply_signed_mta() -> None:
    from finstack_quant.margin import CsaSpec

    csa = CsaSpec.usd_regulatory().with_im("simm", 40, 10_000.0, 1_500.0, True)
    result = csa.apply_im_terms(24_000.0, 13_000.0)
    assert result.gross_initial_margin == 24_000.0
    assert result.required_collateral == 14_000.0
    assert result.current_collateral == 13_000.0
    assert result.transfer == 0.0
    assert result.segregated
    assert result.currency == "USD"
    # IM is already MPOR-adjusted. Applying contract terms must not scale it again.
    boundary = csa.with_im("simm", 40, 10_000.0, 1_000.0, False)
    assert boundary.apply_im_terms(24_000.0, 13_000.0).transfer == 1_000.0
    returned = boundary.apply_im_terms(24_000.0, 16_000.0)
    assert returned.transfer == -2_000.0
    assert not returned.segregated
    for invalid in [-1.0, float("nan"), float("inf")]:
        with pytest.raises(ValueError, match=r"finite|nonnegative|non-negative"):
            csa.apply_im_terms(invalid, 0.0)
        with pytest.raises(ValueError, match=r"finite|nonnegative|non-negative"):
            csa.apply_im_terms(24_000.0, invalid)
