"""Cross-boundary regression cases for checked Campisi fixed-income attribution."""

from datetime import date
from decimal import Decimal
import json

import pytest

from finstack_quant.core.money import Money
from finstack_quant.portfolio import (
    FiAttributionConfig,
    FiAttributionResult,
    FiPositionSnapshot,
    FiQuote,
    FiQuoteLeg,
    FiReportingBasis,
    PortfolioError,
    campisi_attribution,
    campisi_carino_link,
)


def _config(
    sectors: list[str],
    start: str = "2025-01-01",
    end: str = "2025-03-15",
) -> FiAttributionConfig:
    return FiAttributionConfig(
        date.fromisoformat(start),
        date.fromisoformat(end),
        "USD",
        FiReportingBasis("portfolio", "benchmark", "reinvested", "dirty settlement", "USD native", "net of fees"),
        sectors,
    )


def _position(
    id: str,
    sector: str,
    weight: str,
    total_return: float,
    coupon: Decimal = Decimal(0),
) -> FiPositionSnapshot:
    return FiPositionSnapshot(
        id, sector, Decimal(weight), total_return, Money(80, "USD"), Money.from_decimal(coupon, "USD")
    )


def _leg(basis: str, duration: float, opening: float, closing: float) -> FiQuoteLeg:
    return FiQuoteLeg(basis, duration, FiQuote(basis, opening), FiQuote(basis, closing))


def test_dated_coupon_and_qualified_shocks() -> None:
    position = _position("BOND", "Core", "1", 0.02, Decimal(5)).with_quote_legs(
        treasury=_leg("UST 5Y par", 5.0, 0.03, 0.031),
        spread=_leg("USD-OIS settlement Z", 4.0, -0.0015, 0.001),
    )
    benchmark = _position("CASH", "Core", "1", 0.0)
    result = campisi_attribution(portfolio=[position], benchmark=[benchmark], config=_config(["Core"]))
    breakdown = result.portfolio_components
    assert breakdown == pytest.approx({
        "carry": 0.0125,
        "treasury": -0.005,
        "spread": -0.01,
        "residual": 0.0225,
        "total": 0.02,
    })
    assert result.reconciliation_check(tolerance=1e-13).is_reconciled
    assert FiAttributionResult.from_json(result.to_json()).to_json() == result.to_json()


def test_one_sided_report_stores_comparator_without_invented_observation() -> None:
    portfolio = [_position("C1", "Core", "0.75", 0.03), _position("P1", "Portfolio", "0.25", 0.098)]
    benchmark = [_position("C2", "Core", "0.75", 0.014), _position("B1", "Benchmark", "0.25", 0.01)]
    result = campisi_attribution(portfolio, benchmark, _config(["Benchmark", "Core", "Portfolio"]))
    assert result.active_return == pytest.approx(0.034)
    assert [row["sector"] for row in result.sectors] == ["Benchmark", "Core", "Portfolio"]
    bonus = result.sectors[-1]
    assert bonus["benchmark_comparator"] == pytest.approx(0.013)
    assert bonus["benchmark_return"] is None
    assert bonus["allocation"] == 0.0
    missing = result.sectors[0]
    assert missing["portfolio_return"] is None
    assert missing["residual"] == 0.0
    assert list(result.to_pandas().columns) == [
        "sector",
        "portfolio_weight",
        "benchmark_weight",
        "portfolio_return",
        "benchmark_return",
        "benchmark_comparator",
        "allocation",
        "active_carry",
        "active_treasury",
        "active_spread",
        "residual",
        "total_active",
    ]


@pytest.mark.parametrize("invalid", ["0.99999999999999999999999", "1.00000000000000000000001"])
def test_signed_weights_sum_exactly_at_decimal_precision(invalid: str) -> None:
    benchmark = [_position("CORE", "Core", "1", 0.01)]
    with pytest.raises(PortfolioError, match="weights must sum to one"):
        campisi_attribution([_position("BAD", "Core", invalid, 0.02)], benchmark, _config(["Core"]))


def test_exact_net_weight_not_fixed_fraction_of_gross() -> None:
    def report(short: str, rest: str) -> FiAttributionResult:
        portfolio = [
            _position("LONG", "Pair", "0.5", 0.03),
            _position("SHORT", "Pair", short, 0.01),
            _position("CASH", "Core", rest, 0.0),
        ]
        benchmark = [_position("CASH", "Core", "1", 0.0)]
        return campisi_attribution(portfolio, benchmark, _config(["Pair", "Core"]))

    assert report("-0.499999999999", "0.999999999999").sectors[0]["portfolio_weight"] == pytest.approx(1e-12)
    with pytest.raises(PortfolioError, match="exactly zero"):
        report("-0.5", "1")


def test_live_quote_bases_must_match_and_spread_can_cross_zero() -> None:
    bond = _position("BOND", "Core", "1", 0.01)
    benchmark = [_position("CASH", "Core", "1", 0.0)]
    bad = FiQuoteLeg("Z-settlement", 3.0, FiQuote("Z-settlement", 0.0), FiQuote("OAS", 0.001))
    with pytest.raises(PortfolioError, match="quote and sensitivity bases must match"):
        campisi_attribution([bond.with_quote_legs(spread=bad)], benchmark, _config(["Core"]))
    results = [
        campisi_attribution(
            [bond.with_quote_legs(spread=_leg("Z", 3.0, spread, spread + 0.001))], benchmark, _config(["Core"])
        )
        for spread in (-0.002, 0.0, 0.015)
    ]
    assert all(result.portfolio_components["spread"] == pytest.approx(-0.003) for result in results)


def test_actual_calendar_windows_and_typed_carino_link() -> None:
    holding = _position("BOND", "Core", "1", 0.02, Decimal(5))
    benchmark = _position("CASH", "Core", "1", 0.0)
    first = campisi_attribution([holding], [benchmark], _config(["Core"], "2025-01-01", "2025-02-01"))
    second = campisi_attribution([holding], [benchmark], _config(["Core"], "2025-02-01", "2025-03-01"))
    assert first.total_active_carry == pytest.approx(5 / 80 * 31 / 365)
    assert second.total_active_carry == pytest.approx(5 / 80 * 28 / 365)
    result = campisi_carino_link(periods=[first, second])
    assert result.portfolio_return_compounded == pytest.approx(1.02**2 - 1)
    assert result.linked_active_carry + result.linked_residual == pytest.approx(1.02**2 - 1)


@pytest.mark.parametrize("field", ["active_return", "total_active_carry", "total_residual"])
def test_restored_result_reconciles_to_native_source(field: str) -> None:
    bond = _position("BOND", "Core", "1", 0.02, Decimal(5))
    result = campisi_attribution([bond], [bond], _config(["Core"]))
    document = json.loads(result.to_json())
    document[field] += 0.001
    with pytest.raises(ValueError, match="source observations"):
        FiAttributionResult.from_json(json.dumps(document))


@pytest.mark.parametrize("tolerance", [float("nan"), float("inf"), -0.01])
def test_reconciliation_requires_valid_absolute_tolerance(tolerance: float) -> None:
    position = _position("BOND", "Core", "1", 0.02)
    result = campisi_attribution([position], [position], _config(["Core"]))
    with pytest.raises(PortfolioError, match="finite and nonnegative"):
        result.reconciliation_check(tolerance)
