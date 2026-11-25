"""Linear commodity, FX, repo and listed-future golden builders.

QuantLib has no instrument class for these products. Each fixture takes its
discount factors, year fractions and business-day arithmetic from QuantLib and
evaluates the contractual cash flows here, independently of Finstack.
"""

from __future__ import annotations

from collections.abc import Callable
from itertools import pairwise
import math
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA,
    VALUATION_DATE,
    central_difference,
    flat_discount_curve,
    flat_forward_curve,
    market_snapshot,
    metadata,
    ql_date,
    ql_date_to_iso,
)

CAPTURED_ON = "2026-11-24"
# Both sides evaluate the same dated cash flows on the same flat curves; only
# double-precision evaluation order differs.
ARITHMETIC_REL = 1e-9
ATTRIBUTES = {"tags": ["golden", "quantlib"], "meta": {}}


def _year_fraction(date: str, day_count: ql.DayCounter | None = None) -> float:
    return (day_count or ql.Actual365Fixed()).yearFraction(ql_date(VALUATION_DATE), ql_date(date))


def _discount(rate: float, date: str) -> float:
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    return ql.FlatForward(evaluation_date, rate, ql.Actual365Fixed(), ql.Continuous).discount(ql_date(date))


def _price_curve(curve_id: str, knots: list[tuple[float, float]], level_kind: str = "price") -> dict[str, Any]:
    return {
        "type": "price",
        "id": curve_id,
        "base": VALUATION_DATE,
        "day_count": "act_365f",
        "level_kind": level_kind,
        "knot_points": [list(knot) for knot in knots],
        "interp_style": "linear",
        "extrapolation": "flat",
    }


def _curve_price(knots: list[tuple[float, float]], date: str) -> float:
    """Linear interpolation of a price curve on Act/365F time from the valuation date."""
    time = _year_fraction(date)
    interpolation = ql.LinearInterpolation([t for t, _ in knots], [p for _, p in knots])
    return interpolation(time, True)


def _fixture(
    *,
    name: str,
    domain: str,
    description: str,
    product: str,
    source_detail: str,
    market: dict[str, Any],
    instrument_type: str,
    spec: dict[str, Any],
    expected: dict[str, float],
    reason: str,
) -> dict[str, Any]:
    meta = metadata(name=name, domain=domain, description=description, product=product, source_detail=source_detail)
    return {
        "schema": SCHEMA,
        "metadata": {**meta, "captured_on": CAPTURED_ON, "last_reviewed_on": CAPTURED_ON},
        "kind": "pricing",
        "model": "discounting",
        "market": market,
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {"type": instrument_type, "spec": {**spec, "attributes": ATTRIBUTES}},
        },
        "expected": expected,
        "tolerances": {metric: {"rel": ARITHMETIC_REL, "tolerance_reason": reason} for metric in expected},
    }


def _listed_terms(
    *, contracts: float, multiplier: float, currency: str, entry_price: float, last_trading_date: str, settlement: str
) -> dict[str, Any]:
    return {
        "contracts": contracts,
        "multiplier": multiplier,
        "currency": currency,
        "entry_price": entry_price,
        "last_trading_date": last_trading_date,
        "settlement_date": settlement,
        "position": "long",
        "settlement": {"type": "cash"},
    }


def build_commodity_forward() -> dict[str, Any]:
    """Build a six-month cash-settled WTI forward fixture."""
    knots = [(0.0, 78.0), (0.25, 79.5), (1.0, 82.0), (5.0, 85.0)]
    contract_price, quantity, rate = 79.0, 100_000.0, 0.04
    maturity = "2026-10-30"
    payment = ql_date_to_iso(
        ql.UnitedStates(ql.UnitedStates.FederalReserve).advance(ql_date(maturity), 2, ql.Days, ql.Following)
    )
    forward = _curve_price(knots, maturity)

    def npv(rate_shift: float = 0.0) -> float:
        return (forward - contract_price) * quantity * _discount(rate + rate_shift, payment)

    expected = {
        "delta": quantity * _discount(rate, payment),
        "dv01": central_difference(npv, 0.0),
        "npv": npv(),
    }
    return _fixture(
        name="wti_forward_6m_quantlib",
        domain="commodity.commodity_forward",
        description="Six-month cash-settled WTI forward off a linear price curve, paid two business days after maturity.",
        product="commodity_forward",
        source_detail=(
            f"QuantLib {ql.__version__}: linear interpolation of the price curve at maturity {maturity} "
            f"(F = {forward!r}), payment {payment} two Federal Reserve business days later, FlatForward "
            f"discount factor (continuous Act/365F, {rate}); NPV = (F - {contract_price}) * {quantity:.0f} * DF."
        ),
        market=market_snapshot([flat_discount_curve("USD-OIS", rate), _price_curve("WTI-FORWARD", knots)]),
        instrument_type="commodity_forward",
        spec={
            "id": "WTI-FORWARD-6M-QUANTLIB",
            "commodity_type": "Energy",
            "underlying_ticker": "CL",
            "unit": "BBL",
            "currency": "USD",
            "quantity": quantity,
            "multiplier": 1.0,
            "maturity": maturity,
            "settlement": "cash",
            "position": "long",
            "contract_price": contract_price,
            "forward_curve_id": "WTI-FORWARD",
            "discount_curve_id": "USD-OIS",
            "settlement_days": 2,
            "settlement_calendar_id": "usny",
        },
        expected=expected,
        reason=(
            "One cash flow: the forward read off the same linear price curve, discounted on the same flat curve "
            "to a payment date both sides derive from the US calendar; only floating-point evaluation order differs."
        ),
    )


def build_commodity_future() -> dict[str, Any]:
    """Build a commodity future fixture settling on the average of three fixings."""
    knots = [(0.0, 104.0), (0.5, 107.0), (1.0, 108.5), (5.0, 110.0)]
    contracts, multiplier, entry_price = 10.0, 100.0, 105.25
    fixing_dates = ["2026-12-29", "2026-12-30", "2026-12-31"]
    fair_price = sum(_curve_price(knots, date) for date in fixing_dates) / len(fixing_dates)
    expected = {
        "delta": contracts * multiplier,
        "futures_price": fair_price,
        "npv": contracts * multiplier * (fair_price - entry_price),
    }
    return _fixture(
        name="iron_ore_average_future_quantlib",
        domain="listed.commodity_future",
        description="Long commodity future settling on the arithmetic average of three daily fixings.",
        product="commodity_future",
        source_detail=(
            f"QuantLib {ql.__version__}: linear interpolation of the price curve on {', '.join(fixing_dates)}, "
            f"averaged to {fair_price!r}; undiscounted variation margin "
            f"{contracts:.0f} * {multiplier:.0f} * (fair - {entry_price})."
        ),
        market=market_snapshot([_price_curve("IRON-ORE-FORWARD", knots)]),
        instrument_type="commodity_future",
        spec={
            "id": "IRON-ORE-AVERAGE-FUTURE-QUANTLIB",
            "underlying_ticker": "TSI-62-FE",
            "terms": _listed_terms(
                contracts=contracts,
                multiplier=multiplier,
                currency="USD",
                entry_price=entry_price,
                last_trading_date="2026-12-31",
                settlement="2027-01-04",
            ),
            "forward_curve_id": "IRON-ORE-FORWARD",
            "fixing": {"type": "arithmetic_average", "fixing_dates": fixing_dates, "past_fixings": []},
        },
        expected=expected,
        reason=(
            "The mean of three prices read off the same linear curve, times contracts and multiplier, with no "
            "discounting on either side; only floating-point evaluation order differs."
        ),
    )


def _weekdays(start: ql.Date, end: ql.Date, *, include_end: bool) -> list[ql.Date]:
    calendar = ql.WeekendsOnly()
    last = end if include_end else end - 1
    return [start + offset for offset in range(last - start + 1) if calendar.isBusinessDay(start + offset)]


def build_commodity_swap() -> dict[str, Any]:
    """Build a three-month monthly-averaged natural gas swap fixture."""
    knots = [(0.0, 3.40), (0.25, 3.55), (0.5, 3.80), (5.0, 4.20)]
    quantity, fixed_price, rate = 10_000.0, 3.50, 0.04
    boundaries = ["2026-06-01", "2026-07-01", "2026-08-01", "2026-09-01"]

    averages: list[float] = []
    for index, (start, end) in enumerate(pairwise(boundaries)):
        final = index == len(boundaries) - 2
        days = _weekdays(ql_date(start), ql_date(end), include_end=final)
        averages.append(sum(_curve_price(knots, ql_date_to_iso(day)) for day in days) / len(days))

    def npv(rate_shift: float = 0.0) -> float:
        return sum(
            quantity * (average - fixed_price) * _discount(rate + rate_shift, end)
            for average, end in zip(averages, boundaries[1:], strict=True)
        )

    expected = {"dv01": central_difference(npv, 0.0), "npv": npv()}
    return _fixture(
        name="natgas_swap_3m_quantlib",
        domain="commodity.commodity_swap",
        description="Pay-fixed natural gas swap with three monthly periods, each averaging weekday prices.",
        product="commodity_swap",
        source_detail=(
            f"QuantLib {ql.__version__}: periods {' / '.join(boundaries)}; each floating price is the mean of the "
            "linear price curve over the weekdays from the period start up to, but not including, the period end "
            "(the last period includes its end date); each net payment of "
            f"{quantity:.0f} * (average - {fixed_price}) is discounted on a FlatForward curve "
            f"(continuous Act/365F, {rate}) from the period end. Averages: {averages!r}."
        ),
        market=market_snapshot([flat_discount_curve("USD-OIS", rate), _price_curve("NG-FORWARD", knots)]),
        instrument_type="commodity_swap",
        spec={
            "id": "NATGAS-SWAP-3M-QUANTLIB",
            "commodity_type": "Energy",
            "underlying_ticker": "NG",
            "unit": "MMBTU",
            "currency": "USD",
            "quantity": quantity,
            "fixed_price": fixed_price,
            "forward_curve_id": "NG-FORWARD",
            "side": "pay",
            "start_date": boundaries[0],
            "maturity": boundaries[-1],
            "frequency": {"count": 1, "unit": "months"},
            "business_day_convention": "modified_following",
            "discount_curve_id": "USD-OIS",
            "past_fixings": [],
        },
        expected=expected,
        reason=(
            "Three net payments built from weekday averages of the same linear price curve and discounted on the "
            "same flat curve; only floating-point evaluation order differs."
        ),
    )


def _fx_matrix(spot: float) -> dict[str, Any]:
    return {
        "config": {"pivot_currency": "USD", "cache_capacity": 256},
        "quotes": [["EUR", "USD", spot]],
        "pinned_quotes": [],
        "provider_quotes": [],
        "provider_pinned_quotes": [],
    }


def build_fx_spot() -> dict[str, Any]:
    """Build a EUR/USD spot fixture settling two joint business days ahead."""
    spot, notional, rate = 1.10, 1_000_000.0, 0.04
    calendar = ql.JointCalendar(ql.TARGET(), ql.UnitedStates(ql.UnitedStates.FederalReserve))
    settlement = ql_date_to_iso(calendar.advance(ql_date(VALUATION_DATE), 2, ql.Days, ql.Following))
    # `spot_rate` and `inverse_rate` are left out: Finstack derives them from the
    # discounted value, so with a discount curve they are not the market spot.
    expected = {"npv": notional * spot * _discount(rate, settlement)}
    return _fixture(
        name="eurusd_spot_quantlib",
        domain="fx.fx_spot",
        description="EUR/USD spot position valued as its USD proceeds discounted from the spot settlement date.",
        product="fx_spot",
        source_detail=(
            f"QuantLib {ql.__version__}: settlement {settlement}, two business days after {VALUATION_DATE} on the "
            f"joint TARGET and US Federal Reserve calendar; NPV = {notional:.0f} * {spot} * FlatForward discount "
            f"factor (continuous Act/365F, {rate})."
        ),
        market=market_snapshot([flat_discount_curve("USD-OIS", rate)], fx=_fx_matrix(spot)),
        instrument_type="fx_spot",
        spec={
            "id": "EURUSD-SPOT-QUANTLIB",
            "base_currency": "EUR",
            "quote_currency": "USD",
            "settlement_days": 2,
            "notional": {"amount": f"{notional:.0f}", "currency": "EUR"},
            "base_calendar_id": "target2",
            "quote_calendar_id": "usny",
            "domestic_discount_curve_id": "USD-OIS",
        },
        expected=expected,
        reason=(
            "One quote-currency cash flow at the market spot rate, discounted on the same flat curve to a "
            "settlement date both sides derive from the joint calendar; only floating-point evaluation order differs."
        ),
    )


def build_fx_future() -> dict[str, Any]:
    """Build a EUR/USD currency future fixture at its covered-interest-parity price."""
    spot, entry_price, domestic_rate, foreign_rate = 1.10, 1.1050, 0.04, 0.025
    contracts, multiplier = 4.0, 125_000.0
    settlement = "2026-12-16"

    def fair_price(domestic_shift: float = 0.0, foreign_shift: float = 0.0) -> float:
        return (
            spot
            * _discount(foreign_rate + foreign_shift, settlement)
            / _discount(domestic_rate + domestic_shift, settlement)
        )

    def npv(shift: float = 0.0) -> float:
        return contracts * multiplier * (fair_price(shift, shift) - entry_price)

    expected = {
        "delta": contracts * multiplier,
        "dv01": central_difference(npv, 0.0),
        "futures_price": fair_price(),
        "npv": npv(),
    }
    return _fixture(
        name="eurusd_future_dec26_quantlib",
        domain="listed.fx_future",
        description="Long EUR/USD currency future valued at its covered-interest-parity fair price.",
        product="fx_future",
        source_detail=(
            f"QuantLib {ql.__version__}: F = {spot} * DF_EUR / DF_USD to {settlement} on FlatForward curves "
            f"(continuous Act/365F, EUR {foreign_rate}, USD {domestic_rate}); undiscounted variation margin "
            f"{contracts:.0f} * {multiplier:.0f} * (F - {entry_price}); DV01 shifts both curves together."
        ),
        market=market_snapshot(
            [flat_discount_curve("USD-OIS", domestic_rate), flat_discount_curve("EUR-OIS", foreign_rate)],
            fx=_fx_matrix(spot),
        ),
        instrument_type="fx_future",
        spec={
            "id": "EURUSD-FUTURE-DEC26-QUANTLIB",
            "base_currency": "EUR",
            "terms": _listed_terms(
                contracts=contracts,
                multiplier=multiplier,
                currency="USD",
                entry_price=entry_price,
                last_trading_date="2026-12-14",
                settlement=settlement,
            ),
            "domestic_discount_curve_id": "USD-OIS",
            "foreign_discount_curve_id": "EUR-OIS",
        },
        expected=expected,
        reason=(
            "Covered-interest-parity forward from the same spot and flat curves, times contracts and multiplier, "
            "undiscounted on both sides; only floating-point evaluation order differs."
        ),
    )


def build_repo() -> dict[str, Any]:
    """Build a seasoned four-week term repo fixture."""
    cash, repo_rate, rate = 10_000_000.0, 0.0525, 0.04
    start, maturity = "2026-04-23", "2026-05-21"
    interest = cash * repo_rate * ql.Actual360().yearFraction(ql_date(start), ql_date(maturity))

    def npv(rate_shift: float = 0.0) -> float:
        return (cash + interest) * _discount(rate + rate_shift, maturity)

    expected = {
        "dv01": central_difference(npv, 0.0),
        "effective_rate": repo_rate,
        "npv": npv(),
        "repo_interest": interest,
    }
    return _fixture(
        name="usd_term_repo_4w_seasoned_quantlib",
        domain="rates.repo",
        description="Four-week term repo one week after its start, valued from the cash lender's side.",
        product="repo",
        source_detail=(
            f"QuantLib {ql.__version__}: the cash was lent on {start}, before the valuation date, so only the "
            f"repayment {cash:.0f} * (1 + {repo_rate} * Actual/360 from {start} to {maturity}) remains; it is "
            f"discounted on a FlatForward curve (continuous Act/365F, {rate})."
        ),
        market=market_snapshot(
            [flat_discount_curve("USD-OIS", rate)],
            prices={"UST_10Y_PRICE": {"price": {"amount": "1020.0", "currency": "USD"}}},
        ),
        instrument_type="repo",
        spec={
            "id": "USD-TERM-REPO-4W-QUANTLIB",
            "cash_amount": {"amount": f"{cash:.0f}", "currency": "USD"},
            "collateral": {
                "collateral_type": "general",
                "instrument_id": "UST-10Y",
                "quantity": 10_000.0,
                "market_value_id": "UST_10Y_PRICE",
            },
            "repo_rate": str(repo_rate),
            "start_date": start,
            "maturity": maturity,
            "haircut": 0.02,
            "repo_type": "term",
            "triparty": False,
            "day_count": "act_360",
            "business_day_convention": "following",
            "calendar_id": "usny",
            "discount_curve_id": "USD-OIS",
        },
        expected=expected,
        reason=(
            "One repayment with simple Actual/360 interest, discounted on the same flat curve; only "
            "floating-point evaluation order differs."
        ),
    )


def build_equity_total_return_future() -> dict[str, Any]:
    """Build an equity total-return future fixture."""
    spot, distributions, funding, spread_bp = 5300.0, 62.5, 41.25, 35.0
    contracts, multiplier, entry_price = 100.0, 10.0, 5250.0
    settlement = "2027-12-17"
    spread_time = _year_fraction(settlement, ql.Actual360())
    fair_price = spot + distributions - funding + spot * spread_bp * 1e-4 * spread_time
    expected = {
        # The spread leg scales with spot, so the fair price moves by more than one per unit of spot.
        "delta": contracts * multiplier * (1.0 + spread_bp * 1e-4 * spread_time),
        "futures_price": fair_price,
        "npv": contracts * multiplier * (fair_price - entry_price),
    }

    def scalar(amount: float) -> dict[str, Any]:
        return {"price": {"amount": str(amount), "currency": "EUR"}}

    return _fixture(
        name="sx5e_total_return_future_dec27_quantlib",
        domain="listed.equity_total_return_future",
        description="Long equity index total-return future priced from spot, accrued legs and a traded spread.",
        product="equity_total_return_future",
        source_detail=(
            f"QuantLib {ql.__version__}: fair price = spot {spot} + accrued distributions {distributions} "
            f"- accrued funding {funding} + spot * {spread_bp} bp * Actual/360 from {VALUATION_DATE} to "
            f"{settlement}; undiscounted variation margin {contracts:.0f} * {multiplier:.0f} * "
            f"(fair - {entry_price})."
        ),
        market=market_snapshot(
            [],
            prices={
                "SX5E-CLOSE": scalar(spot),
                "TESX-ACCRUED-DISTRIBUTIONS": scalar(distributions),
                "TESX-ACCRUED-FUNDING": scalar(funding),
                "TESX-SPREAD-BPS": {"unitless": spread_bp},
            },
        ),
        instrument_type="equity_total_return_future",
        spec={
            "id": "SX5E-TOTAL-RETURN-FUTURE-DEC27-QUANTLIB",
            "underlying_ticker": "SX5E",
            "terms": _listed_terms(
                contracts=contracts,
                multiplier=multiplier,
                currency="EUR",
                entry_price=entry_price,
                last_trading_date="2027-12-16",
                settlement=settlement,
            ),
            "spot_id": "SX5E-CLOSE",
            "accrued_distributions_id": "TESX-ACCRUED-DISTRIBUTIONS",
            "accrued_funding_id": "TESX-ACCRUED-FUNDING",
            "spread_bp_id": "TESX-SPREAD-BPS",
            "spread_day_count": "act_360",
        },
        expected=expected,
        reason=(
            "Four market scalars combined by the contract's total-return formula with a QuantLib Actual/360 "
            "year fraction; only floating-point evaluation order differs."
        ),
    )


def build_volatility_index_future() -> dict[str, Any]:
    """Build a volatility index future fixture off a linear index curve."""
    knots = [(0.0, 18.0), (0.25, 20.5), (0.5, 21.5), (2.0, 23.0)]
    contracts, multiplier, entry_price = 25.0, 1000.0, 20.75
    settlement = "2026-09-16"
    mark = _curve_price(knots, settlement)
    expected = {"npv": contracts * multiplier * (mark - entry_price)}
    return _fixture(
        name="vix_future_sep26_quantlib",
        domain="listed.volatility_index_future",
        description="Long volatility index future marked off a linear index futures curve.",
        product="volatility_index_future",
        source_detail=(
            f"QuantLib {ql.__version__}: linear interpolation of the index curve at settlement {settlement} "
            f"(mark = {mark!r}); undiscounted variation margin {contracts:.0f} * {multiplier:.0f} * "
            f"(mark - {entry_price})."
        ),
        market=market_snapshot([_price_curve("VIX", knots, level_kind="vol_index")]),
        instrument_type="volatility_index_future",
        spec={
            "id": "VIX-FUTURE-SEP26-QUANTLIB",
            "terms": _listed_terms(
                contracts=contracts,
                multiplier=multiplier,
                currency="USD",
                entry_price=entry_price,
                last_trading_date=settlement,
                settlement=settlement,
            ),
            "vol_index_curve_id": "VIX",
        },
        expected=expected,
        reason=(
            "One level read off the same linear curve, times contracts and multiplier, undiscounted on both "
            "sides; only floating-point evaluation order differs."
        ),
    )


def build_equity_total_return_swap() -> dict[str, Any]:
    """Build a spot-starting equity total return swap fixture.

    Each quarter the receiver gets the index's total return and pays a simple
    floating rate plus a spread. With a continuous dividend yield the price
    return over a future period is the forward ratio less one, and the
    dividends paid in the period, net of withholding tax, are what the yield
    takes out of the forward.
    """
    notional, spot, dividend_yield, tax_rate = 5_000_000.0, 5300.0, 0.015, 0.15
    discount_rate, projection_rate, spread_bp = 0.04, 0.043, 75.0
    dates = ["2026-04-30", "2026-07-30", "2026-10-30"]
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    projection = ql.FlatForward(evaluation_date, projection_rate, ql.Actual365Fixed(), ql.Continuous)
    total_return_leg = 0.0
    financing_leg = 0.0
    for start, end in pairwise(dates):
        growth = _discount(discount_rate, start) / _discount(discount_rate, end)
        carry = math.exp(-dividend_yield * (_year_fraction(end) - _year_fraction(start)))
        period_return = growth * carry - 1.0 + growth * (1.0 - carry) * (1.0 - tax_rate)
        total_return_leg += notional * period_return * _discount(discount_rate, end)
        accrual = ql.Actual360().yearFraction(ql_date(start), ql_date(end))
        compound = projection.discount(ql_date(start)) / projection.discount(ql_date(end))
        financing_leg += notional * (compound - 1.0 + spread_bp * 1e-4 * accrual) * _discount(discount_rate, end)
    return _fixture(
        name="spx_total_return_swap_6m_quantlib",
        domain="equity.equity_total_return_swap",
        description="Spot-starting six-month quarterly equity total return swap, receiving the total return.",
        product="equity_total_return_swap",
        source_detail=(
            f"QuantLib {ql.__version__} flat Act/365F continuous discount ({discount_rate}) and projection "
            f"({projection_rate}) curves. Per period: total return = forward ratio less one plus the dividends "
            f"the {dividend_yield} yield removes from the forward, net of {tax_rate} withholding tax; financing = "
            f"simple projection-curve forward plus {spread_bp} bp on Actual/360. Both paid at period end; value "
            "is the total-return leg less the financing leg."
        ),
        market=market_snapshot(
            [flat_discount_curve("USD-OIS", discount_rate), flat_forward_curve("USD-SOFR-3M", projection_rate)],
            prices={
                "SPX-DIVYIELD": {"unitless": dividend_yield},
                "SPX-SPOT": {"price": {"amount": str(spot), "currency": "USD"}},
            },
        ),
        instrument_type="trs_equity",
        spec={
            "id": "SPX-TOTAL-RETURN-SWAP-6M-QUANTLIB",
            "notional": {"amount": str(int(notional)), "currency": "USD"},
            "underlying": {
                "ticker": "SPX",
                "spot_id": "SPX-SPOT",
                "div_yield_id": "SPX-DIVYIELD",
                "currency": "USD",
            },
            "financing_leg": {
                "discount_curve_id": "USD-OIS",
                "forward_curve_id": "USD-SOFR-3M",
                "spread_bp": str(int(spread_bp)),
                "day_count": "act_360",
                "compounding": "simple",
            },
            "schedule": {
                "start": dates[0],
                "end": dates[-1],
                "params": {
                    "frequency": {"count": 3, "unit": "months"},
                    "day_count": "act_365f",
                    "business_day_convention": "following",
                    "calendar_id": "weekends_only",
                    "stub": "none",
                    "end_of_month": False,
                    "payment_lag_days": 0,
                },
            },
            "side": "receive",
            "initial_level": None,
            "dividend_tax_rate": tax_rate,
            "dividend_settlement": "on_dividend_date",
        },
        expected={"npv": total_return_leg - financing_leg},
        reason=(
            "Both sides evaluate the same carry and simple-forward formulas on the same flat curves; only "
            "floating-point evaluation order differs."
        ),
    )


LINEAR_BUILDERS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "equity_total_return_swap": (
        "equity_total_return_swap/spx_total_return_swap_6m_quantlib.json",
        build_equity_total_return_swap,
    ),
    "commodity_forward": ("commodity_forward/wti_forward_6m_quantlib.json", build_commodity_forward),
    "commodity_future": ("commodity_future/iron_ore_average_future_quantlib.json", build_commodity_future),
    "commodity_swap": ("commodity_swap/natgas_swap_3m_quantlib.json", build_commodity_swap),
    "fx_spot": ("fx_spot/eurusd_spot_quantlib.json", build_fx_spot),
    "fx_future": ("fx_future/eurusd_future_dec26_quantlib.json", build_fx_future),
    "repo": ("repo/usd_term_repo_4w_seasoned_quantlib.json", build_repo),
    "equity_total_return_future": (
        "equity_total_return_future/sx5e_total_return_future_dec27_quantlib.json",
        build_equity_total_return_future,
    ),
    "volatility_index_future": (
        "volatility_index_future/vix_future_sep26_quantlib.json",
        build_volatility_index_future,
    ),
}
