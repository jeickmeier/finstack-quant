# finstack-quant-py/tests/test_reporting_theme.py
from __future__ import annotations

import dataclasses

from finstack_quant.reporting.theme import INSTITUTIONAL, Theme


def test_institutional_is_a_theme() -> None:
    assert isinstance(INSTITUTIONAL, Theme)
    assert INSTITUTIONAL.name == "institutional"


def test_to_css_scopes_all_rules() -> None:
    css = INSTITUTIONAL.to_css("fq-ts")
    assert css.startswith("<style>")
    assert css.rstrip().endswith("</style>")
    # Every rule is scoped under the container class.
    assert ".fq-ts" in css
    assert INSTITUTIONAL.ink in css  # token color emitted


def test_theme_is_overridable() -> None:
    dark = dataclasses.replace(INSTITUTIONAL, name="dark", canvas="#0b0f17")
    assert dark.canvas == "#0b0f17"
    assert INSTITUTIONAL.canvas != "#0b0f17"  # frozen original unchanged


def test_to_css_has_scrollbox_rule() -> None:
    css = INSTITUTIONAL.to_css("fq-ts")
    assert "fq-scroll" in css
