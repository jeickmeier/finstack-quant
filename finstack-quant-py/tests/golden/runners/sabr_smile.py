"""SABR closed-form smile runner.

Each fixture lists SABR parameters (alpha, beta, nu, rho), forward, expiry,
and a list of strikes with output keys; expected outputs are SABR implied
volatilities at those strikes. The runner instantiates `SabrSmile` and
returns the implied vol per strike key.
"""

from __future__ import annotations

from finstack_quant.valuations import SabrParameters, SabrSmile
from tests.golden.schema import GoldenFixture


def run(fixture: GoldenFixture) -> dict[str, float]:
    body = fixture.body
    params = SabrParameters(
        float(body["alpha"]),
        float(body["beta"]),
        float(body["nu"]),
        float(body["rho"]),
    )
    smile = SabrSmile(params, float(body["forward"]), float(body["time_to_expiry"]))
    strikes = body["strikes"]
    keys = [entry["key"] for entry in strikes]
    strike_values = [float(entry["strike"]) for entry in strikes]
    vols = smile.generate_smile(strike_values)
    return dict(zip(keys, (float(v) for v in vols), strict=True))
