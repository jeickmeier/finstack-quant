"""Parity tests for core module: currency, money, dates, market data, linalg.

Validates that the Python bindings produce results consistent with the
underlying Rust implementation.
"""

from datetime import date
from decimal import Decimal
from inspect import signature
import json
import math

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import (
    DayCount,
    DayCountContext,
    HolidayCalendar,
    PeriodId,
    SifmaSettlementClass,
    Tenor,
    TenorUnit,
    build_periods,
    sifma_settlement_date,
    sifma_settlement_date_for_class,
)
from finstack_quant.core.market_data import (
    BaseCorrelationCurve,
    CreditIndexData,
    DiscountCurve,
    ForwardCurve,
    FxConversionPolicy,
    FxMatrix,
    HazardCurve,
    InflationIndex,
    MarketContext,
    ScalarTimeSeries,
)
from finstack_quant.core.money import Money
from finstack_quant.core.types import Bps, CreditRating, Percentage, Rate


def test_tenor_constructor_uses_checked_rust_validation() -> None:
    with pytest.raises(ValueError, match="count must be positive"):
        Tenor(0, TenorUnit.MONTHS)
    with pytest.raises(ValueError, match="exceeds maximum"):
        Tenor(2**32 - 1, TenorUnit.YEARS)


def test_bps_constructor_rejects_rounded_i32_overflow() -> None:
    assert Bps(2_147_483_647).as_bps == 2_147_483_647
    with pytest.raises(ValueError, match="overflow"):
        Bps(2_147_483_648)


class TestCurrencyParity:
    """Currency construction and property access match Rust."""

    def test_construction_and_properties(self) -> None:
        """USD should have code 'USD', numeric 840, decimals 2."""
        usd = Currency("USD")
        assert usd.code == "USD"
        assert usd.numeric == 840
        assert usd.decimals == 2

    def test_case_insensitive(self) -> None:
        """Lowercase input should resolve identically."""
        usd1 = Currency("USD")
        usd2 = Currency("usd")
        assert usd1.code == usd2.code
        assert usd1.numeric == usd2.numeric

    def test_equality(self) -> None:
        """Same-code currencies are equal; different codes are not."""
        usd1 = Currency("USD")
        usd2 = Currency("USD")
        eur = Currency("EUR")
        assert usd1 == usd2
        assert usd1 != eur

    def test_major_currency_numeric_codes(self) -> None:
        """Major currencies map to their ISO 4217 numeric codes."""
        assert Currency("USD").numeric == 840
        assert Currency("EUR").numeric == 978
        assert Currency("GBP").numeric == 826
        assert Currency("JPY").numeric == 392

    def test_invalid_code_raises(self) -> None:
        """An unrecognised code should raise."""
        with pytest.raises(Exception, match=r"[Uu]nknown|[Ii]nvalid|Currency"):
            Currency("INVALID")


class TestMoneyParity:
    """Money arithmetic and construction match Rust."""

    @pytest.fixture
    def usd(self) -> Currency:
        """Shared USD currency."""
        return Currency("USD")

    def test_construction(self, usd: Currency) -> None:
        """Amount and currency round-trip correctly."""
        m = Money(100.50, usd)
        assert m.amount == pytest.approx(100.50)
        assert m.currency.code == "USD"

    def test_addition(self, usd: Currency) -> None:
        """Same-currency addition."""
        result = Money(100.0, usd) + Money(50.0, usd)
        assert result.amount == pytest.approx(150.0)
        assert result.currency.code == "USD"

    def test_subtraction(self, usd: Currency) -> None:
        """Same-currency subtraction."""
        result = Money(100.0, usd) - Money(30.0, usd)
        assert result.amount == pytest.approx(70.0)

    def test_multiplication(self, usd: Currency) -> None:
        """Scalar multiplication."""
        result = Money(100.0, usd) * 2.5
        assert result.amount == pytest.approx(250.0)

    def test_division(self, usd: Currency) -> None:
        """Scalar division."""
        result = Money(100.0, usd) / 4.0
        assert result.amount == pytest.approx(25.0)

    def test_negation(self, usd: Currency) -> None:
        """Unary negation."""
        result = -Money(100.0, usd)
        assert result.amount == pytest.approx(-100.0)

    def test_currency_mismatch_raises(self) -> None:
        """Adding different currencies should raise."""
        m1 = Money(100.0, Currency("USD"))
        m2 = Money(50.0, Currency("EUR"))
        with pytest.raises(Exception, match=r"[Cc]urrency|[Mm]ismatch"):
            m1 + m2

    def test_zero_value(self, usd: Currency) -> None:
        """Zero money stores correctly."""
        assert Money(0.0, usd).amount == pytest.approx(0.0)

    def test_negative_value(self, usd: Currency) -> None:
        """Negative amounts are allowed."""
        assert Money(-50.0, usd).amount == pytest.approx(-50.0)

    def test_large_value(self, usd: Currency) -> None:
        """Trillion-scale amounts survive the round trip."""
        assert Money(1e12, usd).amount == pytest.approx(1e12)

    def test_small_value(self, usd: Currency) -> None:
        """Sub-cent amounts survive the round trip."""
        assert Money(0.01, usd).amount == pytest.approx(0.01)

    def test_nonzero_scalar_right_subtraction_rejected(self, usd: Currency) -> None:
        """Money rejects nonzero scalar - Money just like nonzero scalar + Money."""
        with pytest.raises(TypeError, match="unsupported right operand"):
            5.0 - Money(1.0, usd)


class TestCoreTypesParity:
    """Core scalar type contracts."""

    def test_rate_from_percent_uses_fallible_rust_validation(self) -> None:
        assert Rate.from_percent(2.5).as_decimal == pytest.approx(0.025)
        for value in (math.nan, math.inf, -math.inf):
            with pytest.raises(ValueError, match="finite"):
                Rate.from_percent(value)

    def test_rate_hash_matches_equality_for_signed_zero(self) -> None:
        pos_zero = Rate(0.0)
        neg_zero = Rate(-0.0)

        assert pos_zero == neg_zero
        assert hash(pos_zero) == hash(neg_zero)

    def test_student_t_invalid_degrees_of_freedom_raises(self) -> None:
        from finstack_quant.core.math import special_functions

        with pytest.raises(ValueError, match="df"):
            special_functions.student_t_cdf(0.0, 0.0)
        with pytest.raises(ValueError, match="df"):
            special_functions.student_t_inv_cdf(0.5, math.nan)

    def test_percentage_hash_matches_equality_for_signed_zero(self) -> None:
        pos_zero = Percentage(0.0)
        neg_zero = Percentage(-0.0)

        assert pos_zero == neg_zero
        assert hash(pos_zero) == hash(neg_zero)

    def test_credit_rating_notches_and_warf_are_preserved(self) -> None:
        assert CreditRating.from_name("BBB+") == CreditRating.BBB_PLUS
        assert CreditRating.from_name("Baa1") == CreditRating.BBB_PLUS
        assert CreditRating.BBB_PLUS.name == "BBB+"
        assert CreditRating.BBB_PLUS.warf > 0.0


class TestDayCountParity:
    """Day-count convention calculations match Rust."""

    def test_act360_year_fraction(self) -> None:
        """ACT/360: 182 calendar days / 360."""
        start, end = date(2024, 1, 1), date(2024, 7, 1)
        yf = DayCount.ACT_360.year_fraction(start, end)
        assert yf == pytest.approx(182.0 / 360.0, abs=1e-10)

    def test_act365f_year_fraction(self) -> None:
        """ACT/365F: 182 calendar days / 365."""
        start, end = date(2024, 1, 1), date(2024, 7, 1)
        yf = DayCount.ACT_365F.year_fraction(start, end)
        assert yf == pytest.approx(182.0 / 365.0, abs=1e-10)

    def test_thirty360_year_fraction(self) -> None:
        """30/360: exactly 6 months = 0.5."""
        start, end = date(2024, 1, 15), date(2024, 7, 15)
        yf = DayCount.THIRTY_360.year_fraction(start, end)
        assert yf == pytest.approx(180.0 / 360.0, abs=1e-10)

    def test_thirty_e_360_isda_termination_context(self) -> None:
        start, end = date(2025, 1, 31), date(2025, 2, 28)
        regular = DayCount.THIRTY_E_360_ISDA.year_fraction(start, end)
        terminal = DayCount.THIRTY_E_360_ISDA.year_fraction(start, end, DayCountContext(end_is_termination_date=True))
        assert regular == pytest.approx(30.0 / 360.0)
        assert terminal == pytest.approx(28.0 / 360.0)

    def test_calendar_days_static(self) -> None:
        """calendar_days is a static method returning signed day count."""
        start, end = date(2024, 1, 1), date(2024, 1, 31)
        assert DayCount.calendar_days(start, end) == 30

    def test_same_date_zero_fraction(self) -> None:
        """Year fraction is zero when start == end."""
        d = date(2024, 1, 1)
        assert DayCount.ACT_360.year_fraction(d, d) == pytest.approx(0.0)

    def test_coupon_period_is_validated_by_core(self) -> None:
        with pytest.raises(Exception, match="coupon period start must be before end"):
            DayCountContext(coupon_period=(date(2025, 7, 1), date(2025, 1, 1)))

    def test_calendar_metadata_exposes_weekend_rule(self) -> None:
        metadata = HolidayCalendar("usny").metadata
        assert metadata is not None
        assert metadata.weekend_rule == "saturday_sunday"


class TestPeriodParity:
    """Period ID construction and build_periods match Rust."""

    def test_monthly_period_id(self) -> None:
        """Monthly PeriodId round-trips code, year, index."""
        pid = PeriodId.month(2024, 6)
        assert pid.code == "2024M06"
        assert pid.year == 2024
        assert pid.index == 6

    def test_quarterly_period_id(self) -> None:
        """Quarterly PeriodId round-trips."""
        pid = PeriodId.quarter(2024, 2)
        assert pid.code == "2024Q2"
        assert pid.year == 2024
        assert pid.index == 2

    def test_annual_period_id(self) -> None:
        """Annual PeriodId round-trips."""
        pid = PeriodId.annual(2024)
        assert pid.code == "2024"
        assert pid.year == 2024

    def test_invalid_period_ids_are_rejected(self) -> None:
        with pytest.raises(ValueError, match=r"(?i)invalid"):
            PeriodId.month(2025, 13)
        with pytest.raises(ValueError, match=r"(?i)invalid"):
            PeriodId.quarter(2025, 0)
        with pytest.raises(ValueError, match=r"(?i)invalid"):
            PeriodId.week(2021, 53)
        with pytest.raises(ValueError, match=r"(?i)invalid"):
            PeriodId.day(2025, 366)

    def test_build_periods_quarterly(self) -> None:
        """Build 4 quarterly periods from a range string."""
        plan = build_periods("2024Q1..Q4", None)
        assert len(plan.periods) == 4
        assert plan.periods[0].id.code == "2024Q1"
        assert plan.periods[3].id.code == "2024Q4"

    def test_build_periods_monthly(self) -> None:
        """Build 12 monthly periods from a range string."""
        plan = build_periods("2024M01..M12", None)
        assert len(plan.periods) == 12
        assert plan.periods[0].id.code == "2024M01"
        assert plan.periods[11].id.code == "2024M12"

    def test_build_periods_with_actuals(self) -> None:
        """Actuals cutoff correctly partitions periods."""
        plan = build_periods("2024Q1..Q4", "2024Q2")
        assert len(plan.periods) == 4
        assert plan.periods[0].is_actual
        assert plan.periods[1].is_actual
        assert not plan.periods[2].is_actual
        assert not plan.periods[3].is_actual


def test_sifma_settlement_classes_and_default() -> None:
    assert SifmaSettlementClass.from_agency_term("FNMA", 30) == SifmaSettlementClass.A
    assert SifmaSettlementClass.from_agency_term("GNMA", 30) == SifmaSettlementClass.C
    assert SifmaSettlementClass.from_agency_term("FNMA", 15) == SifmaSettlementClass.B
    assert sifma_settlement_date(1, 2026) == date(2026, 1, 14)
    assert sifma_settlement_date_for_class(1, 2026, SifmaSettlementClass.B) == date(2026, 1, 20)


class TestDiscountCurveParity:
    """Discount curve operations match Rust."""

    @pytest.fixture
    def curve(self) -> DiscountCurve:
        """Standard test curve."""
        return DiscountCurve(
            "USD-TEST",
            date(2024, 1, 1),
            [(0.0, 1.0), (1.0, 0.95), (5.0, 0.75), (10.0, 0.50)],
            day_count="act_365f",
        )

    def test_construction(self, curve: DiscountCurve) -> None:
        """ID and base_date survive construction."""
        assert curve.id == "USD-TEST"
        assert curve.base_date == date(2024, 1, 1)

    def test_df_at_knot(self, curve: DiscountCurve) -> None:
        """Discount factor at an exact knot matches the input."""
        assert curve.df(1.0) == pytest.approx(0.95, abs=1e-10)

    def test_df_interpolation(self) -> None:
        """Interpolated DF lies between adjacent knots."""
        curve = DiscountCurve(
            "USD-TEST",
            date(2024, 1, 1),
            [(0.0, 1.0), (1.0, 0.95), (2.0, 0.90)],
            day_count="act_365f",
        )
        df = curve.df(1.5)
        assert 0.89 < df < 0.96

    def test_zero_rate_consistency(self, curve: DiscountCurve) -> None:
        """exp(-z * t) recovers the discount factor."""
        t = 1.0
        df = curve.df(t)
        z = curve.zero(t)
        assert df == pytest.approx(math.exp(-z * t), abs=1e-8)

    def test_df_at_time_zero(self, curve: DiscountCurve) -> None:
        """DF at t=0 is 1.0."""
        assert curve.df(0.0) == pytest.approx(1.0, abs=1e-10)

    def test_default_day_count_uses_rust_curve_id_inference(self) -> None:
        """USD discount curves default to the Rust-inferred Act/360 market basis."""
        curve = DiscountCurve(
            "USD-OIS",
            date(2024, 1, 1),
            [(0.0, 1.0), (1.0, 0.95)],
        )
        context = MarketContext()
        context.insert(curve)

        state = json.loads(context.to_json())
        assert state["curves"][0]["day_count"] == "Act360"

    def test_explicit_day_count_still_overrides_curve_id_inference(self) -> None:
        """Users can still override the inferred day-count convention explicitly."""
        curve = DiscountCurve(
            "USD-OIS",
            date(2024, 1, 1),
            [(0.0, 1.0), (1.0, 0.95)],
            day_count="act_365f",
        )
        context = MarketContext()
        context.insert(curve)

        state = json.loads(context.to_json())
        assert state["curves"][0]["day_count"] == "Act365F"

    def test_flat_uses_continuous_compounding(self) -> None:
        curve = DiscountCurve.flat("USD-OIS", date(2024, 1, 1), 0.04)

        for t in [0.0, 0.25, 1.0, 5.0, 30.0]:
            assert curve.df(t) == pytest.approx(math.exp(-0.04 * t), abs=1e-12)
        assert curve.forward(2.0, 9.0) == pytest.approx(0.04, abs=1e-12)

    def test_negative_rate_validation_mode_accepts_increasing_discount_factors(self) -> None:
        knots = [(0.0, 1.0), (1.0, 1.002), (2.0, 1.004)]
        with pytest.raises(ValueError, match="non-increasing"):
            DiscountCurve("CHF-OIS", date(2024, 1, 1), knots)

        curve = DiscountCurve(
            "CHF-OIS",
            date(2024, 1, 1),
            knots,
            validation_mode="negative_rate_friendly",
            forward_floor=-0.01,
        )
        assert curve.df(2.0) == pytest.approx(1.004)
        assert curve.forward(0.0, 1.0) < 0.0

    def test_negative_rate_validation_mode_enforces_forward_floor(self) -> None:
        with pytest.raises(ValueError, match="below minimum"):
            DiscountCurve(
                "CHF-OIS",
                date(2024, 1, 1),
                [(0.0, 1.0), (1.0, 1.02)],
                validation_mode="negative_rate_friendly",
                forward_floor=-0.01,
            )

    @pytest.mark.parametrize("forward_floor", [float("nan"), float("inf"), -float("inf")])
    def test_negative_rate_validation_mode_requires_finite_floor(self, forward_floor: float) -> None:
        with pytest.raises(ValueError, match="forward_floor must be finite"):
            DiscountCurve(
                "CHF-OIS",
                date(2024, 1, 1),
                [(0.0, 1.0), (1.0, 1.002)],
                validation_mode="negative_rate_friendly",
                forward_floor=forward_floor,
            )


class TestForwardCurveParity:
    """Forward curve operations match Rust."""

    def test_construction(self) -> None:
        """ID and base_date survive construction."""
        curve = ForwardCurve(
            "USD-SOFR",
            0.25,
            knots=[(0.0, 0.04), (1.0, 0.045), (5.0, 0.05)],
            base_date=date(2024, 1, 1),
            day_count="act_360",
        )
        assert curve.id == "USD-SOFR"
        assert curve.base_date == date(2024, 1, 1)

    def test_rate_at_knot(self) -> None:
        """Forward rate at an exact knot matches input."""
        curve = ForwardCurve(
            "USD-SOFR",
            0.25,
            knots=[(0.0, 0.04), (1.0, 0.045)],
            base_date=date(2024, 1, 1),
            day_count="act_360",
        )
        assert curve.rate(1.0) == pytest.approx(0.045, abs=1e-10)

    def test_preexisting_optional_arguments_remain_positional(self) -> None:
        """day_count, interp, and extrapolation preserve their positional order."""
        curve = ForwardCurve(
            "USD-SOFR",
            0.25,
            [(0.0, 0.04), (1.0, 0.045)],
            date(2024, 1, 1),
            "act_360",
            "linear",
            "flat_forward",
        )
        assert curve.rate(1.0) == pytest.approx(0.045, abs=1e-10)
        assert curve.projection_grid is None

    def test_reset_lag_is_constructible_and_readonly(self) -> None:
        curve = ForwardCurve(
            "USD-SOFR",
            0.25,
            [(0.0, 0.04), (1.0, 0.045)],
            date(2024, 1, 1),
            "act_360",
            "linear",
            "flat_forward",
            None,
            3,
        )
        assert curve.reset_lag == 3
        with pytest.raises(AttributeError):
            curve.reset_lag = 2

    def test_named_factory_avoids_positional_order_ambiguity(self) -> None:
        curve = ForwardCurve.from_knots(
            "USD-SOFR",
            tenor=0.25,
            base_date=date(2024, 1, 1),
            knots=[(0.0, 0.04), (1.0, 0.045)],
            day_count="act_360",
        )
        assert curve.rate(1.0) == pytest.approx(0.045, abs=1e-10)

    def test_explicit_projection_grid_is_constructible_and_exposed(self) -> None:
        """Contractual boundaries round-trip through the canonical Rust curve."""
        last_reset = 91.0 / 360.0
        projection_grid = [0.0, last_reset, 183.0 / 360.0]
        curve = ForwardCurve(
            "USD-SOFR",
            0.25,
            knots=[(0.0, 0.04), (last_reset, 0.045)],
            base_date=date(2024, 1, 1),
            day_count="act_360",
            projection_grid=projection_grid,
        )
        assert curve.projection_grid == pytest.approx(projection_grid, abs=1e-14)
        assert curve.rate_between(0.0, last_reset) == pytest.approx(0.04, abs=1e-14)
        assert curve.rate_between(last_reset, projection_grid[-1]) == pytest.approx(0.045, abs=1e-14)

    @pytest.mark.parametrize(
        ("t1", "t2"),
        [(0.0, 0.0), (0.5, 0.25), (math.nan, 0.25), (0.0, math.inf)],
    )
    def test_rate_between_rejects_invalid_intervals(self, t1: float, t2: float) -> None:
        curve = ForwardCurve(
            "USD-SOFR",
            0.25,
            knots=[(0.0, 0.04), (1.0, 0.045)],
            base_date=date(2024, 1, 1),
        )
        with pytest.raises(ValueError, match=r"(rate_between requires|Invalid input|invalid input)"):
            curve.rate_between(t1, t2)

    def test_constructor_runtime_signature_matches_stub(self) -> None:
        assert str(signature(ForwardCurve)) == (
            "(id, tenor, knots, base_date, day_count=None, interp='linear', "
            "extrapolation='flat_forward', projection_grid=None, reset_lag=None)"
        )

    def test_projection_grid_defaults_to_legacy_numeric_tenor_mode(self) -> None:
        curve = ForwardCurve(
            "USD-SOFR",
            0.25,
            knots=[(0.0, 0.04), (1.0, 0.05), (5.0, 0.06)],
            base_date=date(2024, 1, 1),
        )
        assert curve.projection_grid is None


class TestFxMatrixParity:
    """FX matrix operations match Rust."""

    def test_direct_quote(self) -> None:
        """Direct EUR/USD lookup returns the stored rate."""
        fx = FxMatrix()
        eur, usd = Currency("EUR"), Currency("USD")
        fx.set_quote(eur, usd, 1.10)
        result = fx.rate(eur, usd, date(2024, 1, 1), FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.10, abs=1e-10)

    def test_inverse_quote(self) -> None:
        """Inverse USD/EUR lookup returns 1/rate."""
        fx = FxMatrix()
        eur, usd = Currency("EUR"), Currency("USD")
        fx.set_quote(eur, usd, 1.10)
        result = fx.rate(usd, eur, date(2024, 1, 1), FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.0 / 1.10, abs=1e-8)

    def test_same_currency_unity(self) -> None:
        """Same-currency rate is 1.0."""
        fx = FxMatrix()
        usd = Currency("USD")
        result = fx.rate(usd, usd, date(2024, 1, 1), FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.0, abs=1e-10)

    def test_triangulation_via_usd_pivot(self) -> None:
        """Cross triangulation (EUR->GBP via USD) returns the implied rate."""
        fx = FxMatrix()
        usd, eur, gbp = Currency("USD"), Currency("EUR"), Currency("GBP")
        fx.set_quote(eur, usd, 1.10)
        fx.set_quote(gbp, usd, 1.25)
        result = fx.rate(eur, gbp, date(2024, 1, 1), FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.10 / 1.25, abs=1e-10)
        assert result.triangulated is True

    def test_zero_rate_raises(self) -> None:
        """Setting a zero FX rate should raise."""
        fx = FxMatrix()
        eur, usd = Currency("EUR"), Currency("USD")
        with pytest.raises(Exception, match=r"(?i)(positive|rate|invalid input parameter)"):
            fx.set_quote(eur, usd, 0.0)

    def test_policy_as_string(self) -> None:
        """FxConversionPolicy can also be passed as its enum variant."""
        fx = FxMatrix()
        usd = Currency("USD")
        result = fx.rate(usd, usd, date(2024, 1, 1), FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.0)

    def test_date_scoped_quote_does_not_shadow_other_dates(self) -> None:
        fx = FxMatrix()
        eur, usd = Currency("EUR"), Currency("USD")
        fixing_date = date(2024, 1, 2)
        fx.set_quote_on(eur, usd, fixing_date, "cashflow_date", 1.10)

        result = fx.rate(eur, usd, fixing_date, FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.10)
        with pytest.raises(KeyError, match="FX"):
            fx.rate(eur, usd, date(2024, 1, 3), FxConversionPolicy.CASHFLOW_DATE)

    def test_explicit_and_pinned_quotes_survive_market_context_json_roundtrip(self) -> None:
        fx = FxMatrix()
        eur, gbp, usd = Currency("EUR"), Currency("GBP"), Currency("USD")
        fx.set_quote(eur, usd, 1.10)
        pinned_date = date(2024, 1, 2)
        fx.set_quote_on(gbp, usd, pinned_date, FxConversionPolicy.CASHFLOW_DATE, 1.25)

        context = MarketContext()
        context.insert_fx(fx)
        restored = MarketContext.from_json(context.to_json())

        pinned = restored.fx.rate(gbp, usd, pinned_date, FxConversionPolicy.CASHFLOW_DATE)
        explicit = restored.fx.rate(
            eur,
            usd,
            date(2024, 1, 3),
            FxConversionPolicy.CASHFLOW_DATE,
        )
        assert pinned.rate == pytest.approx(1.25)
        assert pinned.triangulated is False
        assert explicit.rate == pytest.approx(1.10)
        assert explicit.triangulated is False


class TestMarketContextParity:
    """Market context insert / retrieve matches Rust."""

    def test_insert_and_get_discount(self) -> None:
        """Insert a discount curve and retrieve by ID."""
        mc = MarketContext()
        curve = DiscountCurve(
            "USD-OIS",
            date(2024, 1, 1),
            [(0.0, 1.0), (1.0, 0.95)],
            day_count="act_365f",
        )
        mc.insert(curve)
        retrieved = mc.get_discount("USD-OIS")
        assert retrieved.id == "USD-OIS"

    def test_insert_and_get_forward(self) -> None:
        """Insert a forward curve and retrieve by ID."""
        mc = MarketContext()
        curve = ForwardCurve(
            "USD-SOFR",
            0.25,
            knots=[(0.0, 0.04), (1.0, 0.045)],
            base_date=date(2024, 1, 1),
            day_count="act_360",
        )
        mc.insert(curve)
        retrieved = mc.get_forward("USD-SOFR")
        assert retrieved.id == "USD-SOFR"

    def test_insert_and_get_base_correlation(self) -> None:
        mc = MarketContext()
        curve = BaseCorrelationCurve("CDX-IG-CORR", [(3.0, 0.20), (10.0, 0.45)])

        mc.insert(curve)

        retrieved = mc.get_base_correlation("CDX-IG-CORR")
        assert retrieved.id == "CDX-IG-CORR"
        assert retrieved.correlation(3.0) == pytest.approx(0.20)

    def test_insert_and_get_credit_index(self) -> None:
        hazard = HazardCurve(
            "CDX-IG-HAZARD",
            date(2024, 1, 1),
            [(0.0, 0.01), (5.0, 0.015)],
        )
        correlation = BaseCorrelationCurve("CDX-IG-CORR", [(3.0, 0.20), (10.0, 0.45)])
        index = CreditIndexData(125, 0.40, hazard, correlation)
        mc = MarketContext()

        mc.insert_credit_index("CDX-IG", index)

        retrieved = mc.get_credit_index("CDX-IG")
        assert retrieved.num_constituents == 125
        assert retrieved.recovery_rate == pytest.approx(0.40)

    def test_insert_and_get_price_returns_value_and_optional_currency(self) -> None:
        mc = MarketContext()
        mc.insert_price("EQUITY-SPOT", 185.25, "USD")
        mc.insert_price("DIVIDEND-YIELD", 0.005)

        spot_value, spot_currency = mc.get_price("EQUITY-SPOT")
        dividend_value, dividend_currency = mc.get_price("DIVIDEND-YIELD")
        assert spot_value == Decimal("185.25")
        assert spot_currency == "USD"
        assert dividend_value == pytest.approx(0.005)
        assert dividend_currency is None

    @pytest.mark.parametrize("value", [math.nan, math.inf, -math.inf])
    def test_insert_price_rejects_non_finite_values(self, value: float) -> None:
        mc = MarketContext()

        with pytest.raises(ValueError, match="finite"):
            mc.insert_price("INVALID", value)

        with pytest.raises(ValueError, match="finite"):
            mc.insert_price("INVALID-MONEY", value, Currency("USD"))

        with pytest.raises(KeyError):
            mc.get_price("INVALID")
        with pytest.raises(KeyError):
            mc.get_price("INVALID-MONEY")

    def test_insert_price_preserves_decimal_money_and_accepts_currency_wrapper(self) -> None:
        mc = MarketContext()

        mc.insert_price("EQUITY-SPOT", Decimal("185.2500000000000000001"), Currency("USD"))

        restored = MarketContext.from_json(mc.to_json())
        value, currency = restored.get_price("EQUITY-SPOT")
        assert value == Decimal("185.2500000000000000001")
        assert currency == "USD"

    def test_unitless_price_accepts_only_exactly_representable_decimal(self) -> None:
        mc = MarketContext()
        mc.insert_price("EXACT", Decimal("0.5"))
        assert mc.get_price("EXACT") == (0.5, None)

        with pytest.raises(ValueError, match="exactly representable"):
            mc.insert_price("INEXACT", Decimal("0.1"))

    def test_insert_and_get_scalar_time_series(self) -> None:
        observations = [(date(2024, 1, 1), 100.0), (date(2024, 1, 3), 104.0)]
        series = ScalarTimeSeries(
            "EQUITY-HISTORY",
            observations,
            currency="USD",
            interpolation="linear",
        )
        mc = MarketContext()

        mc.insert_series(series)
        retrieved = mc.get_series("EQUITY-HISTORY")

        assert retrieved.id == "EQUITY-HISTORY"
        assert retrieved.currency == Currency("USD")
        assert retrieved.interpolation == "linear"
        assert retrieved.observations == observations
        assert retrieved.value_on(date(2024, 1, 2)) == pytest.approx(102.0)

    @pytest.mark.parametrize("value", [math.nan, math.inf, -math.inf])
    def test_scalar_time_series_rejects_non_finite_observations(self, value: float) -> None:
        with pytest.raises(ValueError, match="finite"):
            ScalarTimeSeries("INVALID", [(date(2024, 1, 1), value)])

    def test_scalar_time_series_decimal_values_require_exact_f64_roundtrip(self) -> None:
        series = ScalarTimeSeries(
            "EXACT",
            [(date(2024, 1, 1), Decimal("100.25"))],
            currency=Currency("USD"),
        )
        assert series.observations == [(date(2024, 1, 1), 100.25)]
        assert series.currency == Currency("USD")

        with pytest.raises(ValueError, match="exactly representable"):
            ScalarTimeSeries(
                "INEXACT",
                [(date(2024, 1, 1), Decimal("0.1"))],
            )

    def test_scalar_time_series_json_roundtrip(self) -> None:
        series = ScalarTimeSeries(
            "EQUITY-HISTORY",
            [(date(2024, 1, 1), 100.0), (date(2024, 1, 3), 104.0)],
            interpolation="linear",
        )

        restored = ScalarTimeSeries.from_json(series.to_json())

        assert restored.id == series.id
        assert restored.observations == series.observations
        assert restored.interpolation == series.interpolation

    def test_inflation_index_typed_context_and_json_roundtrip(self) -> None:
        observations = [(date(2024, 1, 1), 300.0), (date(2024, 2, 1), 301.5)]
        index = InflationIndex("US-CPI", observations, Currency("USD"), interpolation="linear")
        mc = MarketContext()

        mc.insert_inflation_index(index)
        restored_context = MarketContext.from_json(mc.to_json())
        restored = restored_context.get_inflation_index("US-CPI")
        restored_directly = InflationIndex.from_json(index.to_json())

        assert restored.id == "US-CPI"
        assert restored.currency == Currency("USD")
        assert restored.interpolation == "linear"
        assert restored.observations == observations
        assert restored.value_on(date(2024, 1, 16)) == pytest.approx(300.7258064516129)
        assert len(restored) == 2
        assert restored_directly.to_json() == index.to_json()

    @pytest.mark.parametrize("value", [math.nan, math.inf, -math.inf])
    def test_inflation_index_rejects_non_finite_observations(self, value: float) -> None:
        with pytest.raises(ValueError, match="finite"):
            InflationIndex("INVALID", [(date(2024, 1, 1), value)], "USD")

    def test_inflation_index_rejects_duplicate_observation_dates(self) -> None:
        with pytest.raises(ValueError, match="strictly increasing"):
            InflationIndex(
                "INVALID",
                [(date(2024, 1, 1), 300.0), (date(2024, 1, 1), 301.0)],
                "USD",
            )

    def test_getter_surface(self) -> None:
        """MarketContext exposes the canonical getter names (no legacy aliases)."""
        mc = MarketContext()
        for name in [
            "get_base_correlation",
            "get_credit_index",
            "get_discount",
            "get_forward",
            "get_hazard",
            "get_inflation_index",
            "get_price",
            "get_series",
            "insert",
            "insert_inflation_index",
            "insert_series",
            "fx",
        ]:
            assert hasattr(mc, name), f"missing {name}"
        for old_name in ["discount", "forward", "hazard"]:
            assert not hasattr(mc, old_name), f"unexpected legacy getter {old_name}"


class TestLinalgParity:
    """Core linalg bindings match Rust exports."""

    def test_exports_and_constants(self) -> None:
        """Module exports CholeskyError and tolerance constants."""
        from finstack_quant.core.math import linalg

        assert hasattr(linalg, "CholeskyError")
        assert hasattr(linalg, "cholesky_solve")
        assert pytest.approx(1e-10) == linalg.SINGULAR_THRESHOLD
        assert pytest.approx(1e-6) == linalg.DIAGONAL_TOLERANCE
        assert pytest.approx(1e-6) == linalg.SYMMETRY_TOLERANCE

    def test_cholesky_decomposition(self) -> None:
        """Cholesky decomposition of a 2x2 SPD matrix."""
        from finstack_quant.core.math.linalg import cholesky_decomposition

        lower = cholesky_decomposition([[4.0, 2.0], [2.0, 3.0]])
        assert lower[0][0] == pytest.approx(2.0)
        assert lower[1][0] == pytest.approx(1.0)
        assert lower[1][1] == pytest.approx(math.sqrt(2.0))
        assert lower[0][1] == pytest.approx(0.0)

    def test_cholesky_solve(self) -> None:
        """Cholesky solve recovers the exact solution."""
        from finstack_quant.core.math.linalg import cholesky_decomposition, cholesky_solve

        chol = cholesky_decomposition([[4.0, 2.0], [2.0, 3.0]])
        x = cholesky_solve(chol, [1.0, 1.0])
        assert x == pytest.approx([0.125, 0.25])

    def test_cholesky_solve_singular_raises(self) -> None:
        """Singular factor triggers the dedicated CholeskyError."""
        from finstack_quant.core.math.linalg import CholeskyError, cholesky_solve

        with pytest.raises(CholeskyError, match=r"(?i)invalid|singular|zero|solve"):
            cholesky_solve([[0.0]], [1.0])


class TestScheduleParity:
    """Schedule types are accessible from dates module."""

    def test_stub_kind_variants_exist(self) -> None:
        """StubKind enum variants are importable from dates."""
        from finstack_quant.core.dates import StubKind

        assert StubKind.NONE is not None
        assert StubKind.SHORT_FRONT is not None
        assert StubKind.SHORT_BACK is not None
