"""Production audit calculations through the rebuilt canonical Python API."""

from __future__ import annotations

from datetime import date
import json
from pathlib import Path

import pytest

from finstack_quant.core.market_data import DiscountCurve, FxMatrix, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments import Bond, price_instrument


def test_structured_credit_constructor_derives_first_payment() -> None:
    from finstack_quant.valuations.instruments import StructuredCredit
    from tests.tests_typed_helpers import structured_credit_pool, structured_credit_tranches

    deal = StructuredCredit.new_abs(
        "NEW-ABS",
        structured_credit_pool(),
        structured_credit_tranches(),
        date(2026, 9, 9),
        date(2031, 1, 15),
        "USD-OIS",
    )
    assert deal.first_payment_date == date(2026, 10, 9)


def test_structured_credit_recovery_override_matches_model() -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2]
            / "finstack-quant/valuations/tests/fixtures/production_structured_credit.json"
        ).read_text()
    )

    def value() -> float:
        return price_instrument(
            json.dumps(fixture["instrument"]), json.dumps(fixture["market"]), fixture["as_of"], "discounting", []
        ).value.amount

    expected = value()
    spec = fixture["instrument"]["instrument"]["spec"]
    spec["recovery_spec"] = {"rate": 0.1, "recovery_lag": 0}
    spec["behavior_overrides"]["recovery_rate"] = 0.7
    spec["behavior_overrides"]["recovery_lag_months"] = 9
    assert value() == pytest.approx(expected, abs=1e-6)


def test_structured_credit_inactive_reinvestment_matches_no_reinvestment() -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2]
            / "finstack-quant/valuations/tests/fixtures/production_structured_credit.json"
        ).read_text()
    )
    spec = fixture["instrument"]["instrument"]["spec"]
    spec["prepayment_spec"] = {"cpr": 0.36, "curve": None}
    spec["default_spec"] = {"cdr": 0.0, "curve": None}
    for tranche in spec["tranches"]["tranches"]:
        tranche["is_revolving"] = True
        tranche["can_reinvest"] = True

    def value() -> float:
        return price_instrument(
            json.dumps(fixture["instrument"]), json.dumps(fixture["market"]), fixture["as_of"], "discounting", []
        ).value.amount

    expected = value()
    spec["pool"]["reinvestment_period"] = {
        "end_date": "2030-01-01",
        "is_active": False,
        "criteria": {"max_price": 100.0, "min_yield": 0.0, "maintain_credit_quality": True, "maintain_wal": True},
    }
    assert value() == pytest.approx(expected, abs=1e-6)


@pytest.mark.parametrize(("end_date", "period_days"), [("2024-04-02", [91]), ("2025-01-02", [91, 91, 92, 92])])
@pytest.mark.parametrize(("max_price", "min_yield"), [(99.0, 0.0), (100.0, 0.5)])
def test_structured_credit_ineligible_reinvestment_conserves_cash(
    max_price: float, min_yield: float, end_date: str, period_days: list[int]
) -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2]
            / "finstack-quant/valuations/tests/fixtures/production_structured_credit.json"
        ).read_text()
    )
    spec = fixture["instrument"]["instrument"]["spec"]
    fixture["as_of"] = spec["closing_date"] = "2024-01-02"
    spec["first_payment_date"] = "2024-04-02"
    spec["maturity"] = spec["pool"]["assets"][0]["maturity"] = end_date
    spec["prepayment_spec"] = {"cpr": 0.36, "curve": None}
    spec["default_spec"] = {"cdr": 0.0, "curve": None}
    tranche = spec["tranches"]["tranches"][0]
    tranche.update(
        seniority="equity", coupon={"fixed": {"rate": 0.0}}, maturity=end_date, is_revolving=True, can_reinvest=True
    )
    spec["pool"]["reinvestment_period"] = {
        "end_date": end_date,
        "is_active": True,
        "criteria": {
            "max_price": max_price,
            "min_yield": min_yield,
            "maintain_credit_quality": True,
            "maintain_wal": True,
        },
    }
    # A zero discount curve makes the independent comparison a cash identity.
    for curve in fixture["market"]["curves"]:
        if curve["type"] == "discount":
            curve["base"] = fixture["as_of"]
            curve["knot_points"] = [[0.0, 1.0], [20.0, 1.0]]
    result = price_instrument(
        json.dumps(fixture["instrument"]), json.dumps(fixture["market"]), fixture["as_of"], "discounting", []
    )
    q = 0.64**0.25
    expected = 100_000_000.0 * (1.0 + 0.07 * sum(q**i * days / 360.0 for i, days in enumerate(period_days)))
    assert result.value.amount == pytest.approx(expected, abs=1e-6)


def test_structured_credit_predefaulted_collateral_pays_recovery_once() -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2]
            / "finstack-quant/valuations/tests/fixtures/production_structured_credit.json"
        ).read_text()
    )
    asset = fixture["instrument"]["instrument"]["spec"]["pool"]["assets"][0]
    asset.update(
        is_defaulted=True, default_date=fixture["as_of"], recovery_amount={"amount": "70000000", "currency": "USD"}
    )
    fixture["market"]["curves"][0]["knot_points"] = [[0.0, 1.0], [20.0, 1.0]]
    result = price_instrument(
        json.dumps(fixture["instrument"]), json.dumps(fixture["market"]), fixture["as_of"], "discounting", []
    )
    assert result.value.amount == pytest.approx(70_000_000.0, abs=1e-6)


def test_active_volatility_override_separates_scalar_and_node_risk() -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures/production_equity.json"
        ).read_text()
    )
    spec = fixture["instrument"]["instrument"]["spec"]
    spec["exercise_style"] = "european"
    spec["strike"] = 120.0
    metrics = ["vega", "vanna", "volga", "bucketed_vega"]

    def price() -> ValuationResult:
        return price_instrument(
            json.dumps(fixture["instrument"]), json.dumps(fixture["market"]), fixture["as_of"], "black76", metrics
        )

    reference = price()
    spec["instrument_pricing_overrides"] = {"market_quotes": {"implied_volatility": 0.2}}
    for remove_surface in [False, True]:
        if remove_surface:
            fixture["market"]["surfaces"] = []
        result = price()
        for metric in ["vega", "vanna", "volga"]:
            assert abs(reference.get_metric(metric)) > 1e-8
            assert result.get_metric(metric) == pytest.approx(reference.get_metric(metric), abs=1e-10)
        assert result.get_metric("bucketed_vega") == 0.0
        assert result.get_metric("bucketed_vega_residual") == result.get_metric("vega")
        assert result.metric_units()["bucketed_vega_residual"] == "currency"


def test_fx_source_precedence_is_independent_of_orientation() -> None:
    fx = FxMatrix()
    fx.set_quote("USD", "EUR", 0.8)
    fx.set_quote_on("EUR", "USD", "2025-01-02", "cashflow_date", 1.3)
    assert fx.rate("EUR", "USD", "2025-01-02").rate == pytest.approx(1.25)
    assert fx.rate("USD", "EUR", "2025-01-02").rate == pytest.approx(0.8)
    fx.set_quote("USD", "GBP", 0.8)
    assert fx.rate("EUR", "GBP", "2025-01-02").rate == pytest.approx(1.0)


def test_zero_rate_bond_risk_can_cross_into_negative_rates() -> None:
    import math

    as_of = date(2025, 1, 2)
    bond = Bond.fixed("ZERO-RATE-RISK", Money(100.0, "USD"), 0.0, as_of, date(2026, 1, 2), "none", "FLAT")
    payload = json.loads(bond.to_json())
    payload["instrument"]["spec"]["settlement_days"] = 0
    payload["instrument"]["spec"]["cashflow_spec"]["fixed"]["business_day_convention"] = "unadjusted"
    bond = Bond.from_json(json.dumps(payload))
    market = MarketContext().insert(DiscountCurve.flat("FLAT", as_of, 0.0))
    result = bond.price(market, as_of, metrics=["dv01"])
    assert result.get_metric("dv01") == pytest.approx(-100.0 * math.sinh(0.0001), abs=1e-9)


def test_icma_mid_coupon_duration_and_convexity() -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures/production_icma.json"
        ).read_text()
    )
    result = price_instrument(
        json.dumps(fixture["instrument"]),
        json.dumps(fixture["market"]),
        "2024-05-31",
        "discounting",
        ["ytm", "duration_mac", "convexity", "z_spread"],
    )
    base = 1.0 + result.get_metric("ytm") / 2.0
    # Half of the Feb-29/Aug-31 reference coupon is accrued: dirty price 101.
    first = 2.0 / base**0.5
    last = 102.0 / base**1.5
    assert result.get_metric("duration_mac") == pytest.approx((0.25 * first + 0.75 * last) / 101.0, abs=1e-9)
    assert result.get_metric("convexity") == pytest.approx(
        (0.25 * 0.75 * first + 0.75 * 1.25 * last) / base**2 / 10100.0, abs=1e-10
    )
    assert result.get_metric("z_spread") > 0.0


def test_normal_sabr_beta_zero_uses_arithmetic_moneyness() -> None:
    import math

    from finstack_quant.models.volatility import SabrModel, SabrParameters

    alpha, nu, rho, expiry = 0.008, 0.7, -0.35, 2.0
    model = SabrModel(SabrParameters(alpha, 0.0, nu, rho))
    # Hagan beta=0 normal expansion: z = nu * (F-K) / alpha.
    for forward, strike in [(-0.01, -0.006), (-0.002, 0.002), (0.01, 0.014)]:
        z = nu * (forward - strike) / alpha
        chi = math.log((math.sqrt(1 - 2 * rho * z + z * z) + z - rho) / (1 - rho))
        expected = alpha * z / chi * (1 + (2 - 3 * rho * rho) * nu * nu * expiry / 24)
        assert model.implied_vol(forward, strike, expiry) == pytest.approx(expected, abs=1e-14)


def test_sabr_fit_rejects_large_quote_error() -> None:
    from finstack_quant.models.volatility import SabrCalibrator

    with pytest.raises(RuntimeError, match="no acceptable"):
        SabrCalibrator().calibrate(100.0, [90.0, 95.0, 100.0, 105.0, 110.0], [0.22, 0.20, 0.19, 0.195, 0.21], 1.0, 0.5)


def test_clamped_cube_preserves_quote_and_invalid_domain() -> None:
    import math

    from finstack_quant.core.market_data import SabrParameterData, VolCube
    from finstack_quant.models.volatility import get_cube_vol, get_cube_vol_clamped

    low = VolCube("LOW", [1.0], [5.0], [SabrParameterData(0.0001, 1.0, 0.0, 0.3)], [0.02])
    assert get_cube_vol_clamped(low, 1.0, 5.0, 0.02) == pytest.approx(get_cube_vol(low, 1.0, 5.0, 0.02), abs=1e-14)
    invalid = VolCube("INVALID", [1.0], [5.0], [SabrParameterData(0.008, 0.5, 0.0, 0.3)], [-0.01])
    assert math.isnan(get_cube_vol_clamped(invalid, 1.0, 5.0, -0.01))


def test_hull_white_requires_a_quote_fit_budget() -> None:
    from finstack_quant.calibration.hull_white import CapFloorCalibrationConfig

    with pytest.raises(TypeError):
        CapFloorCalibrationConfig(fixed_kappa=0.05)


def test_hull_white_partial_coupon_spread_through_json_pricer() -> None:
    import math

    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures/production_hw.json"
        ).read_text()
    )
    as_of = date.fromisoformat(fixture["as_of"])
    expiry = date(2026, 4, 1)
    result = price_instrument(
        json.dumps(fixture["instrument"]), json.dumps(fixture["market"]), fixture["as_of"], "hull_white_1f", []
    )
    expected = 0.0
    start = expiry
    for end in [date(2027, 1, 1), date(2028, 1, 1)]:
        tau = (end - start).days / 365
        expected += (math.expm1(0.04 * tau) + (0.01 - 0.02) * tau) * math.exp(-0.04 * (end - as_of).days / 365)
        start = end
    assert result.value.amount == pytest.approx(expected * 10_000_000, abs=0.01)


def test_hull_white_quote_budget_controls_fit_and_survives_pickle() -> None:
    import math
    import pickle

    from finstack_quant.calibration.hull_white import (
        CapFloorCalibrationConfig,
        CapFloorQuote,
        calibrate_hull_white_to_cap_floors,
    )

    curve = DiscountCurve.flat("HW", date(2025, 1, 1), 0.03)
    strike = math.expm1(0.03 * 0.25) / 0.25
    quotes = [CapFloorQuote(5.0, strike, vol) for vol in [0.009, 0.010, 0.011]]
    results = []
    for budget in [0.0005, 0.0011]:
        config = CapFloorCalibrationConfig(budget, frequency="quarterly", fixed_kappa=0.05)
        config = pickle.loads(pickle.dumps(config))  # noqa: S301 - locally constructed round-trip fixture
        assert config.fit_tolerance == budget
        results.append(calibrate_hull_white_to_cap_floors(curve, quotes, config))
    assert results[0][0].sigma == results[1][0].sigma
    assert not results[0][1].success
    assert results[1][1].success
    assert results[1][1].max_residual == pytest.approx(0.001, abs=1e-8)


def test_inflation_reference_months_compounding_and_registry_contracts() -> None:
    from finstack_quant.core.dates import DayCount
    from finstack_quant.core.market_data import InflationIndex
    from finstack_quant.valuations.market import ConventionRegistry

    index = InflationIndex(
        "US-CPI", [(date(2024, 10, 31), 300.0), (date(2024, 11, 30), 303.0)], "USD", interpolation="linear", lag="3M"
    )
    assert index.value_on(date(2025, 1, 16)) == pytest.approx(300 + 3 * 15 / 31, abs=1e-12)
    assert DayCount.ONE_ONE.year_fraction(date(2024, 1, 1), date(2029, 1, 1)) == 1.0
    registry = ConventionRegistry()
    assert registry.require_inflation_swap("USD-CPI").day_count == "one_one"
    assert registry.require_inflation_swap("USD-CPI").interpolation == "linear"
    assert registry.require_inflation_swap("UK-RPI").calendar_id == "gblo"
    assert registry.resolve_cds("JPY", "isda_as").day_count == "act_360"
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures/production_inflation.json"
        ).read_text()
    )

    def price() -> ValuationResult:
        return price_instrument(
            json.dumps(fixture["instrument"]), json.dumps(fixture["market"]), fixture["as_of"], "discounting", []
        )

    expected = 1_000_000 * (0.10 - (1.02**5 - 1))
    assert price().value.amount == pytest.approx(expected, abs=1e-7)
    del fixture["instrument"]["instrument"]["spec"]["base_cpi"]
    with pytest.raises((KeyError, RuntimeError), match="US-CPI"):
        price()


def test_convertible_clean_exercise_and_variable_conversion() -> None:
    from finstack_quant.valuations.instruments import ConvertibleBond

    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures/production_convertible.json"
        ).read_text()
    )
    result = price_instrument(
        json.dumps(fixture["instrument"]), json.dumps(fixture["market"]), fixture["as_of"], "tree", []
    )
    assert result.value.amount == pytest.approx(1000 + 50 * 90 / 365, abs=1e-8)
    spec = fixture["instrument"]["instrument"]["spec"]
    spec["conversion"]["policy"] = {
        "mandatory_variable": {
            "conversion_date": spec["maturity"],
            "lower_conversion_price": 80.0,
            "upper_conversion_price": 120.0,
        }
    }
    bond = ConvertibleBond.from_json(json.dumps(fixture["instrument"]))
    for spot, expected in [(40.0, 0.5), (90.0, 1.0), (180.0, 1.5)]:
        fixture["market"]["prices"]["AAPL"]["unitless"] = spot
        market_json = json.dumps(fixture["market"])
        assert bond.parity(market_json) == pytest.approx(expected, abs=1e-12)
        assert bond.conversion_premium(market_json, 1100.0) == pytest.approx(1100 / (1000 * expected) - 1, abs=1e-12)


def test_convertible_cross_gamma_uses_the_effective_volatility_quote() -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures/production_convertible.json"
        ).read_text()
    )
    spec = fixture["instrument"]["instrument"]["spec"]
    spec["call_put"] = None
    spec["fixed_coupon"] = None
    spec["conversion"]["ratio"] = 10.0

    def price(spot: float, vol: float, metrics: list[str]) -> ValuationResult:
        fixture["market"]["prices"]["AAPL"]["unitless"] = spot
        fixture["market"]["prices"]["AAPL-VOL"]["unitless"] = vol
        return price_instrument(
            json.dumps(fixture["instrument"]), json.dumps(fixture["market"]), fixture["as_of"], "tree", metrics
        )

    expected = (
        price(90.9, 0.41, []).value.amount
        - price(90.9, 0.39, []).value.amount
        - price(89.1, 0.41, []).value.amount
        + price(89.1, 0.39, []).value.amount
    ) / 4.0
    assert abs(expected) > 1e-8
    assert price(90.0, 0.4, ["cross_gamma_spot_vol"]).get_metric("cross_gamma_spot_vol") == pytest.approx(
        expected, abs=1e-8
    )
    spec["instrument_pricing_overrides"] = {"market_quotes": {"implied_volatility": 0.4}}
    assert price(90.0, 0.1, ["cross_gamma_spot_vol"]).get_metric("cross_gamma_spot_vol") == pytest.approx(
        expected, abs=1e-8
    )


def test_mortgage_settlement_seasoned_io_and_financing() -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures/production_mortgage.json"
        ).read_text()
    )

    def price(name: str, metrics: list[str]) -> ValuationResult:
        return price_instrument(
            json.dumps(fixture[name]), json.dumps(fixture["market"]), fixture["as_of"], "discounting", metrics
        )

    # 1000 purchased current face, a full monthly 4% coupon, less ten days' accrued interest.
    assert price("tba", []).value.amount == pytest.approx(1000 * 0.04 * (1 / 12 - 10 / 360), abs=1e-9)
    # Seasoned IO has 60000 reference balance, not the original 100000.
    assert price("cmo", []).value.amount == pytest.approx(60000 * 0.01 / 12, abs=1e-9)
    mbs = price("mbs", ["duration_mod", "dv01"])
    assert mbs.get_metric("duration_mod") > 0
    assert mbs.get_metric("dv01") < 0
    roll = price("roll", ["implied_financing_rate", "roll_specialness"])
    # With a zero-rate discount curve and no repo override, reference financing is zero.
    assert roll.get_metric("roll_specialness") == pytest.approx(
        -roll.get_metric("implied_financing_rate") * 10000, abs=1e-9
    )


def test_tba_delivery_excludes_prior_receivable_and_adjusts_payment_date() -> None:
    import math

    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures/production_mortgage.json"
        ).read_text()
    )
    fixture["tba"]["instrument"]["spec"]["assumed_pool"]["issue_date"] = "2025-01-01"
    fixture["market"]["curves"][0]["knot_points"] = [[0.0, 1.0], [40.0, math.exp(-0.04 * 40)]]
    result = price_instrument(
        json.dumps(fixture["tba"]), json.dumps(fixture["market"]), fixture["as_of"], "discounting", []
    )
    # March coupon pays April 27 (April 25 is Saturday). February's payment is the seller's.
    payment_time = (date(2026, 4, 27) - date(2026, 3, 1)).days / 365
    expected = (1000 + 1000 * 0.04 / 12) * math.exp(-0.04 * payment_time)
    expected -= (1000 + 1000 * 0.04 * 10 / 360) * math.exp(-0.04 * 10 / 365)
    assert result.value.amount == pytest.approx(expected, abs=1e-9)


def test_credit_tranche_loss_and_complete_issuer_coverage() -> None:
    fixtures = Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures"
    f = json.loads((fixtures / "production_credit_tranche.json").read_text())
    reference = json.loads((fixtures / "production_tranche_loss_reference.json").read_text())
    result = price_instrument(
        json.dumps(f["instrument"]), json.dumps(f["market"]), f["as_of"], "hazard_rate", ["expected_loss"]
    )
    assert result.get_metric("expected_loss") == pytest.approx(
        1e6 * reference["large_homogeneous_pool"]["expected_loss"], abs=2e-4
    )
    f["market"]["credit_indices"][0]["num_constituents"] = 2
    f["market"]["credit_indices"][0]["issuer_credit_curve_ids"] = {"A": "HZ"}
    with pytest.raises(ValueError, match=r"complete coverage|num_constituents"):
        MarketContext.from_json(json.dumps(f["market"]))


def test_cds_option_premium_settlement_does_not_change_variance() -> None:
    f = json.loads(
        (
            Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures/production_cds_option.json"
        ).read_text()
    )
    values = []
    for settlement in ["2025-01-06", "2025-07-01"]:
        f["instrument"]["instrument"]["spec"]["cash_settlement_date"] = settlement
        values.append(
            price_instrument(
                json.dumps(f["instrument"]), json.dumps(f["market"]), f["as_of"], "bloomberg_cdso", []
            ).value.amount
        )
    assert values[0] == pytest.approx(values[1], abs=1e-8)


def test_bespoke_cds_frequency_and_stub_price_the_configured_payments() -> None:
    import math

    from finstack_quant.valuations.instruments import PremiumLegSpec

    f = json.loads(
        (
            Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures/production_cds_premium.json"
        ).read_text()
    )
    for stub, dates in [
        ("short_front", [date(2025, 4, 1), date(2025, 10, 1), date(2026, 4, 1)]),
        ("long_front", [date(2025, 10, 1), date(2026, 4, 1)]),
    ]:
        premium = f["instrument"]["instrument"]["spec"]["premium"]
        premium["stub"] = stub
        assert not PremiumLegSpec.from_json(json.dumps(premium)).standard_imm_dates
        value = price_instrument(
            json.dumps(f["instrument"]), json.dumps(f["market"]), f["as_of"], "hazard_rate", []
        ).value.amount
        previous = date(2025, 1, 1)
        expected = 0.0
        for payment in dates:
            expected -= (
                10.0 * (payment - previous).days / 360 * math.exp(-0.03 * (payment - date(2025, 1, 1)).days / 365)
            )
            previous = payment
        assert value == pytest.approx(expected, abs=1e-10)


def test_cds_option_exercise_payment_cannot_precede_expiry() -> None:
    f = json.loads(
        (
            Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures/production_cds_option.json"
        ).read_text()
    )
    f["instrument"]["instrument"]["spec"]["exercise_settlement_date"] = "2025-12-31"
    with pytest.raises((ValueError, RuntimeError), match=r"exercise_settlement_date.*expiry"):
        price_instrument(json.dumps(f["instrument"]), json.dumps(f["market"]), f["as_of"], "bloomberg_cdso", [])


def test_structured_credit_representative_collateral_preserves_pv() -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2]
            / "finstack-quant/valuations/tests/fixtures/production_structured_credit.json"
        ).read_text()
    )
    pool = fixture["instrument"]["instrument"]["spec"]["pool"]

    def value() -> float:
        return price_instrument(
            json.dumps(fixture["instrument"]), json.dumps(fixture["market"]), fixture["as_of"], "discounting", []
        ).value.amount

    expected = value()
    asset = pool["assets"].pop()
    pool["rep_lines"] = [
        {
            "id": "REP",
            "asset_type": asset["asset_type"],
            "balance": asset["balance"],
            "rate": asset["rate"],
            "spread_bp": asset["spread_bp"],
            "index_id": asset["index_id"],
            "maturity": asset["maturity"],
            "seasoning_months": 0,
            "day_count": asset["day_count"],
            "cpr": None,
            "cdr": None,
            "recovery_rate": None,
        }
    ]
    assert value() == pytest.approx(expected, abs=1e-6)
    pool["assets"].append(asset)
    with pytest.raises(ValueError, match="exactly one of assets, representative lines or instruments"):
        value()


def test_structured_credit_enhancement_includes_current_collateral_and_reserve() -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2]
            / "finstack-quant/valuations/tests/fixtures/production_structured_credit.json"
        ).read_text()
    )
    spec = fixture["instrument"]["instrument"]["spec"]
    spec["tranches"]["tranches"][0]["current_balance"]["amount"] = "80000000"
    spec["pool"]["reserve_account"]["amount"] = "20000000"
    result = price_instrument(
        json.dumps(fixture["instrument"]),
        json.dumps(fixture["market"]),
        fixture["as_of"],
        "discounting",
        ["abs_ce_level"],
    )
    assert result.get_metric("abs_ce_level") == pytest.approx(100.0 / 3.0, abs=1e-12)
