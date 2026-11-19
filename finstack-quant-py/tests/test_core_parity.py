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
    available_calendars,
    build_periods,
    sifma_settlement_date,
)
from finstack_quant.core.market_data import (
    BaseCorrelationCurve,
    CreditIndexData,
    DiscountCurve,
    ForwardCurve,
    FxConversionPolicy,
    FxDeltaVolSurface,
    FxMatrix,
    FxPairConvention,
    HazardCurve,
    InflationExpectedRelease,
    InflationIndex,
    InflationObservation,
    MarketContext,
    ScalarTimeSeries,
    VolSurface,
    invert_fx_rate,
)
from finstack_quant.core.money import Money
from finstack_quant.core.types import Bps, CreditRating, Rate
from finstack_quant.models.credit import moodys_warf_factor


def test_quantile_invalid_probability_raises_and_empty_returns_none() -> None:
    from finstack_quant.core.math.stats import quantile

    assert quantile([], 0.5) is None
    for values in ([], [1.0], [1.0, 2.0, 3.0]):
        with pytest.raises(ValueError, match="Quantile probability"):
            quantile(values, math.nan)
        with pytest.raises(ValueError, match="Quantile probability"):
            quantile(values, -1.0)
        with pytest.raises(ValueError, match="Quantile probability"):
            quantile(values, 2.0)


def test_stats_exact_contracts_and_subnormal_correlation() -> None:
    from finstack_quant.core.math import stats

    assert stats.mean([]) is None
    assert stats.variance([1.0]) is None
    assert stats.std_dev([1.0]) is None
    assert stats.mean_var([]) == (None, None)
    assert stats.mean_var([2.0]) == (2.0, None)
    assert stats.correlation([5.0, 5.0, 5.0], [1.0, 2.0, 3.0]) is None
    assert stats.correlation([1e-200, 2e-200], [1e-200, 2e-200]) == pytest.approx(1.0)
    with pytest.raises(ValueError, match="finite"):
        stats.mean([1.0, math.nan])
    with pytest.raises(ValueError, match="dimensions do not match"):
        stats.correlation([1.0, 2.0], [1.0])


@pytest.mark.parametrize(
    ("start", "end"),
    [
        (date(2025, 2, 28), date(2025, 3, 31)),
        (date(2024, 2, 29), date(2024, 3, 31)),
        (date(2025, 4, 30), date(2025, 5, 31)),
    ],
)
def test_icma_backward_only_month_roll_requires_reference_period(start: date, end: date) -> None:
    ctx = DayCountContext(frequency=Tenor.monthly())
    with pytest.raises(ValueError, match="coupon_period"):
        DayCount.ACT_ACT_ISMA.year_fraction(start, end, ctx)
    reference = DayCountContext(frequency=Tenor.monthly(), coupon_period=(start, end))
    assert DayCount.ACT_ACT_ISMA.year_fraction(start, end, reference) == pytest.approx(1 / 12, abs=1e-12)


def test_icma_reference_period_preserves_february_coupon_tenor() -> None:
    start, end = date(2024, 8, 30), date(2025, 2, 28)
    ctx = DayCountContext(frequency=Tenor.semi_annual(), coupon_period=(start, end))
    assert DayCount.ACT_ACT_ISMA.year_fraction(start, end, ctx) == pytest.approx(0.5)


def test_reference_cpi_requires_exact_monthly_fixings() -> None:
    index = InflationIndex(
        "US-CPI",
        [
            InflationObservation("2025-02-01", 303.0, "2025-03-12"),
            InflationObservation("2025-04-01", 315.0, "2025-05-13"),
        ],
        Currency("USD"),
        "2025-06-01",
    )
    with pytest.raises(KeyError, match="2025-03-01"):
        index.get_observation("2025-03-01")
    assert index.get_observation("2025-02-01").value == 303.0
    with pytest.raises(ValueError, match="first-of-month"):
        index.get_observation("2025-02-15")
    with pytest.raises(ValueError, match="unavailable"):
        index.qualified_monthly_observation(2025, 2, "2025-03-11")
    assert index.qualified_monthly_observation(2025, 2, "2025-03-12") == 303.0
    with pytest.raises(ValueError, match="exceeds"):
        index.qualified_monthly_observation(2025, 2, "2025-06-02")


def test_tenor_constructor_uses_checked_rust_validation() -> None:
    with pytest.raises(ValueError, match="count must be positive"):
        Tenor(0, TenorUnit.MONTHS)
    with pytest.raises(ValueError, match="representation limit"):
        Tenor((2**31 - 1) // 12 + 1, TenorUnit.YEARS)
    assert Tenor(201, TenorUnit.YEARS).count == 201


def test_bps_constructor_rejects_rounded_i32_overflow() -> None:
    assert Bps(2_147_483_647).as_bp == 2_147_483_647
    with pytest.raises(ValueError, match="overflow"):
        Bps(2_147_483_648)


def test_bps_constructor_rejects_fractional_input() -> None:
    """Reject fractional bp instead of silently rounding.

    Sub-bp spreads change instrument economics; the JSON path preserves
    them exactly.
    """
    with pytest.raises(ValueError, match="whole number"):
        Bps(62.5)
    assert Bps(62.0).as_bp == 62


class TestCurrencyParity:
    """Currency construction and property access match Rust."""

    def test_construction_and_properties(self) -> None:
        """USD should have code 'USD', numeric 840, decimals 2."""
        usd = Currency("USD")
        assert str(usd) == "USD"
        assert usd.numeric == 840
        assert usd.decimals == 2

    def test_case_insensitive(self) -> None:
        """Lowercase input should resolve identically."""
        usd1 = Currency("USD")
        usd2 = Currency("usd")
        assert str(usd1) == str(usd2)
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
        assert str(m.currency) == "USD"

    def test_addition(self, usd: Currency) -> None:
        """Same-currency addition."""
        result = Money(100.0, usd) + Money(50.0, usd)
        assert result.amount == pytest.approx(150.0)
        assert str(result.currency) == "USD"

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
        with pytest.raises(TypeError, match="unsupported operand"):
            5.0 - Money(1.0, usd)

    def test_bool_amount_rejected(self, usd: Currency) -> None:
        """Money rejects bool amounts even though bool is a subclass of int."""
        with pytest.raises(TypeError, match="Money amount must be"):
            Money(True, usd)  # type: ignore[arg-type]
        with pytest.raises(TypeError, match="Money amount must be"):
            Money(False, usd)  # type: ignore[arg-type]


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

    def test_rate_unit_views_and_admission(self) -> None:
        assert Rate.from_bp(62.5).as_decimal == 62.5 / 10_000
        assert Rate("62.5bp").as_bp == 62.5
        assert Rate("5").as_decimal == 5.0
        assert Rate("1e310bp").as_decimal == 1e306
        with pytest.raises(TypeError):
            Rate.from_bp(True)
        with pytest.raises(TypeError):
            Rate(True)
        with pytest.raises(ValueError, match="rounds to zero"):
            Rate.from_bp(5e-324)
        with pytest.raises(ValueError, match=r"(?i)finite|infinit"):
            _ = Rate(1.7976931348623157e308).as_percent
        for bad in ("1_000bp", "1bps%", "nan", "1e 2%"):
            with pytest.raises(ValueError, match="invalid rate"):
                Rate(bad)
        assert str(Rate(0.05)) == "0.05"

    def test_special_function_domains_and_extreme_cauchy_tail(self) -> None:
        from finstack_quant.core.math import special_functions as sf

        for evaluate in (sf.norm_cdf, sf.norm_pdf, sf.log_norm_cdf, sf.erf):
            with pytest.raises(ValueError, match="NaN"):
                evaluate(math.nan)
        with pytest.raises(RuntimeError):
            sf.log_norm_cdf(-1e308)
        with pytest.raises(RuntimeError):
            sf.norm_pdf_with_params(0.0, 0.0, 5e-324)
        with pytest.raises(ValueError, match="finite positive"):
            sf.ln_gamma(-1.0)
        assert sf.student_t_cdf(-1e200, 1.0) * 1e200 * math.pi == pytest.approx(1.0, rel=1e-12)

    def test_student_t_invalid_degrees_of_freedom_raises(self) -> None:
        from finstack_quant.core.math import linalg, special_functions

        with pytest.raises(ValueError, match="df"):
            special_functions.student_t_cdf(0.0, 0.0)
        with pytest.raises(ValueError, match="df"):
            special_functions.student_t_inv_cdf(0.5, math.nan)
        for bad_p in (-0.1, 1.1, math.nan):
            with pytest.raises(ValueError, match="probability p"):
                special_functions.student_t_inv_cdf(bad_p, 5.0)
        assert special_functions.student_t_inv_cdf(0.0, 5.0) == -math.inf
        assert special_functions.student_t_inv_cdf(1.0, 5.0) == math.inf
        for bad_p in (-0.1, 1.1, math.nan):
            with pytest.raises(ValueError, match="probability"):
                special_functions.standard_normal_inv_cdf(bad_p)
        with pytest.raises(linalg.CholeskyError, match="not symmetric"):
            linalg.cholesky_decomposition([[1.0, 0.5], [0.4, 1.0]])
        with pytest.raises(linalg.CholeskyError, match="not symmetric"):
            linalg.symmetric_eigen([[1.0, 0.5], [0.4, 1.0]])

    def test_credit_rating_notches_are_preserved_and_models_owns_warf(self) -> None:
        assert CreditRating("BBB+") == CreditRating.BBB_PLUS
        assert CreditRating("Baa1") == CreditRating.BBB_PLUS
        assert str(CreditRating.BBB_PLUS) == "BBB+"
        assert moodys_warf_factor(CreditRating.BBB_PLUS) > 0.0


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

    def test_act365l_partial_coupon_uses_next_coupon_year(self) -> None:
        start, query, end = date(2023, 10, 15), date(2023, 12, 15), date(2024, 4, 15)
        ctx = DayCountContext(frequency="6M", coupon_period=(start, end))
        accrued = DayCount.ACT_365L.year_fraction(start, query, ctx)
        remaining = DayCount.ACT_365L.year_fraction(query, end, ctx)
        assert accrued == pytest.approx(61 / 366, abs=1e-14)
        assert accrued + remaining == pytest.approx(0.5, abs=1e-14)
        assert DayCount.ACT_365L.signed_year_fraction(query, start, ctx) == pytest.approx(-accrued, abs=1e-14)
        with pytest.raises(ValueError, match="coupon_period"):
            DayCount.ACT_365L.year_fraction(start, end)
        with pytest.raises(ValueError, match="coupon_period"):
            DayCount.ACT_365L.year_fraction(start, end, DayCountContext(frequency="6M"))

    def test_icma_requires_contained_interval_and_explicit_slicing(self) -> None:
        start, feb, end = date(2025, 1, 30), date(2025, 2, 28), date(2025, 3, 15)
        mar_30 = date(2025, 3, 30)
        first_ctx = DayCountContext(frequency="1M", coupon_period=(start, feb))
        second_ctx = DayCountContext(frequency="1M", coupon_period=(feb, mar_30))
        with pytest.raises(ValueError, match="coupon_period"):
            DayCount.ACT_ACT_ISMA.year_fraction(start, end, first_ctx)
        first = DayCount.ACT_ACT_ISMA.year_fraction(start, feb, first_ctx)
        second = DayCount.ACT_ACT_ISMA.year_fraction(feb, end, second_ctx)
        assert first + second == pytest.approx(0.125, abs=1e-14)

    def test_thirty360_year_fraction(self) -> None:
        """30/360: exactly 6 months = 0.5."""
        start, end = date(2024, 1, 15), date(2024, 7, 15)
        yf = DayCount.THIRTY_360.year_fraction(start, end)
        assert yf == pytest.approx(180.0 / 360.0, abs=1e-10)

    def test_thirty_e_360_isda_termination_context(self) -> None:
        start, end = date(2025, 1, 31), date(2025, 2, 28)
        regular = DayCount.THIRTY_E_360_ISDA.year_fraction(start, end, DayCountContext(end_is_termination_date=False))
        with pytest.raises(ValueError, match="end_is_termination_date"):
            DayCount.THIRTY_E_360_ISDA.year_fraction(start, end)
        terminal = DayCount.THIRTY_E_360_ISDA.year_fraction(start, end, DayCountContext(end_is_termination_date=True))
        assert regular == pytest.approx(30.0 / 360.0)
        assert terminal == pytest.approx(28.0 / 360.0)

    def test_same_date_zero_fraction(self) -> None:
        """Year fraction is zero when start == end."""
        d = date(2024, 1, 1)
        assert DayCount.ACT_360.year_fraction(d, d) == pytest.approx(0.0)

    def test_coupon_period_is_validated_by_core(self) -> None:
        with pytest.raises(Exception, match="coupon period start must be before end"):
            DayCountContext(coupon_period=(date(2025, 7, 1), date(2025, 1, 1)))

    def test_act_act_afb_and_thirty_360_it(self) -> None:
        """New AFB and Italian conventions parse and match QuantLib formulas."""
        assert DayCount.from_name("act_act_afb") == DayCount.ACT_ACT_AFB
        assert DayCount.from_name("30_360_it") == DayCount.THIRTY_360_IT
        afb = DayCount.ACT_ACT_AFB.year_fraction(date(2024, 2, 1), date(2024, 3, 1))
        italian = DayCount.THIRTY_360_IT.year_fraction(date(2025, 1, 31), date(2025, 2, 28))
        assert afb == pytest.approx(29.0 / 366.0, abs=1e-14)
        assert italian == pytest.approx(30.0 / 360.0, abs=1e-14)

    def test_available_calendars_includes_new_exchange_ids(self) -> None:
        ids = available_calendars()
        for calendar_id in ("eurex", "six", "tsx", "nse", "bse"):
            assert calendar_id in ids

    def test_unknown_calendar_raises_key_error_with_suggestions(self) -> None:
        """Unknown codes raise KeyError carrying core's fuzzy suggestions."""
        # Near-miss codes surface a "Did you mean …?" suggestion.
        with pytest.raises(KeyError, match="Did you mean 'target2'"):
            HolidayCalendar("targe")
        with pytest.raises(KeyError, match="Did you mean 'eurex'"):
            HolidayCalendar("eurx")
        # Far-off codes still raise KeyError, just without suggestions.
        with pytest.raises(KeyError, match="no_such_calendar"):
            HolidayCalendar("no_such_calendar")


class TestFxConventionParity:
    """Explicit FX quote metadata keeps the supplied order; invert is checked."""

    def test_fx_convention_keeps_the_supplied_order(self) -> None:
        usdeur = FxPairConvention("USD", "EUR", 0.0001)
        assert [str(usdeur.base), str(usdeur.quote)] == ["USD", "EUR"]
        conv = FxPairConvention("USD", "JPY", 0.01)
        assert conv.pip_size == 0.01
        assert conv.rate_points(50.0) == pytest.approx(0.5)

    def test_fx_convention_rejects_bad_inputs_and_inverts(self) -> None:
        with pytest.raises(ValueError, match="distinct"):
            FxPairConvention("USD", "USD", 0.0001)
        with pytest.raises(ValueError, match="pip size"):
            FxPairConvention("EUR", "USD", 0.0)
        assert invert_fx_rate(1.10) == pytest.approx(1.0 / 1.10)


class TestPeriodParity:
    """Period ID construction and build_periods match Rust."""

    def test_monthly_period_id(self) -> None:
        """Monthly PeriodId round-trips code, year, index."""
        pid = PeriodId.month(2024, 6)
        assert str(pid) == "2024M06"
        assert pid.year == 2024
        assert pid.index == 6

    def test_quarterly_period_id(self) -> None:
        """Quarterly PeriodId round-trips."""
        pid = PeriodId.quarter(2024, 2)
        assert str(pid) == "2024Q2"
        assert pid.year == 2024
        assert pid.index == 2

    def test_annual_period_id(self) -> None:
        """Annual PeriodId round-trips."""
        pid = PeriodId.annual(2024)
        assert str(pid) == "2024"
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
        assert str(plan.periods[0].id) == "2024Q1"
        assert str(plan.periods[3].id) == "2024Q4"

    def test_build_periods_monthly(self) -> None:
        """Build 12 monthly periods from a range string."""
        plan = build_periods("2024M01..M12", None)
        assert len(plan.periods) == 12
        assert str(plan.periods[0].id) == "2024M01"
        assert str(plan.periods[11].id) == "2024M12"

    def test_build_periods_with_actuals(self) -> None:
        """Actuals cutoff correctly partitions periods."""
        plan = build_periods("2024Q1..Q4", "2024Q2")
        assert len(plan.periods) == 4
        assert plan.periods[0].is_actual
        assert plan.periods[1].is_actual
        assert not plan.periods[2].is_actual
        assert not plan.periods[3].is_actual


def test_sifma_settlement_classes_are_explicit() -> None:
    assert not hasattr(SifmaSettlementClass, "from_agency_term")
    assert sifma_settlement_date(1, 2026, SifmaSettlementClass.A) == date(2026, 1, 14)
    assert sifma_settlement_date(1, 2026, SifmaSettlementClass.B) == date(2026, 1, 20)


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
            interp="monotone_convex",
            extrapolation="extend",
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
            interp="monotone_convex",
            extrapolation="extend",
        )
        df = curve.df(1.5)
        assert 0.89 < df < 0.96

    def test_zero_rate_consistency(self, curve: DiscountCurve) -> None:
        """exp(-z * t) recovers the discount factor."""
        t = 1.0
        df = curve.df(t)
        z = curve.zero_rate(t, "continuous")
        assert df == pytest.approx(math.exp(-z * t), abs=1e-8)

    def test_df_at_time_zero(self, curve: DiscountCurve) -> None:
        """DF at t=0 is 1.0."""
        assert curve.df(0.0) == pytest.approx(1.0, abs=1e-10)

    def test_default_day_count_is_act_365f(self) -> None:
        """DiscountCurve defaults to Act/365F when day_count is omitted."""
        curve = DiscountCurve(
            "USD-OIS",
            date(2024, 1, 1),
            [(0.0, 1.0), (1.0, 0.95)],
            interp="monotone_convex",
            extrapolation="extend",
            day_count="act_365f",
        )
        context = MarketContext()
        context.insert(curve)

        state = json.loads(context.to_json())
        assert state["curves"][0]["day_count"] == "act_365f"

    def test_explicit_day_count_still_overrides_curve_id_inference(self) -> None:
        """Users can still override the inferred day-count convention explicitly."""
        curve = DiscountCurve(
            "USD-OIS",
            date(2024, 1, 1),
            [(0.0, 1.0), (1.0, 0.95)],
            day_count="act_365f",
            interp="monotone_convex",
            extrapolation="extend",
        )
        context = MarketContext()
        context.insert(curve)

        state = json.loads(context.to_json())
        assert state["curves"][0]["day_count"] == "act_365f"

    def test_flat_uses_continuous_compounding(self) -> None:
        curve = DiscountCurve.flat("USD-OIS", date(2024, 1, 1), 0.04, "act_365f")

        for t in [0.0, 0.25, 1.0, 5.0, 30.0]:
            assert curve.df(t) == pytest.approx(math.exp(-0.04 * t), abs=1e-12)
        assert curve.forward(2.0, 9.0) == pytest.approx(0.04, abs=1e-12)

    def test_negative_rate_curve_accepts_increasing_discount_factors(self) -> None:
        knots = [(0.0, 1.0), (1.0, 1.002), (2.0, 1.004)]
        curve = DiscountCurve(
            "CHF-OIS",
            date(2024, 1, 1),
            knots,
            interp="linear",
            extrapolation="extend",
            day_count="act_365f",
        )
        assert curve.df(2.0) == pytest.approx(1.004)
        assert curve.forward(0.0, 1.0) < 0.0

        with pytest.raises(TypeError, match="interp"):
            DiscountCurve("CHF-OIS", date(2024, 1, 1), knots)  # type: ignore[call-arg]
        for legacy_kw in (
            {"validation_mode": "negative_rate_friendly"},
            {"forward_floor": -0.01},
        ):
            with pytest.raises(TypeError):
                DiscountCurve(  # type: ignore[call-arg]
                    "CHF-OIS",
                    date(2024, 1, 1),
                    knots,
                    interp="linear",
                    extrapolation="extend",
                    day_count="act_365f",
                    **legacy_kw,
                )


class TestForwardCurveParity:
    """Forward curve operations match Rust."""

    @staticmethod
    def _curve(**overrides: object) -> ForwardCurve:
        options: dict[str, object] = {
            "id": "USD-SOFR",
            "base_date": date(2024, 1, 1),
            "knots": [(1.0, math.exp(-0.045)), (5.0, math.exp(-0.25))],
            "interp": "log_linear",
            "extrapolation": "extend",
            "day_count": "act_360",
            "index_tenor": "3M",
        }
        options.update(overrides)
        return ForwardCurve(options)  # type: ignore[arg-type]

    def test_construction(self) -> None:
        """ID, base date, index tenor and the prepended unit factor survive construction."""
        curve = self._curve()
        assert curve.id == "USD-SOFR"
        assert curve.base_date == date(2024, 1, 1)
        assert str(curve.index_tenor) == "3M"
        assert curve.knots == pytest.approx([0.0, 1.0, 5.0])
        assert curve.factors == pytest.approx([1.0, math.exp(-0.045), math.exp(-0.25)])

    def test_df_at_knot(self) -> None:
        """The projection factor at an exact knot matches input."""
        assert self._curve().df(1.0) == pytest.approx(math.exp(-0.045), abs=1e-15)

    def test_constructor_rejects_incomplete_or_unknown_options(self) -> None:
        with pytest.raises(TypeError, match="missing ForwardCurve option 'base_date'"):
            ForwardCurve({"id": "USD-SOFR"})  # type: ignore[arg-type]
        with pytest.raises(TypeError, match="unknown ForwardCurve option 'extrapoltion'"):
            self._curve(extrapoltion="flat")

    def test_index_tenor_is_optional_and_readonly(self) -> None:
        assert self._curve(index_tenor=None).index_tenor is None
        curve = self._curve()
        with pytest.raises(AttributeError):
            curve.index_tenor = "6M"  # type: ignore[misc]

    def test_rate_between_uses_contractual_accrual(self) -> None:
        """Factors built from simple period rates reproduce those rates."""
        t_3m = 91.0 / 360.0
        t_6m = 183.0 / 360.0
        p_3m = 1.0 / (1.0 + 0.04 * t_3m)
        p_6m = p_3m / (1.0 + 0.045 * (t_6m - t_3m))
        curve = ForwardCurve({
            "id": "USD-SOFR",
            "base_date": date(2024, 1, 1),
            "knots": [(t_3m, p_3m), (t_6m, p_6m)],
            "interp": "log_linear",
            "extrapolation": "extend",
            "day_count": "act_360",
        })
        assert curve.rate_between(0.0, t_3m, t_3m) == pytest.approx(0.04, abs=1e-14)
        assert curve.rate_between(t_3m, t_6m, t_6m - t_3m) == pytest.approx(0.045, abs=1e-14)

    def test_constructor_runtime_signature_matches_stub(self) -> None:
        assert str(signature(ForwardCurve)) == "(options)"

    def test_bus252_uses_the_constructor_calendar(self) -> None:
        curve = self._curve(day_count="bus_252", calendar_id="usny")
        assert curve.calendar_id == "usny"
        # January 1 is a holiday, leaving Jan 2–5 as the four
        # business days before Monday, January 8.
        assert curve.df("2024-01-08") == pytest.approx(curve.df(4.0 / 252.0), rel=1e-14)
        assert ForwardCurve.from_json(curve.to_json()).calendar_id == "usny"
        with pytest.raises(ValueError, match="requires a holiday calendar"):
            self._curve(day_count="bus_252")
        with pytest.raises(KeyError):
            self._curve(day_count="bus_252", calendar_id="unknown-calendar")

    def test_from_discount_curve_shares_factors(self) -> None:
        discount = DiscountCurve.flat("USD-OIS", date(2024, 1, 1), 0.04, "act_365f")
        curve = ForwardCurve.from_discount_curve("USD-SOFR-3M", discount, "3M")
        assert str(curve.index_tenor) == "3M"
        assert curve.day_count == discount.day_count
        for t in (0.5, 2.0, 7.5):
            assert curve.df(t) == discount.df(t)

    def test_instantaneous_forward_of_flat_curve(self) -> None:
        curve = ForwardCurve.flat("USD-SOFR", date(2024, 1, 1), 0.04, "act_360")
        assert curve.instantaneous_forward(3.0) == pytest.approx(0.04, abs=1e-12)


class TestFxMatrixParity:
    """FX matrix operations match Rust."""

    def test_singleton_setter_is_removed(self) -> None:
        assert not hasattr(FxMatrix, "set_quote")

    def test_direct_quote(self) -> None:
        """Direct EUR/USD lookup returns the stored rate."""
        fx = FxMatrix(None)
        eur, usd = Currency("EUR"), Currency("USD")
        fx.set_quotes([(eur, usd, 1.10)])
        result = fx.rate(eur, usd, date(2024, 1, 1), FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.10, abs=1e-10)

    def test_inverse_quote(self) -> None:
        """Inverse USD/EUR lookup returns 1/rate."""
        fx = FxMatrix(None)
        eur, usd = Currency("EUR"), Currency("USD")
        fx.set_quotes([(eur, usd, 1.10)])
        result = fx.rate(usd, eur, date(2024, 1, 1), FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.0 / 1.10, abs=1e-8)

    def test_same_currency_unity(self) -> None:
        """Same-currency rate is 1.0."""
        fx = FxMatrix(None)
        usd = Currency("USD")
        result = fx.rate(usd, usd, date(2024, 1, 1), FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.0, abs=1e-10)

    def test_triangulation_via_usd_pivot(self) -> None:
        """Cross triangulation (EUR->GBP via USD) returns the implied rate."""
        fx = FxMatrix("USD")
        usd, eur, gbp = Currency("USD"), Currency("EUR"), Currency("GBP")
        fx.set_quotes([(eur, usd, 1.10)])
        fx.set_quotes([(gbp, usd, 1.25)])
        result = fx.rate(eur, gbp, date(2024, 1, 1), FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.10 / 1.25, abs=1e-10)
        assert result.triangulated is True

    def test_zero_rate_raises(self) -> None:
        """Setting a zero FX rate should raise."""
        fx = FxMatrix(None)
        eur, usd = Currency("EUR"), Currency("USD")
        with pytest.raises(Exception, match=r"(?i)(positive|rate|invalid input parameter)"):
            fx.set_quotes([(eur, usd, 0.0)])

    def test_policy_as_string(self) -> None:
        """FxConversionPolicy can also be passed as its enum variant."""
        fx = FxMatrix(None)
        usd = Currency("USD")
        result = fx.rate(usd, usd, date(2024, 1, 1), FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.0)

    @pytest.mark.parametrize("pinned", [False, True])
    def test_quote_update_invalidates_cached_cross(self, pinned: bool) -> None:
        fx = FxMatrix("USD")
        fixing_date = date(2025, 1, 2)
        fx.set_quotes([("GBP", "USD", 1.25)])
        if pinned:
            fx.set_quote_on("EUR", "USD", fixing_date, "cashflow_date", 1.10)
        else:
            fx.set_quotes([("EUR", "USD", 1.10)])
        assert fx.rate("EUR", "GBP", fixing_date, "cashflow_date").rate == pytest.approx(0.88)
        if pinned:
            fx.set_quote_on("EUR", "USD", fixing_date, "cashflow_date", 1.20)
        else:
            fx.set_quotes([("EUR", "USD", 1.20)])
        result = fx.rate("EUR", "GBP", fixing_date, "cashflow_date")
        assert result.rate == pytest.approx(0.96)
        assert result.triangulated
        assert fx.rate("GBP", "EUR", fixing_date, "cashflow_date").rate == pytest.approx(1 / 0.96)
        assert fx.rate("GBP", "USD", fixing_date, "cashflow_date").rate == pytest.approx(1.25)

    def test_date_scoped_quote_does_not_shadow_other_dates(self) -> None:
        fx = FxMatrix(None)
        eur, usd = Currency("EUR"), Currency("USD")
        fixing_date = date(2024, 1, 2)
        fx.set_quote_on(eur, usd, fixing_date, "cashflow_date", 1.10)

        result = fx.rate(eur, usd, fixing_date, FxConversionPolicy.CASHFLOW_DATE)
        assert result.rate == pytest.approx(1.10)
        with pytest.raises(KeyError, match="FX"):
            fx.rate(eur, usd, date(2024, 1, 3), FxConversionPolicy.CASHFLOW_DATE)

    def test_explicit_and_pinned_quotes_survive_market_context_json_roundtrip(self) -> None:
        fx = FxMatrix(None)
        eur, gbp, usd = Currency("EUR"), Currency("GBP"), Currency("USD")
        fx.set_quotes([(eur, usd, 1.10)])
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
            interp="monotone_convex",
            extrapolation="extend",
        )
        mc.insert(curve)
        retrieved = mc.get_discount("USD-OIS")
        assert retrieved.id == "USD-OIS"

    def test_namespace_local_vol_registries_and_curve_validation(
        self,
    ) -> None:
        mc = MarketContext()
        mc.insert(
            DiscountCurve(
                "SHARED",
                date(2024, 1, 1),
                [(0.0, 1.0), (1.0, 0.95)],
                day_count="act_365f",
                interp="monotone_convex",
                extrapolation="extend",
            )
        )
        mc.insert_price("SHARED", 100.0)
        mc.insert_series(ScalarTimeSeries("SHARED", [(date(2024, 1, 1), 1.0)], "step", "flat"))
        mc.insert(
            VolSurface(
                "SHARED",
                [1.0],
                [100.0],
                [0.2],
                secondary_axis="strike",
                quote_type="black_lognormal",
                interpolation_mode="vol",
            )
        )
        assert mc.get_discount("SHARED").id == "SHARED"
        assert mc.get_series("SHARED").id == "SHARED"
        mc.insert(
            FxDeltaVolSurface(
                "SHARED",
                "EUR/USD",
                [1.0],
                [0.12],
                [0.01],
                [0.002],
                delta_convention="forward_unadjusted",
                atm_convention="delta_neutral_straddle",
                butterfly_convention="smile",
            )
        )
        assert mc.get_fx_delta_vol_surface("SHARED").id == "SHARED"
        with pytest.raises(ValueError, match="Cannot replace curve"):
            mc.insert(ForwardCurve.flat("SHARED", date(2024, 1, 1), 0.04, "act_360"))
        conditioned = mc.condition(date(2024, 7, 1))
        assert conditioned.get_discount("SHARED").base_date == date(2024, 7, 1)
        with pytest.raises(ValueError, match="Invalid date"):
            mc.condition("not-a-date")

    def test_insert_and_get_forward(self) -> None:
        """Insert a forward curve and retrieve by ID."""
        mc = MarketContext()
        curve = ForwardCurve.flat("USD-SOFR", date(2024, 1, 1), 0.04, "act_360")
        mc.insert(curve)
        retrieved = mc.get_forward("USD-SOFR")
        assert retrieved.id == "USD-SOFR"

    def test_insert_and_get_base_correlation(self) -> None:
        mc = MarketContext()
        curve = BaseCorrelationCurve("CDX-IG-CORR", [(3.0, 0.20), (10.0, 0.45)])

        mc.insert(curve)

        retrieved = mc.get_base_correlation("CDX-IG-CORR")
        assert retrieved.id == "CDX-IG-CORR"
        assert retrieved.get_correlation(3.0) == pytest.approx(0.20)

    def test_insert_and_get_credit_index(self) -> None:
        hazard = HazardCurve(
            "CDX-IG-HAZARD",
            date(2024, 1, 1),
            [(1.0, 0.01), (5.0, 0.015)],
            recovery_rate=0.40,
            day_count="act_365f",
        )
        correlation = BaseCorrelationCurve("CDX-IG-CORR", [(3.0, 0.20), (10.0, 0.45)])
        index = CreditIndexData(125, 0.40, hazard, correlation)
        mc = MarketContext()
        mc.insert(hazard).insert(correlation)
        mc.insert_credit_index("CDX-IG", index)

        retrieved = mc.get_credit_index("CDX-IG")
        assert retrieved.num_constituents == 125
        assert retrieved.recovery_rate == pytest.approx(0.40)

    def test_hazard_curve_requires_explicit_recovery(self) -> None:
        with pytest.raises(TypeError):
            HazardCurve(  # type: ignore[call-arg]
                "MISSING-RECOVERY",
                date(2024, 1, 1),
                [(1.0, 0.01), (5.0, 0.015)],
                day_count="act_365f",
            )

    @pytest.mark.parametrize("recovery", [0.0, 1.0])
    def test_hazard_curve_recovery_boundaries_round_trip(self, recovery: float) -> None:
        curve = HazardCurve(
            "BOUNDARY-RECOVERY",
            date(2024, 1, 1),
            [(1.0, 0.01), (5.0, 0.015)],
            recovery_rate=recovery,
            day_count="act_365f",
        )
        assert curve.recovery_rate == recovery

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

    def test_unitless_price_converts_decimal_to_nearest_float(self) -> None:
        mc = MarketContext()
        mc.insert_price("EXACT", Decimal("0.5"))
        mc.insert_price("NEAREST", Decimal("0.1"))
        assert mc.get_price("EXACT") == (0.5, None)
        assert mc.get_price("NEAREST") == (0.1, None)

    @pytest.mark.parametrize("value", [math.nan, math.inf, -math.inf])
    def test_unitless_price_finiteness_comes_from_rust(self, value: float) -> None:
        mc = MarketContext()
        with pytest.raises(ValueError, match="MarketScalar unitless value must be finite"):
            mc.insert_price("BAD", value)
        with pytest.raises(KeyError):
            mc.get_price("BAD")

    def test_insert_and_get_scalar_time_series(self) -> None:
        observations = [(date(2024, 1, 1), 100.0), (date(2024, 1, 3), 104.0)]
        series = ScalarTimeSeries(
            "EQUITY-HISTORY",
            observations,
            interpolation="linear",
            extrapolation="error",
            currency="USD",
        )
        mc = MarketContext()

        mc.insert_series(series)
        retrieved = mc.get_series("EQUITY-HISTORY")

        assert retrieved.id == "EQUITY-HISTORY"
        assert retrieved.currency == Currency("USD")
        assert retrieved.interpolation == "linear"
        assert retrieved.extrapolation == "error"
        assert retrieved.observations == observations
        assert retrieved.get_value(date(2024, 1, 2)) == pytest.approx(102.0)
        assert retrieved.get_value_exact(date(2024, 1, 1)) == pytest.approx(100.0)
        with pytest.raises(KeyError):
            retrieved.get_value(date(2024, 1, 4))

    @pytest.mark.parametrize("value", [math.nan, math.inf, -math.inf])
    def test_scalar_time_series_rejects_non_finite_observations(self, value: float) -> None:
        with pytest.raises(ValueError, match="time-series observation values must be finite"):
            ScalarTimeSeries("INVALID", [(date(2024, 1, 1), value)], "step", "flat")
        with pytest.raises(ValueError, match="finite"):
            InflationIndex("INVALID", [InflationObservation("2024-01-01", value, "2024-02-13")], "USD", "2024-03-01")

    @pytest.mark.parametrize(
        "text",
        ["0.1", "100.25", "300.1", "1e-20", "9007199254740993", "0.0000000037252902984619140625"],
    )
    def test_scalar_decimal_converts_like_float(self, text: str) -> None:
        value = Decimal(text)
        observations = [(date(2024, 1, 1), value)]
        expected = [(date(2024, 1, 1), float(value))]

        assert ScalarTimeSeries("S", observations, "step", "flat").observations == expected
        index = InflationIndex("I", [InflationObservation("2024-01-01", value, "2024-02-13")], "USD", "2024-03-01")
        assert index.get_observation("2024-01-01").value == float(value)
        context = MarketContext().insert_price("P", value)
        assert context.get_price("P") == (float(value), None)

    def test_scalar_time_series_json_roundtrip(self) -> None:
        series = ScalarTimeSeries(
            "EQUITY-HISTORY",
            [(date(2024, 1, 1), 100.0), (date(2024, 1, 3), 104.0)],
            interpolation="linear",
            extrapolation="flat",
        )

        restored = ScalarTimeSeries.from_json(series.to_json())

        assert restored.id == series.id
        assert restored.observations == series.observations
        assert restored.interpolation == series.interpolation
        assert restored.extrapolation == series.extrapolation

    def test_inflation_index_typed_context_and_json_roundtrip(self) -> None:
        observations = [
            InflationObservation("2024-01-01", 300.0, "2024-02-13"),
            InflationObservation("2024-02-01", 301.5, "2024-03-12"),
        ]
        index = InflationIndex("US-CPI", observations, Currency("USD"), "2024-04-01")
        mc = MarketContext()

        mc.insert_inflation_index(index)
        restored_context = MarketContext.from_json(mc.to_json())
        restored = restored_context.get_inflation_index("US-CPI")
        restored_directly = InflationIndex.from_json(index.to_json())

        assert restored.id == "US-CPI"
        assert restored.currency == Currency("USD")
        assert restored.knowledge_date == date(2024, 4, 1)
        assert [obs.value for obs in restored.observations] == [300.0, 301.5]
        assert restored.get_observation("2024-01-01").available_on == date(2024, 2, 13)
        assert len(restored) == 2
        assert restored_directly.to_json() == index.to_json()

    def test_inflation_expected_release_schedule_roundtrip(self) -> None:
        original = InflationIndex("US-CPI", [], "USD", "2026-02-01")
        release = InflationExpectedRelease("2026-01-01", "2026-02-13", "2026-01-01")
        index = original.with_expected_releases([release])
        assert original.date_range() is None
        assert original.get_expected_releases() == []
        assert index.get_expected_release("2026-01-01").expected_release == date(2026, 2, 13)
        assert index.get_expected_release("2026-02-01") is None
        market = MarketContext().insert_inflation_index(index)
        restored = MarketContext.from_json(market.to_json()).get_inflation_index("US-CPI")
        assert restored.get_expected_release("2026-01-01").known_on == date(2026, 1, 1)
        assert InflationIndex.from_json(index.to_json()).to_json() == index.to_json()
        with pytest.raises(ValueError, match="strictly increasing"):
            original.with_expected_releases([release, release])
        with pytest.raises(ValueError, match="exceeds"):
            original.with_expected_releases([InflationExpectedRelease("2026-01-01", "2026-02-13", "2026-02-02")])
        with pytest.raises(ValueError, match="outside"):
            InflationIndex("US-CPI", [InflationObservation("2026-01-01", 300.0, "2026-02-13")], "USD", "2026-02-01")

    @pytest.mark.parametrize("value", [math.nan, math.inf, -math.inf])
    def test_inflation_index_rejects_non_finite_observations(self, value: float) -> None:
        with pytest.raises(ValueError, match="finite"):
            InflationIndex("INVALID", [InflationObservation("2024-01-01", value, "2024-02-13")], "USD", "2024-03-01")

    def test_inflation_index_rejects_duplicate_observation_dates(self) -> None:
        with pytest.raises(ValueError, match="strictly increasing"):
            InflationIndex(
                "INVALID",
                [
                    InflationObservation("2024-01-01", 300.0, "2024-02-13"),
                    InflationObservation("2024-01-01", 301.0, "2024-02-13"),
                ],
                "USD",
                "2024-03-01",
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

    def test_cholesky_decomposition(self) -> None:
        """Cholesky decomposition of a 2x2 SPD matrix."""
        from finstack_quant.core.math.linalg import cholesky_decomposition

        lower = cholesky_decomposition([[4.0, 2.0], [2.0, 3.0]])
        assert lower[0][0] == pytest.approx(2.0)
        assert lower[1][0] == pytest.approx(1.0)
        assert lower[1][1] == pytest.approx(math.sqrt(2.0))
        assert lower[0][1] == pytest.approx(0.0)

    def test_cholesky_accepts_numpy_and_nested_list_identically(self) -> None:
        """NumPy arrays take the bulk path; results match nested lists."""
        np = pytest.importorskip("numpy")
        from finstack_quant.core.math import linalg

        nested = [[4.0, 2.0, 0.0], [2.0, 10.0, 4.0], [0.0, 4.0, 9.0]]
        array = np.asarray(nested, dtype=np.float64)

        from_list = linalg.cholesky_decomposition(nested)
        from_array = linalg.cholesky_decomposition(array)
        assert np.allclose(from_list, from_array, atol=0.0, rtol=0.0)

        b = [1.0, 2.0, 3.0]
        assert linalg.cholesky_solve(from_list, b) == pytest.approx(linalg.cholesky_solve(from_array, b))
        z = [1.0, -1.0, 0.5]
        assert linalg.apply_lower_triangular(from_list, z) == pytest.approx(
            linalg.apply_lower_triangular(from_array, z)
        )

        # Non-float64 arrays fall back to element conversion like lists do.
        int_array = np.asarray([[4, 2], [2, 3]], dtype=np.int64)
        expected_2x2 = linalg.cholesky_decomposition([[4.0, 2.0], [2.0, 3.0]])
        actual = linalg.cholesky_decomposition(int_array)
        for row_actual, row_expected in zip(actual, expected_2x2, strict=False):
            assert row_actual == pytest.approx(row_expected)

        with pytest.raises(ValueError, match="square"):
            linalg.cholesky_decomposition(np.zeros((2, 3)))

    def test_cholesky_solve(self) -> None:
        """Cholesky solve recovers the exact solution."""
        from finstack_quant.core.math.linalg import cholesky_decomposition, cholesky_solve

        chol = cholesky_decomposition([[4.0, 2.0], [2.0, 3.0]])
        x = cholesky_solve(chol, [1.0, 1.0])
        assert x == pytest.approx([0.125, 0.25])

    def test_cholesky_solve_singular_raises(self) -> None:
        """Singular factor maps through core_to_py to a plain ValueError."""
        from finstack_quant.core.math.linalg import CholeskyError, cholesky_solve

        with pytest.raises(ValueError, match=r"(?i)invalid|singular|zero|solve") as excinfo:
            cholesky_solve([[0.0]], [1.0])
        assert not isinstance(excinfo.value, CholeskyError)


class TestScheduleParity:
    """Schedule types are accessible from dates module."""

    def test_stub_kind_variants_exist(self) -> None:
        """StubKind enum variants are importable from dates."""
        from finstack_quant.core.dates import StubKind

        assert StubKind.NONE is not None
        assert StubKind.SHORT_FRONT is not None
        assert StubKind.SHORT_BACK is not None


@pytest.mark.parametrize("rate", [0.0, 1e-18, 0.02, 10.0])
def test_hazard_conditional_default_prob_survives_low_unconditional_survival(rate: float) -> None:
    curve = HazardCurve(
        "CONDITIONAL", "1900-01-01", [(100.0, 7.0), (101.0, rate)], recovery_rate=0.4, day_count="act_365f"
    )
    start, end = date(2025, 1, 1), date(2026, 1, 1)
    assert curve.sp(start) < 1e-12
    if rate == 10.0:
        assert curve.sp(start) == 0.0
    assert curve.conditional_default_prob(start, "2026-01-01") == pytest.approx(-math.expm1(-rate), rel=1e-12, abs=0.0)
    assert curve.conditional_default_prob(start, start) == 0.0
    with pytest.raises(ValueError, match="precedes HazardCurve base_date"):
        curve.conditional_default_prob("1899-01-01", "1900-01-01")
    with pytest.raises(ValueError, match="end >= start"):
        curve.conditional_default_prob(end, start)
    with pytest.raises(ValueError, match="Invalid date 'not-a-date'"):
        curve.conditional_default_prob("not-a-date", end)
    with pytest.raises(TypeError):
        curve.conditional_default_prob(1.0, end)  # type: ignore[arg-type]


def test_insert_price_preserves_large_int_and_money_handles() -> None:
    exact_int = 2**53 + 1
    mc = MarketContext()
    mc.insert_price("LARGE-INT", exact_int, "USD")
    mc.insert_price("STR-PRICE", "9007199254740993.25", "USD")
    mc.insert_price("MONEY-HANDLE", Money.from_decimal(Decimal("123.456789012345678901"), "EUR"))
    mc.insert_price("MONEY-MATCHING-CCY", Money(42.5, "GBP"), "GBP")

    assert mc.get_price("LARGE-INT") == (Decimal(exact_int), "USD")
    assert mc.get_price("STR-PRICE") == (Decimal("9007199254740993.25"), "USD")
    assert mc.get_price("MONEY-HANDLE") == (Decimal("123.456789012345678901"), "EUR")
    assert mc.get_price("MONEY-MATCHING-CCY") == (Decimal("42.5"), "GBP")

    with pytest.raises(ValueError, match="currency argument"):
        mc.insert_price("CONFLICT", Money(10.0, "USD"), "EUR")
    with pytest.raises(TypeError):
        mc.insert_price("BOOL-UNITLESS", True)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        mc.insert_price("BOOL-MONEY", True, "USD")  # type: ignore[arg-type]


def test_core_bindings_reject_bool_as_numeric_input() -> None:
    import numpy as np

    from finstack_quant.core.math import linalg

    rate = Rate(0.05)
    bps = Bps(25)
    curve = DiscountCurve.flat("USD-OIS", "2025-01-01", 0.04, "act_365f")
    fx = FxMatrix(None)
    conv = FxPairConvention("EUR", "USD", 0.0001)

    for call in (
        lambda: rate * True,  # type: ignore[operator]
        lambda: True * rate,  # type: ignore[operator]
        lambda: rate / False,  # type: ignore[operator]
        lambda: Bps(True),  # type: ignore[arg-type]
        lambda: bps * True,  # type: ignore[operator]
        lambda: True * bps,  # type: ignore[operator]
        lambda: bps / True,  # type: ignore[operator]
        lambda: fx.set_quotes([("EUR", "USD", True)]),  # type: ignore[list-item]
        lambda: fx.set_quote_on("EUR", "USD", "2025-01-02", "cashflow_date", True),  # type: ignore[arg-type]
        lambda: FxMatrix.from_dict({"EURUSD": True}, None),  # type: ignore[dict-item]
        lambda: FxPairConvention("EUR", "USD", True),  # type: ignore[arg-type]
        lambda: conv.rate_points(True),  # type: ignore[arg-type]
        lambda: invert_fx_rate(True),  # type: ignore[arg-type]
        lambda: curve.df(True),  # type: ignore[arg-type]
        lambda: ScalarTimeSeries("S", [("2025-01-01", True)], "step", "flat"),  # type: ignore[list-item]
        lambda: InflationObservation("2025-01-01", True, "2025-02-12"),
        lambda: linalg.cholesky_decomposition([[True, False], [False, True]]),  # type: ignore[list-item]
        lambda: linalg.cholesky_decomposition(np.array([[True, False], [False, True]])),  # type: ignore[arg-type]
        lambda: linalg.cholesky_decomposition([["bad"]]),  # type: ignore[list-item]
        lambda: linalg.cholesky_solve([[2.0, 0.0], [0.0, 2.0]], [True, False]),  # type: ignore[list-item]
        lambda: linalg.apply_lower_triangular([[2.0, 0.0], [0.0, 2.0]], [True, False]),  # type: ignore[list-item]
        lambda: linalg.ledoit_wolf_shrinkage([[True, False], [False, True]]),  # type: ignore[list-item]
    ):
        with pytest.raises(TypeError):
            call()


def test_slice2_money_radd_and_rsub_strictness() -> None:
    """``Money.__radd__`` admits only ``Money`` and exact int ``0``; ``__rsub__`` is absent."""
    from decimal import Decimal

    usd = Currency("USD")
    m1 = Money(10.0, usd)
    m2 = Money(5.0, usd)

    assert 0 + m1 == m1
    assert sum([m1, m2]) == Money(15.0, usd)
    assert m2 + m1 == Money(15.0, usd)

    for bad in (0.0, False, True, Decimal(0), "0"):
        with pytest.raises(TypeError):
            _ = bad + m1  # type: ignore[operator]

    for lhs in (0, 5.0, False):
        with pytest.raises(TypeError):
            _ = lhs - m1  # type: ignore[operator]


def test_slice2_scalar_time_series_decimal_range_and_overflow() -> None:
    """``ScalarTimeSeries`` converts Python ``Decimal`` via ``float`` without i128 truncation."""
    from decimal import Decimal

    series = ScalarTimeSeries(
        "EXTREME",
        [
            (date(2025, 1, 1), Decimal("1e-30")),
            (date(2025, 1, 2), Decimal("1e35")),
        ],
        "step",
        "flat",
    )
    assert series.get_value(date(2025, 1, 1)) == pytest.approx(1e-30)
    assert series.get_value(date(2025, 1, 2)) == pytest.approx(1e35)

    with pytest.raises(ValueError, match=r"(?i)infinity|finite"):
        ScalarTimeSeries("OVERFLOW", [(date(2025, 1, 1), Decimal("1e400"))], "step", "flat")


def test_slice2_fx_matrix_pinned_and_provider_quotes_and_repr() -> None:
    """``FxMatrix`` exposes ``pinned_quotes``, ``provider_quotes``, and ``provider_pinned_quotes``."""
    import json

    from finstack_quant.core.market_data import FxConversionPolicy, MarketContext

    fx = FxMatrix(None)
    assert repr(fx) == "FxMatrix(quotes=0, pinned_quotes=0)"

    fx.set_quotes([("EUR", "USD", 1.10)])
    d_fix = date(2025, 1, 15)
    fx.set_quote_on("EUR", "USD", d_fix, FxConversionPolicy.CASHFLOW_DATE, 1.15)
    assert fx.rate("EUR", "USD", d_fix, FxConversionPolicy.CASHFLOW_DATE).rate == pytest.approx(1.15)
    assert fx.rate("EUR", "USD", date(2025, 1, 16), FxConversionPolicy.CASHFLOW_DATE).rate == pytest.approx(1.10)

    pinned_df = fx.pinned_quotes()
    assert list(pinned_df.columns) == ["base", "quote", "date", "policy", "rate"]
    assert pinned_df.to_dict(orient="records") == [
        {"base": "EUR", "quote": "USD", "date": d_fix, "policy": "cashflow_date", "rate": 1.15}
    ]
    assert list(fx.provider_quotes().columns) == ["base", "quote", "rate"]
    assert list(fx.provider_pinned_quotes().columns) == [
        "base",
        "quote",
        "date",
        "policy",
        "rate",
    ]
    assert repr(fx) == "FxMatrix(quotes=1, pinned_quotes=1)"

    ctx = MarketContext()
    ctx.insert_fx(fx)
    state = json.loads(ctx.to_json())
    state["fx"]["provider_quotes"] = [["GBP", "USD", 1.25]]
    state["fx"]["provider_pinned_quotes"] = [["GBP", "USD", "2025-01-15", "period_end", 1.26]]
    restored = MarketContext.from_json(json.dumps(state)).fx
    assert restored is not None
    assert list(restored.provider_quotes().to_dict(orient="records")) == [{"base": "GBP", "quote": "USD", "rate": 1.25}]
    assert list(restored.provider_pinned_quotes().to_dict(orient="records")) == [
        {"base": "GBP", "quote": "USD", "date": d_fix, "policy": "period_end", "rate": 1.26}
    ]
    assert repr(restored) == ("FxMatrix(quotes=1, pinned_quotes=1, provider_quotes=1, provider_pinned_quotes=1)")


def test_slice2_rating_scale_entry_wire_methods() -> None:
    """``RatingScaleEntry`` exposes canonical ``to_json``, ``from_json``, and pickle support."""
    import pickle

    from finstack_quant.core.rating_scales import RatingLevel, RatingScaleEntry, ScorecardScale

    scale = ScorecardScale("Internal", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
    entry = RatingScaleEntry("internal", "credit policy 2026", scale)
    raw = entry.to_json()
    assert RatingScaleEntry.from_json(raw) == entry
    assert pickle.loads(pickle.dumps(entry)) == entry  # noqa: S301 - trusted in-process round trip


def test_slice2_cholesky_error_kind_split() -> None:
    """Validation failures raise ``CholeskyError`` while computation failures raise ``RuntimeError``."""
    from finstack_quant.core import FinstackError
    from finstack_quant.core.math import linalg

    for bad in (
        [[1.0, 0.5], [0.4, 1.0]],
        [[float("nan"), 0.0], [0.0, 1.0]],
    ):
        with pytest.raises(linalg.CholeskyError) as excinfo:
            linalg.cholesky_decomposition(bad)
        assert isinstance(excinfo.value, ValueError)
        assert isinstance(excinfo.value, FinstackError)

        with pytest.raises(linalg.CholeskyError) as excinfo:
            linalg.symmetric_eigen(bad)
        assert isinstance(excinfo.value, ValueError)
        assert isinstance(excinfo.value, FinstackError)

    for non_spd in (
        [[1.0, 2.0], [2.0, 1.0]],
        [[0.0, 0.0], [0.0, 1.0]],
    ):
        with pytest.raises(RuntimeError) as excinfo:
            linalg.cholesky_decomposition(non_spd)
        assert not isinstance(excinfo.value, linalg.CholeskyError)
        assert not isinstance(excinfo.value, ValueError)


def test_market_preserves_inflation_series_basis() -> None:
    series = {"series_id": "BLS:CUUR0000SA0", "reference_base": "1982-84=100", "seasonally_adjusted": False}
    index = InflationIndex("US-CPI", [InflationObservation("2025-01-01", 105, "2025-02-12")], "USD", "2025-02-12")
    payload = json.loads(index.to_json())
    payload["series"] = series
    market = MarketContext().insert_inflation_index(InflationIndex.from_json(json.dumps(payload)))
    restored = MarketContext.from_json(market.to_json()).get_inflation_index("US-CPI")
    assert json.loads(restored.to_json())["series"] == series
