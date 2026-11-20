"""Reconcile the archived CDSO payer using independent ISDA forward and strike amounts."""

from copy import deepcopy
from datetime import date, timedelta
import json
from math import exp, log, sqrt
from pathlib import Path
from statistics import NormalDist
from typing import NamedTuple

import pytest
import QuantLib as ql  # noqa: N813

from finstack_quant.calibration import calibrate
from finstack_quant.valuations.instruments import price_instrument

FIXTURE = (
    Path(__file__).resolve().parents[2]
    / "finstack-quant/valuations/tests/golden/data/pricing/bloomberg/cds_option/cdx_ig_46_payer_atm_jun26.json"
)


SCREEN_BREAKEVEN_BP = 57.8824


def test_cdso_inputs_match_archived_screens() -> None:
    """CDSW shows one index maturity. CDSO shows the volatility and market value."""
    fixture = json.loads(FIXTURE.read_text())
    spec = fixture["instrument"]["instrument"]["spec"]
    assert spec["instrument_pricing_overrides"]["market_quotes"]["implied_volatility"] == 0.3663
    assert spec["attributes"]["meta"]["spread_vol_pct"] == "36.63"
    envelope = fixture["market"]["envelope"]
    assert all(vol == 0.3663 for vol in envelope["prior_market"][0]["vols_row_major"])
    (quote,) = [row for row in envelope["market_data"] if row["kind"] == "cds_quote"]
    assert quote["pillar"] == {"date": "2031-06-20"}
    assert quote["spread_bp"] == 53.6264
    assert fixture["expected"]["npv"] == 118_781.76
    assert fixture["tolerances"]["npv"]["abs"] == 6.0


class _IsdaCds:
    """Value the index coupon with a flat hazard at one QuantLib evaluation date."""

    def __init__(self, trade, today, spec, discount):
        self.trade = ql.Date.from_date(trade)
        ql.Settings.instance().evaluationDate = self.trade
        knots = [today + timedelta(days=round(t * 365)) for t in discount.knots]
        dates = [trade, *(day for day in knots if day > trade)]
        self.curve = ql.YieldTermStructureHandle(
            ql.DiscountCurve(
                [ql.Date.from_date(day) for day in dates],
                [discount.df(day) / discount.df(trade) for day in dates],
                ql.Actual365Fixed(),
            )
        )
        self.schedule = ql.Schedule(
            self.trade,
            ql.DateParser.parseISO(spec["underlying_maturity"]),
            ql.Period(ql.Quarterly),
            ql.UnitedStates(ql.UnitedStates.Settlement),
            ql.Following,
            ql.Unadjusted,
            ql.DateGeneration.CDS,
            False,
        )
        self.recovery = spec["recovery_rate"]
        self.notional = float(spec["notional"]["amount"])
        self.cds = self._trade_at_coupon(float(spec["coupon_bp"]) / 10_000)
        self.settlement = self.cds.accrualRebate().date()

    def _trade_at_coupon(self, rate):
        return ql.CreditDefaultSwap(
            ql.Protection.Buyer,
            self.notional,
            rate,
            self.schedule,
            ql.Following,
            ql.Actual360(),
            True,
            True,
            self.trade,
            ql.FaceValueClaim(),
            ql.Actual360(True),
            True,
            self.trade,
            3,
        )

    def par_hazard(self, spread):
        """Solve for the flat annual intensity that makes a CDS at the supplied coupon worth zero."""
        return self._trade_at_coupon(spread).impliedHazardRate(
            0, self.curve, ql.Actual365Fixed(), self.recovery, 1e-13, ql.CreditDefaultSwap.ISDA
        )

    def set_hazard(self, cds, hazard):
        """Attach the flat credit curve and ISDA engine at this valuation date."""
        survival = ql.DefaultProbabilityTermStructureHandle(
            ql.FlatHazardRate(self.trade, ql.QuoteHandle(ql.SimpleQuote(hazard)), ql.Actual365Fixed())
        )
        cds.setPricingEngine(
            ql.IsdaCdsEngine(
                survival,
                self.recovery,
                self.curve,
                False,
                ql.IsdaCdsEngine.Taylor,
                ql.IsdaCdsEngine.HalfDayBias,
                ql.IsdaCdsEngine.Piecewise,
            )
        )
        return survival

    def clean_cash(self, hazard):
        """Return signed buyer clean principal in USD at CDS settlement."""
        self.set_hazard(self.cds, hazard)
        return self.cds.NPV() / self.curve.discount(self.settlement)


class _ReferenceCash(NamedTuple):
    """Spot cash in settlement dollars and option amounts in valuation-date dollars."""

    accrued: float
    invoice: float
    forward_pv: float
    strike_pv: float
    breakeven_payoff_pv: float


@pytest.fixture(scope="module")
def isda_cash() -> _ReferenceCash:
    """Roll the observed one-pillar market and price the CDSO exercise amounts."""
    fixture = json.loads(FIXTURE.read_text())
    spec = fixture["instrument"]["instrument"]["spec"]
    envelope = fixture["market"]["envelope"]
    (quote,) = [row for row in envelope["market_data"] if row["kind"] == "cds_quote"]
    discount = calibrate(envelope).market.get_discount(spec["discount_curve_id"])
    today = date.fromisoformat(fixture["metadata"]["valuation_date"])
    expiry = date.fromisoformat(spec["expiry"])
    payment = date.fromisoformat(spec["exercise_settlement_date"])
    df = discount.df(payment)
    with ql.SavedSettings():
        spot = _IsdaCds(today, today, spec, discount)
        hazard = spot.par_hazard(quote["spread_bp"] / 10_000)
        accrued = -spot.cds.accrualRebate().amount()
        invoice = spot.clean_cash(hazard) + accrued

        exercise = _IsdaCds(expiry, today, spec, discount)
        assert exercise.settlement == ql.Date.from_date(payment)
        survival = exp(-hazard * (expiry - today).days / 365)
        # A surviving name delivers the CDS. A pre-expiry default contributes LGD at settlement.
        front_end_loss = (1 - survival) * (1 - exercise.recovery) * exercise.notional
        forward = survival * exercise.clean_cash(hazard) + front_end_loss
        strike = exercise.clean_cash(exercise.par_hazard(float(spec["strike"]["spread"])))
        breakeven = exercise.clean_cash(exercise.par_hazard(SCREEN_BREAKEVEN_BP / 10_000))
    return _ReferenceCash(accrued, invoice, df * forward, df * strike, df * (breakeven - strike))


def test_cdx_ig_46_independent_cds_settlement(isda_cash) -> None:
    """Check QuantLib's CDSW invoice and the discounted payoff at the displayed breakeven."""
    assert round(isda_cash.accrued) == -136_111
    assert abs(isda_cash.invoice - (-2_273_847)) < 6.0
    assert isda_cash.breakeven_payoff_pv == pytest.approx(118_786.09, abs=0.01, rel=0)
    print(
        f"QuantLib CDSW invoice {isda_cash.invoice:.2f}, payoff at CDSO breakeven {isda_cash.breakeven_payoff_pv:.2f}"
    )


class _Payer:
    """Reprice this unit-factor, loss-free index using alternative forward or strike cash."""

    def __init__(self):
        self.fixture = json.loads(FIXTURE.read_text())
        self.instrument = self.fixture["instrument"]
        self.spec = self.instrument["instrument"]["spec"]
        self.envelope = self.fixture["market"]["envelope"]
        self.market = calibrate(self.envelope).market
        (quote,) = [row for row in self.envelope["market_data"] if row["kind"] == "cds_quote"]
        self.spot_bp = quote["spread_bp"]
        self.today = date.fromisoformat(self.fixture["metadata"]["valuation_date"])
        self.discount = self.market.get_discount(self.spec["discount_curve_id"])
        self.df = self.discount.df(date.fromisoformat(self.spec["exercise_settlement_date"]))
        self.coupon = {"spread": str(float(self.spec["coupon_bp"]) / 10_000)}
        self.strike = self.spec["strike"]
        assert self.spec["underlying_is_index"]
        assert self.spec["index_factor"] == 1
        assert self.spec["realized_loss"] == 0
        assert not self.spec.get("knockout", False)

    def result(self, curves, strike, side="call", metrics=()):
        option = deepcopy(self.instrument)
        terms = option["instrument"]["spec"]
        terms.update(strike=strike, option_type=side)
        if "clean_price_pct" in strike:
            terms["strike_index_factor"] = self.spec["index_factor"]
        return price_instrument(option, curves, self.today, model=self.fixture["model"], metrics=list(metrics))

    def price(self, curves, strike, side="call"):
        return float(self.result(curves, strike, side).price)

    def parity(self, curves, strike):
        return self.price(curves, strike) - self.price(curves, strike, "put")

    def clean_strike(self, strike_pv):
        fraction = strike_pv / (float(self.spec["notional"]["amount"]) * self.df)
        return {"clean_price_pct": f"{100 * (1 - fraction):.14f}"}

    def market_at_spread(self, spread_bp):
        bumped = deepcopy(self.envelope)
        (quote,) = [row for row in bumped["market_data"] if row["kind"] == "cds_quote"]
        quote["spread_bp"] = spread_bp
        return calibrate(bumped).market

    def fit_market(self, residual):
        bp = ql.Brent().solve(lambda bp: residual(self.market_at_spread(bp)), 1e-10, self.spot_bp, 0.05)
        return self.market_at_spread(bp)

    def strike_at_premium(self, market, target, guess):
        return ql.Brent().solve(lambda pv: self.price(market, self.clean_strike(pv)) - target, 1e-5, guess, 100.0)


def test_cdx_ig_46_quantlib_forward_and_strike(isda_cash) -> None:
    """Substitute each ISDA cash amount into the production payer and check both orders."""
    p = _Payer()
    native_forward = p.parity(p.market, p.coupon)
    native_strike = native_forward - p.parity(p.market, p.strike)
    # For this unit-factor no-knockout index at fixed vol, the credit curve enters
    # the premium only through the forward cash target. Match it by coupon-strike parity.
    forward_market = p.fit_market(lambda curves: p.parity(curves, p.coupon) - isda_cash.forward_pv)
    native_cash_strike = p.clean_strike(native_strike)
    ql_cash_strike = p.clean_strike(isda_cash.strike_pv)
    baseline = p.price(p.market, p.strike)
    forward_only = p.price(forward_market, p.strike)
    strike_only = p.price(p.market, ql_cash_strike)
    both = p.price(forward_market, ql_cash_strike)

    # Recover the original spread-strike payoff through the independent cash-strike branch.
    assert p.price(p.market, native_cash_strike) == pytest.approx(baseline, abs=0.01, rel=0)
    assert p.price(forward_market, native_cash_strike) == pytest.approx(forward_only, abs=0.01, rel=0)
    assert p.parity(forward_market, p.coupon) == pytest.approx(isda_cash.forward_pv, abs=0.01, rel=0)
    assert p.parity(forward_market, ql_cash_strike) == pytest.approx(
        isda_cash.forward_pv - isda_cash.strike_pv, abs=0.01, rel=0
    )
    assert p.parity(p.market, ql_cash_strike) == pytest.approx(native_forward - isda_cash.strike_pv, abs=0.01, rel=0)

    # Average the two orders so their nonlinear interaction is shared equally.
    forward_effect = ((forward_only - baseline) + (both - strike_only)) / 2
    strike_effect = ((strike_only - baseline) + (both - forward_only)) / 2
    assert forward_effect == pytest.approx(-2_461.42, abs=0.01, rel=0)
    assert strike_effect == pytest.approx(850.51, abs=0.01, rel=0)

    # Invert the screen premium separately for each input, holding the other at
    # its native value. These are alternative explanations of one observation.
    target = p.fixture["expected"]["npv"]
    target_market = p.fit_market(lambda curves: p.price(curves, p.strike) - target)
    target_forward = p.parity(target_market, p.coupon)
    target_strike = p.strike_at_premium(p.market, target, native_strike)
    assert p.price(target_market, p.strike) == pytest.approx(target, abs=0.01, rel=0)
    assert p.price(p.market, p.clean_strike(target_strike)) == pytest.approx(target, abs=0.01, rel=0)
    print(
        f"Discounted USD            Finstack         QuantLib\n"
        f"Forward             {native_forward:14.2f}   {isda_cash.forward_pv:14.2f}\n"
        f"Strike              {native_strike:14.2f}   {isda_cash.strike_pv:14.2f}\n"
        f"Payer PV: baseline {baseline:.2f}, QL forward {forward_only:.2f}, QL strike {strike_only:.2f}, both {both:.2f}\n"
        f"PV changes: forward {forward_only - baseline:+.2f}, strike {strike_only - baseline:+.2f}, "
        f"both {both - baseline:+.2f}; Bloomberg less both {target - both:.2f}\n"
        f"Order-averaged effects: forward {forward_effect:+.2f}, strike {strike_effect:+.2f}\n"
        f"Single-input cash shifts to the screen: forward {target_forward - native_forward:+.2f} "
        f"or strike {target_strike - native_strike:+.2f}"
    )


def test_cdx_ig_46_screen_constraints(isda_cash) -> None:
    """Fit the premium plus one screen observation and check the other observations separately."""
    p = _Payer()
    screen = p.fixture["expected"]
    target = screen["npv"]
    base = p.price(p.market, p.strike)
    forward = p.parity(p.market, p.coupon)
    strike = forward - p.parity(p.market, p.strike)
    be_strike = {"spread": str(SCREEN_BREAKEVEN_BP / 10_000)}
    native_be_mark = forward - p.parity(p.market, be_strike)
    ql_be_mark = isda_cash.strike_pv + isda_cash.breakeven_payoff_pv

    def delta(curves):
        # Keep the contractual spread coordinate for the screen delta. A
        # clean-price option requests a different (curve hedge ratio) metric.
        return p.result(curves, p.strike, metrics=["delta"]).get_metric("delta")

    def vega(curves, cash):
        return p.result(curves, p.clean_strike(cash), metrics=["vega"]).get_metric("vega")

    def fit_strike(curves):
        return p.strike_at_premium(curves, target, strike)

    def report(label, curves, cash):
        cash_strike = p.clean_strike(cash)
        fitted = p.price(curves, cash_strike)
        assert fitted == pytest.approx(target, rel=0, abs=0.01)
        forward_only = p.price(curves, p.strike)
        strike_only = p.price(p.market, cash_strike)
        forward_effect = ((forward_only - base) + (fitted - strike_only)) / 2
        strike_effect = ((strike_only - base) + (fitted - forward_only)) / 2
        be_error = native_be_mark - cash - target
        d, v = delta(curves), vega(curves, cash)
        print(
            f"{label}: forward {forward_effect:+.2f}, strike {strike_effect:+.2f}, delta {100 * d:.5f}%, "
            f"vega {v:.2f}, breakeven error on native mark {be_error:+.2f}"
        )
        return forward_effect, strike_effect, d, v, be_error

    # Given a reference value for the delivered CDS at breakeven, the option
    # premium determines a cash strike. Solving for the forward then uses only price.
    for label, mark, expected_forward_effect in [
        ("Finstack breakeven mark", native_be_mark, 400.21),
        ("QuantLib breakeven mark", ql_be_mark, -444.94),
    ]:
        cash = mark - target
        cash_strike = p.clean_strike(cash)
        curves = p.fit_market(lambda m, k=cash_strike: p.price(m, k) - target)
        f_effect, k_effect, d, v, _ = report(label, curves, cash)
        assert f_effect == pytest.approx(expected_forward_effect, abs=0.01, rel=0)
        assert f_effect + k_effect == pytest.approx(target - base, abs=0.01, rel=0)
        assert d > screen["delta"] + 0.00005
        assert v < screen["vega"] - 40

    # Invert the quoted-strike Black delta. Check its forward coordinate
    # against the public pricer and against the four-decimal screen ATM field.
    vol = p.spec["instrument_pricing_overrides"]["market_quotes"]["implied_volatility"]
    stddev = vol * sqrt((date.fromisoformat(p.spec["expiry"]) - p.today).days / 365)
    strike_bp = float(p.strike["spread"]) * 10_000

    def forward_bp(d):
        return strike_bp * exp(stddev * NormalDist().inv_cdf(d) - 0.5 * stddev**2)

    delta_curves = p.fit_market(lambda m: delta(m) - screen["delta"])
    fitted_forward_bp = p.result(delta_curves, p.strike, metrics=["par_spread"]).get_metric("par_spread")
    assert fitted_forward_bp == pytest.approx(forward_bp(screen["delta"]), abs=0.00001, rel=0)
    print(
        f"Delta-implied forward {fitted_forward_bp:.6f} bp, rounded-delta interval "
        f"{forward_bp(screen['delta'] - 0.00005):.6f} to {forward_bp(screen['delta'] + 0.00005):.6f} bp, "
        f"displayed ATM Black delta {100 * NormalDist().cdf(0.5 * stddev):.5f}%"
    )
    _, _, d, v, be_error = report("Screen delta", delta_curves, fit_strike(delta_curves))
    assert d == pytest.approx(screen["delta"], abs=1e-8, rel=0)
    assert v == pytest.approx(3363.84, abs=0.01, rel=0)
    assert be_error == pytest.approx(2765.01, abs=0.01, rel=0)

    # Repeat at the four corners of the displayed delta rounding and the external NPV band.
    corner_vegas = []
    band = p.fixture["tolerances"]["npv"]["abs"]
    for rounded_delta in (screen["delta"] - 0.00005, screen["delta"] + 0.00005):
        curves = p.fit_market(lambda m, d=rounded_delta: delta(m) - d)
        for premium in (target - band, target + band):
            cash = p.strike_at_premium(curves, premium, strike)
            corner_vegas.append(vega(curves, cash))
    assert max(corner_vegas) < screen["vega"] - 47
    print(f"Vega at delta/NPV band corners {min(corner_vegas):.2f} to {max(corner_vegas):.2f}")

    # Along the same premium contour, solve for the one-point vega. Check
    # the delta and the breakeven cash implied by that solution.
    vega_curves = p.fit_market(lambda m: vega(m, fit_strike(m)) - screen["vega"])
    _, _, d, v, _ = report("Screen vega", vega_curves, fit_strike(vega_curves))
    assert v == pytest.approx(screen["vega"], abs=0.01, rel=0)
    assert d == pytest.approx(0.5542342, abs=1e-6, rel=0)


def test_cdx_ig_46_isda_expiry_values() -> None:
    """Replace only the spread-to-CDS-value function inside the lognormal expectation."""
    p = _Payer()
    forward = p.parity(p.market, p.coupon)
    strike = forward - p.parity(p.market, p.strike)
    expiry = date.fromisoformat(p.spec["expiry"])
    vol = p.spec["instrument_pricing_overrides"]["market_quotes"]["implied_volatility"]
    strike_spread = float(p.strike["spread"])
    normal = NormalDist()

    with ql.SavedSettings():
        cds = _IsdaCds(expiry, p.today, p.spec, p.discount)

        def mark(spread):
            return p.df * cds.clean_cash(cds.par_hazard(spread))

        # Exercise when the ISDA value exceeds the original native cash strike.
        exercise_spread = ql.Brent().solve(lambda s: mark(s) - strike, 1e-14, strike_spread, 0.0001)

        def premium(sigma, order):
            integral = ql.GaussLegendreIntegration(order)
            stddev = sigma * sqrt((expiry - p.today).days / 365)

            def expectation(mean, lower, cash):
                half_width = (9.0 - lower) / 2

                def integrand(node):
                    z = lower + half_width * (1 + node)
                    spread = mean * exp(stddev * z - 0.5 * stddev**2)
                    return normal.pdf(z) * (mark(spread) - cash)

                return half_width * integral(integrand)

            mean = ql.Brent().solve(lambda m: expectation(m, -9.0, 0.0) - forward, 1e-14, strike_spread, 0.0001)
            assert expectation(mean, -9.0, 0.0) == pytest.approx(forward, abs=0.001, rel=0)
            boundary = (log(exercise_spread / mean) + 0.5 * stddev**2) / stddev
            return expectation(mean, boundary, strike)

        values = [premium(sigma, 32) for sigma in (vol, vol + 0.01)]
        for sigma, value in zip((vol, vol + 0.01), values, strict=True):
            assert premium(sigma, 48) == pytest.approx(value, abs=0.01, rel=0)
    native = p.result(p.market, p.strike, metrics=["vega"])
    assert values[0] - float(native.price) == pytest.approx(103.65, abs=0.01, rel=0)
    assert values[1] - values[0] == pytest.approx(3369.22, abs=0.01, rel=0)
    print(
        f"ISDA expiry values only: premium {values[0]:.2f} ({values[0] - float(native.price):+.2f}), "
        f"one-point vega {values[1] - values[0]:.2f} ({values[1] - values[0] - native.get_metric('vega'):+.2f})"
    )


def test_cdx_ig_46_quantlib_black_option_payoff() -> None:
    """Reproduce the upstream CDS option's Black value and unconditional payer default claim."""
    p = _Payer()
    expiry = date.fromisoformat(p.spec["expiry"])
    end = ql.Date.from_date(expiry)
    strike = float(p.strike["spread"])
    sigma = p.spec["instrument_pricing_overrides"]["market_quotes"]["implied_volatility"]
    with ql.SavedSettings():
        # The core QuantLib instrument uses the underlying coupon as its strike.
        # It therefore requires a proxy trade with a 55.2848 bp running coupon.
        delivery = _IsdaCds(expiry, p.today, p.spec, p.discount)
        proxy = delivery._trade_at_coupon(strike)
        spot = _IsdaCds(p.today, p.today, p.spec, p.discount)
        survival = spot.set_hazard(proxy, spot.par_hazard(p.spot_bp / 10_000))
        engine = ql.BlackCdsOptionEngine(survival, spot.recovery, spot.curve, ql.QuoteHandle(ql.SimpleQuote(sigma)))
        knockout = ql.CdsOption(proxy, ql.EuropeanExercise(end), True)
        non_knockout = ql.CdsOption(proxy, ql.EuropeanExercise(end), False)
        knockout.setPricingEngine(engine)
        non_knockout.setPricingEngine(engine)
        stddev = sigma * sqrt((expiry - p.today).days / 365)
        annuity = abs(proxy.couponLegNPV() / strike)
        assert knockout.NPV() == pytest.approx(
            ql.blackFormula(ql.Option.Call, strike, proxy.fairSpread(), stddev, annuity), rel=0, abs=0.01
        )
        loss_pv = spot.notional * (1 - spot.recovery) * survival.defaultProbability(end) * spot.curve.discount(end)
        assert non_knockout.NPV() - knockout.NPV() == pytest.approx(loss_pv, rel=0, abs=0.01)
        print(
            f"QuantLib proxy: knockout {knockout.NPV():.2f}, non-knockout {non_knockout.NPV():.2f}, "
            f"front-end protection {loss_pv:.2f}"
        )
