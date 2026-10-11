"""Identity checks for compact materialization baseline documents."""

from __future__ import annotations

import re

_BASELINE_NAME = re.compile(r"^[a-z0-9][a-z0-9.-]*$")
_SHA256 = re.compile(r"^[0-9a-f]{64}$")


def require_baseline_name(value: object) -> str:
    """Return a baseline name, rejecting anything that is not a lowercase token."""
    if not isinstance(value, str) or _BASELINE_NAME.fullmatch(value) is None:
        raise ValueError("baseline_name must match ^[a-z0-9][a-z0-9.-]*$")
    return value


def require_tree_revision(value: object) -> str:
    """Return a tree revision, rejecting a missing or empty value."""
    if not isinstance(value, str) or not value:
        raise ValueError("tree_revision must be a non-empty string")
    return value


def require_sha256(value: object, label: str) -> str:
    """Return a lowercase SHA-256 digest, rejecting any other string."""
    if not isinstance(value, str) or _SHA256.fullmatch(value) is None:
        raise ValueError(f"{label} must be a lowercase SHA-256 digest")
    return value
