"""IMM, CDS-roll, and unadjusted monthly roll date bindings."""

from __future__ import annotations

from collections.abc import Callable
import datetime

import pytest

from finstack_quant.core import dates


def test_third_wednesday_and_friday_match_the_market_conventions() -> None:
    """March 2025: IMM lands on the 19th, listed-option expiry on the 21st."""
    assert dates.third_wednesday(3, 2025) == datetime.date(2025, 3, 19)
    assert dates.third_friday(3, 2025) == datetime.date(2025, 3, 21)
    # February 2025 starts on a Saturday — the third Friday is the 21st.
    assert dates.third_friday(2, 2025) == datetime.date(2025, 2, 21)


def test_month_out_of_range_raises() -> None:
    with pytest.raises(ValueError, match="invalid month: 13"):
        dates.third_wednesday(13, 2025)
    with pytest.raises(ValueError, match="invalid month: 0"):
        dates.imm_option_expiry(0, 2025)


def test_imm_and_cds_roll_navigation() -> None:
    """IMM dates are quarterly third Wednesdays; CDS rolls are the 20ths."""
    ref = datetime.date(2025, 5, 1)
    assert dates.next_imm(ref) == datetime.date(2025, 6, 18)
    assert dates.is_imm_date(datetime.date(2025, 3, 19))
    assert not dates.is_imm_date(datetime.date(2025, 3, 20))

    assert dates.next_cds_date(ref) == datetime.date(2025, 6, 20)
    assert dates.prev_cds_date(ref) == datetime.date(2025, 3, 20)
    assert dates.is_cds_date(datetime.date(2025, 6, 20))

    # Semi-annual rolls are the March and September dates only.
    assert dates.prev_cds_semiannual_roll(ref) == datetime.date(2025, 3, 20)
    assert dates.next_semiannual_cds_maturity(ref) == datetime.date(2025, 6, 20)


def test_option_expiries() -> None:
    """IMM option expiry precedes its future; monthly rolls are unadjusted."""
    imm_expiry = dates.imm_option_expiry(3, 2025)
    assert imm_expiry == datetime.date(2025, 3, 14)
    assert imm_expiry < dates.third_wednesday(3, 2025)

    ref = datetime.date(2025, 5, 1)
    assert dates.next_imm_option_expiry(ref) == datetime.date(2025, 6, 13)
    assert dates.next_third_friday(ref) == datetime.date(2025, 5, 16)
    assert dates.next_third_friday(ref) == dates.third_friday(5, 2025)


def test_monthly_third_friday_is_explicitly_unadjusted() -> None:
    """The primitive includes Good Friday instead of claiming exchange expiry."""
    assert dates.next_third_friday(datetime.date(2025, 4, 1)) == datetime.date(2025, 4, 18)


def test_next_helpers_are_strictly_forward_looking() -> None:
    """`next_*` never returns its own argument."""
    imm = datetime.date(2025, 3, 19)
    assert dates.next_imm(imm) > imm
    expiry = dates.next_third_friday(datetime.date(2025, 5, 1))
    assert dates.next_third_friday(expiry) > expiry


@pytest.mark.parametrize(
    "helper",
    [
        dates.next_imm,
        dates.next_cds_date,
        dates.next_semiannual_cds_maturity,
        dates.next_imm_option_expiry,
        dates.next_third_friday,
    ],
)
def test_forward_rolls_at_maximum_date_raise_value_error(
    helper: Callable[[datetime.date], datetime.date],
) -> None:
    """Missing successors fail promptly rather than panicking or looping."""
    with pytest.raises(ValueError, match="supported date range"):
        helper(datetime.date.max)


@pytest.mark.parametrize("helper", [dates.prev_cds_date, dates.prev_cds_semiannual_roll])
def test_backward_rolls_cannot_return_a_non_python_year(
    helper: Callable[[datetime.date], datetime.date],
) -> None:
    """A canonical date before year 1 cannot cross Python's date boundary."""
    with pytest.raises(ValueError, match=r"(?i)year|date|range"):
        helper(datetime.date.min)


@pytest.mark.parametrize("helper", [dates.third_wednesday, dates.third_friday, dates.imm_option_expiry])
@pytest.mark.parametrize("year", [-10000, 0, 10000, 2147483647, -2147483648])
def test_expiry_years_outside_python_range_raise_value_error(
    helper: Callable[[int, int], datetime.date], year: int
) -> None:
    """Caller-supplied years never reach unchecked calendar construction."""
    with pytest.raises(ValueError, match=r"(?i)year|date|range"):
        helper(3, year)


def test_date_boundary_roll_inclusivity_is_preserved() -> None:
    """Inclusive maturity/roll conventions keep an existing boundary date."""
    last_cds = datetime.date(9999, 12, 20)
    assert dates.next_semiannual_cds_maturity(last_cds) == last_cds
    with pytest.raises(ValueError, match="supported date range"):
        dates.next_cds_date(last_cds)
    first_semiannual = datetime.date(1, 3, 20)
    assert dates.prev_cds_semiannual_roll(first_semiannual) == first_semiannual


@pytest.mark.parametrize("year", [-10000, 0, 10000, 2147483647, -2147483648])
def test_estimated_sifma_rejects_unrepresentable_years(year: int) -> None:
    """Projection estimates share the fallible date boundary."""
    with pytest.raises(ValueError, match=r"(?i)year|date|range"):
        dates.estimated_sifma_settlement_date_for_class(3, year, dates.SifmaSettlementClass.A)
