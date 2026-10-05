"""Tests for Monte Carlo pricing through the pricer and engine surfaces."""

from collections.abc import Callable
import json
import math
import pickle

import pytest

from finstack_quant.models.monte_carlo import (
    Estimate,
    EuropeanPricer,
    LrmGreeks,
    PathDependentPricer,
    PathSummary,
    finite_diff_delta,
    finite_diff_gamma,
    heston_satisfies_feller,
    simulate_paths,
)

GreekFunction = Callable[..., Estimate]


class TestEuropeanPricer:
    """EuropeanPricer produces reasonable option prices under GBM."""

    @pytest.fixture
    def pricer(self) -> EuropeanPricer:
        """Deterministic pricer with enough paths for rough convergence."""
        return EuropeanPricer(num_paths=50_000, seed=42)

    def test_call_price_positive(self, pricer: EuropeanPricer) -> None:
        """ATM call price should be positive."""
        result = pricer.price_call(
            spot=100.0,
            strike=100.0,
            rate=0.05,
            div_yield=0.0,
            vol=0.20,
            expiry=1.0,
        )
        assert result.mean.amount > 0.0

    def test_put_price_positive(self, pricer: EuropeanPricer) -> None:
        """ATM put price should be positive."""
        result = pricer.price_put(
            spot=100.0,
            strike=100.0,
            rate=0.05,
            div_yield=0.0,
            vol=0.20,
            expiry=1.0,
        )
        assert result.mean.amount > 0.0

    def test_call_itm_more_expensive(self, pricer: EuropeanPricer) -> None:
        """A lower strike (deeper ITM) call should be more expensive."""
        atm = pricer.price_call(
            spot=100.0,
            strike=100.0,
            rate=0.05,
            div_yield=0.0,
            vol=0.20,
            expiry=1.0,
        )
        itm = pricer.price_call(
            spot=100.0,
            strike=80.0,
            rate=0.05,
            div_yield=0.0,
            vol=0.20,
            expiry=1.0,
        )
        assert itm.mean.amount > atm.mean.amount

    def test_put_call_parity_approx(self, pricer: EuropeanPricer) -> None:
        """Put-call parity: C - P ≈ e^(-rT) * (S*e^((r-q)T) - K)."""
        spot, strike, r, q, vol, expiry = 100.0, 100.0, 0.05, 0.0, 0.20, 1.0
        call = pricer.price_call(
            spot=spot,
            strike=strike,
            rate=r,
            div_yield=q,
            vol=vol,
            expiry=expiry,
        )
        put = pricer.price_put(
            spot=spot,
            strike=strike,
            rate=r,
            div_yield=q,
            vol=vol,
            expiry=expiry,
        )
        lhs = call.mean.amount - put.mean.amount
        forward = spot * math.exp((r - q) * expiry)
        rhs = math.exp(-r * expiry) * (forward - strike)
        assert lhs == pytest.approx(rhs, abs=2.0)

    def test_result_attributes(self, pricer: EuropeanPricer) -> None:
        """MoneyEstimate exposes stderr, ci_lower, ci_upper, num_paths."""
        result = pricer.price_call(
            spot=100.0,
            strike=100.0,
            rate=0.05,
            div_yield=0.0,
            vol=0.20,
            expiry=1.0,
        )
        assert result.stderr > 0.0
        assert result.ci_lower.amount < result.mean.amount
        assert result.ci_upper.amount > result.mean.amount
        assert result.num_paths == 50_000

    def test_seed_reproducibility(self) -> None:
        """Same seed produces identical results."""
        p1 = EuropeanPricer(num_paths=10_000, seed=123)
        p2 = EuropeanPricer(num_paths=10_000, seed=123)
        r1 = p1.price_call(spot=100.0, strike=100.0, rate=0.05, div_yield=0.0, vol=0.20, expiry=1.0)
        r2 = p2.price_call(spot=100.0, strike=100.0, rate=0.05, div_yield=0.0, vol=0.20, expiry=1.0)
        assert r1.mean.amount == pytest.approx(r2.mean.amount, abs=1e-10)


_GBM_SPEC = {
    "process": {"type": "gbm", "r": 0.05, "q": 0.01, "sigma": 0.2},
    "initial_state": [100.0],
    "time_grid": {"type": "uniform", "expiry": 1.0, "num_steps": 4},
    "num_paths": 3,
    "seed": 42,
}


def test_simulate_paths_is_typed_deterministic_and_shaped() -> None:
    first = simulate_paths(_GBM_SPEC)
    second = simulate_paths(json.dumps(_GBM_SPEC))
    assert isinstance(first, PathSummary)
    assert first.times == second.times == [0.0, 0.25, 0.5, 0.75, 1.0]
    assert first.values == second.values
    assert first.num_paths == 3
    assert first.num_simulated_paths == 3
    assert first.dim == 1
    assert first.factor_names == ["spot"]
    assert len(first.values) == 3 * 5
    assert first.values[0::5] == [100.0, 100.0, 100.0]
    assert repr(first) == 'PathSummary(paths=3, points=5, factors=["spot"])'


def test_path_summary_round_trips_through_json_and_pickle() -> None:
    summary = simulate_paths(_GBM_SPEC)
    pickled = pickle.loads(pickle.dumps(summary))  # noqa: S301 - trusted in-process round trip
    for restored in (PathSummary.from_json(summary.to_json()), pickled):
        assert restored.values == summary.values
        assert restored.times == summary.times
        assert restored.factor_names == summary.factor_names
    with pytest.raises(ValueError, match="invalid PathSummary JSON"):
        PathSummary.from_json("{}")


@pytest.mark.parametrize(
    ("process", "initial_state", "factor_names"),
    [
        (
            {
                "type": "heston",
                "r": 0.04,
                "q": 0.01,
                "kappa": 2.0,
                "theta": 0.05,
                "sigma_v": 0.3,
                "rho": -0.7,
                "v0": 0.03,
            },
            [100.0, 0.03],
            ["spot", "variance"],
        ),
        (
            {
                "type": "hull_white_1f",
                "kappa": 0.8,
                "volatility": {"times": [0.0], "values": [0.01]},
                "theta_curve": [0.05],
                "theta_times": [0.0],
            },
            [0.02],
            ["short_rate"],
        ),
        ({"type": "cir", "kappa": 0.5, "theta": 0.04, "sigma": 0.1}, [0.03], ["short_rate"]),
        (
            {
                "type": "multi_gbm",
                "assets": [{"r": 0.04, "q": 0.01, "sigma": 0.25}, {"r": 0.04, "q": 0.03, "sigma": 0.15}],
                "correlation": [1.0, 0.6, 0.6, 1.0],
            },
            [100.0, 50.0],
            ["spot_0", "spot_1"],
        ),
    ],
)
def test_simulate_paths_reaches_non_gbm_processes(
    process: dict[str, object], initial_state: list[float], factor_names: list[str]
) -> None:
    summary = simulate_paths({**_GBM_SPEC, "process": process, "initial_state": initial_state})
    dim = len(factor_names)
    assert summary.factor_names == factor_names
    assert summary.dim == dim
    assert len(summary.values) == 3 * 5 * dim
    assert summary.values[:dim] == initial_state
    assert all(math.isfinite(value) for value in summary.values)


def test_simulate_paths_antithetic_stores_each_stream_and_its_mirror() -> None:
    plain = simulate_paths(_GBM_SPEC)
    paired = simulate_paths({**_GBM_SPEC, "antithetic": True})
    assert paired.num_paths == 3
    assert paired.num_simulated_paths == 6
    # Stored path 2k is stream k; path 2k + 1 negates its normal draws.
    assert paired.values[0:5] == plain.values[0:5]
    assert paired.values[10:15] == plain.values[5:10]
    drift = (0.05 - 0.01 - 0.5 * 0.2**2) * 0.25
    up = math.log(paired.values[1] / 100.0) - drift
    down = math.log(paired.values[6] / 100.0) - drift
    assert up == pytest.approx(-down, abs=1e-12)


def test_simulate_paths_scheme_changes_the_paths_but_not_the_seeded_draws() -> None:
    exact = simulate_paths(_GBM_SPEC)
    euler = simulate_paths({**_GBM_SPEC, "scheme": "euler"})
    assert euler.values != exact.values
    assert simulate_paths({**_GBM_SPEC, "scheme": "default"}).values == exact.values


@pytest.mark.parametrize(
    ("spec", "message"),
    [
        ({**_GBM_SPEC, "paths": 3}, "unknown field"),
        ({**_GBM_SPEC, "process": {"type": "local_vol"}}, "unknown variant"),
        ({**_GBM_SPEC, "process": {"type": "gbm", "r": 0.05, "q": 0.0, "sigma": -0.2}}, "sigma"),
        ({**_GBM_SPEC, "initial_state": [-100.0]}, "initial"),
        ({**_GBM_SPEC, "initial_state": [100.0, 1.0]}, "initial_state"),
        ({**_GBM_SPEC, "num_paths": 0}, "num_paths"),
        (
            {**_GBM_SPEC, "process": {"type": "cir", "kappa": 0.5, "theta": 0.04, "sigma": 0.1}, "scheme": "milstein"},
            "scheme 'milstein' is not available for process 'cir'",
        ),
        ("not json", "invalid PathSimulationSpec"),
        (42, "invalid PathSimulationSpec"),
    ],
)
def test_simulate_paths_maps_invalid_specs_to_value_error(spec: object, message: str) -> None:
    with pytest.raises(ValueError, match=message):
        simulate_paths(spec)  # type: ignore[arg-type]


def test_lrm_greeks_share_the_plain_asian_price_and_bracket_the_bump_delta() -> None:
    pricer = PathDependentPricer(20_000, 7, use_parallel=False)
    args = (100.0, 100.0, 0.04, 0.01, 0.25, 1.0)
    greeks = pricer.price_with_lrm_greeks(*args, True, num_steps=12)

    assert isinstance(greeks, LrmGreeks)
    assert greeks.price.mean.amount == pricer.price_asian_call(*args, num_steps=12).mean.amount
    assert greeks.price.mean.currency.code == "USD"
    assert greeks.delta.num_paths == greeks.vega.num_paths == 20_000

    bump = 0.5
    up = pricer.price_asian_call(100.0 + bump, *args[1:], num_steps=12).mean.amount
    down = pricer.price_asian_call(100.0 - bump, *args[1:], num_steps=12).mean.amount
    assert abs(greeks.delta.mean - (up - down) / (2 * bump)) < 5 * greeks.delta.stderr
    assert greeks.vega.stderr > 0.0

    put = pricer.price_with_lrm_greeks(*args, False, num_steps=12, currency="EUR")
    assert put.delta.mean < 0.0
    assert put.price.mean.currency.code == "EUR"


def test_lrm_greeks_round_trip_and_tabulate() -> None:
    greeks = PathDependentPricer(2_000, 7, use_parallel=False).price_with_lrm_greeks(
        100.0, 100.0, 0.04, 0.01, 0.25, 1.0, True, num_steps=12
    )

    assert set(json.loads(greeks.to_json())) == {"price", "delta", "vega"}
    pickled = pickle.loads(pickle.dumps(greeks))  # noqa: S301 - trusted in-process round trip
    for restored in (LrmGreeks.from_json(greeks.to_json()), pickled):
        assert restored.to_json() == greeks.to_json()
    with pytest.raises(ValueError, match="invalid LrmGreeks JSON"):
        LrmGreeks.from_json('{"price": 1.0}')

    frame = greeks.to_dataframe()
    assert list(frame.index) == ["price", "delta", "vega"]
    assert list(frame.columns) == ["mean", "stderr", "ci_lower", "ci_upper"]
    assert frame.loc["delta", "mean"] == greeks.delta.mean
    assert frame.loc["price", "ci_upper"] == greeks.price.ci_upper.amount
    assert "<table" in greeks._repr_html_()
    assert repr(greeks).startswith("LrmGreeks(price=")


def test_lrm_greeks_default_arguments_fit_the_capture_cap() -> None:
    # 100,000 default paths x (32 default steps + 1) stays under 4,000,000 points.
    greeks = PathDependentPricer().price_with_lrm_greeks(100.0, 100.0, 0.04, 0.01, 0.25, 1.0, True)
    assert greeks.price.num_paths == 100_000
    assert 0.0 < greeks.delta.mean < 1.0


@pytest.mark.parametrize(
    ("pricer_kwargs", "overrides", "message"),
    [
        ({}, {"num_steps": 252}, "must not exceed 4000000"),
        ({"num_paths": 500}, {"spot": 0.0}, "initial_spot|spot"),
        ({"num_paths": 500}, {"vol": 0.0}, "vol"),
        ({"num_paths": 500}, {"expiry": 0.0}, "."),
        ({"num_paths": 500, "antithetic": True}, {}, "antithetic"),
        ({"num_paths": 512, "use_sobol": True, "use_parallel": False}, {}, "Sobol"),
    ],
)
def test_lrm_greeks_map_unsupported_runs_to_value_error(
    pricer_kwargs: dict[str, object], overrides: dict[str, float], message: str
) -> None:
    inputs = {
        "spot": 100.0,
        "strike": 100.0,
        "rate": 0.04,
        "div_yield": 0.01,
        "vol": 0.25,
        "expiry": 1.0,
        "is_call": True,
        "num_steps": 12,
    } | overrides
    with pytest.raises(ValueError, match=message):
        PathDependentPricer(**pricer_kwargs).price_with_lrm_greeks(**inputs)


def test_heston_feller_uses_inclusive_predicate_without_validation() -> None:
    assert heston_satisfies_feller(2.0, 0.04, 0.3)
    assert heston_satisfies_feller(1.0, 0.045, 0.3)
    assert not heston_satisfies_feller(1.0, 0.04, 0.5)
    assert not heston_satisfies_feller(0.0, 0.04, 0.3)


@pytest.mark.parametrize(
    "greek",
    [
        finite_diff_delta,
        finite_diff_gamma,
    ],
)
@pytest.mark.parametrize(
    ("spot", "bump_size", "message"),
    [
        (0.0, 0.01, "initial_spot"),
        (float("nan"), 0.01, "initial_spot"),
        (100.0, 0.0, "bump_size"),
        (100.0, float("inf"), "bump_size"),
        (1e-8, 2.0, "central stencil"),
    ],
)
def test_finite_difference_greeks_map_invalid_stencils_to_value_error(
    greek: GreekFunction,
    spot: float,
    bump_size: float,
    message: str,
) -> None:
    with pytest.raises(ValueError, match=message):
        greek(
            spot=spot,
            strike=100.0,
            rate=0.05,
            div_yield=0.0,
            vol=0.2,
            expiry=1.0,
            num_paths=8,
            num_steps=1,
            bump_size=bump_size,
            is_call=True,
        )
