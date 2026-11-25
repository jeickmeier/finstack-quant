"""Native QuantLib convertible-bond golden builders."""

from __future__ import annotations

from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .bonds import _hazard_curve
from .common import (
    SCHEMA,
    VALUATION_DATE,
    central_difference,
    constant_vol_surface,
    flat_discount_curve,
    market_snapshot,
    metadata,
    ql_date,
)

CAPTURED_ON = "2026-11-23"
MATURITY = "2029-04-30"
NOTIONAL = 1_000_000.0
SPOT = 100.0
# Shares delivered for the whole notional; QuantLib quotes the ratio per 100 of redemption.
CONVERSION_RATIO = 10_000.0
VOLATILITY = 0.30
DIVIDEND_YIELD = 0.03
FLAT_RATE = 0.04
# One lattice step per calendar day between valuation and maturity (1096 days).
TREE_STEPS = 1096
REFERENCE_STEPS = 17_536


def _captured(meta: dict[str, Any]) -> dict[str, Any]:
    return {**meta, "captured_on": CAPTURED_ON, "last_reviewed_on": CAPTURED_ON}


def _zero_coupon_convertible(
    rate: float = FLAT_RATE,
    *,
    spot: float = SPOT,
    credit_spread: float = 0.0,
) -> float:
    """Price the zero-coupon convertible for the full notional on QuantLib's CRR lattice.

    With a zero credit spread the Tsiveriotis-Fernandes cash and equity
    components discount at the same risk-free rate and the lattice is the
    plain Black-Scholes convertible tree.
    """
    evaluation_date = ql_date(VALUATION_DATE)
    maturity = ql_date(MATURITY)
    ql.Settings.instance().evaluationDate = evaluation_date
    day_count = ql.Actual365Fixed()
    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(spot)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, DIVIDEND_YIELD, day_count, ql.Continuous)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, rate, day_count, ql.Continuous)),
        ql.BlackVolTermStructureHandle(ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), VOLATILITY, day_count)),
    )
    schedule = ql.Schedule(
        evaluation_date,
        maturity,
        ql.Period(1, ql.Years),
        ql.NullCalendar(),
        ql.Unadjusted,
        ql.Unadjusted,
        ql.DateGeneration.Forward,
        False,
    )
    redemption = 100.0
    bond = ql.ConvertibleZeroCouponBond(
        ql.AmericanExercise(evaluation_date, maturity),
        CONVERSION_RATIO * redemption / NOTIONAL,
        ql.CallabilitySchedule(),
        evaluation_date,
        0,
        day_count,
        schedule,
        redemption,
    )
    bond.setPricingEngine(
        ql.BinomialCRRConvertibleEngine(process, REFERENCE_STEPS, ql.QuoteHandle(ql.SimpleQuote(credit_spread)))
    )
    return bond.NPV() * NOTIONAL / redemption


def build_zero_coupon_convertible() -> dict[str, Any]:
    """Build a credit-risk-free zero-coupon convertible bond fixture."""
    name = "usd_convertible_zero_coupon_3y_quantlib"
    lattice = (
        f"Both sides price the same Black-Scholes convertible on a binomial lattice: QuantLib at "
        f"{REFERENCE_STEPS} steps (within about 2e-6 of its own extrapolated continuous limit) and Finstack at {TREE_STEPS} steps."
    )
    npv_reason = (
        f"{lattice} The bound is the lattice discretization at {TREE_STEPS} steps: QuantLib's CRR, "
        "Jarrow-Rudd, Tian and Trigeorgis lattices at 1096 and 1097 steps all lie within 3.6e-5 of the "
        "reference, and the tolerance allows about three times that."
    )
    dv01_reason = (
        f"{lattice} DV01 is a 1bp parallel continuous zero shift of the risk-free curve. QuantLib's "
        "fixed-node lattices (CRR, Trigeorgis) at 1096 and 1097 steps lie within 2.4e-4 of the reference "
        "DV01; the tolerance allows about four times that, in line with one step in 1096."
    )
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name=name,
                domain="fixed_income.convertible",
                description=(
                    "QuantLib native three-year USD zero-coupon convertible bond, convertible at any time, "
                    "with no issuer credit risk."
                ),
                product="convertible_zero_coupon",
                source_detail=(
                    f"QuantLib {ql.__version__} BinomialCRRConvertibleEngine ({REFERENCE_STEPS} steps) on a "
                    f"ConvertibleZeroCouponBond with American conversion, spot {SPOT}, flat {VOLATILITY} "
                    f"volatility, {DIVIDEND_YIELD} continuous dividend yield and a flat {FLAT_RATE} continuous "
                    "Act/365F curve. The credit spread is zero and the Finstack bond has no credit curve: "
                    "with issuer credit the two models differ (QuantLib's Tsiveriotis-Fernandes split "
                    "against Finstack's jump-to-default equity tree), so only the credit-free lattice is "
                    "compared. The bond pays no coupon because QuantLib forfeits a coupon on conversion "
                    "and Finstack retains it."
                ),
            )
        ),
        "kind": "pricing",
        "model": "tree",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", FLAT_RATE)],
            prices={
                "ACME-DIVYIELD": {"unitless": DIVIDEND_YIELD},
                "ACME-SPOT": {"price": {"amount": str(SPOT), "currency": "USD"}},
            },
            surfaces=[constant_vol_surface("ACME-VOL-QL", VOLATILITY, strikes=[50.0, 100.0, 150.0])],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "convertible_bond",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "notional": {"amount": str(int(NOTIONAL)), "currency": "USD"},
                    "issue_date": VALUATION_DATE,
                    "maturity": MATURITY,
                    "discount_curve_id": "USD-OIS",
                    "conversion": {
                        "ratio": CONVERSION_RATIO,
                        "policy": "voluntary",
                        "anti_dilution": "none",
                        "dividend_adjustment": "none",
                    },
                    "spot_id": "ACME-SPOT",
                    "vol_surface_id": "ACME-VOL-QL",
                    "div_yield_id": "ACME-DIVYIELD",
                    "cashflow_spec": {
                        "fixed": {
                            "rate": "0",
                            "frequency": {"count": 12, "unit": "months"},
                            "day_count": "act_365f",
                            "business_day_convention": "unadjusted",
                            "stub": "none",
                            "calendar_id": "weekends_only",
                        }
                    },
                    "instrument_pricing_overrides": {"model_config": {"tree_steps": TREE_STEPS}},
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": {
            "dv01": central_difference(_zero_coupon_convertible, FLAT_RATE),
            "npv": _zero_coupon_convertible(),
        },
        "tolerances": {
            "dv01": {"rel": 1e-3, "tolerance_reason": dv01_reason},
            "npv": {"rel": 1e-4, "tolerance_reason": npv_reason},
        },
    }


CREDIT_SPREAD = 0.02


def _credit_convertible(
    *,
    product: str,
    name: str,
    description: str,
    spot: float = SPOT,
) -> dict[str, Any]:
    """Build a credit-risky zero-coupon convertible fixture on the Tsiveriotis-Fernandes model."""

    def price(rate: float = FLAT_RATE) -> float:
        return _zero_coupon_convertible(rate, spot=spot, credit_spread=CREDIT_SPREAD)

    lattice = (
        f"Both sides price the Tsiveriotis-Fernandes convertible on a binomial lattice: QuantLib at "
        f"{REFERENCE_STEPS} steps and Finstack at {TREE_STEPS} steps."
    )
    npv_reason = (
        f"{lattice} With a credit spread QuantLib's lattice converges more slowly than the credit-free "
        "one: its at-the-money price moves by 2.1e-4 from 1096 to 4384 steps and 8.5e-5 from 4384 to "
        f"{REFERENCE_STEPS}, so the reference is within about 1e-4 of its own limit. QuantLib compounds "
        "the blended rate simply per step and Finstack continuously, a difference of order one step."
    )
    dv01_reason = (
        f"{lattice} The reference DV01 is a central difference over 10bp either side, per bp: QuantLib's "
        "lattice price is not smooth in the rate at 1bp, and its own 1bp DV01 moves by 1.6% between "
        "1096, 4384 and 17536 steps, while the 10bp estimate is stable to 0.1%. Finstack's DV01 is a 1bp "
        f"bump on a {TREE_STEPS}-step lattice, so the bound is the 1bp estimator's dispersion."
    )
    spec: dict[str, Any] = {
        "id": name.upper().replace("_", "-"),
        "notional": {"amount": str(int(NOTIONAL)), "currency": "USD"},
        "issue_date": VALUATION_DATE,
        "maturity": MATURITY,
        "discount_curve_id": "USD-OIS",
        "credit_curve_id": "ACME-HAZARD",
        "recovery_rate": 0.0,
        "conversion": {
            "ratio": CONVERSION_RATIO,
            "policy": "voluntary",
            "anti_dilution": "none",
            "dividend_adjustment": "none",
        },
        "spot_id": "ACME-SPOT",
        "vol_surface_id": "ACME-VOL-QL",
        "div_yield_id": "ACME-DIVYIELD",
        "cashflow_spec": {
            "fixed": {
                "rate": "0",
                "frequency": {"count": 12, "unit": "months"},
                "day_count": "act_365f",
                "business_day_convention": "unadjusted",
                "stub": "none",
                "calendar_id": "weekends_only",
            }
        },
        "instrument_pricing_overrides": {"model_config": {"tree_steps": TREE_STEPS}},
        "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
    }
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name=name,
                domain="fixed_income.convertible",
                description=description,
                product=product,
                source_detail=(
                    f"QuantLib {ql.__version__} BinomialCRRConvertibleEngine ({REFERENCE_STEPS} steps) on a "
                    f"ConvertibleZeroCouponBond with American conversion and a flat credit spread of "
                    f"{CREDIT_SPREAD}: spot {spot}, flat {VOLATILITY} volatility, {DIVIDEND_YIELD} continuous "
                    f"dividend yield, flat {FLAT_RATE} continuous Act/365F curve. The Finstack bond carries a "
                    f"flat {CREDIT_SPREAD} hazard curve with zero recovery, which is the same spread. The bond "
                    "pays no coupon because QuantLib forfeits a coupon on conversion and Finstack retains it. Issuer "
                    "calls and holder puts are not compared: on a cash redemption Finstack resets the "
                    "conversion probability to zero, as the model's cash-only component requires, and "
                    "QuantLib leaves it unchanged."
                ),
            )
        ),
        "kind": "pricing",
        "model": "tsiveriotis_fernandes",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", FLAT_RATE), _hazard_curve("ACME-HAZARD", CREDIT_SPREAD)],
            prices={
                "ACME-DIVYIELD": {"unitless": DIVIDEND_YIELD},
                "ACME-SPOT": {"price": {"amount": str(spot), "currency": "USD"}},
            },
            surfaces=[constant_vol_surface("ACME-VOL-QL", VOLATILITY, strikes=[50.0, 100.0, 150.0])],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {"type": "convertible_bond", "spec": spec},
        },
        "expected": {"dv01": central_difference(price, FLAT_RATE, 1e-3) / 10.0, "npv": price()},
        "tolerances": {
            "dv01": {"rel": 2e-2, "tolerance_reason": dv01_reason},
            "npv": {"rel": 1e-4, "tolerance_reason": npv_reason},
        },
    }


def build_tf_convertible_at_the_money() -> dict[str, Any]:
    """Build a credit-risky at-the-money zero-coupon convertible fixture."""
    return _credit_convertible(
        product="tf_convertible_at_the_money",
        name="usd_convertible_tf_atm_3y_quantlib",
        description="QuantLib Tsiveriotis-Fernandes three-year zero-coupon convertible, at the money, 2% credit spread.",
    )


def build_tf_convertible_out_of_the_money() -> dict[str, Any]:
    """Build a credit-risky deep out-of-the-money zero-coupon convertible fixture."""
    return _credit_convertible(
        product="tf_convertible_out_of_the_money",
        name="usd_convertible_tf_otm_3y_quantlib",
        description="QuantLib Tsiveriotis-Fernandes three-year zero-coupon convertible, spot at 40% of the conversion price.",
        spot=40.0,
    )


def build_tf_convertible_in_the_money() -> dict[str, Any]:
    """Build a credit-risky deep in-the-money zero-coupon convertible fixture."""
    return _credit_convertible(
        product="tf_convertible_in_the_money",
        name="usd_convertible_tf_itm_3y_quantlib",
        description="QuantLib Tsiveriotis-Fernandes three-year zero-coupon convertible, spot at 250% of the conversion price.",
        spot=250.0,
    )
