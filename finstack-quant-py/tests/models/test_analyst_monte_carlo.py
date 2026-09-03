"""Executable financial checks behind analyst lessons 2.6 through 2.8.

Exact checks cover deterministic replay and payoff identities. Comparisons with
analytical expectations use four sample standard errors, not rounded prices or
an assumption that a particular realization's pricing error decreases.
"""

import datetime
import json
import math
from pathlib import Path
import statistics

import numpy as np
import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.models import asian_option_price, barrier_call, bs_price
from finstack_quant.models.monte_carlo import (
    EuropeanPricer,
    LsmcPricer,
    PathDependentPricer,
    simulate_gbm_paths,
)
from finstack_quant.valuations.instruments import price_instrument


def test_european_serial_parallel_and_seed_replay_are_identical() -> None:
    """Both runtime paths return the same estimate, interval, and sample counts."""
    estimates = [
        EuropeanPricer(num_paths=4096, seed=17, use_parallel=parallel).price_call(
            100.0, 100.0, 0.05, 0.0, 0.2, 1.0, num_steps=1
        )
        for parallel in (False, True, False)
    ]
    signatures = [
        (
            item.mean.amount,
            item.stderr,
            item.ci_lower.amount,
            item.ci_upper.amount,
            item.num_paths,
            item.num_simulated_paths,
        )
        for item in estimates
    ]
    assert signatures[0] == signatures[1] == signatures[2]
    assert abs(estimates[0].mean.amount - bs_price(100, 100, 0.05, 0, 0.2, 1, True)) < 4 * estimates[0].stderr


def test_maturity_only_lsmc_counts_pairs_and_uses_pair_mean_standard_error() -> None:
    """N paired estimators and 2N plain estimators have the same path budget."""
    pairs = 2048
    seed = 17
    spot, strike, rate, vol, expiry = 100.0, 100.0, 0.05, 0.2, 1.0
    captured = simulate_gbm_paths(spot, rate, 0.0, vol, expiry, 1, 2 * pairs, seed=seed)
    terminals = [row[-1] for row in captured.paths]
    discount = math.exp(-rate * expiry)
    plain_payoffs = [discount * max(strike - terminal, 0.0) for terminal in terminals]
    reflection_product = (spot * math.exp((rate - 0.5 * vol**2) * expiry)) ** 2
    paired_payoffs = [
        0.5 * (plain_payoffs[index] + discount * max(strike - reflection_product / terminal, 0.0))
        for index, terminal in enumerate(terminals[:pairs])
    ]
    results = []
    for antithetic, count, payoffs in ((True, pairs, paired_payoffs), (False, 2 * pairs, plain_payoffs)):
        result = LsmcPricer(
            num_paths=count, seed=seed, use_parallel=False, num_steps=1, antithetic=antithetic
        ).price_american_put(spot, strike, rate, 0.0, vol, expiry)
        assert result.num_paths == count
        assert result.num_simulated_paths == 2 * pairs
        assert result.mean.amount == pytest.approx(statistics.mean(payoffs), abs=1e-12)
        assert result.stderr == pytest.approx(statistics.stdev(payoffs) / math.sqrt(count), abs=1e-12)
        results.append(result)
    # A result for this monotone put payoff, not a guarantee for all payoffs.
    assert results[0].stderr < results[1].stderr


def test_captured_gbm_rejects_antithetic_request() -> None:
    """Path capture must fail explicitly rather than silently drop pairing."""
    with pytest.raises(ValueError, match="antithetic"):
        simulate_gbm_paths(100, 0.05, 0.0, 0.2, 1.0, 12, 16, seed=17, antithetic=True)


def test_structured_note_redemption_profiles_through_supported_json_route() -> None:
    """Known fixings isolate the three redemption contracts from MC noise."""
    example = (
        Path(__file__).resolve().parents[3]
        / "finstack-quant/valuations/tests/instruments/json_examples/autocallable.json"
    )
    envelope = json.loads(example.read_text())
    spec = envelope["instrument"]["spec"]
    as_of = datetime.date(2026, 1, 1)
    market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.0))
    market.insert_price("SPX-SPOT", Money(100.0, "USD"))
    spec.update(
        observation_dates=["2025-12-30", "2025-12-31"],
        payment_dates=["2026-01-02", "2026-01-02"],
        expiry="2026-01-02",
        initial_level=100.0,
        autocall_barriers=[3.0, 3.0],
        coupon_barriers=[0.0, 0.0],
        coupons=[0.0, 0.0],
        final_barrier=0.7,
        participation_rate=1.0,
        cap_level=1.5,
        notional={"amount": "1000", "currency": "USD"},
    )
    for terminal in (50.0, 70.0, 90.0, 100.0, 125.0, 160.0):
        spec["past_fixings"] = [["2025-12-30", 100.0], ["2025-12-31", terminal]]
        ratio = terminal / 100.0
        contracts = (
            ({"capital_protection": {"floor": 1.0}}, max(1.0, min(ratio, 1.5))),
            ({"participation": {"rate": 0.5}}, 1.0 + 0.5 * max(min(ratio, 1.5) - 1.0, 0.0)),
            ({"knock_in_put": {"strike": 100.0}}, ratio if ratio <= 0.7 else 1.0),
        )
        for payoff_type, expected_redemption in contracts:
            spec["final_payoff_type"] = payoff_type
            result = price_instrument(json.dumps(envelope), market, as_of)
            assert result.price == pytest.approx(1000.0 * expected_redemption, abs=1e-10)

    # A prior knock-in with full spot recovery returns principal, not put intrinsic.
    spec["final_payoff_type"] = {"knock_in_put": {"strike": 100.0}}
    spec["past_fixings"] = [["2025-12-30", 50.0], ["2025-12-31", 100.0]]
    assert price_instrument(json.dumps(envelope), market, as_of).price == pytest.approx(1000.0)


def test_nested_barrier_monitoring_on_identical_paths_has_pathwise_order() -> None:
    """Quarterly, monthly, daily monitoring reuse one captured fine path set."""
    paths = np.asarray(simulate_gbm_paths(100, 0.05, 0.0, 0.2, 1.0, 252, 4096, seed=17).paths)
    terminal_payoff = math.exp(-0.05) * np.maximum(paths[:, -1] - 100.0, 0.0)
    monitored = [
        terminal_payoff * (np.max(paths[:, :: 252 // observations], axis=1) < 120.0) for observations in (4, 12, 252)
    ]
    assert np.all(monitored[0] >= monitored[1])
    assert np.all(monitored[1] >= monitored[2])
    assert monitored[0].mean() > monitored[2].mean()
    continuous = barrier_call(100, 100, 120, 0.05, 0.0, 0.2, 1.0, "up", "out")
    assert 0 < continuous < bs_price(100, 100, 0.05, 0.0, 0.2, 1.0, True)
    fine_se = monitored[-1].std(ddof=1) / math.sqrt(len(paths))
    # Continuous <= discrete is an expectation ordering; allow sampling error.
    assert monitored[-1].mean() + 4 * fine_se >= continuous


def test_asian_exact_one_fixing_and_geometric_benchmark() -> None:
    """Arithmetic TW is approximate; one terminal fixing reduces exactly to BS."""
    args = (100.0, 100.0, 0.05, 0.0, 0.2, 1.0)
    vanilla = bs_price(*args, True)
    for convention in ("arithmetic", "geometric"):
        assert asian_option_price(*args, 1, convention, True) == pytest.approx(vanilla, abs=1e-12)
    european = EuropeanPricer(4096, 17, False).price_call(*args, num_steps=1)
    asian = PathDependentPricer(4096, 17, False, False).price_asian_call(*args, num_steps=1)
    assert asian.mean.amount == european.mean.amount
    # The two payoff reducers may round their sample variance one ulp apart.
    assert asian.stderr == pytest.approx(european.stderr, abs=1e-14)

    paths = np.asarray(simulate_gbm_paths(100, 0.05, 0.0, 0.2, 1.0, 12, 4096, seed=17).paths)
    fixings = paths[:, 1:]  # Observations at T/12, ..., T; the initial spot is excluded.
    arithmetic = math.exp(-0.05) * np.maximum(fixings.mean(axis=1) - 100, 0)
    geometric = math.exp(-0.05) * np.maximum(np.exp(np.log(fixings).mean(axis=1)) - 100, 0)
    assert np.all(arithmetic + 1e-12 >= geometric)
    exact_geometric = asian_option_price(*args, 12, "geometric", True)
    geometric_se = geometric.std(ddof=1) / math.sqrt(len(paths))
    assert abs(geometric.mean() - exact_geometric) < 4 * geometric_se


def test_lsmc_holdout_seed_is_independent_and_replayable() -> None:
    """Two-pass pricing rejects reuse of training shocks and replays held-out runs."""
    args = (100.0, 100.0, 0.05, 0.0, 0.3, 1.0)
    pricer = LsmcPricer(2048, 17, False, 20, "polynomial", 3, True)
    with pytest.raises(ValueError, match="pricing_seed"):
        pricer.price_american_put_unbiased(*args, pricing_seed=17)
    first = pricer.price_american_put_unbiased(*args, pricing_seed=1017)
    second = pricer.price_american_put_unbiased(*args, pricing_seed=1017)
    assert first.mean.amount == second.mean.amount
    assert first.stderr == second.stderr
    assert first.num_paths == 2048
    assert first.num_simulated_paths == 4096
    assert 0 < first.mean.amount < 100
    assert math.isfinite(first.stderr)
    assert first.stderr > 0
    # No requirement that a fitted policy's finite-sample price exceeds BS,
    # or that a held-out realization is below its in-sample realization.
