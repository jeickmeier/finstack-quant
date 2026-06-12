"""Tests for Monte Carlo pricing: European pricer and convenience functions."""

import math

import pytest

from finstack.monte_carlo import (
    EuropeanPricer,
    price_european_call,
    price_european_put,
)


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


class TestPriceEuropeanCallFunction:
    """Module-level price_european_call convenience function."""

    def test_produces_positive_price(self) -> None:
        """ATM call has a positive price."""
        result = price_european_call(
            spot=100.0,
            strike=100.0,
            rate=0.05,
            div_yield=0.0,
            vol=0.20,
            expiry=1.0,
            num_paths=10_000,
            seed=42,
        )
        assert result.mean.amount > 0.0

    def test_currency_default_is_usd(self) -> None:
        """Default currency should be USD."""
        result = price_european_call(
            spot=100.0,
            strike=100.0,
            rate=0.05,
            div_yield=0.0,
            vol=0.20,
            expiry=1.0,
            num_paths=10_000,
            seed=42,
        )
        assert result.mean.currency.code == "USD"


class TestPriceEuropeanPutFunction:
    """Module-level price_european_put convenience function."""

    def test_produces_positive_price(self) -> None:
        """ATM put has a positive price."""
        result = price_european_put(
            spot=100.0,
            strike=100.0,
            rate=0.05,
            div_yield=0.0,
            vol=0.20,
            expiry=1.0,
            num_paths=10_000,
            seed=42,
        )
        assert result.mean.amount > 0.0
