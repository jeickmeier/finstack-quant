"""Inflation golden builders on QuantLib's zero-inflation machinery.

Every fixture shares one deterministic market: a flat continuously compounded
nominal curve and a flat annually compounded zero-inflation curve whose base is
the last published monthly fixing. QuantLib's ``ZeroInflationIndex`` supplies
every monthly CPI level and ``CPI.laggedFixing`` every contractual reference
index (three-month lag, linear daily interpolation weighted by the contractual
month).

The zero-coupon swap is priced by QuantLib's native ``ZeroCouponInflationSwap``.
QuantLib's year-on-year instruments forecast from a separate year-on-year
curve rather than the ratio of zero-index fixings, so the year-on-year swap and
cap/floor fixtures take
the reference indices, discount factors and the Black/Bachelier formula from
QuantLib and assemble the contractual cash flows here, independently of
Finstack.
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
    constant_vol_surface,
    flat_discount_curve,
    metadata,
    ql_date,
    ql_date_to_iso,
)

CAPTURED_ON = "2026-11-24"
# Both sides discount the same dated cash flows off the same CPI levels and the
# same flat curve; only double-precision evaluation order differs.
ARITHMETIC_REL = 1e-9
# Both sides evaluate the same Black-76 / Bachelier closed form on identical
# forward, strike, variance and discount inputs.
CLOSED_FORM_REL = 5e-9

INDEX_ID = "US-CPI"
DISCOUNT_ID = "USD-OIS"
NOTIONAL = 10_000_000.0
DISCOUNT_RATE = 0.04
INFLATION_RATE = 0.025
RATE_BUMP = 1e-4
VOL_POINT = 0.01
LAG = ql.Period(3, ql.Months)
# Published monthly fixings on the valuation date; the last one is the base of
# the zero-inflation curve.
OBSERVATIONS = (
    ("2026-01-01", 299.0, "2026-02-13"),
    ("2026-02-01", 299.6, "2026-03-13"),
    ("2026-03-01", 300.0, "2026-04-14"),
)
CURVE_BASE = "2026-03-01"
CURVE_END = "2031-06-01"

YOY_START = "2026-05-18"
YOY_MATURITY = "2029-05-18"


def _captured(meta: dict[str, Any]) -> dict[str, Any]:
    return {**meta, "captured_on": CAPTURED_ON, "last_reviewed_on": CAPTURED_ON}


def _tolerances(expected: dict[str, float], rel: float, reason: str) -> dict[str, Any]:
    return {metric: {"rel": rel, "tolerance_reason": reason} for metric in expected}


def _option_tolerances(expected: dict[str, float], reason: str) -> dict[str, Any]:
    """Closed-form bound on the premium, propagated to the bump-and-reprice metrics.

    The premium agrees within the libraries' normal-CDF bound. Each sensitivity
    is half the difference of two such premiums, so its absolute error is
    bounded by that relative bound times the premium, whatever the size of the
    sensitivity itself.
    """
    bump_bound = CLOSED_FORM_REL * abs(expected["npv"])
    return {
        metric: (
            {"rel": CLOSED_FORM_REL, "tolerance_reason": reason}
            if metric == "npv"
            else {"abs": bump_bound, "tolerance_reason": reason}
        )
        for metric in expected
    }


def _evaluation_date() -> ql.Date:
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    return evaluation_date


def _discount_curve(shift: float = 0.0) -> ql.YieldTermStructureHandle:
    return ql.YieldTermStructureHandle(
        ql.FlatForward(_evaluation_date(), DISCOUNT_RATE + shift, ql.Actual365Fixed(), ql.Continuous)
    )


def _cpi_index(shift: float = 0.0) -> ql.ZeroInflationIndex:
    """Return the monthly CPI index projected off a flat zero-inflation curve.

    ``shift`` moves the annually compounded zero-inflation rate; published
    fixings, including the curve's base fixing, do not move.
    """
    evaluation_date = _evaluation_date()
    ql.IndexManager.instance().clearHistories()
    handle = ql.RelinkableZeroInflationTermStructureHandle()
    index = ql.ZeroInflationIndex(
        "CPI", ql.CustomRegion("USA", "US"), False, ql.Monthly, ql.Period(1, ql.Months), ql.USDCurrency(), handle
    )
    for reference_month, level, _ in OBSERVATIONS:
        index.addFixing(ql_date(reference_month), level)
    rate = INFLATION_RATE + shift
    curve = ql.ZeroInflationCurve(
        evaluation_date,
        [ql_date(CURVE_BASE), ql_date(CURVE_BASE) + ql.Period(40, ql.Years)],
        [rate, rate],
        ql.Monthly,
        ql.Actual365Fixed(),
    )
    handle.linkTo(curve)
    return index


def _reference_cpi(index: ql.ZeroInflationIndex, date: ql.Date) -> float:
    return ql.CPI.laggedFixing(index, date, LAG, ql.CPI.Linear)


def _anchor_months(dates: list[ql.Date]) -> list[ql.Date]:
    """Return the monthly fixings each lagged, interpolated reference index reads."""
    months = set()
    for date in dates:
        first = ql.Date(1, date.month(), date.year()) - LAG
        months.update({first, first + ql.Period(1, ql.Months)})
    return sorted(months)


def _market(reference_dates: list[ql.Date], surfaces: list[dict[str, Any]] | None = None) -> dict[str, Any]:
    """Build the Finstack twin of the QuantLib market.

    The inflation curve carries QuantLib's projected monthly fixings as knots,
    so every monthly level Finstack reads is a QuantLib index fixing.
    """
    index = _cpi_index()
    base = ql_date(CURVE_BASE)
    day_count = ql.Actual365Fixed()
    knots = []
    month = base
    while month <= ql_date(CURVE_END):
        knots.append([day_count.yearFraction(base, month), index.fixing(month)])
        month = month + ql.Period(1, ql.Months)
    published = {reference_month for reference_month, _, _ in OBSERVATIONS}
    releases = [
        {
            "reference_month": ql_date_to_iso(month),
            "expected_release": ql_date_to_iso(month + ql.Period(1, ql.Months) + 14),
            "known_on": VALUATION_DATE,
        }
        for month in _anchor_months(reference_dates)
        if ql_date_to_iso(month) not in published
    ]
    return {
        "kind": "snapshot",
        "data": {
            "schema_version": 1,
            "curves": [
                flat_discount_curve(DISCOUNT_ID, DISCOUNT_RATE),
                {
                    "type": "inflation",
                    "id": INDEX_ID,
                    "base_date": CURVE_BASE,
                    "day_count": "act_365f",
                    "knot_points": knots,
                    "interp_style": "log_linear",
                    "extrapolation": "flat",
                },
            ],
            "fx": None,
            "surfaces": surfaces or [],
            "prices": {},
            "series": [],
            "inflation_indices": [
                {
                    "id": INDEX_ID,
                    "currency": "USD",
                    "knowledge_date": VALUATION_DATE,
                    "observations": [
                        {"reference_month": reference_month, "value": level, "available_on": available_on}
                        for reference_month, level, available_on in OBSERVATIONS
                    ],
                    "expected_releases": releases,
                }
            ],
            "dividends": [],
            "credit_indices": [],
            "fx_delta_vol_surfaces": [],
            "vol_cubes": [],
            "hierarchy": None,
        },
    }


def build_zero_coupon_inflation_swap() -> dict[str, Any]:
    """Build a native QuantLib five-year pay-fixed zero-coupon inflation swap fixture."""
    start, maturity = "2026-04-30", "2031-04-30"
    fixed_rate = 0.0225

    def swap(rate_shift: float = 0.0, inflation_shift: float = 0.0) -> ql.ZeroCouponInflationSwap:
        # QuantLib's `Receiver` receives the inflation leg and pays the fixed
        # leg, which is Finstack's pay-fixed side; the leg signs are asserted below.
        instrument = ql.ZeroCouponInflationSwap(
            ql.Swap.Receiver,
            NOTIONAL,
            ql_date(start),
            ql_date(maturity),
            ql.WeekendsOnly(),
            ql.ModifiedFollowing,
            ql.Thirty360(ql.Thirty360.BondBasis),
            fixed_rate,
            _cpi_index(inflation_shift),
            LAG,
            ql.CPI.Linear,
        )
        instrument.setPricingEngine(ql.DiscountingSwapEngine(_discount_curve(rate_shift)))
        return instrument

    base = swap()
    fixed_leg_pv, inflation_leg_pv = base.fixedLegNPV(), base.inflationLegNPV()
    if not (fixed_leg_pv < 0.0 < inflation_leg_pv):
        raise RuntimeError("QuantLib swap type no longer maps to pay-fixed, receive-inflation")
    expected = {
        "dv01": central_difference(lambda shift: swap(rate_shift=shift).NPV(), 0.0, RATE_BUMP),
        "inflation01": central_difference(lambda shift: swap(inflation_shift=shift).NPV(), 0.0, RATE_BUMP),
        # Finstack reports each leg as the unsigned present value of its payment;
        # QuantLib signs the paid fixed leg negative (asserted above).
        "fixed_leg_pv": -fixed_leg_pv,
        "inflation_leg_pv": inflation_leg_pv,
        "npv": base.NPV(),
        "par_rate": base.fairRate(),
    }
    reason = (
        "QuantLib native ZeroCouponInflationSwap and Finstack discount the same two bullet cash flows off "
        "identical monthly CPI levels; tolerance allows only cross-engine floating-point residual."
    )
    name = "usd_zc_inflation_swap_5y_quantlib"
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name=name,
                domain="rates.inflation_swap",
                description=(
                    "QuantLib native 5Y pay-fixed zero-coupon inflation swap on flat nominal and zero-inflation curves."
                ),
                product="zero_coupon_inflation_swap",
                source_detail=(
                    f"QuantLib {ql.__version__} ZeroCouponInflationSwap with DiscountingSwapEngine on a flat "
                    "Act/365F continuous nominal curve and a flat annually compounded Act/365F ZeroInflationCurve "
                    "based on the March 2026 fixing. Three-month lag and CPI.Linear interpolation; the start "
                    "reference index comes from the published January and February 2026 fixings. The Finstack "
                    "inflation curve knots are QuantLib's projected monthly index fixings. DV01 and Inflation01 "
                    "are central one-basis-point differences of the QuantLib NPV in the nominal zero rate and the "
                    "zero-inflation rate."
                ),
            )
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": _market([ql_date(start), ql_date(maturity)]),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "inflation_swap",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "start_date": start,
                    "maturity": maturity,
                    "fixed_rate": str(fixed_rate),
                    "inflation_index_id": INDEX_ID,
                    "discount_curve_id": DISCOUNT_ID,
                    "day_count": "one_one",
                    "side": "pay",
                    "lag": {"months": 3},
                    "interpolation": "linear",
                    "base_cpi": None,
                    "business_day_convention": "modified_following",
                    "calendar_id": "weekends_only",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": _tolerances(expected, ARITHMETIC_REL, reason),
    }


def _yoy_periods() -> list[tuple[ql.Date, ql.Date]]:
    """Return the three unadjusted annual periods; every payment date is a weekday."""
    start = ql_date(YOY_START)
    dates = [start + ql.Period(years, ql.Years) for years in range(4)]
    if dates[-1] != ql_date(YOY_MATURITY) or any(date.weekday() in (ql.Saturday, ql.Sunday) for date in dates):
        raise RuntimeError("year-on-year schedule must end on maturity and avoid weekends")
    return list(pairwise(dates))


def _yoy_period_terms(rate_shift: float, inflation_shift: float) -> list[tuple[float, float, float, float]]:
    """Return ``(accrual, year_on_year_rate, discount_factor, option_time)`` per period.

    The reference indices and discount factors are QuantLib's. The option time
    is Act/365F from the valuation date to the lagged end reference date.
    """
    index = _cpi_index(inflation_shift)
    discount = _discount_curve(rate_shift)
    evaluation_date = _evaluation_date()
    accrual_day_count = ql.Thirty360(ql.Thirty360.BondBasis)
    terms = []
    for start, end in _yoy_periods():
        accrual = accrual_day_count.yearFraction(start, end)
        ratio = _reference_cpi(index, end) / _reference_cpi(index, start)
        option_time = ql.Actual365Fixed().yearFraction(evaluation_date, end - LAG)
        terms.append((accrual, (ratio - 1.0) / accrual, discount.discount(end), option_time))
    return terms


def _yoy_spec(name: str) -> dict[str, Any]:
    return {
        "id": name.upper().replace("_", "-"),
        "notional": {"amount": "10000000", "currency": "USD"},
        "start_date": YOY_START,
        "maturity": YOY_MATURITY,
        "frequency": {"count": 1, "unit": "years"},
        "inflation_index_id": INDEX_ID,
        "discount_curve_id": DISCOUNT_ID,
        "day_count": "30_360",
        "lag": {"months": 3},
        "interpolation": "linear",
        "business_day_convention": "modified_following",
        "calendar_id": "weekends_only",
        "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
    }


def _yoy_reference_dates() -> list[ql.Date]:
    return sorted({date for period in _yoy_periods() for date in period})


def build_yoy_inflation_swap() -> dict[str, Any]:
    """Build a three-year pay-fixed year-on-year inflation swap fixture."""
    fixed_rate = 0.0225

    def npv(rate_shift: float = 0.0, inflation_shift: float = 0.0) -> float:
        # Pay fixed: each period receives N * (I_end / I_start - 1) and pays N * K * accrual.
        return sum(
            NOTIONAL * accrual * (yoy_rate - fixed_rate) * discount_factor
            for accrual, yoy_rate, discount_factor, _ in _yoy_period_terms(rate_shift, inflation_shift)
        )

    expected = {
        "dv01": central_difference(lambda shift: npv(rate_shift=shift), 0.0, RATE_BUMP),
        "inflation01": central_difference(lambda shift: npv(inflation_shift=shift), 0.0, RATE_BUMP),
        "npv": npv(),
    }
    reason = (
        "Both sides discount the same three annual net cash flows off identical QuantLib monthly CPI levels "
        "and the same flat curve; only double-precision evaluation order differs."
    )
    name = "usd_yoy_inflation_swap_3y_quantlib"
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name=name,
                domain="rates.yoy_inflation_swap",
                description=(
                    "3Y pay-fixed annual year-on-year inflation swap on QuantLib reference indices and discount "
                    "factors, without a convexity adjustment."
                ),
                product="yoy_inflation_swap",
                source_detail=(
                    f"QuantLib {ql.__version__} supplies the reference indices (CPI.laggedFixing on a "
                    "ZeroInflationIndex with a flat annually compounded Act/365F ZeroInflationCurve, three-month "
                    "lag, CPI.Linear) and the FlatForward discount factors. QuantLib's YearOnYearInflationSwap "
                    "forecasts from a separate year-on-year curve instead of the zero-index ratio, so the "
                    "contractual net cash flows N * [(I_end / I_start - 1) - K * accrual] are summed in the "
                    "generator with 30/360 accruals and no year-on-year convexity adjustment. DV01 and "
                    "Inflation01 are central one-basis-point differences in the nominal zero rate and the "
                    "zero-inflation rate."
                ),
            )
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": _market(_yoy_reference_dates()),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "yoy_inflation_swap",
                "spec": {**_yoy_spec(name), "fixed_rate": str(fixed_rate), "side": "pay", "base_cpi": None},
            },
        },
        "expected": expected,
        "tolerances": _tolerances(expected, ARITHMETIC_REL, reason),
    }


def _build_yoy_cap_floor(
    *,
    product: str,
    name: str,
    rate_option_type: str,
    strike: float,
    volatility: float,
    normal: bool,
) -> dict[str, Any]:
    option_type = ql.Option.Call if rate_option_type == "cap" else ql.Option.Put

    def npv(rate_shift: float = 0.0, inflation_shift: float = 0.0, vol_shift: float = 0.0) -> float:
        # Each optionlet pays N * accrual * max(+/-(rate - K), 0) on the period end date.
        total = 0.0
        for accrual, yoy_rate, discount_factor, option_time in _yoy_period_terms(rate_shift, inflation_shift):
            std_dev = (volatility + vol_shift) * math.sqrt(option_time)
            formula: Callable[..., float] = ql.bachelierBlackFormula if normal else ql.blackFormula
            total += NOTIONAL * accrual * formula(option_type, strike, yoy_rate, std_dev, discount_factor)
        return total

    expected = {
        "dv01": central_difference(lambda shift: npv(rate_shift=shift), 0.0, RATE_BUMP),
        "inflation01": central_difference(lambda shift: npv(inflation_shift=shift), 0.0, RATE_BUMP),
        "npv": npv(),
        "vega": central_difference(lambda shift: npv(vol_shift=shift), 0.0, VOL_POINT),
    }
    model_name = "Bachelier" if normal else "Black-76"
    reason = (
        f"Both sides evaluate the same {model_name} closed form per optionlet on identical QuantLib forward "
        "year-on-year rates, discount factors and Act/365F option times."
    )
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name=name,
                domain="rates.inflation_cap_floor",
                description=(
                    f"3Y annual year-on-year inflation {rate_option_type} priced with QuantLib's {model_name} "
                    "formula on QuantLib reference indices and discount factors."
                ),
                product=product,
                source_detail=(
                    f"QuantLib {ql.__version__} supplies the reference indices (CPI.laggedFixing on a "
                    "ZeroInflationIndex with a flat annually compounded Act/365F ZeroInflationCurve, three-month "
                    "lag, CPI.Linear), the FlatForward discount factors and the "
                    f"{'bachelierBlackFormula' if normal else 'blackFormula'} optionlet value. QuantLib's "
                    "year-on-year cap/floor engines forecast from a separate year-on-year curve instead of the "
                    "zero-index ratio, so the generator assembles the strip itself: "
                    "forward rate (I_end / I_start - 1) / accrual with 30/360 accruals, flat volatility, and "
                    "Act/365F option time from the valuation date to the lagged end reference date. DV01, "
                    "Inflation01 and vega are central differences for one basis point of the nominal zero rate, "
                    "one basis point of the zero-inflation rate and one volatility point."
                ),
            )
        ),
        "kind": "pricing",
        "model": "normal" if normal else "black76",
        "market": _market(
            _yoy_reference_dates(),
            surfaces=[
                constant_vol_surface(
                    "USD-CPI-YOY-VOL-QL",
                    volatility,
                    quote_type="normal" if normal else "black_lognormal",
                    strikes=[0.0, strike, 0.06] if normal else [0.01, strike, 0.06],
                )
            ],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "inflation_cap_floor",
                "spec": {
                    **_yoy_spec(name),
                    "rate_option_type": rate_option_type,
                    "strike": str(strike),
                    "stub": "short_front",
                    "vol_surface_id": "USD-CPI-YOY-VOL-QL",
                    "volatility_expiry": "reference_date",
                },
            },
        },
        "expected": expected,
        "tolerances": _option_tolerances(expected, reason),
    }


def build_yoy_inflation_cap_black() -> dict[str, Any]:
    """Build a three-year Black-76 year-on-year inflation cap fixture."""
    return _build_yoy_cap_floor(
        product="yoy_inflation_cap_black",
        name="usd_yoy_inflation_cap_black_quantlib",
        rate_option_type="cap",
        strike=0.03,
        volatility=0.25,
        normal=False,
    )


def build_yoy_inflation_floor_normal() -> dict[str, Any]:
    """Build a three-year Bachelier year-on-year inflation floor fixture."""
    return _build_yoy_cap_floor(
        product="yoy_inflation_floor_normal",
        name="usd_yoy_inflation_floor_normal_quantlib",
        rate_option_type="floor",
        strike=0.02,
        volatility=0.015,
        normal=True,
    )


def build_inflation_linked_bond() -> dict[str, Any]:
    """Build a native QuantLib seasoned capital-indexed bond fixture without a deflation floor."""
    issue, maturity = "2025-11-18", "2030-11-18"
    real_coupon = 0.0125
    base_cpi = 297.0
    schedule = ql.Schedule(
        ql_date(issue),
        ql_date(maturity),
        ql.Period(6, ql.Months),
        ql.NullCalendar(),
        ql.Unadjusted,
        ql.Unadjusted,
        ql.DateGeneration.Backward,
        False,
    )

    def npv(rate_shift: float = 0.0) -> float:
        bond = ql.CPIBond(
            0,
            NOTIONAL,
            False,
            base_cpi,
            LAG,
            _cpi_index(),
            ql.CPI.Linear,
            schedule,
            [real_coupon],
            ql.ActualActual(ql.ActualActual.ISMA, schedule),
            ql.Unadjusted,
            ql_date(issue),
        )
        evaluation_date = _evaluation_date()
        return ql.CashFlows.npv(bond.cashflows(), _discount_curve(rate_shift), False, evaluation_date, evaluation_date)

    expected = {"dv01": central_difference(npv, 0.0, RATE_BUMP), "npv": npv()}
    reason = (
        "QuantLib native CPIBond and Finstack discount the same indexed coupons and redemption off identical "
        "monthly CPI levels; tolerance allows only cross-engine floating-point residual."
    )
    name = "usd_inflation_linked_bond_5y_quantlib"
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name=name,
                domain="fixed_income.inflation_linked_bond",
                description="QuantLib native seasoned five-year capital-indexed bond with no deflation floor.",
                product="inflation_linked_bond",
                source_detail=(
                    f"QuantLib {ql.__version__} CPIBond (growthOnly false, base CPI {base_cpi}, three-month lag, "
                    "CPI.Linear interpolation, ActualActual ISMA coupons on unadjusted dates); cash flows after "
                    "the valuation date discounted with CashFlows.npv on a flat Act/365F continuous nominal "
                    "curve. The Finstack inflation curve knots are QuantLib's projected monthly index fixings. "
                    "DV01 is a central one-basis-point difference in the nominal zero rate."
                ),
            )
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": _market([date for date in schedule if date > ql_date(VALUATION_DATE)]),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "inflation_linked_bond",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "real_coupon": str(real_coupon),
                    "frequency": {"count": 6, "unit": "months"},
                    "day_count": "act_act_isma",
                    "issue_date": issue,
                    "maturity": maturity,
                    "base_cpi": base_cpi,
                    "base_date": issue,
                    "indexation_method": "tips",
                    "lag": {"months": 3},
                    "deflation_protection": "none",
                    "business_day_convention": "unadjusted",
                    "stub": "none",
                    "calendar_id": None,
                    "discount_curve_id": DISCOUNT_ID,
                    "inflation_index_id": INDEX_ID,
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": _tolerances(expected, ARITHMETIC_REL, reason),
    }
