"""Generate independent SABR derivative pins with 80-digit Decimal differences.

These references differentiate the existing Hagan/Obloj formula and its
piecewise chi/Taylor/smoothstep definitions. They do not import production
Rust code or use its analytic derivatives. Interior derivatives use central
h=1e-25 differences; fit-bound derivatives use inward h=1e-18 differences.
Run with uv run --no-project python <this file> to regenerate the adjacent JSON.
"""

from decimal import Decimal, localcontext
from itertools import product
import json
from pathlib import Path


def chi(z: Decimal, rho: Decimal) -> Decimal:
    """Evaluate the piecewise chi definition with decimal arithmetic."""
    series = z + rho * z**2 / 2 + (3 * rho * rho - 1) * z**3 / 6 + rho * (5 * rho * rho - 3) * z**4 / 8
    if abs(z) < Decimal("1e-5"):
        return series
    exact = (((1 - 2 * rho * z + z * z).sqrt() + z - rho) / (1 - rho)).ln()
    if abs(z) > Decimal("1e-3"):
        return exact
    t = (abs(z) - Decimal("1e-5")) / (Decimal("1e-3") - Decimal("1e-5"))
    weight = t * t * (3 - 2 * t)
    return (1 - weight) * series + weight * exact


def ratio(z: Decimal, rho: Decimal, normal: bool) -> Decimal:
    """Evaluate the normal or lognormal small-z ratio convention."""
    if abs(z) < Decimal("1e-6" if normal else "1e-5"):
        cubic = Decimal(0) if normal else rho * (5 - 6 * rho * rho) * z**3 / 24
        return 1 - rho * z / 2 + (2 - 3 * rho * rho) * z * z / 12 + cubic
    return z / chi(z, rho)


def vol(alpha: Decimal, beta: Decimal, nu: Decimal, rho: Decimal, forward: Decimal, strike: Decimal) -> Decimal:
    """Evaluate a two-year Hagan/Obloj volatility at fixed forward and strike."""
    time = Decimal(2)
    if beta < Decimal("1e-4"):
        return (
            alpha * ratio(nu / alpha * (forward - strike), rho, True) * (1 + (2 - 3 * rho * rho) * nu * nu * time / 24)
        )
    if abs(1 - beta) < Decimal("1e-4"):
        beta = Decimal(1)
    if abs(forward - strike) / max(abs(forward), abs(strike)) < Decimal("1e-8"):
        fb = forward ** (1 - beta)
        return (
            alpha
            / fb
            * (
                1
                + time
                * (
                    (1 - beta) ** 2 * alpha * alpha / (24 * fb * fb)
                    + rho * beta * nu * alpha / (4 * fb)
                    + (2 - 3 * rho * rho) * nu * nu / 24
                )
            )
        )
    midpoint = (forward * strike).sqrt()
    fb = midpoint ** (1 - beta)
    log_moneyness = (forward / strike).ln()
    z = nu / alpha * fb * log_moneyness
    correction = 1 + (1 - beta) ** 2 * log_moneyness**2 / 24 + (1 - beta) ** 4 * log_moneyness**4 / 1920
    return (
        alpha
        / (fb * correction)
        * ratio(z, rho, False)
        * (
            1
            + time
            * (
                (1 - beta) ** 2 * alpha * alpha / (24 * midpoint ** (2 * (1 - beta)))
                + rho * beta * nu * alpha / (4 * fb)
                + (2 - 3 * rho * rho) * nu * nu / 24
            )
        )
    )


GradientCase = list[float | list[float]]


def gradient_case(values: list[Decimal], *, inward: bool = False) -> GradientCase | None:
    """Differentiate valid volatility cases; return alpha/nu/rho partials."""
    if vol(*values) <= 0:
        return None
    derivatives = []
    for index in [0, 2, 3]:
        step = Decimal("1e-18" if inward and index != 0 else "1e-25")
        up = list(values)
        down = list(values)
        if inward and index != 0:
            if (index == 2 and values[index] == 2) or (index == 3 and values[index] > 0):
                step = -step
            up[index] += step
            derivative = (vol(*up) - vol(*values)) / step
        else:
            up[index] += step
            down[index] -= step
            derivative = (vol(*up) - vol(*down)) / (2 * step)
        derivatives.append(float(derivative))
    return [*[float(value) for value in values], derivatives]


def seam_references() -> tuple[list[list[float]], list[list[bool | float]]]:
    """Differentiate both sides of the chi and ratio branch seams."""
    h = Decimal("1e-25")
    chi_rows: list[list[float]] = []
    ratio_rows: list[list[bool | float]] = []
    for cutoff, side, sign, correlation in product(
        ["1e-6", "1e-5", "1e-3"], ["0.99999999", "1.00000001"], [-1, 1], ["-0.99", "0", "0.99"]
    ):
        z = Decimal(cutoff) * Decimal(side) * sign
        rho = Decimal(correlation)
        if cutoff != "1e-6":
            chi_rows.append([
                float(z),
                float(rho),
                float((chi(z + h, rho) - chi(z - h, rho)) / (2 * h)),
                float((chi(z, rho + h) - chi(z, rho - h)) / (2 * h)),
            ])
        ratio_rows.extend(
            [
                normal,
                float(z),
                float(rho),
                float((ratio(z + h, rho, normal) - ratio(z - h, rho, normal)) / (2 * h)),
                float((ratio(z, rho + h, normal) - ratio(z, rho - h, normal)) / (2 * h)),
            ]
            for normal in [False, True]
        )
    return chi_rows, ratio_rows


def gradient_references() -> tuple[list[GradientCase], list[GradientCase]]:
    """Cover beta limits, small nu, ATM wings, and inward parameter bounds."""
    grid: list[GradientCase] = []
    boundaries: list[GradientCase] = []
    for strings in product(
        ["0", "0.00005", "0.0001", "0.5", "0.99995", "1"],
        ["0.02", "0.3"],
        ["0.0011", "0.5", "1.999"],
        ["-0.989", "-0.3", "0", "0.989"],
        ["-0.5", "-1e-4", "-1e-7", "0", "1e-7", "1e-4", "0.5"],
    ):
        beta, alpha, nu, rho, relative = map(Decimal, strings)
        case = gradient_case([alpha, beta, nu, rho, Decimal("0.02"), Decimal("0.02") * (1 + relative)])
        if case is not None:
            grid.append(case)
    for strings in product(["0", "0.5", "1"], ["0.001", "2"], ["-0.99", "0.99"], ["0.005", "0.01", "0.02"]):
        beta, nu, rho, strike = map(Decimal, strings)
        case = gradient_case([Decimal("0.1"), beta, nu, rho, Decimal("0.01"), strike], inward=True)
        if case is not None:
            boundaries.append(case)
    return grid, boundaries


def generate() -> dict[str, object]:
    """Build reproducible derivative fixtures without production code imports."""
    with localcontext() as context:
        context.prec = 80
        chi_rows, ratio_rows = seam_references()
        grid, boundaries = gradient_references()
        return {
            "precision_digits": 80,
            "central_step": "1e-25",
            "inward_step": "1e-18",
            "chi": chi_rows,
            "ratio": ratio_rows,
            "grid": grid,
            "boundaries": boundaries,
        }


if __name__ == "__main__":
    destination = Path(__file__).with_name("sabr_derivative_references.json")
    destination.write_text(json.dumps(generate(), indent=2) + "\n")
