"""Convertible-bond golden builders: QuantLib without credit, an independent lattice with it."""

from __future__ import annotations

from datetime import date
import math
from typing import Any

import numpy as np
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
# Steps of the independent two-component lattice that supplies the reference.
TWO_COMPONENT_STEPS = 35_072


def _two_component_lattice(rate: float = FLAT_RATE, *, spot: float = SPOT, steps: int = TWO_COMPONENT_STEPS) -> float:
    """Price the zero-coupon convertible on a Tsiveriotis-Fernandes (1998) two-component CRR lattice.

    Each node carries the total value and its cash part. Over one step the cash
    part is discounted at the risk-free rate plus the credit spread and the
    remainder at the risk-free rate. The holder converts where the conversion
    value exceeds the total, which leaves no cash part.
    """
    years = (date.fromisoformat(MATURITY) - date.fromisoformat(VALUATION_DATE)).days / 365.0
    dt = years / steps
    up = math.exp(VOLATILITY * math.sqrt(dt))
    prob_up = (math.exp((rate - DIVIDEND_YIELD) * dt) - 1.0 / up) / (up - 1.0 / up)
    risk_free = math.exp(-rate * dt)
    risky = math.exp(-(rate + CREDIT_SPREAD) * dt)
    spots = spot * up ** (2.0 * np.arange(steps + 1) - steps)
    conversion = CONVERSION_RATIO * spots
    total = np.maximum(conversion, NOTIONAL)
    cash = np.where(conversion > NOTIONAL, 0.0, NOTIONAL)
    for _ in range(steps):
        spots = spots[:-1] * up
        expected_total = prob_up * total[1:] + (1.0 - prob_up) * total[:-1]
        expected_cash = prob_up * cash[1:] + (1.0 - prob_up) * cash[:-1]
        hold_cash = expected_cash * risky
        hold_total = (expected_total - expected_cash) * risk_free + hold_cash
        conversion = CONVERSION_RATIO * spots
        converts = conversion > hold_total
        total = np.where(converts, conversion, hold_total)
        cash = np.where(converts, 0.0, hold_cash)
    return float(total[0])


def _credit_convertible(
    *,
    product: str,
    name: str,
    description: str,
    spot: float = SPOT,
) -> dict[str, Any]:
    """Build a credit-risky zero-coupon convertible fixture on the Tsiveriotis-Fernandes model."""

    def price(rate: float = FLAT_RATE) -> float:
        return _two_component_lattice(rate, spot=spot)

    lattice = (
        f"The reference is an independent two-component binomial lattice at {TWO_COMPONENT_STEPS} steps, "
        f"written in the generator; Finstack prices at {TREE_STEPS} steps."
    )
    npv_reason = (
        f"{lattice} The reference lattice moves by 1.3e-6 at the money and 2.4e-5 deep out of the money "
        f"between 17536 and {TWO_COMPONENT_STEPS} steps, and by 1.3e-5 from there to 70144 steps, so it is "
        "within about 2.5e-5 of its limit. The bound adds the 1e-4 band the credit-free convertible fixture "
        "allows a 1096-step lattice."
    )
    dv01_reason = (
        f"{lattice} The reference DV01 is a central difference over 10bp either side, per bp, because a "
        f"lattice price is not smooth in the rate at 1bp. Finstack's DV01 is a 1bp bump on a {TREE_STEPS}-step "
        "lattice, so the bound is the 1bp estimator's dispersion: 1.6% between step counts when measured "
        "on QuantLib's convertible lattice."
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
        "attributes": {"tags": ["golden"], "meta": {}},
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
                    f"Tsiveriotis-Fernandes (1998) two-component CRR lattice ({TWO_COMPONENT_STEPS} steps) coded in "
                    "scripts/golden/quantlib/convertible.py, independent of Finstack: each node carries the total "
                    "value and its cash part, the cash part is discounted at the risk-free rate plus the spread and "
                    "the remainder at the risk-free rate, and conversion leaves no cash part. Zero-coupon bond with "
                    f"American conversion, spot {spot}, flat {VOLATILITY} volatility, {DIVIDEND_YIELD} continuous "
                    f"dividend yield, flat {FLAT_RATE} continuous Act/365F curve and a flat {CREDIT_SPREAD} spread. "
                    f"The Finstack bond carries a flat {CREDIT_SPREAD} hazard curve with zero recovery, which is the "
                    "same spread. QuantLib's BinomialCRRConvertibleEngine is not the reference: it discounts each "
                    "node at one blended rate and converges about 0.25% lower at the money."
                ),
            )
            | {"source": "formula"}
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
        name="usd_convertible_tf_atm_3y",
        description="Tsiveriotis-Fernandes three-year zero-coupon convertible, at the money, 2% credit spread.",
    )


def build_tf_convertible_out_of_the_money() -> dict[str, Any]:
    """Build a credit-risky deep out-of-the-money zero-coupon convertible fixture."""
    return _credit_convertible(
        product="tf_convertible_out_of_the_money",
        name="usd_convertible_tf_otm_3y",
        description="Tsiveriotis-Fernandes three-year zero-coupon convertible, spot at 40% of the conversion price.",
        spot=40.0,
    )


def build_tf_convertible_in_the_money() -> dict[str, Any]:
    """Build a credit-risky deep in-the-money zero-coupon convertible fixture."""
    return _credit_convertible(
        product="tf_convertible_in_the_money",
        name="usd_convertible_tf_itm_3y",
        description="Tsiveriotis-Fernandes three-year zero-coupon convertible, spot at 250% of the conversion price.",
        spot=250.0,
    )
