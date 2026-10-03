"""Calibration bindings preserve canonical validation and loading limits."""

import json

import pytest

from finstack_quant.calibration import (
    CalibrationEnvelope,
    CalibrationPlan,
    CalibrationResult,
    CalibrationStep,
    CdsQuote,
    CdsTrancheQuote,
    CollateralEntry,
    DividendScheduleDatum,
    FxSpotDatum,
    InflationQuote,
    PriceDatum,
    RateQuote,
    SolverConfig,
    ValidationConfig,
    VolQuote,
    XccyQuote,
    calibrate,
    validate_butterfly_call_convexity,
    validate_butterfly_spread,
    validate_calendar_spread,
    validate_calendar_spread_with_forwards,
    validate_surface,
    validate_surface_with_forwards,
    validate_vol_bounds,
)
from finstack_quant.calibration.hull_white import CapFloorQuote, SwaptionQuote
from finstack_quant.core.market_data import VolSurface
from finstack_quant.portfolio import ContractLimitExceededError


@pytest.mark.parametrize("spread", [float("nan"), float("inf"), float("-inf")])
def test_swap_quote_rejects_nonfinite_optional_spread(spread: float) -> None:
    with pytest.raises(ValueError, match="spread_decimal must be finite"):
        RateQuote.swap("USD-SWAP-2Y", "USD-SOFR-OIS", "2Y", 0.04, spread_decimal=spread)


@pytest.mark.parametrize("spread", [None, 0.0, 0.001, -0.001])
def test_swap_quote_preserves_finite_or_absent_spread(spread: float | None) -> None:
    quote = RateQuote.swap("USD-SWAP-2Y", "USD-SOFR-OIS", "2Y", 0.04, spread_decimal=spread)
    assert json.loads(quote.to_json())["spread_decimal"] == spread
    result = calibrate(CalibrationPlan([CalibrationStep.discount("USD-OIS", "USD", "2026-09-30", quotes=[quote])]))
    assert result.success


@pytest.mark.parametrize("seasonal_factors", [None, {"monthly_adjustments": [0.01, -0.01] * 6}])
def test_inflation_step_rejects_removed_seasonal_factors(seasonal_factors: object) -> None:
    with pytest.raises(ValueError, match="seasonal_factors"):
        CalibrationStep.inflation(
            "USD-CPI",
            "USD",
            "2026-09-30",
            "USD-OIS",
            "USA-CPI-U",
            "3M",
            320.0,
            quote_set="cpi-quotes",
            seasonal_factors=seasonal_factors,
        )


def test_cap_floor_step_preserves_required_index_and_rejects_frequency_override() -> None:
    arguments = {
        "id": "HW-CAPS",
        "discount_curve_id": "EUR-OIS",
        "forward_curve_id": "EUR-EURIBOR-3M",
        "index_id": "EUR-EURIBOR-3M",
        "currency": "EUR",
        "base_date": "2026-09-30",
        "quote_set": "caps",
        "fit_tolerance": 1e-4,
    }
    step = CalibrationStep.cap_floor_hull_white(**arguments)
    encoded = json.loads(step.to_json())
    assert encoded["index_id"] == "EUR-EURIBOR-3M"
    assert "payment_frequency" not in encoded
    assert CalibrationStep.from_json(step.to_json()).params["index_id"] == "EUR-EURIBOR-3M"

    with pytest.raises(ValueError, match="payment_frequency"):
        CalibrationStep.cap_floor_hull_white(**arguments, payment_frequency="quarterly")

    del encoded["index_id"]
    with pytest.raises(ValueError, match="index_id"):
        CalibrationStep.from_json(json.dumps(encoded))


@pytest.mark.parametrize("settings", [{"tolerance": -1.0}, {"tolerance": 0.0}, {"max_iterations": 0}])
def test_solver_config_constructor_and_json_reject_invalid_settings(settings: dict) -> None:
    with pytest.raises(ValueError, match=r"must be (?:.*positive|at least 1)"):
        SolverConfig(**settings)
    with pytest.raises(ValueError, match=r"must be (?:.*positive|at least 1)"):
        SolverConfig.from_json(json.dumps(settings))


def test_result_loading_rejects_metadata_beyond_canonical_depth_limit() -> None:
    payload = json.loads(calibrate(CalibrationPlan([])).to_json())
    metadata: dict = {"leaf": 1}
    for _ in range(110):
        metadata = {"child": metadata}
    payload["result"]["report"]["explanation"] = {
        "type": "calibration",
        "entries": [{"kind": "computation_step", "name": "probe", "description": "nested", "metadata": metadata}],
    }
    with pytest.raises(ContractLimitExceededError, match="depth"):
        CalibrationResult.from_json(json.dumps(payload))


def test_cap_floor_quote_rejects_lognormal_construction_and_json() -> None:
    with pytest.raises(ValueError, match="normal/Bachelier"):
        CapFloorQuote(5.0, 0.04, 0.2, is_cap=True, is_normal_vol=False)
    with pytest.raises(ValueError, match="normal/Bachelier"):
        CapFloorQuote.from_json(
            json.dumps({"maturity": 5.0, "strike": 0.04, "volatility": 0.2, "is_cap": True, "is_normal_vol": False})
        )


def test_omitted_optional_fields_take_the_rust_authoring_defaults_and_accept_none() -> None:
    """CFCC-003: optional authoring fields reach the Rust consts; None is the omitted value."""
    futures = json.loads(RateQuote.futures("F", "CME:SR3", "2026-09-15", 96.5, convexity_adjustment=None).to_json())
    assert futures["convexity_adjustment"] == 0.0
    option = json.loads(VolQuote.option_vol("O", "AAPL", "2027-05-08", 155.0, 0.28, option_type=None).to_json())
    assert option["option_vol"]["option_type"] == "call"
    hazard = CalibrationStep.hazard("ACME", "ACME", "USD", "2026-05-08", "USD-OIS", 0.4, seniority=None)
    assert hazard.params["seniority"] == "senior"
    assert CalibrationStep.vol_surface("V", "2026-05-08", "AAPL", model=None).params["model"] == "sabr"
    assert CalibrationStep.parametric("P", "2026-05-08", model=None).params["model"] == "ns"


def test_hull_white_quote_flags_are_required() -> None:
    """CFCC-003: Rust has no default for the vol unit or the cap/floor side."""
    with pytest.raises(TypeError, match="is_normal_vol"):
        SwaptionQuote(1.0, 5.0, 0.20)  # type: ignore[call-arg]
    with pytest.raises(TypeError, match="is_cap"):
        CapFloorQuote(5.0, 0.04, 0.007)  # type: ignore[call-arg]
    assert SwaptionQuote(1.0, 5.0, 0.20, False).is_normal_vol is False
    assert CapFloorQuote(5.0, 0.04, 0.007, False, True).is_cap is False


def _surface(vols: list[list[float]]) -> VolSurface:
    return VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], vols)


def test_typed_market_data_feeds_the_envelope() -> None:
    """CFCC-009: typed quotes and datums are accepted wherever market data is."""
    quotes = [
        RateQuote.deposit("USD-DEP-3M", "USD-SOFR-OIS", "3M", 0.052),
        RateQuote.swap("USD-SWAP-2Y", "USD-SOFR-OIS", "2Y", 0.049),
    ]
    plan = CalibrationPlan([CalibrationStep.discount("USD-OIS", "USD", "2026-05-08", quotes=quotes)])
    data = [
        InflationQuote.inflation_swap("ZC5Y", "2031-05-08", 0.025, "USA-CPI-U", "USD"),
        InflationQuote.yoy_inflation_swap("YOY5Y", "2031-05-08", 0.024, "USA-CPI-U", "1Y", "USD"),
        XccyQuote("X5Y", "EUR-USD", "5Y", -12.5, spot_fx=1.1),
        CdsTrancheQuote("IG-3-7", "CDX.NA.IG", 42, 0.03, 0.07, "2031-06-20", 0.01, 100.0, "USD", "isda_na"),
        FxSpotDatum("EURUSD", "EUR", "USD", 1.1),
        PriceDatum("AAPL", 187.5),
        DividendScheduleDatum({"id": "AAPL-DIVS", "underlying": "AAPL", "events": []}),
        CollateralEntry("EUR", "USD"),
    ]
    envelope = CalibrationEnvelope(plan, market_data=data)
    kinds = [datum["kind"] for datum in envelope.market_data[2:]]
    assert kinds == [
        "inflation_quote",
        "inflation_quote",
        "xccy_quote",
        "cds_tranche_quote",
        "fx_spot",
        "price",
        "dividend_schedule",
        "collateral",
    ]
    assert calibrate(envelope).success
    for datum in data:
        assert type(datum).from_json(datum.to_json()).to_json() == datum.to_json()


def test_typed_quotes_expose_fields_and_reject_invalid_inputs() -> None:
    yoy = InflationQuote.yoy_inflation_swap("YOY5Y", "2031-05-08", 0.024, "USA-CPI-U", "1Y", "USD")
    assert (yoy.type, yoy.rate, yoy.index, yoy.frequency, yoy.convention) == (
        "yoy_inflation_swap",
        0.024,
        "USA-CPI-U",
        "1Y",
        "USD",
    )
    assert str(yoy.maturity) == "2031-05-08"
    with pytest.raises(ValueError, match="invalid InflationQuote"):
        InflationQuote.inflation_swap("ZC", "2031-05-08", float("nan"), "USA-CPI-U", "USD")
    with pytest.raises(ValueError, match="spot_fx"):
        XccyQuote("X", "EUR-USD", "5Y", -12.5, spot_fx=-1.0)
    with pytest.raises(ValueError, match="attachment"):
        CdsTrancheQuote("T", "CDX.NA.IG", 42, 0.08, 0.07, "2031-06-20", 0.01, 100.0, "USD", "isda_na")
    cds = CdsQuote.par_spread("ACME-5Y", "ACME", "USD", "xr14", "5Y", 120.0, 0.4)
    assert (cds.entity, cds.recovery_rate) == ("ACME", 0.4)
    assert cds.convention == {"currency": "USD", "doc_clause": "xr14"}
    assert cds.pillar == {"tenor": {"count": 5, "unit": "years"}}
    assert PriceDatum("AAPL", 187.5, "USD").currency == "USD"
    assert FxSpotDatum("EURUSD", "EUR", "USD", 1.1).from_ == "EUR"


def test_surface_validators_accept_a_clean_surface_and_reject_arbitrage() -> None:
    """CFCC-009: the Rust vol-surface checks run on a standalone surface."""
    clean = _surface([[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
    config = ValidationConfig()
    validate_surface(clean, config)
    validate_calendar_spread(clean, config)
    validate_butterfly_spread(clean, config)
    validate_vol_bounds(clean, config)
    validate_surface_with_forwards(clean, config, [100.0, 100.0])
    validate_calendar_spread_with_forwards(clean, config, [100.0, 100.0])
    validate_butterfly_call_convexity(clean, config, [100.0, 100.0])

    inverted = _surface([[0.30, 0.30, 0.30], [0.10, 0.10, 0.10]])
    with pytest.raises(ValueError, match=r"(?i)calendar"):
        validate_calendar_spread(inverted, config)
    validate_calendar_spread(inverted, ValidationConfig(check_arbitrage=False))
    with pytest.raises(ValueError, match="forward"):
        validate_surface_with_forwards(clean, config, [100.0])
    with pytest.raises(ValueError, match="volatility"):
        validate_vol_bounds(clean, ValidationConfig(max_volatility=0.1))
