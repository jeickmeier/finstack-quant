"""Independent Bachelier reference for the normal swaption calibration fixture.

Run with ``uv run --no-sync python scripts/golden/normal_swaption_reference.py``;
add ``--write`` to refresh this internal formula fixture's reference values.
Curve calibration, underlying forward projection, and cube evaluation are canonical
Rust inputs. This script independently edits the authored rate quotes and recalibrates
the linked rate steps, holding the original vol cube fixed. It computes the fixed-leg
annuity, Bachelier price/vega, and central quote DV01 without the library pricer.
This is a fixture-specific reference, not a general swaption pricing implementation.
Formula: QuantLib ql/pricingengines/blackformula.cpp, bachelierBlackFormula.
https://github.com/lballabio/QuantLib/blob/master/ql/pricingengines/blackformula.cpp
"""

import argparse
import copy
from datetime import date, timedelta
import json
import math
from pathlib import Path

from finstack_quant.calibration import calibrate
from finstack_quant.core.market_data import MarketContext
from finstack_quant.models.volatility import get_cube_vol_clamped
from finstack_quant.valuations.instruments import Swaption, price_instrument

root = Path(__file__).resolve().parents[2]
fixture_path = (
    root
    / "finstack-quant/valuations/tests/golden/data/pricing/regression_goldens/swaption/usd_swaption_normal_vol_self_test.json"
)
f = json.loads(fixture_path.read_text())
snapshot = json.loads(calibrate(json.dumps(f["market"]["envelope"])).market.to_json())
market = MarketContext.from_json(json.dumps(snapshot))
option = Swaption.from_json(json.dumps(f["instrument"]))
as_of = date.fromisoformat(f["metadata"]["valuation_date"])
spec = f["instrument"]["instrument"]["spec"]
if (spec["settlement"], spec["cash_settlement_method"]) != ("cash", "isda_par_par"):
    raise ValueError("reference requires fixture cash ISDA par-par settlement")
if (spec["option_type"], spec["vol_model"]) != ("put", "normal"):
    raise ValueError("reference requires fixture normal receiver")
leg = spec["underlying_fixed_leg"]
if (leg["start"], leg["end"], leg["day_count"], leg["payment_lag_days"]) != (
    "2027-05-05",
    "2032-05-05",
    "act_360",
    2,
):
    raise ValueError("reference requires fixture annual May fixed leg")
expiry = date.fromisoformat(spec["expiry"])
t = (expiry - as_of).days / 365
strike = float(spec["underlying_fixed_leg"]["rate"])
notional = float(spec["notional"]["amount"])


# The fixture has no USNY holidays around its May 5 coupons or payment lags.
# Accrual dates stay contractual; only payment dates adjust, then add two days.
def following(d):
    """Adjust one known May fixture date to the following weekday."""
    while d.weekday() >= 5:
        d += timedelta(days=1)
    return d


def payment(d):
    """Apply the fixture payment adjustment and two-business-day lag."""
    d = following(d)
    for _ in range(2):
        d = following(d + timedelta(days=1))
    return d


periods = [(date(y - 1, 5, 5), date(y, 5, 5), payment(date(y, 5, 5))) for y in range(2028, 2033)]
sigma = get_cube_vol_clamped(market.get_vol_cube(spec["vol_surface_id"]), t, 5, strike, "normal")


def value(m):
    """Calculate independent Bachelier price, vega, forward, and annuity."""
    disc = m.get_discount("USD-OIS")
    annuity = sum((end - start).days / 360 * disc.df_between_dates(as_of, pay) for start, end, pay in periods)
    forward = option.forward_swap_rate(m, as_of)
    stddev = sigma * math.sqrt(t)
    d = (forward - strike) / stddev
    phi = math.exp(-d * d / 2) / math.sqrt(2 * math.pi)
    put = (strike - forward) * 0.5 * math.erfc(d / math.sqrt(2)) + stddev * phi
    vega = notional * annuity * math.sqrt(t) * phi * 0.01
    return notional * annuity * put, vega, forward, annuity


base = value(market)


def bumped(bp):
    """Shift authored deposit, swap and FRA rates; rebuild only the linked rate steps."""
    envelope = copy.deepcopy(f["market"]["envelope"])
    plan = envelope["plan"]
    plan["steps"] = [step for step in plan["steps"] if step["kind"] in {"discount", "forward"}]
    if [step["kind"] for step in plan["steps"]] != ["discount", "forward"]:
        raise ValueError("reference requires the linked discount and forward steps in order")
    discount, forward = plan["steps"]
    if forward["discount_curve_id"] != discount["curve_id"]:
        raise ValueError("reference requires the projection step to use the shocked discount curve")
    plan["quote_sets"] = {step["quote_set"]: plan["quote_sets"][step["quote_set"]] for step in plan["steps"]}
    rate_ids = {quote_id for quote_set in plan["quote_sets"].values() for quote_id in quote_set}
    quotes = [quote for quote in envelope["market_data"] if quote.get("id") in rate_ids]
    if len(quotes) != len(rate_ids) or {quote["id"] for quote in quotes} != rate_ids:
        raise ValueError("reference requires each authored rate quote exactly once")
    for quote in quotes:
        if quote["kind"] != "rate_quote" or quote["type"] not in {"deposit", "swap", "fra"}:
            raise ValueError("reference requires decimal deposit, swap and FRA rates")
        quote["rate"] += bp * 1e-4
    envelope["market_data"] = quotes

    shifted_rates = json.loads(calibrate(json.dumps(envelope)).market.to_json())["curves"]
    shifted = copy.deepcopy(snapshot)
    if {(curve["type"], curve["id"]) for curve in shifted_rates} != {
        (curve["type"], curve["id"]) for curve in shifted["curves"]
    }:
        raise ValueError("reference requires the shocked market to replace exactly the original rate curves")
    shifted["curves"] = shifted_rates
    return MarketContext.from_json(json.dumps(shifted))


if not all(
    math.isclose(replayed, original, rel_tol=1e-12) for replayed, original in zip(value(bumped(0)), base, strict=True)
):
    raise AssertionError("zero quote shock must reproduce the base Bachelier inputs and price")
pv_up, pv_down = value(bumped(1))[0], value(bumped(-1))[0]
dv01 = (pv_up - pv_down) / 2
actual = price_instrument(json.dumps(f["instrument"]), market, as_of, model="normal", metrics=["vega", "dv01"])
reference = {"npv": base[0], "vega": base[1], "dv01": dv01}
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument(
    "--write", action="store_true", help="Refresh only the internal formula reference and its provenance"
)
if parser.parse_args().write:
    recaptured_on = date.today().isoformat()
    f["expected"] = reference
    f["metadata"].update(
        description=(
            "Independent Bachelier reference for a 10MM USD out-of-the-money 1Y-into-5Y receiver swaption "
            "on the three-step calibrated rates/normal-SABR market."
        ),
        captured_on=recaptured_on,
        last_reviewed_on=recaptured_on,
        regen_command="uv run --no-sync python scripts/golden/normal_swaption_reference.py --write",
        source_detail=(
            f"Independent formula recapture {recaptured_on}: scripts/golden/normal_swaption_reference.py calibrates "
            "the original market envelope and takes its forward and normal cube quote as inputs. It independently "
            "sums contractual ACT/360 annual fixed accruals times payment-date discount factors, computes receiver "
            "Bachelier N*A*((K-F)*Phi(-d)+sigmaN*sqrt(T)*phi(d)) with Python math.erfc, computes vega per 0.01 "
            "normal-vol change, and centrally reprices +/-1bp shifts to the authored deposit, swap and FRA rates. "
            "Both linked rate-calibration steps are rerun, retaining each source curve's interpolation and clock "
            "(ACT/365F discount, ACT/360 projection). The base normal-vol cube is held fixed: this is rate-quote "
            "DV01, not a continuous factor shock or a vol recalibration. The zero quote shock reproduces the base "
            "price and inputs. Formula reference: "
            "https://github.com/lballabio/QuantLib/blob/master/ql/pricingengines/blackformula.cpp "
            f"(bachelierBlackFormula). Inputs: F={base[2]}, K={strike}, sigmaN={sigma} decimal rate/sqrt(year), "
            f"T={t} ACT/365F, A={base[3]} years, N={notional:.0f} USD. Reference NPV={base[0]} USD, "
            f"vega={base[1]} USD per 100bp normal vol, signed quote DV01={dv01} USD per 1bp "
            f"(PV(+1bp)={pv_up}, PV(-1bp)={pv_down}). "
            "This reference isolates option quote-unit, pricing and risk correctness; its calibration and underlying "
            "forward are not an independent external curve benchmark. The FRA quotes end at 1.525 ACT/360 years "
            "while much of the 2027 to 2032 underlying swap prices on the extrapolated projection curve. Contract "
            "terms, market quotes and tolerances are unchanged."
        ),
    )
    f["tolerances"]["dv01"]["tolerance_reason"] = (
        "Independent math.erfc Bachelier reference with +/-1bp par/FRA rate shocks and linked recalibration. "
        "The existing relative tolerance allows the small Linux/macOS calibration variation."
    )
    fixture_path.write_text(json.dumps(f, indent=2, ensure_ascii=False) + "\n")
print(
    json.dumps(
        {
            "reference": reference,
            "inputs": {
                "forward": base[2],
                "annuity": base[3],
                "normal_vol": sigma,
                "strike": strike,
                "expiry": t,
                "periods": [[str(x) for x in row] for row in periods],
            },
            "actual": {"npv": actual.price, "vega": actual.get_metric("vega"), "dv01": actual.get_metric("dv01")},
        },
        indent=2,
    )
)
