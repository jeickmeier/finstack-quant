"""Native QuantLib float-float swap golden builders (single-currency basis and cross-currency)."""

from __future__ import annotations

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
    tolerance,
)

CAPTURED_ON = "2026-11-23"
START = "2026-05-06"
END = "2031-05-06"
# Both engines sum the same simple forwards and discount factors on exactly
# aligned unadjusted schedules; 1e-7 on 10,000,000 notional is the bound the
# other analytic rate fixtures use for cross-engine floating-point residual.
ANALYTICAL_RATE_TOLERANCE = 1e-7
# Par spread in basis points: 1e-7 currency units of NPV residual divided by the
# roughly 4,500 per basis point primary-leg annuity is about 2e-11 bp.
PAR_SPREAD_TOLERANCE_BP = 1e-9


def _captured(meta: dict[str, Any]) -> dict[str, Any]:
    return {**meta, "captured_on": CAPTURED_ON, "last_reviewed_on": CAPTURED_ON}


def _flat_curve(rate: float) -> ql.YieldTermStructureHandle:
    return ql.YieldTermStructureHandle(
        ql.FlatForward(ql_date(VALUATION_DATE), rate, ql.Actual365Fixed(), ql.Continuous)
    )


def _floating_leg(
    *,
    name: str,
    currency: ql.Currency,
    months: int,
    notional: float,
    spread: float,
    projection: ql.YieldTermStructureHandle,
) -> tuple[ql.CashFlow, ...]:
    """Build an unadjusted Act/360 term-rate leg whose index period equals each accrual period."""
    tenor = ql.Period(months, ql.Months)
    index = ql.IborIndex(name, tenor, 0, currency, ql.NullCalendar(), ql.Unadjusted, False, ql.Actual360(), projection)
    schedule = ql.Schedule(
        ql_date(START),
        ql_date(END),
        tenor,
        ql.NullCalendar(),
        ql.Unadjusted,
        ql.Unadjusted,
        ql.DateGeneration.Forward,
        False,
    )
    return ql.IborLeg(
        [notional],
        schedule,
        index,
        paymentDayCounter=ql.Actual360(),
        paymentConvention=ql.Unadjusted,
        fixingDays=[0],
        spreads=[spread],
    )


def _float_leg_spec(
    *, index_id: str, discount_curve_id: str, months: int, spread_bp: str, calendar_id: str | None
) -> dict[str, Any]:
    return {
        "discount_curve_id": discount_curve_id,
        "index_id": index_id,
        "forward_curve_id": index_id,
        "spread_bp": spread_bp,
        "frequency": {"count": months, "unit": "months"},
        "day_count": "act_360",
        "business_day_convention": "unadjusted",
        "calendar_id": calendar_id,
        "fixing_calendar_id": None,
        "stub": "none",
        "reset_lag_days": 0,
        "payment_lag_days": 0,
        "end_of_month": False,
        "start": START,
        "end": END,
        "compounding": "simple",
    }


def build_basis_swap() -> dict[str, Any]:
    """Build a native QuantLib five-year USD 1M-versus-3M basis swap fixture."""
    notional = 10_000_000.0
    discount_rate = 0.04
    primary_rate = 0.0415
    reference_rate = 0.043
    spread_bp = 9.0
    ql.Settings.instance().evaluationDate = ql_date(VALUATION_DATE)
    ql.IndexManager.instance().clearHistories()

    def priced_swap(discount_shift: float, primary_shift: float, reference_shift: float) -> ql.Swap:
        primary = _floating_leg(
            name="USD-TERM-1M",
            currency=ql.USDCurrency(),
            months=1,
            notional=notional,
            spread=spread_bp * 1e-4,
            projection=_flat_curve(primary_rate + primary_shift),
        )
        reference = _floating_leg(
            name="USD-TERM-3M",
            currency=ql.USDCurrency(),
            months=3,
            notional=notional,
            spread=0.0,
            projection=_flat_curve(reference_rate + reference_shift),
        )
        # ql.Swap pays its first leg and receives its second: pay reference, receive primary.
        swap = ql.Swap(reference, primary)
        swap.setPricingEngine(ql.DiscountingSwapEngine(_flat_curve(discount_rate + discount_shift)))
        return swap

    swap = priced_swap(0.0, 0.0, 0.0)
    pv_reference = -swap.legNPV(0)
    pv_primary = swap.legNPV(1)
    if pv_reference <= 0.0 or pv_primary <= 0.0 or swap.legBPS(1) <= 0.0:
        raise RuntimeError("QuantLib swap leg order is not pay-reference / receive-primary")
    # Finstack's basis-swap DV01 bumps each rate curve on its own and sums the
    # central differences, so the reference does the same per curve.
    dv01 = (
        central_difference(lambda shift: priced_swap(shift, 0.0, 0.0).NPV(), 0.0)
        + central_difference(lambda shift: priced_swap(0.0, shift, 0.0).NPV(), 0.0)
        + central_difference(lambda shift: priced_swap(0.0, 0.0, shift).NPV(), 0.0)
    )
    expected = {
        # Primary spread that zeroes the NPV: the quoted spread less NPV per basis point of primary spread.
        "basis_par_spread": spread_bp - swap.NPV() / swap.legBPS(1),
        "dv01": dv01,
        "npv": swap.NPV(),
        "pv_primary": pv_primary,
        "pv_reference": pv_reference,
    }
    reason = (
        "QuantLib DiscountingSwapEngine and Finstack sum the same simple Act/360 forwards and "
        "discount factors on exactly aligned unadjusted schedules; only floating-point residual remains."
    )
    par_reason = (
        "Same aligned cash flows as the NPV; 1e-7 of NPV residual over the primary annuity of "
        "about 4,500 per basis point is below 1e-9 bp."
    )
    tolerances = {metric: tolerance(ANALYTICAL_RATE_TOLERANCE, reason) for metric in expected}
    tolerances["basis_par_spread"] = tolerance(PAR_SPREAD_TOLERANCE_BP, par_reason)
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name="usd_basis_swap_1m3m_5y_quantlib",
                domain="rates.basis_swap",
                description="QuantLib native five-year USD 1M-plus-spread versus 3M term-rate basis swap.",
                product="basis_swap",
                source_detail=(
                    f"QuantLib {ql.__version__} Swap of two IborLeg legs priced by DiscountingSwapEngine on flat "
                    f"continuous Act/365F curves (discount {discount_rate}, 1M projection {primary_rate}, 3M "
                    f"projection {reference_rate}); receive 1M + {spread_bp} bp, pay 3M flat, unadjusted Act/360 "
                    "periods equal to the index tenors. NPV, leg NPVs and leg BPS are QuantLib outputs; "
                    "basis_par_spread is spread - NPV / primary leg BPS and dv01 is the sum of three 1 bp "
                    "central differences, one per curve, in the generator."
                ),
            )
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot([
            flat_discount_curve("USD-OIS", discount_rate),
            flat_forward_curve("USD-TERM-1M", primary_rate, index_months=1),
            flat_forward_curve("USD-TERM-3M", reference_rate, index_months=3),
        ]),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "basis_swap",
                "spec": {
                    "id": "USD-BASIS-1M3M-5Y-QUANTLIB",
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "primary_leg": _float_leg_spec(
                        index_id="USD-TERM-1M",
                        discount_curve_id="USD-OIS",
                        months=1,
                        spread_bp="9",
                        calendar_id="weekends_only",
                    ),
                    "reference_leg": _float_leg_spec(
                        index_id="USD-TERM-3M",
                        discount_curve_id="USD-OIS",
                        months=3,
                        spread_bp="0",
                        calendar_id="weekends_only",
                    ),
                    "allow_calendar_fallback": False,
                    "allow_same_curve": False,
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": tolerances,
    }


def build_xccy_swap() -> dict[str, Any]:
    """Build a five-year receive-USD pay-EUR float-float cross-currency swap fixture."""
    usd_notional = 10_000_000.0
    eur_notional = 9_000_000.0
    spot = 1.10
    usd_discount_rate = 0.04
    usd_projection_rate = 0.043
    eur_discount_rate = 0.02
    eur_projection_rate = 0.023
    eur_spread_bp = -15.0
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    ql.IndexManager.instance().clearHistories()

    def leg_npv(
        *,
        name: str,
        currency: ql.Currency,
        notional: float,
        spread: float,
        projection_rate: float,
        discount_rate: float,
    ) -> float:
        """Return the QuantLib NPV of a received leg: -N at start, coupons, +N at end."""
        coupons = _floating_leg(
            name=name,
            currency=currency,
            months=3,
            notional=notional,
            spread=spread,
            projection=_flat_curve(projection_rate),
        )
        flows = [
            ql.SimpleCashFlow(-notional, ql_date(START)),
            *coupons,
            ql.SimpleCashFlow(notional, ql_date(END)),
        ]
        return ql.CashFlows.npv(flows, _flat_curve(discount_rate), False, evaluation_date, evaluation_date)

    def npv(shift: float) -> float:
        usd_received = leg_npv(
            name="USD-TERM-3M",
            currency=ql.USDCurrency(),
            notional=usd_notional,
            spread=0.0,
            projection_rate=usd_projection_rate + shift,
            discount_rate=usd_discount_rate + shift,
        )
        eur_received = leg_npv(
            name="EUR-TERM-3M",
            currency=ql.EURCurrency(),
            notional=eur_notional,
            spread=eur_spread_bp * 1e-4,
            projection_rate=eur_projection_rate + shift,
            discount_rate=eur_discount_rate + shift,
        )
        # Receive the USD leg, pay the EUR leg; the EUR leg NPV converts to USD at spot.
        return usd_received - spot * eur_received

    expected = {
        # Finstack's cross-currency DV01 shifts every rate curve of both currencies together at fixed spot.
        "dv01": central_difference(npv, 0.0),
        "npv": npv(0.0),
    }
    reason = (
        "Both engines discount the same simple Act/360 forwards and notional exchanges on exactly aligned "
        "unadjusted schedules and convert the EUR leg at the same spot; only floating-point residual remains."
    )
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name="usdeur_xccy_swap_5y_quantlib",
                domain="rates.xccy_swap",
                description=(
                    "Five-year receive-USD pay-EUR float-float cross-currency swap with initial and final "
                    "notional exchange, valued on QuantLib legs."
                ),
                product="xccy_swap",
                source_detail=(
                    f"QuantLib {ql.__version__} has no generic cross-currency swap instrument. QuantLib parts: each "
                    "currency's IborLeg coupons plus SimpleCashFlow notional exchanges, valued by CashFlows.npv on "
                    f"that currency's flat continuous Act/365F discount curve (USD discount {usd_discount_rate}, "
                    f"projection {usd_projection_rate}; EUR discount {eur_discount_rate}, projection "
                    f"{eur_projection_rate}). Generator arithmetic: NPV = USD leg NPV - {spot} * EUR leg NPV "
                    f"(receive USD {usd_notional:.0f} flat, pay EUR {eur_notional:.0f} {eur_spread_bp} bp, spot "
                    "conversion), and dv01 as the 1 bp central difference with all four curves shifted together."
                ),
            )
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot(
            [
                flat_discount_curve("USD-OIS", usd_discount_rate),
                flat_discount_curve("EUR-OIS", eur_discount_rate),
                flat_forward_curve("USD-TERM-3M", usd_projection_rate, index_months=3),
                flat_forward_curve("EUR-TERM-3M", eur_projection_rate, index_months=3),
            ],
            fx={
                "config": {
                    "pivot_currency": "USD",
                    "cache_capacity": 256,
                },
                "quotes": [["EUR", "USD", spot]],
                "pinned_quotes": [],
                "provider_quotes": [],
                "provider_pinned_quotes": [],
            },
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "xccy_swap",
                "spec": {
                    "id": "USDEUR-XCCY-5Y-QUANTLIB",
                    "leg1": {
                        "notional": {"amount": "10000000", "currency": "USD"},
                        "side": "receive",
                        "leg": _float_leg_spec(
                            index_id="USD-TERM-3M",
                            discount_curve_id="USD-OIS",
                            months=3,
                            spread_bp="0",
                            calendar_id="weekends_only",
                        ),
                    },
                    "leg2": {
                        "notional": {"amount": "9000000", "currency": "EUR"},
                        "side": "pay",
                        "leg": _float_leg_spec(
                            index_id="EUR-TERM-3M",
                            discount_curve_id="EUR-OIS",
                            months=3,
                            spread_bp="-15",
                            calendar_id="weekends_only",
                        ),
                    },
                    "notional_exchange": "initial_and_final",
                    "reporting_currency": "USD",
                    "allow_calendar_fallback": False,
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": {metric: tolerance(ANALYTICAL_RATE_TOLERANCE, reason) for metric in expected},
    }
