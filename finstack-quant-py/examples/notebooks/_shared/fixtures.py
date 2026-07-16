"""Load immutable, versioned notebook demonstration fixtures."""

from __future__ import annotations

import copy
import json
from typing import Any

from .paths import fixture_path


def _catalog(version: str) -> dict[str, Any]:
    path = fixture_path("catalog.json", version=version)
    payload = json.loads(path.read_text(encoding="utf-8"))
    expected = int(version.removeprefix("v"))
    if payload.get("schema_version") != expected:
        msg = f"{path} has an unexpected schema_version"
        raise ValueError(msg)
    return payload


def load_instrument_fixture(name: str, *, version: str = "v1") -> dict[str, Any]:
    """Return an independent copy of a named instrument wire payload."""
    catalog = _catalog(version)
    try:
        payload = catalog["instruments"][name]
    except KeyError as exc:
        msg = f"Unknown instrument fixture {name!r} in {version}"
        raise KeyError(msg) from exc
    return copy.deepcopy(payload)


def load_portfolio_fixture(name: str, *, version: str = "v1") -> dict[str, Any]:
    """Return a portfolio fixture with named instruments expanded in place."""
    catalog = _catalog(version)
    try:
        payload = copy.deepcopy(catalog["portfolios"][name])
    except KeyError as exc:
        msg = f"Unknown portfolio fixture {name!r} in {version}"
        raise KeyError(msg) from exc

    for position in payload["positions"]:
        fixture_name = position.pop("instrument_fixture")
        position["instrument_spec"] = load_instrument_fixture(
            fixture_name, version=version
        )
        position["instrument_id"] = position["instrument_spec"]["spec"]["id"]
    return payload
