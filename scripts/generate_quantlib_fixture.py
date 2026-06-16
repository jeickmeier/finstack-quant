"""Generate QuantLib reference fixtures for finstack parity tests.

Runs QuantLib's analytical/pricing engines on three canonical instruments and
emits JSON fixtures consumed by:

- ``finstack-quant/valuations/tests/sanity_invariants/test_bond_quantlib_external_parity.rs``
  — base valuation parity (NPV, DV01)
- ``finstack-quant/attribution/tests/attribution/quantlib_parity.rs``
  — attribution decomposition parity (carry, rates, residual via
    ``attribute_pnl_metrics_based``)

Run with::

    uv run python scripts/generate_quantlib_fixture.py

Outputs three JSON files under
``finstack-quant/valuations/tests/data/quantlib_parity/``:

- ``bond_5pct_10y_usd.json``        vanilla USD fixed-rate bond
- ``irs_5y_usd.json``               vanilla USD interest-rate swap (5y, semi/quarterly)
- ``fx_forward_1y_eurusd.json``     EUR/USD outright forward (1y)

Each fixture pins:

- the canonical inputs (coupon, maturity, day count, frequencies, calendars,
  rate curves at T0 and T1)
- QuantLib-computed values at T0 and T1 (clean price, dirty price, NPV)
- first-order risk metrics at T0 (DV01, Convexity, Theta)
- the expected one-day attribution decomposition (carry = Theta * dt,
  rates = -DV01 * 10000 * d(yield), residual = remainder).

The fixtures are committed to the repo; the Rust tests do NOT invoke
QuantLib. Re-run this script when the canonical scenarios change.
"""

from __future__ import annotations

import contextlib
import json
from pathlib import Path
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813  (QuantLib's canonical Python alias)

FIXTURE_DIR = (
    Path(__file__).resolve().parents[1] / "finstack-quant" / "valuations" / "tests" / "data" / "quantlib_parity"
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def ql_date_to_iso(d: ql.Date) -> str:
    """QuantLib Date -> ISO8601 calendar date string."""
    return f"{d.year():04d}-{d.month():02d}-{d.dayOfMonth():02d}"


def _flat_yield_handle(rate: float, day_count: ql.DayCounter) -> ql.YieldTermStructureHandle:
    """Build a flat yield-term-structure handle quoted continuously compounded."""
    quote = ql.SimpleQuote(rate)
    ts = ql.FlatForward(
        0,
        ql.UnitedStates(ql.UnitedStates.GovernmentBond),
        ql.QuoteHandle(quote),
        day_count,
        ql.Continuous,
    )
    ts.enableExtrapolation()
    return ql.YieldTermStructureHandle(ts)


def _write_fixture(name: str, data: dict[str, Any]) -> None:
    FIXTURE_DIR.mkdir(parents=True, exist_ok=True)
    path = FIXTURE_DIR / name
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
    print(f"wrote {path}")


# ---------------------------------------------------------------------------
# 1. Vanilla USD fixed-rate bond
# ---------------------------------------------------------------------------


def build_bond_fixture() -> dict[str, Any]:
    """5% semi-annual USD bond, 10y to maturity.

    Convention set is chosen to match finstack-quant's `Bond::fixed` defaults so the
    parity test does not require any special builder calls on the Rust side:

      day count          = 30/360 (Bond Basis)
      frequency          = semi-annual
      business day convention = Following
      settlement days    = 2

    Pricing dates: T0 = 2025-01-15, T1 = 2025-01-16.
    Yield curve: flat 5.0% (T0) -> 5.01% (T1) continuously compounded.
    """
    calendar = ql.UnitedStates(ql.UnitedStates.GovernmentBond)
    day_count = ql.Thirty360(ql.Thirty360.BondBasis)

    t0 = ql.Date(15, 1, 2025)
    t1 = ql.Date(16, 1, 2025)
    issue = ql.Date(15, 1, 2025)
    maturity = ql.Date(15, 1, 2035)
    coupon = 0.05
    settlement_days = 2
    face = 100.0

    schedule = ql.Schedule(
        issue,
        maturity,
        ql.Period(ql.Semiannual),
        calendar,
        ql.Following,
        ql.Following,
        ql.DateGeneration.Backward,
        False,
    )
    bond = ql.FixedRateBond(settlement_days, face, schedule, [coupon], day_count)

    def price_on(date: ql.Date, rate: float) -> tuple[float, float, float, float]:
        ql.Settings.instance().evaluationDate = date
        yts = _flat_yield_handle(rate, day_count)
        bond.setPricingEngine(ql.DiscountingBondEngine(yts))
        npv = bond.NPV()
        clean = bond.cleanPrice()
        dirty = bond.dirtyPrice()
        accrued = bond.accruedAmount()
        return npv, clean, dirty, accrued

    rate_t0 = 0.05
    rate_t1 = 0.0501  # +1 bp

    npv_t0, clean_t0, dirty_t0, accrued_t0 = price_on(t0, rate_t0)
    npv_t1, clean_t1, dirty_t1, accrued_t1 = price_on(t1, rate_t1)

    # DV01 by central difference (+/- 1bp).
    ql.Settings.instance().evaluationDate = t0
    bond.setPricingEngine(ql.DiscountingBondEngine(_flat_yield_handle(rate_t0 + 1e-4, day_count)))
    npv_up = bond.NPV()
    bond.setPricingEngine(ql.DiscountingBondEngine(_flat_yield_handle(rate_t0 - 1e-4, day_count)))
    npv_dn = bond.NPV()
    dv01 = (npv_up - npv_dn) / 2.0  # $ per 1bp rate move

    # Convexity (cash) by second-difference.
    bond.setPricingEngine(ql.DiscountingBondEngine(_flat_yield_handle(rate_t0, day_count)))
    npv_base = bond.NPV()
    convexity_cash = (npv_up + npv_dn - 2.0 * npv_base) / (1e-4) ** 2

    # Theta: one-day P&L holding the curve fixed.
    yts_frozen = _flat_yield_handle(rate_t0, day_count)
    bond.setPricingEngine(ql.DiscountingBondEngine(yts_frozen))
    ql.Settings.instance().evaluationDate = t1
    npv_frozen_t1 = bond.NPV()
    theta_one_day = npv_frozen_t1 - npv_t0

    # Convention: `dv01` is the dollar PV change per 1bp UP shift (computed
    # by `(npv_up − npv_dn)/2`). For a long bond it is negative. The signed
    # rate P&L is therefore `dv01 × Δrate_bp` directly — no sign flip.
    rate_pnl = dv01 * 10_000.0 * (rate_t1 - rate_t0)
    actual_pnl = npv_t1 - npv_t0

    return {
        "instrument": "FixedRateBond",
        "name": "USD-BOND-5PCT-10Y-SEMI",
        "currency": "USD",
        "conventions": {
            "calendar": "UnitedStates::GovernmentBond",
            "day_count": "Thirty360::BondBasis",
            "frequency": "Semiannual",
            "business_day_convention": "Following",
            "date_generation": "Backward",
            "end_of_month": False,
            "settlement_days": settlement_days,
        },
        "spec": {
            "issue_date": ql_date_to_iso(issue),
            "maturity_date": ql_date_to_iso(maturity),
            "face_amount": face,
            "coupon_rate": coupon,
        },
        "scenario": {
            "t0": ql_date_to_iso(t0),
            "t1": ql_date_to_iso(t1),
            "yield_curve": "flat_continuous",
            "rate_t0": rate_t0,
            "rate_t1": rate_t1,
            "rate_shift_bp": (rate_t1 - rate_t0) * 10_000.0,
        },
        "quantlib": {
            "version": ql.__version__,
            "t0": {
                "npv": npv_t0,
                "clean_price": clean_t0,
                "dirty_price": dirty_t0,
                "accrued": accrued_t0,
                "dv01": dv01,
                "convexity_cash": convexity_cash,
                "theta_one_day": theta_one_day,
            },
            "t1": {
                "npv": npv_t1,
                "clean_price": clean_t1,
                "dirty_price": dirty_t1,
                "accrued": accrued_t1,
            },
        },
        "expected_attribution": {
            "actual_pnl": actual_pnl,
            "carry_pnl": theta_one_day,
            "rates_pnl_first_order": rate_pnl,
            "residual_first_order": actual_pnl - theta_one_day - rate_pnl,
        },
    }


# ---------------------------------------------------------------------------
# 2. Vanilla USD interest-rate swap (5y, fixed semi vs SOFR-like quarterly)
# ---------------------------------------------------------------------------


def build_irs_fixture() -> dict[str, Any]:
    """5y USD IRS, fixed 4% semi-annual vs simple float quarterly on a flat curve.

    Uses QuantLib's MakeVanillaSwap with a USD Libor-style index for
    convenience. The "Libor" name is QL's historical placeholder; the
    convention work (Act/360 float, 30/360 fixed) matches the post-LIBOR
    USD vanilla market.
    """
    calendar = ql.UnitedStates(ql.UnitedStates.SOFR)
    day_count = ql.Actual360()

    t0 = ql.Date(15, 1, 2025)
    t1 = ql.Date(16, 1, 2025)
    settlement = calendar.advance(t0, ql.Period(2, ql.Days))
    maturity = calendar.advance(settlement, ql.Period(5, ql.Years))
    notional = 10_000_000.0
    fixed_rate = 0.04

    rate_t0 = 0.05
    rate_t1 = 0.0501

    # Past fixings: with a 2-day spot-start swap whose first coupon fixes
    # ~2 days before the evaluation date, QuantLib needs historical fixings
    # for the float index covering the trade window. Use the flat-curve rate
    # as each fixing so the test scenario stays self-consistent.
    fixing_index = ql.USDLibor(ql.Period(3, ql.Months))
    ql.IndexManager.instance().clearHistories()
    fixing_cal = fixing_index.fixingCalendar()
    for offset in range(-5, 2):
        fixing_date = fixing_cal.advance(t0, ql.Period(offset, ql.Days))
        # Some offsets land on the same business day after rolling — ignore
        # duplicate-fixing errors that QuantLib raises on second insertion.
        with contextlib.suppress(RuntimeError):
            fixing_index.addFixing(fixing_date, 0.05)

    # Build the swap ONCE at t0 (schedule anchored to the t0 spot date) and
    # revalue THE SAME trade for theta / DV01 / the t1 mark by relinking the
    # curve handle and moving the evaluation date. The previous version
    # rebuilt a fresh 5Y swap with MakeVanillaSwap at every date — a
    # constant-maturity roll whose "theta" included the schedule
    # reconstruction effect (~$500/day on this fixture, an order of
    # magnitude above the true trade-level carry) and whose "P&L" was not
    # the P&L of any single trade (quant review M14).
    relink = ql.RelinkableYieldTermStructureHandle()

    def flat_ts(rate: float) -> ql.FlatForward:
        ts = ql.FlatForward(
            0,
            ql.UnitedStates(ql.UnitedStates.GovernmentBond),
            ql.QuoteHandle(ql.SimpleQuote(rate)),
            day_count,
            ql.Continuous,
        )
        ts.enableExtrapolation()
        return ts

    ql.Settings.instance().evaluationDate = t0
    relink.linkTo(flat_ts(rate_t0))
    index = ql.USDLibor(ql.Period(3, ql.Months), relink)
    swap = ql.MakeVanillaSwap(
        ql.Period(5, ql.Years),
        index,
        fixed_rate,
        ql.Period(0, ql.Days),
        nominal=notional,
        swapType=ql.Swap.Payer,
        pricingEngine=ql.DiscountingSwapEngine(relink),
    )
    npv_t0 = swap.NPV()
    fair_rate_t0 = swap.fairRate()

    # DV01 via central difference on the discount curve (same trade).
    relink.linkTo(flat_ts(rate_t0 + 1e-4))
    npv_up = swap.NPV()
    relink.linkTo(flat_ts(rate_t0 - 1e-4))
    npv_dn = swap.NPV()
    dv01 = (npv_up - npv_dn) / 2.0

    # Theta: 1-day P&L of the SAME swap holding the curve flat (the curve's
    # base rolls with the evaluation date; the schedule does not move).
    ql.Settings.instance().evaluationDate = t1
    relink.linkTo(flat_ts(rate_t0))
    theta_one_day = swap.NPV() - npv_t0

    # T1 mark: same swap, t1 curve.
    relink.linkTo(flat_ts(rate_t1))
    npv_t1 = swap.NPV()

    # Convention: `dv01` is the dollar PV change per 1bp UP shift (computed
    # by `(npv_up − npv_dn)/2`). For a long bond it is negative. The signed
    # rate P&L is therefore `dv01 × Δrate_bp` directly — no sign flip.
    rate_pnl = dv01 * 10_000.0 * (rate_t1 - rate_t0)
    actual_pnl = npv_t1 - npv_t0

    return {
        "instrument": "VanillaSwap",
        "name": "USD-IRS-5Y-PAYER",
        "currency": "USD",
        "conventions": {
            "calendar": "UnitedStates::SOFR",
            "day_count_fixed": "Thirty360::BondBasis",
            "day_count_float": "Actual360",
            "fixed_frequency": "Semiannual",
            "float_frequency": "Quarterly",
            "swap_type": "Payer",
            "settlement_days": 2,
        },
        "spec": {
            "trade_date": ql_date_to_iso(t0),
            "settlement_date": ql_date_to_iso(settlement),
            "maturity_date": ql_date_to_iso(maturity),
            "notional": notional,
            "fixed_rate": fixed_rate,
        },
        "scenario": {
            "t0": ql_date_to_iso(t0),
            "t1": ql_date_to_iso(t1),
            "yield_curve": "flat_continuous",
            "rate_t0": rate_t0,
            "rate_t1": rate_t1,
            "rate_shift_bp": (rate_t1 - rate_t0) * 10_000.0,
        },
        "quantlib": {
            "version": ql.__version__,
            "t0": {
                "npv": npv_t0,
                "fair_rate": fair_rate_t0,
                "dv01": dv01,
                "theta_one_day": theta_one_day,
            },
            "t1": {"npv": npv_t1},
        },
        "expected_attribution": {
            "actual_pnl": actual_pnl,
            "carry_pnl": theta_one_day,
            "rates_pnl_first_order": rate_pnl,
            "residual_first_order": actual_pnl - theta_one_day - rate_pnl,
        },
    }


# ---------------------------------------------------------------------------
# 3. EUR/USD 1y outright forward
# ---------------------------------------------------------------------------


def build_fx_forward_fixture() -> dict[str, Any]:
    """EUR/USD outright forward, 1y to maturity, settled in USD.

    Priced under no-arbitrage from interest-rate parity:
      F = S * exp((r_usd - r_eur) * T)
    PV in USD of a contract to pay K EUR / receive S0_eur EUR notional
    is `(F - K) * df_usd(T) * notional`. We use a deliberately simple
    model so the parity test is easy to verify.
    """
    t0 = ql.Date(15, 1, 2025)
    t1 = ql.Date(16, 1, 2025)
    maturity = ql.Date(15, 1, 2026)
    notional_eur = 1_000_000.0
    spot_t0 = 1.10
    spot_t1 = 1.1005
    r_usd_t0 = 0.05
    r_usd_t1 = 0.0501
    r_eur_t0 = 0.03
    r_eur_t1 = 0.0300

    day_count = ql.Actual365Fixed()
    tau_t0 = day_count.yearFraction(t0, maturity)
    tau_t1 = day_count.yearFraction(t1, maturity)

    def forward_npv(date: ql.Date, spot: float, r_usd: float, r_eur: float, strike: float) -> float:
        tau = day_count.yearFraction(date, maturity)
        # Closed-form FX-forward PV under no-arbitrage with continuous rates:
        #   PV_USD = N_EUR * (S * df_eur(T) − K * df_usd(T))
        #          ≡ N_EUR * (F − K) * df_usd(T) where F = S * exp((r_usd−r_eur)T)
        import math

        df_usd = math.exp(-r_usd * tau)
        df_eur = math.exp(-r_eur * tau)
        return notional_eur * (spot * df_eur - strike * df_usd)

    # Strike chosen at the forward at T0 → initial NPV is zero.
    import math

    strike = spot_t0 * math.exp((r_usd_t0 - r_eur_t0) * tau_t0)

    npv_t0 = forward_npv(t0, spot_t0, r_usd_t0, r_eur_t0, strike)
    npv_t1 = forward_npv(t1, spot_t1, r_usd_t1, r_eur_t1, strike)

    # Spot delta (per 1.0 spot move).
    spot_up = forward_npv(t0, spot_t0 + 1e-4, r_usd_t0, r_eur_t0, strike)
    spot_dn = forward_npv(t0, spot_t0 - 1e-4, r_usd_t0, r_eur_t0, strike)
    spot_delta = (spot_up - spot_dn) / (2.0 * 1e-4)

    # USD rate DV01 (per 1bp USD rate move).
    usd_up = forward_npv(t0, spot_t0, r_usd_t0 + 1e-4, r_eur_t0, strike)
    usd_dn = forward_npv(t0, spot_t0, r_usd_t0 - 1e-4, r_eur_t0, strike)
    usd_dv01 = (usd_up - usd_dn) / 2.0

    # EUR rate DV01 (per 1bp EUR rate move).
    eur_up = forward_npv(t0, spot_t0, r_usd_t0, r_eur_t0 + 1e-4, strike)
    eur_dn = forward_npv(t0, spot_t0, r_usd_t0, r_eur_t0 - 1e-4, strike)
    eur_dv01 = (eur_up - eur_dn) / 2.0

    # Theta: 1-day price change holding all market data fixed.
    npv_frozen_t1 = forward_npv(t1, spot_t0, r_usd_t0, r_eur_t0, strike)
    theta_one_day = npv_frozen_t1 - npv_t0

    fx_pnl = spot_delta * (spot_t1 - spot_t0)
    # DV01s above are already the SIGNED PV change per +1bp (central
    # difference of NPV), so first-order P&L is dv01 x move with NO extra
    # negation (quant review M13: the former minus sign flipped the rate
    # factor and inflated residual_first_order from -0.24 to +213).
    usd_rate_pnl = usd_dv01 * 10_000.0 * (r_usd_t1 - r_usd_t0)
    eur_rate_pnl = eur_dv01 * 10_000.0 * (r_eur_t1 - r_eur_t0)
    actual_pnl = npv_t1 - npv_t0

    return {
        "instrument": "FxForward",
        "name": "EURUSD-1Y-FORWARD",
        "currency": "USD",
        "conventions": {
            "day_count": "Actual365Fixed",
            "settlement": "USD (cash-settled)",
            "model": "no-arbitrage forward, simple flat continuously-compounded rates",
        },
        "spec": {
            "base_ccy": "EUR",
            "quote_ccy": "USD",
            "notional_base_ccy": notional_eur,
            "trade_date": ql_date_to_iso(t0),
            "maturity_date": ql_date_to_iso(maturity),
            "strike_at_forward": True,
            "strike": strike,
        },
        "scenario": {
            "t0": ql_date_to_iso(t0),
            "t1": ql_date_to_iso(t1),
            "spot_t0": spot_t0,
            "spot_t1": spot_t1,
            "r_usd_t0": r_usd_t0,
            "r_usd_t1": r_usd_t1,
            "r_eur_t0": r_eur_t0,
            "r_eur_t1": r_eur_t1,
            "tau_t0": tau_t0,
            "tau_t1": tau_t1,
        },
        "quantlib": {
            "version": ql.__version__,
            "t0": {
                "npv_usd": npv_t0,
                "spot_delta_per_unit": spot_delta,
                "usd_dv01": usd_dv01,
                "eur_dv01": eur_dv01,
                "theta_one_day": theta_one_day,
            },
            "t1": {"npv_usd": npv_t1},
        },
        "expected_attribution": {
            "actual_pnl": actual_pnl,
            "carry_pnl": theta_one_day,
            "fx_pnl_first_order": fx_pnl,
            "usd_rate_pnl_first_order": usd_rate_pnl,
            "eur_rate_pnl_first_order": eur_rate_pnl,
            "residual_first_order": actual_pnl - theta_one_day - fx_pnl - usd_rate_pnl - eur_rate_pnl,
        },
    }


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> None:
    """Regenerate all three QuantLib parity fixtures."""
    bond = build_bond_fixture()
    _write_fixture("bond_5pct_10y_usd.json", bond)

    irs = build_irs_fixture()
    _write_fixture("irs_5y_usd.json", irs)

    fxf = build_fx_forward_fixture()
    _write_fixture("fx_forward_1y_eurusd.json", fxf)


if __name__ == "__main__":
    main()
