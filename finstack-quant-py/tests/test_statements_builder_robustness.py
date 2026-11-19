"""Regression tests for statements binding robustness and exact-ID behavior.

Covers two quant-review findings:

- A failed ``compute`` / ``periods`` / mixed-node ``formula`` must not brick the
  builder. The Rust builder methods consume ``self`` and return ``Err`` on a bad
  argument, so the wrapper has to validate *before* taking ownership, or every
  later call fails with a misleading "consumed by build()" error.
- Malformed JSON must raise ``ValueError`` at every entry point, not
  ``RuntimeError`` at some and ``ValueError`` at others.
"""

from __future__ import annotations

import pytest

from finstack_quant import statements


def test_name_normalization_surface_is_removed() -> None:
    builder = statements.ModelBuilder("exact-ids")

    assert not hasattr(builder, "with_name_normalization")


def test_model_builder_from_spec_preserves_state_and_accepts_transformations() -> None:
    original = statements.ModelBuilder("rebuild")
    original.periods("2025Q1..Q2", "2025Q1")
    original.value("revenue", [("2025Q1", 100.0)])
    model = original.build()

    rebuilt = statements.ModelBuilder.from_spec(model)
    rebuilt.compute("margin", "revenue * 0.5")
    transformed = rebuilt.build()

    assert transformed.id == "rebuild"
    assert transformed.has_node("revenue")
    assert transformed.has_node("margin")
    assert transformed.to_json() != model.to_json()


class TestBuilderSurvivesBadInput:
    def test_bad_compute_does_not_brick_the_builder(self) -> None:
        b = statements.ModelBuilder("brick")
        b.periods("2025Q1..Q1", None)
        b.value("revenue", [("2025Q1", 100.0)])

        with pytest.raises(ValueError, match="parse error"):
            b.compute("bad", "revenue -* cogs")  # syntax error

        # The builder must still be usable after the typo.
        b.compute("good", "revenue * 2")
        model = b.build()
        assert model.node_count == 2

    def test_bad_periods_does_not_brick_the_builder(self) -> None:
        b = statements.ModelBuilder("periods-brick")
        with pytest.raises(ValueError, match="invalid period range"):
            b.periods("not-a-range", None)
        # Retry with a valid range must succeed.
        b.periods("2025Q1..Q1", None)
        b.value("revenue", [("2025Q1", 100.0)])
        assert b.build().node_count == 1

    def test_bad_mixed_formula_does_not_brick_the_node_builder(self) -> None:
        b = statements.ModelBuilder("mixed-brick")
        b.periods("2025Q1..Q1", None)
        b.value("revenue", [("2025Q1", 100.0)])
        mixed = b.mixed("margin")
        with pytest.raises(ValueError, match="parse error"):
            mixed.formula("revenue -* 2")  # syntax error
        # The node builder must still be usable.
        mixed.formula("revenue * 0.5")
        b2 = mixed.build()
        assert b2.build().node_count == 2


class TestMalformedJsonRaisesValueError:
    def test_model_from_json(self) -> None:
        with pytest.raises(ValueError, match="validation failed") as malformed:
            statements.FinancialModelSpec.from_json("not json")
        assert "expected ident" in malformed.value.report[0]["message"]

    def test_registry_load_from_json_str(self) -> None:
        # Previously raised RuntimeError (Serde was mapped as operational),
        # inconsistent with model.from_json above.
        with pytest.raises(ValueError, match="Serialization error"):
            statements.Registry().load_from_json_str("not json")
