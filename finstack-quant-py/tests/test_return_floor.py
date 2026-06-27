"""Return-floor MOIC/XIRR Python binding tests.

The return-floor feature is exposed via the existing JSON-native Bond binding:
``return_floor`` is a serde field on the Rust Bond spec, so it is reachable
immediately through the generic ``Bond(spec)`` / ``Bond.from_json(json)``
constructors and the generic ``price_with_metrics(metrics=[...])`` method.
No new Rust binding code is required.

The four metric IDs are:
  - ``"moic"``          — money multiple to maturity
  - ``"moic_to_worst"`` — money multiple to worst exit (min over all paths)
  - ``"xirr"``          — XIRR to maturity
  - ``"xirr_to_worst"`` — XIRR to worst exit

All four are registered as standard metrics in the Rust metric registry
(``MetricId::ALL_STANDARD``), so ``list_standard_metrics()`` will include them.
"""

from __future__ import annotations

import json

from finstack_quant.valuations.instruments import list_standard_metrics
from finstack_quant.valuations.instruments.fixed_income import Bond

# ---------------------------------------------------------------------------
# Market context helpers
# ---------------------------------------------------------------------------


def _market_json(base: str = "2024-01-01", rate_at_5y: float = 0.85) -> str:
    """Minimal discount-only market context for bond pricing."""
    return json.dumps({
        "version": 2,
        "curves": [
            {
                "type": "discount",
                "id": "USD-OIS",
                "base": base,
                "day_count": "Act365F",
                "knot_points": [[0.0, 1.0], [5.0, rate_at_5y]],
                "interp_style": "monotone_convex",
                "extrapolation": "flat_forward",
                "min_forward_rate": None,
                "allow_non_monotonic": False,
                "min_forward_tenor": 1e-6,
            }
        ],
        "fx": None,
        "surfaces": [],
        "prices": {},
        "series": [],
        "inflation_indices": [],
        "dividends": [],
        "credit_indices": [],
        "fx_delta_vol_surfaces": [],
        "vol_cubes": [],
        "collateral": {},
    })


# ---------------------------------------------------------------------------
# Bond spec helpers
# ---------------------------------------------------------------------------


def _plain_bond_spec(return_floor: dict | None = None) -> dict:
    """5-year 10% annual fixed-rate bullet bond, issued at par."""
    spec: dict = {
        "id": "TEST-RETURN-FLOOR-BOND",
        "notional": {"amount": "1000000", "currency": "USD"},
        "issue_date": "2024-01-01",
        "maturity": "2029-01-01",
        "cashflow_spec": {
            "Fixed": {
                "rate": "0.10",
                "freq": {"count": 12, "unit": "months"},
                "dc": "Thirty360",
                "bdc": "following",
                "calendar_id": "weekends_only",
            }
        },
        "discount_curve_id": "USD-OIS",
        "settlement_days": 0,
        "ex_coupon_days": 0,
        "attributes": {},
    }
    if return_floor is not None:
        spec["return_floor"] = return_floor
    return spec


# ---------------------------------------------------------------------------
# Smoke tests: Bond construction with return_floor in spec
# ---------------------------------------------------------------------------


class TestBondConstructionWithReturnFloor:
    """Bond spec dicts with return_floor round-trip through the JSON bridge."""

    def test_moic_floor_via_dict_constructor(self) -> None:
        """Bond(spec=...) accepts a return_floor with Moic kind."""
        spec = _plain_bond_spec(
            return_floor={
                "kind": {"Moic": 1.25},
                "issue_price": "Par",
                "window": "Full",
            }
        )
        bond = Bond(spec=spec)
        assert bond is not None

    def test_moic_floor_via_from_json(self) -> None:
        """Bond.from_json(...) round-trips a bond spec with return_floor."""
        spec = _plain_bond_spec(
            return_floor={
                "kind": {"Moic": 1.30},
                "issue_price": "Par",
                "window": "Full",
            }
        )
        tagged = json.dumps({"type": "bond", "spec": spec})
        bond = Bond.from_json(tagged)
        assert bond is not None

    def test_xirr_floor_via_dict_constructor(self) -> None:
        """Bond(spec=...) accepts a return_floor with Xirr kind."""
        spec = _plain_bond_spec(
            return_floor={
                "kind": {"Xirr": 0.12},
                "issue_price": "Par",
                "window": "Full",
            }
        )
        bond = Bond(spec=spec)
        assert bond is not None

    def test_no_return_floor_still_works(self) -> None:
        """Baseline: Bond without return_floor constructs and prices normally."""
        bond = Bond(spec=_plain_bond_spec())
        assert bond is not None

    def test_return_floor_oid_issue_price(self) -> None:
        """Bond with OID issue price (PctOfPar) in the return floor."""
        spec = _plain_bond_spec(
            return_floor={
                "kind": {"Moic": 1.20},
                "issue_price": {"PctOfPar": 98.0},
                "window": "Full",
            }
        )
        bond = Bond(spec=spec)
        assert bond is not None

    def test_return_floor_from_protection_window(self) -> None:
        """Bond with a From protection window (no-call period)."""
        spec = _plain_bond_spec(
            return_floor={
                "kind": {"Moic": 1.25},
                "issue_price": "Par",
                "window": {"From": "2026-01-01"},
            }
        )
        bond = Bond(spec=spec)
        assert bond is not None


# ---------------------------------------------------------------------------
# Metric registration tests
# ---------------------------------------------------------------------------


class TestReturnFloorMetricsRegistered:
    """The four return-floor metrics appear in the standard metric registry."""

    def test_moic_in_standard_metrics(self) -> None:
        metrics = list_standard_metrics()
        assert "moic" in metrics, f"'moic' not found in standard metrics: {metrics[:20]}"

    def test_moic_to_worst_in_standard_metrics(self) -> None:
        metrics = list_standard_metrics()
        assert "moic_to_worst" in metrics

    def test_xirr_in_standard_metrics(self) -> None:
        metrics = list_standard_metrics()
        assert "xirr" in metrics

    def test_xirr_to_worst_in_standard_metrics(self) -> None:
        metrics = list_standard_metrics()
        assert "xirr_to_worst" in metrics


# ---------------------------------------------------------------------------
# Metric computation tests
# ---------------------------------------------------------------------------


class TestReturnFloorMetricsComputed:
    """price_with_metrics returns sensible values for MOIC/XIRR on a 10% bullet bond."""

    AS_OF = "2024-01-01"
    MARKET = _market_json()

    def _bond_result(self, return_floor: dict | None = None) -> dict:
        bond = Bond(spec=_plain_bond_spec(return_floor=return_floor))
        result_json = bond.price_with_metrics(
            self.MARKET,
            self.AS_OF,
            metrics=["moic", "moic_to_worst", "xirr", "xirr_to_worst"],
        )
        return json.loads(result_json)

    def _measures(self, result: dict) -> dict:
        """Return the ``measures`` dict from a ``ValuationResult``."""
        return result["measures"]

    def test_plain_bond_moic_is_above_one(self) -> None:
        """10% annual 5Y bullet bond at par → MOIC > 1.0 (receives coupons + principal)."""
        result = self._bond_result()
        moic = self._measures(result)["moic"]
        assert moic > 1.0, f"Expected MOIC > 1.0, got {moic}"

    def test_plain_bond_moic_approximate(self) -> None:
        """10% annual 5Y par bullet: MOIC ≈ 1.50 (0.10 × 5 coupons + 1.0 principal)."""
        result = self._bond_result()
        moic = self._measures(result)["moic"]
        # 5 × 0.10 + 1.0 = 1.50
        assert abs(moic - 1.50) < 0.02, f"Expected MOIC ≈ 1.50, got {moic}"

    def test_plain_bond_xirr_approximate(self) -> None:
        """10% annual 5Y par bullet: XIRR ≈ 10% (equals coupon rate at par)."""
        result = self._bond_result()
        xirr = self._measures(result)["xirr"]
        assert abs(xirr - 0.10) < 0.005, f"Expected XIRR ≈ 0.10, got {xirr}"

    def test_moic_to_worst_le_moic_for_bullet(self) -> None:
        """For a bullet bond moic_to_worst == moic (only one exit path)."""
        result = self._bond_result()
        measures = self._measures(result)
        moic = measures["moic"]
        moic_tw = measures["moic_to_worst"]
        # Bullet has only the maturity exit so to-worst == to-maturity.
        assert abs(moic - moic_tw) < 1e-6, f"Bullet bond: expected moic_to_worst == moic, got {moic_tw} vs {moic}"

    def test_floored_bond_moic_same_as_unfloored_for_bullet(self) -> None:
        """MOIC metric is the same with or without a floor on a bullet bond.

        The floor only affects early-call redemption pricing. A bond with no
        calls returns the same MOIC regardless of whether a floor is attached.
        """
        unfloored = self._bond_result()
        floored = self._bond_result(
            return_floor={
                "kind": {"Moic": 1.25},
                "issue_price": "Par",
                "window": "Full",
            }
        )
        moic_uf = self._measures(unfloored)["moic"]
        moic_fl = self._measures(floored)["moic"]
        assert abs(moic_uf - moic_fl) < 1e-6, f"Floored bullet bond MOIC should equal unfloored: {moic_fl} vs {moic_uf}"

    def test_floored_bond_prices_without_error(self) -> None:
        """A bond with a 1.25× MOIC floor prices successfully (no panic/error)."""
        result = self._bond_result(
            return_floor={
                "kind": {"Moic": 1.25},
                "issue_price": "Par",
                "window": "Full",
            }
        )
        assert float(result["value"]["amount"]) > 0

    def test_xirr_floor_bond_prices_without_error(self) -> None:
        """A bond with a 12% XIRR floor prices successfully."""
        result = self._bond_result(
            return_floor={
                "kind": {"Xirr": 0.12},
                "issue_price": "Par",
                "window": "Full",
            }
        )
        assert float(result["value"]["amount"]) > 0


# ---------------------------------------------------------------------------
# to_json round-trip: return_floor survives serialisation
# ---------------------------------------------------------------------------


class TestReturnFloorJsonRoundtrip:
    """return_floor spec survives Bond.to_json() → Bond.from_json() round-trip."""

    def test_moic_floor_round_trips(self) -> None:
        spec = _plain_bond_spec(
            return_floor={
                "kind": {"Moic": 1.35},
                "issue_price": "Par",
                "window": "Full",
            }
        )
        bond = Bond(spec=spec)
        json_out = bond.to_json()
        parsed = json.loads(json_out)
        # Canonical form has type tag at top level
        inner_spec = parsed.get("spec", parsed)
        return_floor = inner_spec.get("return_floor")
        assert return_floor is not None, f"return_floor missing from round-tripped JSON:\n{json_out[:500]}"

    def test_xirr_floor_round_trips(self) -> None:
        spec = _plain_bond_spec(
            return_floor={
                "kind": {"Xirr": 0.15},
                "issue_price": {"PctOfPar": 97.5},
                "window": {"From": "2025-01-01"},
            }
        )
        bond = Bond(spec=spec)
        json_out = bond.to_json()
        parsed = json.loads(json_out)
        inner_spec = parsed.get("spec", parsed)
        return_floor = inner_spec.get("return_floor")
        assert return_floor is not None, f"return_floor missing from round-tripped JSON:\n{json_out[:500]}"
