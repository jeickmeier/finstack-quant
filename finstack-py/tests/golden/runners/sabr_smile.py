"""SABR closed-form smile runner.

Each fixture lists SABR parameters (alpha, beta, nu, rho), forward, expiry,
and a list of strikes with output keys; expected outputs are SABR implied
volatilities at those strikes. The runner instantiates `SabrSmile` and
returns the implied vol per strike key.
"""

from __future__ import annotations

from finstack.valuations import SabrParameters, SabrSmile
from tests.golden.schema import GoldenFixture


def run(fixture: GoldenFixture) -> dict[str, float]:
    inputs = fixture.inputs
    params = SabrParameters(
        float(inputs["alpha"]),
        float(inputs["beta"]),
        float(inputs["nu"]),
        float(inputs["rho"]),
    )
    smile = SabrSmile(params, float(inputs["forward"]), float(inputs["time_to_expiry"]))
    strikes = inputs["strikes"]
    keys = [entry["key"] for entry in strikes]
    strike_values = [float(entry["strike"]) for entry in strikes]
    vols = smile.generate_smile(strike_values)
    return dict(zip(keys, (float(v) for v in vols), strict=True))
