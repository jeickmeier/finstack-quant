"""Independent credit and volatility teaching fixtures.

The credit track extends a fresh copy of the common book. Volatility lessons
extend the base book so they do not depend on the borrower loan introduced in
Part IV; the volatility capstone explicitly opts into the common book. A CLO
BBB holding is a separate tranche reporting sleeve because a native
StructuredCredit instrument prices the entire deal. These factories contain
inputs only; pricing, cashflow allocation, simulation and reconciliation belong
in visible lesson cells.

All market observations are deterministic teaching inputs, not market quotes.
Rates and volatilities are decimals; tranche attachment points are percentages.

>>> sorted(credit_extension())
['ANALYST-CONVERT', 'ANALYST-PIK-CALLABLE', 'ANALYST-REVOLVER', 'CDX-PAYER']
"""

from __future__ import annotations

from datetime import date, timedelta
import json
import math
from typing import Any

from finstack_quant.calibration import calibrate
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import (
    BaseCorrelationCurve,
    CreditIndexData,
    DiscountCurve,
    MarketContext,
    PriceCurve,
    ScalarTimeSeries,
    VolSurface,
)
from finstack_quant.core.money import Money
from finstack_quant.portfolio import Portfolio
from finstack_quant.valuations.instruments import (
    AssetPool,
    ConvertibleBond,
    StructuredCredit,
    Tranche,
    TrancheStructure,
)

from . import analyst_book as common
from .instrument_fixtures import cds_index, fixed_bond, instrument_envelope, revolver

AS_OF = common.AS_OF
TRACKS = ("credit", "vol")
_CORE_STAGES = ("base", "common")


def _resolve_core_stage(stage: str, core_stage: str | None) -> str:
    if stage not in TRACKS:
        raise ValueError(f"stage must be one of {TRACKS}; got {stage!r}")
    resolved = ("common" if stage == "credit" else "base") if core_stage is None else core_stage
    if resolved not in _CORE_STAGES:
        raise ValueError(f"core_stage must be one of {_CORE_STAGES}; got {resolved!r}")
    if stage == "credit" and resolved != "common":
        raise ValueError("credit track requires core_stage='common'")
    return resolved


def credit_extension() -> dict[str, dict[str, Any]]:
    """Return fresh complex-bond, revolver, index-option and convert envelopes.

    Returns:
        Four native holdings, each sized by contractual notional and quantity
        one. The independently priced CLO BBB sleeve is intentionally separate.

    Raises:
        ValueError: If a canonical example cannot be serialized.

    >>> credit_extension()["ANALYST-REVOLVER"]["instrument"]["spec"]["drawn_amount"]["amount"]
    '4000000'
    """
    _, facility = revolver(0)
    facility["spec"].update({
        "id": "ANALYST-REVOLVER",
        "commitment_amount": {"amount": "10000000", "currency": "USD"},
        "drawn_amount": {"amount": "4000000", "currency": "USD"},
        "commitment_date": AS_OF.isoformat(),
        "maturity": "2030-01-15",
        "recovery_rate": 0.4,
        "draw_repay_spec": {"deterministic": []},
    })
    facility["spec"]["base_rate_spec"]["floating"]["spread_bp"] = "325"
    facility["spec"]["fees"] = {
        "commitment_fee_tiers": [{"threshold": "0", "bp": "50"}],
        "usage_fee_tiers": [{"threshold": "0", "bp": "0"}, {"threshold": "0.5", "bp": "25"}],
        "facility_fee_bp": 0.0,
    }
    convert = json.loads(ConvertibleBond.example().to_json())
    convert["instrument"]["spec"].update({
        "id": "ANALYST-CONVERT",
        "notional": {"amount": "1000000", "currency": "USD"},
        "issue_date": AS_OF.isoformat(),
        "maturity": "2030-01-15",
        "discount_curve_id": "USD-OIS",
        "credit_curve_id": "CONVERT-RISKY",
        "underlying_equity_id": "CONVERT",
        "recovery_rate": 0.0,
    })
    convert["instrument"]["spec"]["conversion"]["ratio"] = 10_000.0
    return {
        "ANALYST-PIK-CALLABLE": complex_bond(),
        "ANALYST-REVOLVER": instrument_envelope(facility),
        "CDX-PAYER": credit_index_inputs()["CDX-PAYER"],
        "ANALYST-CONVERT": convert,
    }


def complex_bond() -> dict[str, Any]:
    """Return the eight-year, non-call-three split-PIK amortizer.

    Returns:
        A USD 1m bond with quarterly 8 percent coupon split 75 percent cash,
        25 percent PIK. Total coupon increases 25 bp after year four, retaining
        that split. Amortization pays 1 percent of original face each quarter
        after year five. Calls start at 103 after year three and decline to par.
        Short-rate volatility is 1 percent; quoted prices are added explicitly
        in the yield/OAS exercise, so the fixture retains a model valuation.

    Raises:
        ValueError: If canonical fixture inputs fail downstream validation.

    >>> complex_bond()["instrument"]["spec"]["maturity"]
    '2033-01-15'
    """
    _, bond = fixed_bond(0)
    spec = bond["spec"]
    schedule = spec["cashflow_spec"]["fixed"]
    schedule.pop("rate")
    schedule.update({
        "coupon_type": {"split": {"cash_pct": "0.75", "pik_pct": "0.25"}},
        "frequency": {"count": 3, "unit": "months"},
        "initial_rate": "0.08",
        "step_schedule": [["2029-01-15", "0.0825"]],
    })
    amort_dates = [
        f"{year}-{month:02d}-15" for year in (2030, 2031, 2032) for month in (1, 4, 7, 10) if (year, month) > (2030, 1)
    ]
    spec.update({
        "id": "ANALYST-PIK-CALLABLE",
        "issue_date": AS_OF.isoformat(),
        "maturity": "2033-01-15",
        "cashflow_spec": {
            "amortizing": {
                "base": {"step_up": schedule},
                "schedule": {
                    "custom_principal": {
                        "items": [[day, {"amount": "10000", "currency": "USD"}] for day in amort_dates]
                    }
                },
            }
        },
        "call_put": {
            "puts": [],
            "calls": [
                {
                    "start_date": f"{year}-01-15",
                    "end_date": f"{year + 1}-01-{15 if year == 2032 else 14}",
                    "price_pct_of_par": price,
                }
                for year, price in ((2028, 103.0), (2029, 102.0), (2030, 101.0), (2031, 100.0), (2032, 100.0))
            ],
        },
        "instrument_pricing_overrides": {"market_quotes": {"implied_volatility": 0.01}},
    })
    return instrument_envelope(bond)


def structured_index_inputs() -> dict[str, dict[str, Any]]:
    """Return the index and tranche inputs introduced in lesson 2.8.

    Returns:
        Canonical envelopes for a 125-name index and its 0--3 percent tranche.

    Raises:
        ValueError: If the underlying canonical fixture is invalid.

    >>> structured_index_inputs()["CDX-0-3"]["instrument"]["spec"]["detach_pct"]
    3.0
    """
    _, index = cds_index(0)
    index["spec"].update({"id": "ANALYST-CDX", "series": 43, "notional": {"amount": "1000000", "currency": "USD"}})
    index["spec"]["premium"].update({"start": "2024-12-20", "end": "2029-12-20", "spread_bp": "100"})
    tranche = {
        "type": "cds_tranche",
        "spec": {
            "id": "CDX-0-3",
            "accumulated_loss": 0.0,
            "attach_pct": 0.0,
            "detach_pct": 3.0,
            "attributes": {},
            "business_day_convention": "following",
            "calendar_id": "nyse",
            "credit_index_id": "CDX-INDEX-DATA",
            "day_count": "act_360",
            "discount_curve_id": "USD-OIS",
            "effective_date": "2024-12-20",
            "frequency": {"count": 3, "unit": "months"},
            "index_name": "CDX.NA.IG",
            "maturity": "2029-12-20",
            "notional": {"amount": "1000000", "currency": "USD"},
            "running_coupon_bp": 500.0,
            "series": 43,
            "side": "buy_protection",
            "standard_imm_dates": True,
            "upfront": None,
        },
    }
    return {
        "ANALYST-CDX": instrument_envelope(index),
        "CDX-0-3": instrument_envelope(tranche),
    }


def credit_index_inputs() -> dict[str, dict[str, Any]]:
    """Extend the structured index inputs with the credit-track payer option.

    Returns:
        Fresh index and tranche envelopes plus a non-knockout payer option.
        Option spread strike and coupon are decimal rates.

    Raises:
        ValueError: If an underlying canonical fixture is invalid.

    >>> credit_index_inputs()["CDX-PAYER"]["instrument"]["spec"]["underlying_is_index"]
    True
    """
    inputs = structured_index_inputs()
    option = {
        "type": "cds_option",
        "spec": {
            "id": "CDX-PAYER",
            "attributes": {},
            "cds_maturity": "2029-12-20",
            "credit_curve_id": "CDX-HAZ",
            "discount_curve_id": "USD-OIS",
            "exercise_style": "european",
            "expiry": "2025-06-20",
            "index_factor": 1.0,
            "knockout": False,
            "notional": {"amount": "1000000", "currency": "USD"},
            "option_type": "call",
            "protection_start_convention": "spot",
            "recovery_rate": 0.4,
            "settlement": "cash",
            "strike": {"spread": "0.01"},
            "underlying_cds_coupon": "0.01",
            "underlying_convention": "isda_na",
            "underlying_is_index": True,
            "vol_surface_id": "CDX-OPTION-VOL",
        },
    }
    inputs["CDX-PAYER"] = instrument_envelope(option)
    return inputs


def credit_calibration_envelope(as_of: date = AS_OF) -> dict[str, Any]:
    """Return quote-backed index hazard calibration inputs.

    Args:
        as_of: Curve base date. Tenor quotes retain the same synthetic levels.

    Returns:
        Canonical calibration envelope with 1/3/5/7/10-year CDX par spreads
        of 120 basis points and the common USD discount quotes. The resulting
        hazard curve retains the lossless recipe needed for quote-space CS01.

    Raises:
        OSError: If the common calibration example cannot be read.

    >>> credit_calibration_envelope()["plan"]["steps"][-1]["curve_id"]
    'CDX-HAZ'
    """
    envelope = common.calibration_envelope("base", as_of)
    envelope["plan"]["id"] = "analyst-index-market"
    for step in envelope["plan"]["steps"]:
        if step["kind"] == "hazard":
            step.update({"id": "CDX-HAZ", "curve_id": "CDX-HAZ", "entity": "CDX.NA.IG"})
    for quote in envelope["market_data"]:
        if quote["kind"] == "cds_quote":
            quote.update({"entity": "CDX.NA.IG", "spread_bp": 120.0})
    return envelope


def stochastic_revolver(*, correlation: float = 0.0, num_paths: int = 4096) -> dict[str, Any]:
    """Return a seeded utilization/credit Monte Carlo revolver input.

    Args:
        correlation: Utilization-credit Brownian correlation in [-1, 1].
        num_paths: Positive Monte Carlo path count; default 4096.

    Returns:
        Fresh canonical facility with observed utilization 40 percent and
        utilization target 55 percent. Price with ``monte_carlo_three_factor``.

    Raises:
        ValueError: If the correlation or path count is invalid.

    >>> stochastic_revolver()["instrument"]["spec"]["draw_repay_spec"]["stochastic"]["seed"]
    20250115
    """
    if not -1.0 <= correlation <= 1.0 or num_paths <= 0:
        raise ValueError("correlation must be in [-1,1] and num_paths must be positive")
    payload = credit_extension()["ANALYST-REVOLVER"]
    payload["instrument"]["spec"]["draw_repay_spec"] = {
        "stochastic": {
            "utilization_process": {"mean_reverting": {"target_rate": 0.55, "speed": 1.0, "volatility": 0.10}},
            "num_paths": num_paths,
            "seed": 20250115,
            "antithetic": True,
            "use_sobol_qmc": False,
            "mc_config": {
                "correlation_matrix": None,
                "credit_spread_process": {
                    "market_anchored": {
                        "credit_curve_id": "ACME-HZD",
                        "kappa": 1.0,
                        "implied_vol": 0.40,
                        "tenor_years": 5.0,
                    }
                },
                "interest_rate_process": None,
                "util_credit_corr": correlation,
            },
        }
    }
    return payload


def convertible_limit() -> dict[str, Any]:
    """Return the European zero-coupon convertible analytical-limit fixture.

    Returns:
        A USD 1,000 face bond, ratio ten, no credit spread, dividends or calls,
        convertible only at its one-year maturity. Its payoff is max(1000,10S).

    Raises:
        ValueError: If the canonical convertible example cannot be serialized.

    >>> convertible_limit()["instrument"]["spec"]["conversion"]["ratio"]
    10.0
    """
    payload = credit_extension()["ANALYST-CONVERT"]
    spec = payload["instrument"]["spec"]
    spec.update({
        "id": "CONVERT-LIMIT",
        "notional": {"amount": "1000", "currency": "USD"},
        "maturity": "2026-01-15",
        "credit_curve_id": None,
        "discount_curve_id": "CONVERT-LIMIT-DF",
        "fixed_coupon": None,
        "floating_coupon": None,
        "call_put": None,
        "underlying_equity_id": "CONVERT-LIMIT",
    })
    spec["conversion"].update({"ratio": 10.0, "policy": {"window": {"start": "2026-01-15", "end": "2026-01-15"}}})
    return payload


def clo_deal(*, one_period: bool = False, oc_trigger: float | None = None) -> dict[str, Any]:
    """Return a real cashflow-producing CLO backed by five positive pool assets.

    Args:
        one_period: If true, all collateral and notes mature on 2025-04-15;
            otherwise they mature on 2030-01-15. Both versions have zero CPR,
            zero CDR and no fees, making the base waterfall transparent.
        oc_trigger: Optional senior coverage ratio, such as 1.30. This is a
            ratio (1.30 means 130 percent), not a percentage-point input.

    Returns:
        Fresh canonical whole-deal envelope with 100m assets, 80m AAA, 15m
        BBB and 5m equity. Notes attach at 0/80/95/100 percentage points.

    Raises:
        ValueError: If native pool, tranche or deal validation fails.

    >>> len(clo_deal()["instrument"]["spec"]["pool"]["assets"])
    5
    """
    maturity = date(2025, 4, 15) if one_period else date(2030, 1, 15)
    assets = [
        {
            "id": f"CLO-ASSET-{i + 1}",
            "asset_type": {"type": "first_lien_loan", "industry": industry},
            "balance": {"amount": "20000000", "currency": "USD"},
            "rate": 0.08,
            "spread_bp": None,
            "index_id": None,
            "maturity": maturity.isoformat(),
            "credit_quality": None,
            "industry": industry,
            "obligor_id": f"POOL-OBLIGOR-{i + 1}",
            "is_defaulted": False,
            "recovery_amount": None,
            "purchase_price": None,
            "acquisition_date": AS_OF.isoformat(),
            "day_count": "act_360",
            "smm_override": None,
            "mdr_override": None,
            "contractual_payment": None,
        }
        for i, industry in enumerate(("software", "healthcare", "industrials", "consumer", "services"))
    ]
    pool = AssetPool("ANALYST-CLO-POOL", "clo", "USD").with_assets(assets)
    notes = []
    for iid, seniority, amount, coupon, attach, detach in (
        ("AAA", "senior", 80_000_000.0, 0.04, 0.0, 80.0),
        ("BBB", "mezzanine", 15_000_000.0, 0.06, 80.0, 95.0),
        ("EQUITY", "equity", 5_000_000.0, 0.0, 95.0, 100.0),
    ):
        notes.append(
            Tranche
            .builder()
            .id(iid)
            .seniority(seniority)
            .original_balance(Money(amount, "USD"))
            .coupon_fixed(coupon)
            .maturity(maturity)
            .day_count(DayCount.ACT_360)
            .frequency(Tenor.quarterly())
            .attachment_point(attach)
            .detachment_point(detach)
            .build()
        )
    deal = StructuredCredit.new_clo("ANALYST-CLO", pool, TrancheStructure(notes), AS_OF, maturity, "USD-OIS")
    payload = json.loads(deal.to_json())
    spec = payload["instrument"]["spec"]
    spec.update({
        "first_payment_date": "2025-04-15",
        "payment_calendar_id": "weekends_only",
        "payment_business_day_convention": "unadjusted",
        "fees": None,
        "coverage_triggers": []
        if oc_trigger is None
        else [
            {
                "id": "OC_AAA",
                "tranche_id": "AAA",
                "kind": "oc",
                "trigger_level": oc_trigger,
                "action": "pay_down_senior",
            }
        ],
    })
    spec.update({
        "prepayment_spec": {"cpr": 0.0, "curve": None},
        "default_spec": {"cdr": 0.0, "curve": None},
        "recovery_spec": {"rate": 0.4, "recovery_lag": 0},
    })
    return payload


def clo_bbb_holding() -> dict[str, Any]:
    """Return inputs for the independently priced BBB reporting sleeve.

    Returns:
        A plain record containing deal envelope, tranche identifier, quantity
        one and USD currency; it is not a native Portfolio position.

    Raises:
        ValueError: If the supported CLO fixture cannot be built.

    >>> clo_bbb_holding()["tranche_id"]
    'BBB'
    """
    return {"holding_id": "CLO-BBB", "deal": clo_deal(), "tranche_id": "BBB", "quantity": 1.0, "currency": "USD"}


def futures_inputs() -> dict[str, dict[str, Any]]:
    """Return a two-bond delivery basket and one listed futures option.

    Returns:
        Envelopes for both deliverable bonds, the selected first CTD future,
        and a European premium-paid SPX futures call. Conversion factors are
        teaching inputs; futures prices are quoted per 100 face.

    Raises:
        ValueError: If an underlying canonical bond fixture is invalid.

    >>> futures_inputs()["UST-FUTURE"]["instrument"]["spec"]["ctd_bond_id"]
    'UST-DELIVERABLE-A'
    """
    raw = {}
    for iid, coupon, maturity in (
        ("UST-DELIVERABLE-A", "0.025", "2032-02-15"),
        ("UST-DELIVERABLE-B", "0.045", "2034-02-15"),
    ):
        _, bond = fixed_bond(0)
        bond["spec"].update({
            "id": iid,
            "notional": {"amount": "100000", "currency": "USD"},
            "issue_date": "2024-02-15",
            "maturity": maturity,
            "discount_curve_id": "USD-TREASURY",
        })
        bond["spec"]["cashflow_spec"]["fixed"]["rate"] = coupon
        raw[iid] = instrument_envelope(bond)
    future = {
        "type": "bond_future",
        "spec": {
            "id": "UST-FUTURE",
            "attributes": {},
            "contract_specs": {
                "calendar_id": "nyse",
                "contract_size": 100000.0,
                "repo_day_count": "act_360",
                "settlement_days": 2,
                "standard_coupon": 0.06,
                "standard_maturity_years": 10.0,
                "tick_size": 0.015625,
                "tick_value": 15.625,
            },
            "ctd_bond_id": "UST-DELIVERABLE-A",
            "ctd_bond": raw["UST-DELIVERABLE-A"]["instrument"]["spec"],
            "deliverable_basket": [
                {"bond_id": "UST-DELIVERABLE-A", "conversion_factor": 0.845},
                {"bond_id": "UST-DELIVERABLE-B", "conversion_factor": 0.95},
            ],
            "delivery_end": "2025-09-30",
            "delivery_start": "2025-09-22",
            "discount_curve_id": "USD-TREASURY",
            "repo_curve_id": "USD-REPO",
            "expiry": "2025-09-19",
            "notional": {"amount": "1000000", "currency": "USD"},
            "position": "long",
            "quoted_price": 108.0,
        },
    }
    listed = {
        "type": "equity_future_option",
        "spec": {
            "id": "SPX-FUTURE-CALL",
            "attributes": {},
            "terms": {
                "contracts": 2.0,
                "currency": "USD",
                "day_count": "act_365f",
                "discount_curve_id": "USD-OIS",
                "exercise_style": "european",
                "expiry": "2025-09-19",
                "futures_price": 5300.0,
                "model": "black76",
                "multiplier": 50.0,
                "option_type": "call",
                "position": "long",
                "premium_style": "premium_paid",
                "settlement": {"payment_date": "2025-09-19", "type": "cash"},
                "strike": 5300.0,
                "underlying": "SPX-SEP25",
                "volatility": 0.20,
            },
        },
    }
    raw.update({"UST-FUTURE": instrument_envelope(future), "SPX-FUTURE-CALL": instrument_envelope(listed)})
    return raw


def variance_inputs() -> dict[str, dict[str, Any]]:
    """Return a newly originated variance swap and VIX future/option inputs.

    Returns:
        Canonical envelopes. Variance strike 0.04 means 20 percent volatility
        squared; VIX future quotes are index points and its multiplier is 1000.

    Raises:
        ValueError: If fixture inputs fail downstream native validation.

    >>> variance_inputs()["SPX-VARIANCE"]["instrument"]["spec"]["strike_variance"]
    0.04
    """
    swap = {
        "type": "variance_swap",
        "spec": {
            "id": "SPX-VARIANCE",
            "attributes": {},
            "day_count": "act_365f",
            "discount_curve_id": "USD-OIS",
            "maturity": "2026-01-15",
            "notional": {"amount": "1000000", "currency": "USD"},
            "observation_business_day_convention": "following",
            "observation_calendar_id": "weekends_only",
            "observation_end_of_month": False,
            "observation_frequency": {"count": 1, "unit": "days"},
            "price_series_policy": "adjusted",
            "realized_var_method": "close_to_close",
            "settlement_date": None,
            "side": "receive",
            "start_date": AS_OF.isoformat(),
            "strike_variance": 0.04,
            "underlying_ticker": "SPX",
            "open_series_id": "SPX-OPEN",
            "high_series_id": "SPX-HIGH",
            "low_series_id": "SPX-LOW",
            "close_series_id": "SPX-CLOSE",
        },
    }
    future = {
        "type": "volatility_index_future",
        "spec": {
            "id": "VIX-FUTURE",
            "attributes": {},
            "contract_specs": {"index_id": "VIX", "multiplier": 1000.0, "tick_size": 0.05, "tick_value": 50.0},
            "discount_curve_id": "USD-OIS",
            "expiry": "2025-03-19",
            "settlement_date": "2025-03-19",
            "vol_index_curve_id": "VIX",
            "notional": {"amount": "100000", "currency": "USD"},
            "quoted_price": 21.5,
            "position": "long",
            "settlement_fixing": None,
        },
    }
    option = futures_inputs()["SPX-FUTURE-CALL"]
    option["instrument"]["type"] = "volatility_index_future_option"
    option["instrument"]["spec"]["id"] = "VIX-CALL"
    option["instrument"]["spec"]["terms"].update({
        "contracts": 10.0,
        "multiplier": 100.0,
        "underlying": "VIX-MAR25",
        "futures_price": 22.0,
        "strike": 22.0,
        "volatility": 0.60,
        "expiry": "2025-03-19",
        "settlement": {"payment_date": "2025-03-19", "type": "cash"},
    })
    return {"SPX-VARIANCE": instrument_envelope(swap), "VIX-FUTURE": instrument_envelope(future), "VIX-CALL": option}


def commodity_inputs() -> dict[str, dict[str, Any]]:
    """Return Kirk spread, Asian and single-underlying commodity options.

    Returns:
        Envelopes sharing WTI inputs; the spread uses Brent as its second leg.
        Prices and strikes are USD per barrel, quantity is barrels, and model
        selection remains explicit at the notebook pricing call.

    Raises:
        ValueError: If fixture inputs fail downstream native validation.

    >>> commodity_inputs()["WTI-BRENT-SPREAD"]["instrument"]["spec"]["leg2_forward_curve_id"]
    'BRENT-FORWARD'
    """
    spread = {
        "type": "commodity_spread_option",
        "spec": {
            "id": "WTI-BRENT-SPREAD",
            "attributes": {},
            "correlation": 0.85,
            "currency": "USD",
            "day_count": "act_365f",
            "discount_curve_id": "USD-OIS",
            "expiry": "2025-09-15",
            "leg1_forward_curve_id": "WTI-FORWARD",
            "leg1_vol_surface_id": "WTI-VOL",
            "leg2_forward_curve_id": "BRENT-FORWARD",
            "leg2_vol_surface_id": "BRENT-VOL",
            "notional": 10000.0,
            "option_type": "call",
            "strike": -3.0,
        },
    }
    vanilla = {
        "type": "commodity_option",
        "spec": {
            "id": "WTI-CALL",
            "attributes": {},
            "commodity_type": "Energy",
            "currency": "USD",
            "day_count": "act_365f",
            "discount_curve_id": "USD-OIS",
            "exercise_style": "european",
            "expiry": "2025-09-15",
            "forward_curve_id": "WTI-FORWARD",
            "multiplier": 1.0,
            "option_type": "call",
            "quantity": 1000.0,
            "settlement": "cash",
            "strike": 75.0,
            "ticker": "CL",
            "unit": "BBL",
            "vol_surface_id": "WTI-VOL",
        },
    }
    asian = {
        "type": "commodity_asian_option",
        "spec": {
            "id": "WTI-ASIAN",
            "attributes": {},
            "averaging_method": "arithmetic",
            "commodity_type": "Energy",
            "currency": "USD",
            "day_count": "act_365f",
            "discount_curve_id": "USD-OIS",
            "expiry": "2025-07-02",
            "fixing_dates": ["2025-01-31", "2025-02-28", "2025-03-31", "2025-04-30", "2025-05-30", "2025-06-30"],
            "forward_curve_id": "WTI-FORWARD",
            "option_type": "call",
            "quantity": 1000.0,
            "realized_fixings": [],
            "strike": 75.0,
            "ticker": "CL",
            "unit": "BBL",
            "vol_surface_id": "WTI-VOL",
        },
    }
    forward = {
        "type": "commodity_forward",
        "spec": {
            "id": "WTI-FORWARD-CONTRACT",
            "attributes": {},
            "commodity_type": "Energy",
            "currency": "USD",
            "discount_curve_id": "USD-OIS",
            "forward_curve_id": "WTI-FORWARD",
            "maturity": "2025-09-15",
            "multiplier": 1.0,
            "position": "long",
            "quantity": 1000.0,
            "settlement": "cash",
            "ticker": "CL",
            "unit": "BBL",
            "contract_price": 75.0,
        },
    }
    future = {
        "type": "commodity_future",
        "spec": {
            "id": "WTI-FUTURE",
            "attributes": {},
            "price_curve_id": "WTI-FORWARD",
            "underlying": "CL",
            "settlement": {"type": "single", "observation_date": "2025-09-15", "realized_price": None},
            "terms": {
                "contracts": 1.0,
                "currency": "USD",
                "entry_price": 75.0,
                "last_trading_date": "2025-09-15",
                "multiplier": 1000.0,
                "position": "long",
                "settlement": {"type": "cash"},
                "settlement_date": "2025-09-15",
            },
        },
    }
    gas_swap = {
        "type": "commodity_swap",
        "spec": {
            "id": "NG-SWAP",
            "attributes": {},
            "business_day_convention": "modified_following",
            "commodity_type": "Energy",
            "currency": "USD",
            "discount_curve_id": "USD-OIS",
            "fixed_price": "3.2",
            "floating_index_id": "NG-FORWARD",
            "frequency": {"count": 1, "unit": "months"},
            "maturity": "2025-12-15",
            "quantity": 10000.0,
            "realized_fixings": [],
            "side": "pay",
            "start_date": AS_OF.isoformat(),
            "ticker": "NG",
            "unit": "MMBTU",
        },
    }
    return {
        item["spec"]["id"]: instrument_envelope(item) for item in (spread, vanilla, asian, forward, future, gas_swap)
    }


def vol_extension() -> dict[str, dict[str, Any]]:
    """Return the independent native volatility-track holdings.

    Returns:
        Bond future, variance swap and WTI-Brent spread option envelopes, with
        no credit-track extension or CLO dependency.

    Raises:
        ValueError: If underlying canonical fixture inputs are invalid.

    >>> sorted(vol_extension())
    ['SPX-VARIANCE', 'UST-FUTURE', 'WTI-BRENT-SPREAD']
    """
    return {
        "UST-FUTURE": futures_inputs()["UST-FUTURE"],
        "SPX-VARIANCE": variance_inputs()["SPX-VARIANCE"],
        "WTI-BRENT-SPREAD": commodity_inputs()["WTI-BRENT-SPREAD"],
    }


def book_spec(stage: str, *, core_stage: str | None = None) -> dict[str, Any]:
    """Extend a fresh base or common native portfolio with one track.

    Args:
        stage: Exactly ``credit`` or ``vol``.
        core_stage: Underlying stage from :mod:`analyst_book`. Credit always
            uses ``common``. Volatility defaults to ``base``; pass ``common``
            explicitly for the capstone after the borrower loan is introduced.

    Returns:
        Native PortfolioSpec dictionary; ``tags.stage`` identifies the track.
        The credit BBB holding remains in ``clo_bbb_holding``.

    Raises:
        ValueError: If ``stage`` is unknown or native fixture creation fails.

    >>> "BORROWER-TL" in {row["instrument_id"] for row in book_spec("vol")["positions"]}
    False
    >>> "BORROWER-TL" in {row["instrument_id"] for row in book_spec("vol", core_stage="common")["positions"]}
    True
    """
    resolved_core_stage = _resolve_core_stage(stage, core_stage)
    spec = common.book_spec(resolved_core_stage)
    extension = credit_extension() if stage == "credit" else vol_extension()
    spec.update({"id": f"ANALYST-{stage.upper()}", "tags": {"stage": stage}})
    spec["positions"].extend(
        {
            "position_id": iid,
            "entity_id": "FUND",
            "instrument_id": iid,
            "instrument_spec": envelope["instrument"],
            "quantity": 1.0,
            "unit": "units",
        }
        for iid, envelope in extension.items()
    )
    return spec


def build_book(stage: str, *, core_stage: str | None = None) -> Portfolio:
    """Build a fresh native credit or volatility portfolio.

    Args:
        stage: Exactly ``credit`` or ``vol``.
        core_stage: Underlying stage from :mod:`analyst_book`. Credit always
            uses ``common``. Volatility defaults to ``base``; pass ``common``
            explicitly for the capstone.

    Returns:
        Native Portfolio; credit has a separately reported BBB sleeve.

    Raises:
        ValueError: If the stage or canonical instrument inputs are invalid.

    >>> len(build_book("credit").position_ids)
    12
    """
    return Portfolio.from_spec(json.dumps(book_spec(stage, core_stage=core_stage)))


def ohlc_observations() -> dict[str, list[tuple[date, float]]]:
    """Return aligned, positive synthetic weekday OHLC observations.

    Returns:
        Four series IDs mapped to dated prices from December 2024 through
        AS_OF, with high >= max(open, close) and low <= min(open, close).

    >>> sorted(ohlc_observations())
    ['SPX-CLOSE', 'SPX-HIGH', 'SPX-LOW', 'SPX-OPEN']
    """
    rows: dict[str, list[tuple[date, float]]] = {key: [] for key in ("SPX-OPEN", "SPX-HIGH", "SPX-LOW", "SPX-CLOSE")}
    day, last, i = date(2024, 12, 2), 5100.0, 0
    while day <= AS_OF:
        if day.weekday() < 5:
            opened = last * math.exp(0.001 * math.sin(i))
            closed = opened * math.exp(0.008 * math.cos(i * 0.7))
            for key, value in (
                ("SPX-OPEN", opened),
                ("SPX-HIGH", max(opened, closed) * 1.004),
                ("SPX-LOW", min(opened, closed) * 0.996),
                ("SPX-CLOSE", closed),
            ):
                rows[key].append((day, value))
            last, i = closed, i + 1
        day += timedelta(days=1)
    return rows


def build_structured_market(as_of: date = AS_OF, *, core_stage: str = "base") -> MarketContext:
    """Build the structured-products market introduced in lesson 2.8.

    Args:
        as_of: Discount and hazard curve base date.
        core_stage: Underlying stage from :mod:`analyst_book`; use ``base`` in
            lesson 2.8 and ``common`` only after the term loan is introduced.

    Returns:
        Fresh base or common market with calibrated index hazard, base
        correlation and index state.

    Raises:
        ValueError: If ``core_stage`` is not ``base`` or ``common``.
        RuntimeError: If the shared market calibration fails.
        OSError: If the shared calibration fixture cannot be read.

    >>> build_structured_market().get_credit_index("CDX-INDEX-DATA").num_constituents
    125
    """
    if core_stage not in _CORE_STAGES:
        raise ValueError(f"core_stage must be one of {_CORE_STAGES}; got {core_stage!r}")
    market = common.build_market(core_stage, as_of)
    hazard = calibrate(credit_calibration_envelope(as_of)).market.get_hazard("CDX-HAZ")
    market.insert(hazard)
    correlation = BaseCorrelationCurve(
        "CDX-BASE-CORR",
        [(3.0, 0.25), (7.0, 0.25), (15.0, 0.25), (100.0, 0.25)],
    )
    market.insert(correlation)
    market.insert_credit_index("CDX-INDEX-DATA", CreditIndexData(125, 0.4, hazard, correlation))
    return market


def build_market(stage: str, as_of: date = AS_OF, *, core_stage: str | None = None) -> MarketContext:
    """Build a fresh market sufficient for a track and its proof exercises.

    Args:
        stage: Exactly ``credit`` or ``vol``.
        as_of: Discount, forward and hazard curve base date. Historical OHLC
            observations remain the explicitly dated teaching dataset.
        core_stage: Underlying stage from :mod:`analyst_book`. Credit always
            uses ``common``. Volatility defaults to ``base``; pass ``common``
            explicitly for the capstone.

    Returns:
        Native MarketContext with track-specific identifiers. No track market
        depends on the other track's extension.

    Raises:
        ValueError: If the stage or native market input is invalid.
        RuntimeError: If common market calibration fails.
        OSError: If the common calibration fixture cannot be read.

    >>> build_market("vol").get_price_curve("WTI-FORWARD").id
    'WTI-FORWARD'
    """
    resolved_core_stage = _resolve_core_stage(stage, core_stage)
    if stage == "credit":
        market = build_structured_market(as_of, core_stage=resolved_core_stage)
        market.insert(
            VolSurface(
                "CDX-OPTION-VOL",
                [0.25, 0.5, 1.0, 2.0],
                [0.0025, 0.01, 0.03],
                [[0.40] * 3] * 4,
            )
        )
        for iid, rate in (("CONVERT-RISKY", 0.065), ("CONVERT-LIMIT-DF", 0.04)):
            market.insert(
                DiscountCurve(
                    iid, as_of, [(t, math.exp(-rate * t)) for t in (0.0, 1.0, 5.0, 10.0)], day_count="act_365f"
                )
            )
        for underlying in ("CONVERT", "CONVERT-LIMIT"):
            market.insert_price(underlying, 100.0, "USD")
            market.insert_price(f"{underlying}-VOL", 0.25)
            market.insert_price(f"{underlying}-DIVYIELD", 0.0)
    else:
        market = common.build_market(resolved_core_stage, as_of)
        for iid, rate in (("USD-TREASURY", 0.04), ("USD-REPO", 0.042)):
            market.insert(
                DiscountCurve(
                    iid, as_of, [(t, math.exp(-rate * t)) for t in (0.0, 1.0, 5.0, 10.0, 20.0)], day_count="act_365f"
                )
            )
        market.insert(PriceCurve("VIX", as_of, [(0.0, 20.0), (0.25, 22.0), (0.5, 23.0), (1.0, 24.0)], kind="vol_index"))
        for ticker, prices, sigma in (
            ("WTI", (75.0, 74.0, 72.0, 70.0), 0.30),
            ("BRENT", (78.0, 77.0, 75.0, 73.0), 0.28),
            ("NG", (3.0, 3.3, 3.1, 3.5), 0.45),
        ):
            market.insert(PriceCurve(f"{ticker}-FORWARD", as_of, list(zip((0.0, 0.5, 1.0, 2.0), prices, strict=True))))
            strikes = [1.0, 3.0, 6.0] if ticker == "NG" else [20.0, 75.0, 150.0]
            market.insert(VolSurface(f"{ticker}-VOL", [0.25, 0.5, 1.0, 2.0], strikes, [[sigma] * 3] * 4))
        market.insert_price("SPX", 5200.0, "USD")
        market.insert_price("SPX-DIVYIELD", 0.015)
        strikes = [5200.0 * math.exp(-3.0 + i * 0.05) for i in range(121)]
        market.insert(VolSurface("SPX_VOL", [0.25, 0.5, 1.0, 2.0], strikes, [[0.20] * len(strikes)] * 4))
        for iid, observations in ohlc_observations().items():
            market.insert_series(ScalarTimeSeries(iid, observations))
    return market
