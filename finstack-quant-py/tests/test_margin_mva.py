"""Behavioral tests for MVA Python bindings (margin.compute_mva et al.)."""

from __future__ import annotations

import datetime as dt
import json
import math

import pytest

from finstack_quant.core.market_data import DiscountCurve, HazardCurve
from finstack_quant.margin import (
    ExposureProfile,
    FundingConfig,
    ImDecayProfile,
    ImProfile,
    ImProjectionSpec,
    MarginFundingSpread,
    MvaResult,
    NettingSetId,
    SimmCalculator,
    SimmSensitivities,
    XvaResult,
    compute_bilateral_xva,
    compute_mva,
    im_profile_from_simm,
)

AS_OF = dt.date(2025, 1, 1)
NETTING_SET = NettingSetId.bilateral("dealer", "test-csa")


def expected_im(times: list[float], values: list[float]) -> ImProfile:
    dates = [AS_OF + dt.timedelta(days=round(365 * time)) for time in times]
    return ImProfile(
        as_of=AS_OF,
        currency="USD",
        netting_set=NETTING_SET,
        source_id="dated-im-test",
        dates=dates,
        im_values=values,
    )


def projection(decay: ImDecayProfile) -> ImProjectionSpec:
    return ImProjectionSpec(
        as_of=AS_OF,
        currency="USD",
        netting_set=NETTING_SET,
        scope_source="simm-test",
        dates=[AS_OF + dt.timedelta(days=365 * time) for time in (0, 1, 2, 4)],
        decay=decay,
    )


def flat_discount_curve() -> DiscountCurve:
    knots = [(0.5 * i, 1.0) for i in range(9)]
    return DiscountCurve(
        "USD-OIS", dt.date(2025, 1, 1), knots, interp="log_linear", extrapolation="extend", day_count="act_365f"
    )


def test_decay_profile_factors() -> None:
    assert ImDecayProfile.constant().factor(5.0) == pytest.approx(1.0)
    lin = ImDecayProfile.linear_to_maturity(2.0)
    assert lin.factor(1.0) == pytest.approx(0.5)
    assert lin.factor(3.0) == pytest.approx(0.0)
    sq = ImDecayProfile.sqrt_time(2.0)
    assert sq.factor(1.0) == pytest.approx(0.5**0.5)


def test_compute_mva_flat_spread_arithmetic() -> None:
    # IM = 1e6 constant from valuation through year 2, 50bp flat, DF = 1, no survival:
    # MVA = 0.005 * 1e6 * 2 = 10_000 exactly (see Rust tests).
    profile = expected_im([0, 1, 2], [1e6, 1e6, 1e6])
    result = compute_mva(profile, MarginFundingSpread.flat(50.0), flat_discount_curve(), None)
    assert isinstance(result, MvaResult)
    assert result.mva == pytest.approx(10_000.0, abs=1e-6)
    assert result.average_im == pytest.approx(1_000_000.0, abs=1e-6)


def test_im_profile_from_simm_scales_by_decay() -> None:
    sens = SimmSensitivities("USD")
    sens.add_ir_delta("USD", "5Y", 50_000.0)
    calc = SimmCalculator("v2_6")
    decay = ImDecayProfile.linear_to_maturity(4.0)
    profile = im_profile_from_simm(calc, sens, projection(decay))
    assert profile.times == [0.0, 1.0, 2.0, 4.0]
    assert profile.im_values[0] > 0.0
    assert profile.im_values[2] == pytest.approx(profile.im_values[0] * 0.5)
    assert profile.im_values[3] == pytest.approx(0.0, abs=1e-9)


def test_im_profile_to_dataframe() -> None:
    pd = pytest.importorskip("pandas")
    profile = expected_im([0, 1], [100.0, 50.0])
    df = profile.to_pandas()
    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["date", "im"]
    assert list(df.index) == [0.0, 1.0]
    assert df["im"].tolist() == [100.0, 50.0]


def test_mva_result_to_dataframe_and_json_round_trip() -> None:
    pd = pytest.importorskip("pandas")
    profile = expected_im([0, 1, 2], [1e6, 1e6, 500_000.0])
    result = compute_mva(profile, MarginFundingSpread.flat(50.0), flat_discount_curve(), None)
    df = result.to_pandas()
    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["date", "im"]
    back = MvaResult.from_json(result.to_json())
    assert back.mva == pytest.approx(result.mva)


def test_compute_mva_validation_errors() -> None:
    with pytest.raises(ValueError, match="nonnegative"):
        expected_im([0, 1], [-5.0, 2.0])
    good = expected_im([0, 1], [5.0, 0])
    with pytest.raises(ValueError, match="cover the IM horizon"):
        compute_mva(good, MarginFundingSpread.dated([]), flat_discount_curve(), None)


def test_mva_integrates_spread_and_margin_before_averaging() -> None:
    profile = expected_im([0, 1], [100.0, 0.0])
    spread = MarginFundingSpread.dated([(AS_OF, 0.0), (profile.dates[-1], 100.0)])
    result = compute_mva(profile, spread, flat_discount_curve(), None)
    assert result.mva == pytest.approx(1 / 6)
    assert result.average_im == pytest.approx(50.0)
    assert result.intervals[0]["contribution"] == pytest.approx(result.mva)


def test_dated_margin_funding_reads_a_pandas_series() -> None:
    pd = pytest.importorskip("pandas")
    profile = expected_im([0, 1], [100.0, 0.0])
    spread = MarginFundingSpread.dated(pd.Series([0.0, 100.0], index=profile.dates))
    result = compute_mva(profile, spread, flat_discount_curve(), None)
    assert result.mva == pytest.approx(1 / 6)


def test_compute_mva_keyword_arguments() -> None:
    # Guards against keyword/positional signature drift (text_signature must
    # match the #[pyo3(signature = ...)] parameter names exactly).
    profile = expected_im([0, 1, 2], [1e6, 1e6, 1e6])
    result = compute_mva(
        im_profile=profile,
        funding_spread=MarginFundingSpread.flat(50.0),
        discount_curve=flat_discount_curve(),
        own_survival=None,
    )
    assert result.mva == pytest.approx(10_000.0, abs=1e-6)


def test_im_profile_from_simm_keyword_arguments() -> None:
    sens = SimmSensitivities("USD")
    sens.add_ir_delta("USD", "5Y", 50_000.0)
    calc = SimmCalculator("v2_6")
    decay = ImDecayProfile.linear_to_maturity(maturity_years=4.0)
    profile = im_profile_from_simm(
        calculator=calc,
        sensitivities=sens,
        spec=projection(decay),
    )
    assert profile.times == [0.0, 1.0, 2.0, 4.0]


def test_funding_config_carries_im_profile_for_mva() -> None:
    """``FundingConfig`` exposes the MVA inputs the Rust engine consumes."""
    profile = expected_im([0, 1, 2], [1e6, 1e6, 1e6])
    config = FundingConfig(50.0, 30.0, profile, MarginFundingSpread.flat(20.0))

    assert config.funding_spread_bp == pytest.approx(50.0)
    assert config.effective_benefit_bp() == pytest.approx(30.0)
    assert json.loads(config.to_json())["margin_funding_spread"]["spread_bp"] == 20.0
    assert config.im_profile is not None
    assert config.im_profile.times == [0.0, 1.0, 2.0]


def test_funding_config_without_elected_im_stays_fva_only() -> None:
    config = FundingConfig(45.0)
    assert config.im_profile is None
    assert config.margin_funding_spread is None


@pytest.mark.parametrize(
    ("args", "message"),
    [
        ((-1.0,), "funding_spread_bp"),
        ((25.0, 30.0), "funding_benefit_bp"),
        ((25.0, None, None, MarginFundingSpread.flat(25.0)), "together"),
    ],
)
def test_funding_config_constructor_validates(args: tuple[object, ...], message: str) -> None:
    with pytest.raises(ValueError, match=message):
        FundingConfig(*args)


def test_xva_result_exposes_mva_and_total_xva() -> None:
    """The required total composes all computed XVA legs."""
    payload = json.dumps({
        "cva": 100.0,
        "dva": 30.0,
        "fva": 20.0,
        "mva": 15.0,
        "total_xva": 105.0,
        "epe_profile": [[1.0, 10.0]],
        "ene_profile": [[1.0, 2.0]],
        "max_epe": 10.0,
        "effective_epe_profile": [[1.0, 10.0]],
        "effective_epe": 10.0,
    })
    result = XvaResult.from_json(payload)

    assert result.mva == pytest.approx(15.0)
    assert result.total_xva == pytest.approx(result.cva - result.dva + result.fva + result.mva)


def test_xva_result_funding_legs_are_optional_but_total_is_required() -> None:
    """Payloads may omit funding legs while retaining the required total."""
    payload = json.dumps({
        "cva": 100.0,
        "total_xva": 100.0,
        "epe_profile": [[1.0, 10.0]],
        "ene_profile": [[1.0, 0.0]],
        "max_epe": 10.0,
        "effective_epe_profile": [[1.0, 10.0]],
        "effective_epe": 10.0,
    })
    result = XvaResult.from_json(payload)

    assert result.mva is None
    assert result.total_xva == pytest.approx(100.0)


def flat_hazard_curve(lam: float) -> HazardCurve:
    return HazardCurve("HZ", dt.date(2025, 1, 1), [(1.0, lam), (30.0, lam)], recovery_rate=0.40, day_count="act_365f")


def uniform_exposure(mark: float = 1e6) -> ExposureProfile:
    return ExposureProfile(
        as_of="2025-01-01",
        currency="USD",
        basis={"kind": "external", "source_id": "funding-test", "policy_id": "unsecured"},
        dates=["2025-01-01", "2026-01-01", "2027-01-01"],
        times=[0.0, 1.0, 2.0],
        mtm_values=[mark] * 3,
        epe=[max(mark, 0.0)] * 3,
        ene=[max(-mark, 0.0)] * 3,
        diagnostics={"market_roll_failures": 0, "valuation_failures": 0, "total_time_points": 3},
    )


def test_compute_bilateral_xva_aggregates_mva_into_total() -> None:
    """MVA reaches ``total_xva``; zero hazard pins it to the Rust figure."""
    no_default = flat_hazard_curve(0.0)
    funding = FundingConfig(50.0, 30.0, expected_im([0, 1, 2], [1e6] * 3), MarginFundingSpread.flat(50.0))

    result = compute_bilateral_xva(
        uniform_exposure(),
        no_default,
        no_default,
        flat_discount_curve(),
        0.40,
        0.40,
        funding,
    )

    # Zero hazard ⇒ no credit legs, and MVA is exactly the hand-checked 10_000
    # (flat 1e6 IM, 50bp, DF = 1, grid [1, 2]).
    assert result.cva == pytest.approx(0.0)
    assert result.dva == pytest.approx(0.0)
    assert result.mva == pytest.approx(10_000.0)
    assert result.mva_detail is not None
    assert result.mva_detail.average_im == pytest.approx(1e6)
    assert sum(row["contribution"] for row in result.mva_detail.intervals) == pytest.approx(result.mva)
    assert result.fva > 0.0
    assert result.total_xva == pytest.approx(result.cva - result.dva + result.fva + result.mva)


def test_compute_bilateral_xva_uses_the_exposed_ene_with_im_funding() -> None:
    """Expected posted IM is a funding input and DVA follows the exposed ENE."""
    exposure = uniform_exposure(-1e6)
    args = (
        exposure,
        flat_hazard_curve(0.0),
        flat_hazard_curve(0.03),
        flat_discount_curve(),
        0.40,
        0.40,
    )

    without_im = compute_bilateral_xva(*args)
    with_im = compute_bilateral_xva(
        *args,
        FundingConfig(0.0, 0.0, expected_im([0, 1, 2], [400_000.0] * 3), MarginFundingSpread.flat(0.0)),
    )

    assert without_im.dva is not None
    assert with_im.dva is not None
    assert without_im.dva > 0.0
    assert with_im.dva == without_im.dva


def test_compute_bilateral_xva_rejects_im_exposure_horizon_mismatch() -> None:
    """An IM profile may not silently extrapolate past the exposure horizon."""
    funding = FundingConfig(50.0, None, expected_im([0, 1, 3], [1e6] * 3), MarginFundingSpread.flat(50.0))

    with pytest.raises(ValueError, match="horizon"):
        compute_bilateral_xva(
            uniform_exposure(),
            flat_hazard_curve(0.02),
            flat_hazard_curve(0.03),
            flat_discount_curve(),
            0.40,
            0.40,
            funding,
        )


def test_compute_bilateral_xva_without_funding_has_no_funding_legs() -> None:
    result = compute_bilateral_xva(
        uniform_exposure(),
        flat_hazard_curve(0.02),
        flat_hazard_curve(0.03),
        flat_discount_curve(),
        0.40,
        0.40,
    )

    assert result.fva is None
    assert result.mva is None
    assert result.cva > 0.0
    assert result.total_xva == pytest.approx(result.cva - result.dva)


def test_compute_bilateral_xva_rejects_bad_recovery_rate() -> None:
    with pytest.raises(ValueError, match="recovery_rate"):
        compute_bilateral_xva(
            uniform_exposure(),
            flat_hazard_curve(0.02),
            flat_hazard_curve(0.02),
            flat_discount_curve(),
            1.5,
            0.40,
        )


def test_compute_bilateral_xva_accepts_keyword_arguments() -> None:
    result = compute_bilateral_xva(
        exposure_profile=uniform_exposure(),
        counterparty_hazard_curve=flat_hazard_curve(0.02),
        own_hazard_curve=flat_hazard_curve(0.03),
        discount_curve=flat_discount_curve(),
        counterparty_recovery_rate=0.40,
        own_recovery_rate=0.40,
        funding=None,
    )
    assert math.isfinite(result.total_xva)
