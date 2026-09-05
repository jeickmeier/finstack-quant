"""Behavioral tests for the core value-type ergonomics added to the Python bindings.

Covers ``Rate``/``Bps``/``Percentage`` conversions and string parsing,
``CreditRating`` ordering and string equality, ``Attributes`` mapping dunders,
``Money`` formatting/division/rounding, ``FinstackConfig`` scale overrides,
``ScorecardScale`` validation and the ``core.math`` additions.
"""

from __future__ import annotations

from decimal import Decimal
import math
import pickle

import pytest

from finstack_quant.core.config import FinstackConfig, RoundingMode, ToleranceConfig
from finstack_quant.core.money import Money
from finstack_quant.core.rating_scales import RatingLevel, ScorecardScale, embedded_registry
from finstack_quant.core.types import Attributes, Bps, CreditRating, CurveId, InstrumentId, Percentage, Rate


class TestRateFamily:
    def test_rate_string_constructor(self) -> None:
        assert Rate("5%") == Rate(0.05) == Rate("500bp") == Rate("500 bps") == Rate("0.05")
        with pytest.raises(ValueError, match=r"invalid rate"):
            Rate("five")

    def test_cross_conversions(self) -> None:
        assert Rate(0.05).as_bps == Bps(500)
        assert Rate(0.05).as_percentage == Percentage(5.0)
        assert Bps(250).as_rate == Rate(0.025)
        assert Bps(250).as_percentage == Percentage(2.5)
        assert Bps(250).as_percent == 2.5
        assert Percentage(2.5).as_bps == Bps(250)
        assert Percentage(2.5).as_rate == Rate(0.025)

    def test_predicates_and_abs(self) -> None:
        assert Rate(-0.01).abs() == Rate(0.01)
        assert Rate(-0.01).is_negative()
        assert not Rate(-0.01).is_positive()
        assert Bps(0).is_zero()
        assert Percentage(-1.0).abs().is_positive()

    def test_arithmetic(self) -> None:
        assert (Rate(0.05) + Bps(25)).as_bp == 525
        assert (Rate(0.05) - Bps(25)).as_bp == 475
        assert 2.0 * Rate(0.05) == Rate(0.10)
        assert (Percentage(10.0) + Percentage(2.5)) == Percentage(12.5)
        assert (Percentage(10.0) / 4.0) == Percentage(2.5)
        assert -Percentage(1.0) == Percentage(-1.0)
        assert 3 * Bps(10) == Bps(30)

    def test_json_and_pickle(self) -> None:
        for value in (Rate(0.05), Bps(250), Percentage(12.5)):
            assert type(value).from_json(value.to_json()) == value
            assert pickle.loads(pickle.dumps(value)) == value  # noqa: S301


class TestCreditRating:
    def test_ctor_and_predicates(self) -> None:
        assert CreditRating("Baa1") == CreditRating.BBB_PLUS
        assert CreditRating.BBB_MINUS.is_investment_grade()
        assert CreditRating.BB_PLUS.is_speculative_grade()
        assert not CreditRating.NR.is_speculative_grade()
        assert CreditRating.D.is_default()
        assert CreditRating.BBB_PLUS.to_moodys_string() == "Baa1"

    def test_ordering_and_string_equality(self) -> None:
        assert CreditRating.AAA < CreditRating.BBB < CreditRating.C < CreditRating.NR < CreditRating.D
        assert CreditRating.BBB == "BBB"
        assert CreditRating.BBB == "baa2"
        assert CreditRating.BBB != "not a rating"
        assert CreditRating.BBB.notches_to(CreditRating.BB) == 3
        assert CreditRating.BB.notches_to("BBB") == -3
        assert {CreditRating.BBB, CreditRating("BBB")} == {CreditRating.BBB}

    def test_json_and_pickle(self) -> None:
        assert CreditRating.from_json('"BBB+"') == CreditRating.BBB_PLUS
        assert pickle.loads(pickle.dumps(CreditRating.CCC_MINUS)) == CreditRating.CCC_MINUS  # noqa: S301


class TestIdsAndAttributes:
    def test_ids(self) -> None:
        assert CurveId("A") < CurveId("B")
        assert len(InstrumentId("BOND")) == 4
        assert CurveId("").is_empty()
        assert CurveId.from_json(CurveId("USD-OIS").to_json()) == CurveId("USD-OIS")
        assert pickle.loads(pickle.dumps(InstrumentId("X"))) == InstrumentId("X")  # noqa: S301

    def test_attributes_mapping(self) -> None:
        attrs = Attributes()
        attrs.add_tag("energy")
        attrs.set_meta("region", "NA")
        attrs.set_meta("rank", 3)
        assert attrs.tags == ["energy"]
        assert attrs.has_tag("energy")
        assert attrs["region"] == "NA"
        assert attrs["rank"] == "3"
        assert "region" in attrs
        assert "missing" not in attrs
        assert len(attrs) == 2
        assert attrs.items() == [("rank", "3"), ("region", "NA")]
        assert attrs.matches_selector("tag:energy")
        assert attrs.matches_selector("meta:region=NA")
        assert not attrs.matches_selector("bogus")
        with pytest.raises(KeyError):
            attrs["missing"]
        assert Attributes.from_json(attrs.to_json()) == attrs
        assert pickle.loads(pickle.dumps(attrs)) == attrs  # noqa: S301


class TestMoney:
    def test_string_amount_and_format(self) -> None:
        m = Money("1234567.891", "USD")
        assert m.amount_decimal == Decimal("1234567.891")
        assert m.format(group=",") == "USD 1,234,567.89"
        assert m.format(decimals=0, show_currency=False, rounding="floor") == "1234567"
        assert m.format(decimals=0, show_currency=False, rounding=RoundingMode.CEIL) == "1234568"

    def test_division_and_rounding(self) -> None:
        assert Money(300.0, "USD") / Money(100.0, "USD") == 3.0
        assert Money(300.0, "USD") / 3.0 == Money(100.0, "USD")
        with pytest.raises(ValueError, match="division by zero"):
            Money(1.0, "USD") / 0
        with pytest.raises(ValueError, match="Currency mismatch: expected USD, got EUR"):
            Money(1.0, "USD") / Money(1.0, "EUR")
        with pytest.raises(ValueError, match="Currency mismatch: expected USD, got EUR"):
            _ = Money(1.0, "USD") < Money(1.0, "EUR")
        assert abs(Money(-2.5, "USD")) == Money(2.5, "USD")
        assert -Money(2.5, "USD") == Money(-2.5, "USD")
        assert float(Money(2.5, "USD")) == 2.5
        assert round(Money("2.345", "USD")) == Money("2.34", "USD")
        assert round(Money("2.355", "USD"), 2) == Money("2.36", "USD")

    def test_from_tuple_and_config(self) -> None:
        assert Money.from_tuple((Decimal("1.25"), "EUR")).amount_decimal == Decimal("1.25")
        cfg = FinstackConfig(rounding_mode="floor")
        cfg.set_ingest_scale("USD", 1)
        assert Money(1.29, "USD", config=cfg) == Money("1.2", "USD")

    @pytest.mark.parametrize("amount", [1.2345, "1.2345", Decimal("1.2345")])
    def test_config_rounds_every_input_representation(self, amount: float | str | Decimal) -> None:
        cfg = FinstackConfig()
        cfg.set_ingest_scale("USD", 2)
        assert Money(amount, "USD", config=cfg).amount_decimal == Decimal("1.23")

    @pytest.mark.parametrize("amount", ["9007199254740993.125", Decimal("9007199254740993.125")])
    def test_config_preserves_exact_decimal_before_rounding(self, amount: str | Decimal) -> None:
        cfg = FinstackConfig(rounding_mode="floor")
        cfg.set_ingest_scale("USD", 2)
        assert Money(amount, "USD", config=cfg).amount_decimal == Decimal("9007199254740993.12")
        assert Money(amount, "USD").amount_decimal == Decimal("9007199254740993.125")


class TestConfig:
    def test_scale_overrides_and_eq(self) -> None:
        cfg = FinstackConfig(rounding_mode="floor")
        assert cfg.rounding_mode == RoundingMode.FLOOR
        cfg.set_output_scale("JPY", 2)
        assert cfg.output_scale("JPY") == 2
        assert cfg.output_scale_overrides() == {"JPY": 2}
        assert cfg.ingest_scale_overrides() == {}
        assert cfg == FinstackConfig.from_json(cfg.to_json())
        assert cfg != FinstackConfig()
        assert "floor" in repr(cfg)
        with pytest.raises(ValueError, match=r"unknown rounding mode"):
            RoundingMode.from_name("BANKERS")
        assert RoundingMode.from_json('"ceil"').name == "ceil"
        assert pickle.loads(pickle.dumps(RoundingMode.CEIL)) == RoundingMode.CEIL  # noqa: S301
        t = ToleranceConfig(rate_epsilon=1e-9)
        assert ToleranceConfig.from_json(t.to_json()) == t


class TestRatingScales:
    def test_validation(self) -> None:
        with pytest.raises(ValueError, match=r"blank name"):
            RatingLevel("", 70.0, 65.0)
        with pytest.raises(ValueError, match=r"invalid rating level score"):
            RatingLevel("BBB", 170.0, 65.0)
        with pytest.raises(ValueError, match=r"not ordered best-to-worst"):
            ScorecardScale("bad", [RatingLevel("B", 70.0, 65.0), RatingLevel("A", 90.0, 85.0)])
        scale = ScorecardScale("ok", [RatingLevel("A", 90.0, 85.0), RatingLevel("B", 70.0, 65.0)])
        assert [level.name for level in scale] == ["A", "B"]
        assert scale[-1] == RatingLevel("B", 70.0, 65.0)
        assert scale == ScorecardScale.from_json(scale.to_json())
        assert list(scale.to_dataframe()["name"]) == ["A", "B"]

    def test_registry_scale_ids(self) -> None:
        registry = embedded_registry()
        assert "sp" in registry.scale_ids()
        assert registry == embedded_registry()


class TestMathAdditions:
    def test_stats(self) -> None:
        from finstack_quant.core.math import stats

        assert stats.mean_var([1.0, 2.0, 3.0]) == (2.0, 1.0)
        assert math.isnan(stats.mean_or_nan([]))
        assert stats.median_or_nan([3.0, 1.0, 2.0, 4.0]) == 2.5
        assert stats.finite_count([1.0, float("nan")]) == 1
        assert len(stats.log_returns([1.0, 2.0, 4.0])) == 2
        assert stats.realized_variance([100.0, 101.0, 100.0], annualization_factor=1.0) > 0.0
        with pytest.raises(ValueError, match=r"requires OHLC data"):
            stats.realized_variance([100.0, 101.0], method="parkinson")

    def test_linalg_and_special(self) -> None:
        from finstack_quant.core.math import linalg, special_functions

        values, _ = linalg.symmetric_eigen([[2.0, 0.0], [0.0, 5.0]])
        assert sorted(round(v, 10) for v in values) == [2.0, 5.0]
        _, delta = linalg.ledoit_wolf_shrinkage([[1.0, 1.0], [-1.0, -1.0], [2.0, -2.0], [-2.0, 2.0]])
        assert abs(delta - 17.0 / 18.0) < 1e-12
        with pytest.raises(ValueError, match="square nested list"):
            linalg.cholesky_decomposition([1.0, 2.0])
        assert special_functions.norm_cdf_with_params(1.0, 1.0, 2.0) == pytest.approx(0.5)
        with pytest.raises(ValueError, match=r"std_dev must be finite and positive"):
            special_functions.norm_pdf_with_params(0.0, 0.0, -1.0)
