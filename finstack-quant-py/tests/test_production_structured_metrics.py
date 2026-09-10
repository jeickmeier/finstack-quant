"""Independent structured-credit accrued-interest and effective-rate contracts."""

import json
from pathlib import Path

import pytest

from finstack_quant.valuations.instruments import price_instrument


def fixture() -> dict:
    result = json.loads(
        (
            Path(__file__).resolve().parents[2]
            / "finstack-quant/valuations/tests/fixtures/production_structured_credit.json"
        ).read_text()
    )
    spec = result["instrument"]["instrument"]["spec"]
    spec["prepayment_spec"] = {"cpr": 0.0, "curve": None}
    spec["default_spec"] = {"cdr": 0.0, "curve": None}
    return result


@pytest.mark.parametrize("cdr", [0.0, 0.9])
def test_seasoned_accrued_precedes_projected_losses(cdr: float) -> None:
    f = fixture()
    f["instrument"]["instrument"]["spec"]["default_spec"]["cdr"] = cdr
    result = price_instrument(
        json.dumps(f["instrument"]), json.dumps(f["market"]), "2024-02-15", "discounting", ["accrued"]
    )
    # 100m opening face * 6% annual coupon * 45 actual/360 days.
    assert result.get_metric("accrued") == pytest.approx(750_000.0, abs=1e-6)


def test_effective_model_rates_and_override_precedence() -> None:
    f = fixture()
    spec = f["instrument"]["instrument"]["spec"]
    spec["prepayment_spec"]["cpr"] = 0.36
    spec["default_spec"]["cdr"] = 0.07
    for overridden, expected in [(False, (0.36, 0.07)), (True, (0.18, 0.03))]:
        if overridden:
            spec["behavior_overrides"].update(cpr_annual=0.18, cdr_annual=0.03)
        result = price_instrument(
            json.dumps(f["instrument"]), json.dumps(f["market"]), f["as_of"], "discounting", ["cpr", "cdr"]
        )
        assert result.get_metric("cpr") == pytest.approx(expected[0], abs=1e-12)
        assert result.get_metric("cdr") == pytest.approx(expected[1], abs=1e-12)


def test_recovery_risk_uses_active_override() -> None:
    f = fixture()
    spec = f["instrument"]["instrument"]["spec"]
    spec["default_spec"]["cdr"] = 0.03
    spec["behavior_overrides"]["recovery_rate"] = 0.55
    a = price_instrument(
        json.dumps(f["instrument"]), json.dumps(f["market"]), f["as_of"], "discounting", ["recovery_01"]
    )
    spec["behavior_overrides"]["recovery_rate"] = None
    spec["recovery_spec"]["rate"] = 0.55
    b = price_instrument(
        json.dumps(f["instrument"]), json.dumps(f["market"]), f["as_of"], "discounting", ["recovery_01"]
    )
    assert a.get_metric("recovery_01") == pytest.approx(b.get_metric("recovery_01"), abs=1e-8)
    assert abs(a.get_metric("recovery_01")) > 1.0


def test_clean_dirty_settlement_target_and_typed_date() -> None:
    from datetime import date

    from finstack_quant.valuations.instruments import (
        StructuredCredit,
        structured_credit_tranche_metrics,
        structured_credit_tranche_oas,
    )

    f = fixture()
    spec = f["instrument"]["instrument"]["spec"]
    spec["quote_settlement_date"] = "2024-05-15"
    deal = StructuredCredit.from_json(json.dumps(f["instrument"]))
    assert deal.quote_settlement_date == date(2024, 5, 15)
    builder = StructuredCredit.builder()
    assert builder.quote_settlement_date(date(2024, 5, 15)) is builder
    as_of = "2024-02-15"
    base = price_instrument(
        json.dumps(f["instrument"]),
        json.dumps(f["market"]),
        as_of,
        "discounting",
        ["accrued", "clean_price", "dirty_price"],
    )
    accrued = 100_000_000 * 0.06 * 44 / 360
    assert base.get_metric("accrued") == pytest.approx(accrued, abs=1e-6)
    clean = base.get_metric("clean_price")
    dirty = base.get_metric("dirty_price") * 1_000_000
    assert clean * 1_000_000 + accrued == pytest.approx(dirty, abs=1e-6)
    tranche_id = spec["tranches"]["tranches"][0]["id"]
    metrics = structured_credit_tranche_metrics(deal, tranche_id, json.dumps(f["market"]), as_of, clean)
    assert abs(metrics.z_spread_bp) < 1e-6
    assert metrics.spread_duration == pytest.approx(-metrics.cs01 / (dirty * 1e-4), abs=1e-10)
    config = {
        "num_paths": 1,
        "stochastic_rates": False,
        "stochastic_credit": False,
        "hw_kappa": 0.05,
        "hw_sigma": 0.01,
        "prepay_beta": 7.0,
        "credit_loading": 0.3,
        "seed": 42,
        "tolerance": 1e-10,
    }
    oas = structured_credit_tranche_oas(deal, tranche_id, clean, json.dumps(f["market"]), as_of, config)
    assert abs(oas.oas) < 1e-8
    assert oas.model_price == pytest.approx(clean, abs=1e-8)
    for quote in [{"quoted_clean_price": clean}, {"quoted_dirty_price_currency": dirty}]:
        spec["instrument_pricing_overrides"] = {"market_quotes": quote}
        priced = price_instrument(
            json.dumps(f["instrument"]),
            json.dumps(f["market"]),
            as_of,
            "discounting",
            ["z_spread", "cs01", "spread_duration"],
        )
        assert abs(priced.get_metric("z_spread")) < 1e-8
        assert priced.get_metric("spread_duration") == pytest.approx(
            -priced.get_metric("cs01") / (dirty * 1e-4), abs=1e-10
        )
