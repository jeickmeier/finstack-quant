"""Native QuantLib bond golden builders."""

from __future__ import annotations

from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import SCHEMA_VERSION, VALUATION_DATE, central_difference, market_snapshot, metadata, ql_date, tolerance


def _bond_discount_curve(curve_id: str, rate: float) -> dict[str, Any]:
    """Build the 30/360 flat curve used by fixed-bond parity."""
    import math

    return {
        "type": "discount",
        "id": curve_id,
        "base": VALUATION_DATE,
        "day_count": "Thirty360",
        "knot_points": [[0.0, 1.0], [30.0, math.exp(-rate * 30.0)]],
        "interp_style": "log_linear",
        "extrapolation": "flat_forward",
        "min_forward_rate": None,
        "allow_non_monotonic": False,
        "min_forward_tenor": 1e-6,
        "rate_calibration": None,
    }


def build_fixed_risk_free_bond() -> dict[str, Any]:
    """Build a native QuantLib ten-year fixed-rate risk-free bond fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    issue_date = evaluation_date
    maturity_date = ql.Date(30, 4, 2036)
    face = 100.0
    coupon = 0.05
    curve_rate = 0.04
    day_count = ql.Thirty360(ql.Thirty360.BondBasis)
    calendar = ql.WeekendsOnly()
    ql.Settings.instance().evaluationDate = evaluation_date

    schedule = ql.Schedule(
        issue_date,
        maturity_date,
        ql.Period(6, ql.Months),
        calendar,
        ql.Following,
        ql.Following,
        ql.DateGeneration.Backward,
        False,
    )
    bond = ql.FixedRateBond(2, face, schedule, [coupon], day_count, ql.Following)

    def npv(rate: float) -> float:
        curve = ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, rate, day_count, ql.Continuous))
        bond.setPricingEngine(ql.DiscountingBondEngine(curve))
        return bond.NPV()

    expected = {"dv01": central_difference(npv, curve_rate), "npv": npv(curve_rate)}
    reason = (
        "Independent vanilla-bond validation target: price within 0.02 per 100 notional and "
        "DV01 within 0.1% under matched schedule and curve conventions."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="usd_fixed_10y_risk_free_quantlib",
            domain="fixed_income.bond",
            description="QuantLib native ten-year USD fixed-rate risk-free bond.",
            product="fixed_risk_free_bond",
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot([_bond_discount_curve("USD-OIS", curve_rate)]),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "bond",
                "spec": {
                    "id": "USD-FIXED-10Y-RISK-FREE-QUANTLIB",
                    "notional": {"amount": "100", "currency": "USD"},
                    "issue_date": VALUATION_DATE,
                    "maturity": "2036-04-30",
                    "cashflow_spec": {
                        "Fixed": {
                            "coupon_type": "Cash",
                            "rate": "0.05",
                            "freq": {"count": 6, "unit": "months"},
                            "dc": "Thirty360",
                            "bdc": "following",
                            "calendar_id": "weekends_only",
                            "stub": "ShortFront",
                        }
                    },
                    "discount_curve_id": "USD-OIS",
                    "credit_curve_id": None,
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                    "settlement_days": 2,
                    "ex_coupon_days": 0,
                },
            },
        },
        "expected": expected,
        "tolerances": {
            "npv": tolerance(0.02, reason),
            "dv01": {"rel": 0.001, "tolerance_reason": reason},
        },
    }


def _hazard_curve(curve_id: str, hazard_rate: float) -> dict[str, Any]:
    """Build a flat zero-recovery Finstack hazard curve."""
    return {
        "type": "hazard",
        "id": curve_id,
        "base": VALUATION_DATE,
        "knot_points": [[0.0, hazard_rate], [30.0, hazard_rate]],
        "recovery_rate": 0.0,
        "issuer": None,
        "seniority": None,
        "currency": "USD",
        "day_count": "Act365F",
        "par_points": [],
        "par_interp": "Linear",
        "survival_interp": "log_linear",
        "fx_policy": None,
    }


def build_fixed_hazard_bond() -> dict[str, Any]:
    """Build a native QuantLib fixed-rate zero-recovery hazard bond fixture."""
    fixture = build_fixed_risk_free_bond()
    evaluation_date = ql_date(VALUATION_DATE)
    maturity_date = ql.Date(30, 4, 2031)
    face = 100.0
    coupon = 0.05
    curve_rate = 0.04
    hazard_rate = 0.02
    day_count = ql.Thirty360(ql.Thirty360.BondBasis)
    schedule = ql.Schedule(
        evaluation_date,
        maturity_date,
        ql.Period(6, ql.Months),
        ql.WeekendsOnly(),
        ql.Following,
        ql.Following,
        ql.DateGeneration.Backward,
        False,
    )
    bond = ql.FixedRateBond(2, face, schedule, [coupon], day_count, ql.Following)

    def npv(discount: float, hazard: float) -> float:
        discount_curve = ql.YieldTermStructureHandle(
            ql.FlatForward(evaluation_date, discount, day_count, ql.Continuous)
        )
        default_curve = ql.DefaultProbabilityTermStructureHandle(
            ql.FlatHazardRate(evaluation_date, ql.QuoteHandle(ql.SimpleQuote(hazard)), ql.Actual365Fixed())
        )
        bond.setPricingEngine(ql.RiskyBondEngine(default_curve, 0.0, discount_curve))
        return bond.NPV()

    expected = {
        "cs01": central_difference(lambda hazard: npv(curve_rate, hazard), hazard_rate),
        "dv01": central_difference(lambda discount: npv(discount, hazard_rate), curve_rate),
        "npv": npv(curve_rate, hazard_rate),
    }
    reason = (
        "Independent zero-recovery credit-bond target: price within 0.002 per 100 and DV01/CS01 "
        "within 0.1%; zero recovery removes recovery-payment timing differences."
    )
    fixture["metadata"] = metadata(
        name="usd_fixed_5y_hazard_quantlib",
        domain="fixed_income.bond",
        description="QuantLib native five-year USD fixed-rate hazard bond with zero recovery.",
        product="fixed_hazard_bond",
    )
    fixture["market"] = market_snapshot([
        _bond_discount_curve("USD-OIS", curve_rate),
        _hazard_curve("USD-CREDIT", hazard_rate),
    ])
    spec = fixture["instrument"]["instrument"]["spec"]
    spec["id"] = "USD-FIXED-5Y-HAZARD-QUANTLIB"
    spec["maturity"] = "2031-04-30"
    spec["credit_curve_id"] = "USD-CREDIT"
    fixture["expected"] = expected
    fixture["tolerances"] = {
        "npv": tolerance(0.002, reason),
        "dv01": {"rel": 0.001, "tolerance_reason": reason},
        "cs01": {"rel": 0.001, "tolerance_reason": reason},
    }
    return fixture


def _floating_bond_spec(*, credit_curve_id: str | None) -> dict[str, Any]:
    """Build the shared Finstack floating-rate bond instrument envelope."""
    return {
        "schema": "finstack_quant.instrument/1",
        "instrument": {
            "type": "bond",
            "spec": {
                "id": "USD-FLOATING-5Y-QUANTLIB",
                "notional": {"amount": "100", "currency": "USD"},
                "issue_date": "2026-05-06",
                "maturity": "2031-05-06",
                "cashflow_spec": {
                    "Floating": {
                        "rate_spec": {
                            "index_id": "USD-SOFR-3M",
                            "spread_bp": "100",
                            "gearing": "1",
                            "gearing_includes_spread": True,
                            "index_floor_bp": None,
                            "all_in_floor_bp": None,
                            "all_in_cap_bp": None,
                            "index_cap_bp": None,
                            "reset_freq": {"count": 3, "unit": "months"},
                            "reset_lag_days": 2,
                            "dc": "Act360",
                            "bdc": "following",
                            "calendar_id": "weekends_only",
                            "fixing_calendar_id": "weekends_only",
                            "end_of_month": False,
                            "payment_lag_days": 0,
                        },
                        "coupon_type": "Cash",
                        "freq": {"count": 3, "unit": "months"},
                        "stub": "ShortFront",
                    }
                },
                "discount_curve_id": "USD-OIS",
                "credit_curve_id": credit_curve_id,
                "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                "settlement_days": 2,
                "ex_coupon_days": 0,
            },
        },
    }


def _quantlib_floating_bond_npv(discount: float, projection: float, hazard: float | None) -> float:
    """Price the shared floating-rate bond with native QuantLib engines."""
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    discount_handle = ql.YieldTermStructureHandle(
        ql.FlatForward(evaluation_date, discount, ql.Actual365Fixed(), ql.Continuous)
    )
    projection_handle = ql.YieldTermStructureHandle(
        ql.FlatForward(evaluation_date, projection, ql.Actual365Fixed(), ql.Continuous)
    )
    index = ql.IborIndex(
        "USD-Term-SOFR-3M",
        ql.Period(3, ql.Months),
        2,
        ql.USDCurrency(),
        ql.WeekendsOnly(),
        ql.Following,
        False,
        ql.Actual360(),
        projection_handle,
    )
    schedule = ql.Schedule(
        ql.Date(6, 5, 2026),
        ql.Date(6, 5, 2031),
        ql.Period(3, ql.Months),
        ql.WeekendsOnly(),
        ql.Following,
        ql.Following,
        ql.DateGeneration.Backward,
        False,
    )
    bond = ql.FloatingRateBond(
        2,
        100.0,
        schedule,
        index,
        ql.Actual360(),
        ql.Following,
        2,
        [1.0],
        [0.01],
    )
    ql.setCouponPricer(bond.cashflows(), ql.BlackIborCouponPricer())
    if hazard is None:
        bond.setPricingEngine(ql.DiscountingBondEngine(discount_handle))
    else:
        default_handle = ql.DefaultProbabilityTermStructureHandle(
            ql.FlatHazardRate(evaluation_date, ql.QuoteHandle(ql.SimpleQuote(hazard)), ql.Actual365Fixed())
        )
        bond.setPricingEngine(ql.RiskyBondEngine(default_handle, 0.0, discount_handle))
    return bond.NPV()


def build_floating_risk_free_bond() -> dict[str, Any]:
    """Build a native QuantLib floating-rate risk-free bond fixture."""
    discount_rate = 0.04
    projection_rate = 0.043
    expected = {
        "dv01": central_difference(
            lambda shift: _quantlib_floating_bond_npv(discount_rate + shift, projection_rate + shift, None),
            0.0,
        ),
        "npv": _quantlib_floating_bond_npv(discount_rate, projection_rate, None),
    }
    reason = (
        "Vanilla floater validation target: price within 0.01 per 100 notional and parallel "
        "discount/projection DV01 within 0.5%."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="usd_floating_5y_risk_free_quantlib",
            domain="fixed_income.bond",
            description="QuantLib native five-year USD floating-rate risk-free bond.",
            product="floating_risk_free_bond",
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot([
            _bond_discount_curve("USD-OIS", discount_rate),
            {
                "type": "forward",
                "id": "USD-SOFR-3M",
                "base": VALUATION_DATE,
                "reset_lag": 2,
                "day_count": "Act360",
                "tenor": 0.25,
                "knot_points": [[0.0, projection_rate], [30.0, projection_rate]],
                "interp_style": "linear",
                "extrapolation": "flat_forward",
                "rate_calibration": None,
            },
        ]),
        "instrument": _floating_bond_spec(credit_curve_id=None),
        "expected": expected,
        "tolerances": {
            "npv": tolerance(0.01, reason),
            "dv01": {"rel": 0.005, "tolerance_reason": reason},
        },
    }


def build_floating_hazard_bond() -> dict[str, Any]:
    """Build a native QuantLib floating-rate zero-recovery hazard bond fixture."""
    fixture = build_floating_risk_free_bond()
    discount_rate = 0.04
    projection_rate = 0.043
    hazard_rate = 0.02
    expected = {
        "cs01": central_difference(
            lambda hazard: _quantlib_floating_bond_npv(discount_rate, projection_rate, hazard),
            hazard_rate,
        ),
        "dv01": central_difference(
            lambda shift: _quantlib_floating_bond_npv(discount_rate + shift, projection_rate + shift, hazard_rate),
            0.0,
        ),
        "npv": _quantlib_floating_bond_npv(discount_rate, projection_rate, hazard_rate),
    }
    reason = (
        "Credit floater validation target: price within 0.01 per 100, parallel DV01 within "
        "0.5%, and CS01 within 0.5%; zero recovery isolates survival weighting."
    )
    fixture["metadata"] = metadata(
        name="usd_floating_5y_hazard_quantlib",
        domain="fixed_income.bond",
        description="QuantLib native five-year USD floating-rate hazard bond with zero recovery.",
        product="floating_hazard_bond",
    )
    fixture["market"]["data"]["curves"].append(_hazard_curve("USD-CREDIT", hazard_rate))
    fixture["instrument"] = _floating_bond_spec(credit_curve_id="USD-CREDIT")
    fixture["instrument"]["instrument"]["spec"]["id"] = "USD-FLOATING-5Y-HAZARD-QUANTLIB"
    fixture["expected"] = expected
    fixture["tolerances"] = {
        "npv": tolerance(0.01, reason),
        "dv01": {"rel": 0.005, "tolerance_reason": reason},
        "cs01": {"rel": 0.005, "tolerance_reason": reason},
    }
    return fixture


def build_fixed_callable_oas_bond() -> dict[str, Any]:
    """Build a native QuantLib callable fixed-rate bond OAS fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    maturity_date = ql.Date(30, 4, 2034)
    call_date = ql.Date(30, 4, 2030)
    curve_rate = 0.04
    coupon = 0.055
    target_oas = 0.0125
    volatility = 0.01
    mean_reversion = 0.03
    day_count = ql.Thirty360(ql.Thirty360.BondBasis)
    curve = ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, curve_rate, day_count, ql.Continuous))
    schedule = ql.Schedule(
        evaluation_date,
        maturity_date,
        ql.Period(6, ql.Months),
        ql.WeekendsOnly(),
        ql.Following,
        ql.Following,
        ql.DateGeneration.Backward,
        False,
    )
    calls = ql.CallabilitySchedule()
    calls.append(
        ql.Callability(
            ql.BondPrice(100.0, ql.BondPrice.Clean),
            ql.Callability.Call,
            call_date,
        )
    )
    bond = ql.CallableFixedRateBond(
        2,
        100.0,
        schedule,
        [coupon],
        day_count,
        ql.Following,
        100.0,
        evaluation_date,
        calls,
    )
    model = ql.HullWhite(curve, mean_reversion, volatility)
    bond.setPricingEngine(ql.TreeCallableFixedRateBondEngine(model, 200, curve))
    clean_price = bond.cleanPriceOAS(
        target_oas,
        curve,
        day_count,
        ql.Continuous,
        ql.Semiannual,
    )

    def callable_npv(rate: float) -> float:
        bumped_curve = ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, rate, day_count, ql.Continuous))
        bumped_model = ql.HullWhite(bumped_curve, mean_reversion, volatility)
        bond.setPricingEngine(ql.TreeCallableFixedRateBondEngine(bumped_model, 200, bumped_curve))
        return bond.cleanPriceOAS(
            target_oas,
            bumped_curve,
            day_count,
            ql.Continuous,
            ql.Semiannual,
        )

    fixture = build_fixed_risk_free_bond()
    reason = (
        "Independent callable-bond validation target across 200-step Hull-White trees: OAS "
        "within 1.5bp and option-adjusted DV01 within 5%."
    )
    fixture["metadata"] = metadata(
        name="usd_fixed_callable_8y_oas_quantlib",
        domain="fixed_income.bond",
        description="QuantLib native eight-year USD callable fixed-rate bond OAS recovery.",
        product="fixed_callable_oas_bond",
    )
    fixture["market"] = market_snapshot([_bond_discount_curve("USD-OIS", curve_rate)])
    spec = fixture["instrument"]["instrument"]["spec"]
    spec["id"] = "USD-FIXED-CALLABLE-8Y-OAS-QUANTLIB"
    spec["maturity"] = "2034-04-30"
    spec["cashflow_spec"]["Fixed"]["rate"] = str(coupon)
    spec["pricing_overrides"] = {
        "quoted_clean_price": clean_price,
        "tree_steps": 200,
        "implied_volatility": volatility,
        "mean_reversion": mean_reversion,
        "tree_discount_curve_id": "USD-OIS",
        "oas_quote_compounding": "continuous",
    }
    spec["call_put"] = {
        "calls": [
            {
                "start_date": "2030-04-30",
                "end_date": "2030-04-30",
                "price_pct_of_par": 100.0,
            }
        ],
        "puts": [],
    }
    fixture["expected"] = {
        "dv01": central_difference(callable_npv, curve_rate),
        "oas": target_oas,
    }
    fixture["tolerances"] = {
        "dv01": {"rel": 0.05, "tolerance_reason": reason},
        "oas": tolerance(0.00015, reason),
    }
    return fixture
