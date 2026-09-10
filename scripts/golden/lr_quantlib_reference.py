"""Regenerate independent Leisen-Reimer reference prices with QuantLib 1.43.

Run from the repository root with ``uv run --no-sync python
scripts/golden/lr_quantlib_reference.py``. All rates are continuously compounded;
the model clock is ACT/365F and no cash dividends or settlement lag apply.
"""

import json
from pathlib import Path

import QuantLib


def main() -> None:
    """Write explicitly dated European, American, and Bermudan LR references."""
    base = QuantLib.Date(1, 1, 2025)
    QuantLib.Settings.instance().evaluationDate = base
    expiry = base + 365
    exercise_days = [91, 182, 273, 365]
    cases = []
    for sigma in [0.2, 0.9]:
        process = QuantLib.BlackScholesMertonProcess(
            QuantLib.QuoteHandle(QuantLib.SimpleQuote(100.0)),
            QuantLib.YieldTermStructureHandle(QuantLib.FlatForward(base, 0.02, QuantLib.Actual365Fixed())),
            QuantLib.YieldTermStructureHandle(QuantLib.FlatForward(base, 0.05, QuantLib.Actual365Fixed())),
            QuantLib.BlackVolTermStructureHandle(
                QuantLib.BlackConstantVol(base, QuantLib.NullCalendar(), sigma, QuantLib.Actual365Fixed())
            ),
        )
        for side, option_type in [("call", QuantLib.Option.Call), ("put", QuantLib.Option.Put)]:
            for style, exercise in [
                ("european", QuantLib.EuropeanExercise(expiry)),
                ("american", QuantLib.AmericanExercise(base, expiry)),
                ("bermudan", QuantLib.BermudanExercise([base + day for day in exercise_days])),
            ]:
                for steps in [51, 101, 201]:
                    option = QuantLib.VanillaOption(QuantLib.PlainVanillaPayoff(option_type, 100.0), exercise)
                    option.setPricingEngine(QuantLib.BinomialVanillaEngine(process, "lr", steps))
                    cases.append({
                        "volatility": sigma,
                        "side": side,
                        "style": style,
                        "steps": steps,
                        "pv": option.NPV(),
                    })
    output = {
        "provenance": f"QuantLib {QuantLib.__version__}, BinomialVanillaEngine(process, 'lr', steps)",
        "source": "https://github.com/lballabio/QuantLib/blob/master/ql/methods/lattices/binomialtree.cpp",
        "as_of": "2025-01-01",
        "expiry": "2026-01-01",
        "spot": 100.0,
        "strike": 100.0,
        "rate": 0.05,
        "dividend_yield": 0.02,
        "exercise_days": exercise_days,
        "cases": cases,
    }
    path = Path(__file__).resolve().parents[2] / "finstack-quant/models/tests/fixtures/production_lr_quantlib.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(output, indent=2) + "\n")


if __name__ == "__main__":
    main()
