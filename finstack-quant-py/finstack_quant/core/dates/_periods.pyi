"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/dates/periods.rs``."""

from __future__ import annotations

import datetime
from typing import Iterator, Optional
import pandas as pd

# Periods

class PeriodKind:
    """
    Frequency kind used to label schedule periods (month, quarter, year).

    Immutable, hashable enum-style type. Parsing accepts only ``daily``,
    ``weekly``, ``monthly``, ``quarterly``, ``semi_annual`` and ``annual``.
    Aliases, case variants and whitespace padding are rejected.

    Examples
    --------
    >>> from finstack_quant.core.dates import PeriodKind
    >>> (str(PeriodKind.QUARTERLY), PeriodKind.QUARTERLY.periods_per_year)
    ('quarterly', 4)

    """

    DAILY: PeriodKind
    """Daily periods (252 trading days per year)."""
    WEEKLY: PeriodKind
    """Weekly periods."""
    MONTHLY: PeriodKind
    """Monthly periods."""
    QUARTERLY: PeriodKind
    """Quarterly periods."""
    SEMI_ANNUAL: PeriodKind
    """Semi-annual periods."""
    ANNUAL: PeriodKind
    """Annual periods."""

    @classmethod
    def from_name(cls, name: str) -> PeriodKind:
        """
        Parse a period kind from a string.

        Parameters
        ----------
        name : str
            Exact canonical name: ``daily``, ``weekly``, ``monthly``,
            ``quarterly``, ``semi_annual`` or ``annual``.

        Returns
        -------
        PeriodKind

            Canonical frequency represented by the exact canonical lowercase name.

        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodKind
        >>> PeriodKind.from_name("quarterly") == PeriodKind.QUARTERLY
        True

        """
        ...

    @property
    def periods_per_year(self) -> int:
        """
        Number of periods per year for this frequency.

        Returns
        -------
        int
            Number of periods per year for this frequency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def prior_observation_date(self, first: datetime.date | str) -> datetime.date:
        """
        Observation date one period before *first*.

        Daily and weekly step back 1 and 7 calendar days; monthly, quarterly,
        semi-annual and annual step back 1, 3, 6 and 12 months with month-end
        clamping.

        Parameters
        ----------
        first : datetime.date | str
            First observation date of the series.

        Returns
        -------
        datetime.date
            The prior observation date.

        Raises
        ------
        ValueError
            If *first* is invalid or the prior observation date underflows the supported calendar.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodKind
        >>> PeriodKind.QUARTERLY.prior_observation_date("2025-03-31")
        datetime.date(2024, 12, 31)

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class PeriodId:
    """
    A period identifier such as ``2025Q1`` or ``2025M03``.

    Immutable, hashable value type.

    Examples
    --------
    >>> from finstack_quant.core.dates import PeriodId
    >>> (str(PeriodId.parse("2025Q2")), str(PeriodId.parse("2025Q2").shift(1)))
    ('2025Q2', '2025Q3')

    """

    @classmethod
    def parse(cls, code: str) -> PeriodId:
        """
        Parse a period code string.

        Parameters
        ----------
        code : str
            Period code such as ``"2025Q1"`` or fiscal ``"FY2025W53"``.
            Unmarked weekly identifiers remain strict ISO week-year values.

        Returns
        -------
        PeriodId

            Validated calendar or ``FY`` period encoded by ``code``.

        Raises
        ------
        ValueError
            If *code* cannot be parsed.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> str(PeriodId.parse("2025Q2"))
        '2025Q2'

        """
        ...

    @classmethod
    def month(cls, year: int, month: int) -> PeriodId:
        """
        Build a monthly period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        month : int
            Calendar month number from ``1`` through ``12`` for the period.

        Returns
        -------
        PeriodId
            Calendar-month identifier formatted as ``YYYYMmm``.

        Raises
        ------
        ValueError
            If *month* is outside ``1`` through ``12``.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> str(PeriodId.month(2025, 2))
        '2025M02'

        """
        ...

    @classmethod
    def quarter(cls, year: int, quarter: int) -> PeriodId:
        """
        Build a quarterly period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        quarter : int
            Quarter (1-4).

        Returns
        -------
        PeriodId
            Calendar-quarter identifier formatted as ``YYYYQq``.

        Raises
        ------
        ValueError
            If *quarter* is outside ``1`` through ``4``.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> str(PeriodId.quarter(2025, 2))
        '2025Q2'

        """
        ...

    @classmethod
    def annual(cls, year: int) -> PeriodId:
        """
        Build an annual period identifier.

        Parameters
        ----------
        year : int
            Calendar year.

        Returns
        -------
        PeriodId
            Calendar-year identifier formatted as ``YYYY``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> str(PeriodId.annual(2025))
        '2025'
        """
        ...

    @classmethod
    def half(cls, year: int, half: int) -> PeriodId:
        """
        Build a semi-annual period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        half : int
            Half (1 or 2).

        Returns
        -------
        PeriodId
            Calendar-half identifier formatted as ``YYYYHh``.

        Raises
        ------
        ValueError
            If *half* is not ``1`` or ``2``.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> str(PeriodId.half(2025, 2))
        '2025H2'

        """
        ...

    @classmethod
    def week(cls, year: int, week: int) -> PeriodId:
        """
        Build a weekly period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        week : int
            ISO week number (1-53).

        Returns
        -------
        PeriodId
            ISO week-year identifier formatted as ``YYYYWww``.

        Raises
        ------
        ValueError
            If *week* is not a valid ISO week number for *year*.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> str(PeriodId.week(2025, 2))
        '2025W02'

        """
        ...

    @classmethod
    def day(cls, year: int, ordinal: int) -> PeriodId:
        """
        Build a daily period identifier from an ordinal day.

        Parameters
        ----------
        year : int
            Calendar year.
        ordinal : int
            Ordinal day of the year (1-366).

        Returns
        -------
        PeriodId
            Calendar ordinal-day identifier formatted as ``YYYYDddd``.

        Raises
        ------
        ValueError
            If *ordinal* is not a valid day of *year*.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> str(PeriodId.day(2025, 2))
        '2025D002'

        """
        ...

    @property
    def year(self) -> int:
        """
        Gregorian or fiscal year label.

        Returns
        -------
        int
            Gregorian or fiscal year label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def index(self) -> int:
        """
        Ordinal index within the year.

        Returns
        -------
        int
            Ordinal index within the year.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def kind(self) -> PeriodKind:
        """
        Kind (frequency) of this period.

        Returns
        -------
        PeriodKind
            Kind (frequency) of this period.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_fiscal(self) -> bool:
        """
        Whether this identifier uses fiscal-year (``FY...``) semantics.

        Returns
        -------
        bool
            Whether fiscal holds for this `PeriodId`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def shift(self, periods: int) -> PeriodId:
        """
        Move by a signed number of periods of the same frequency and calendar.

        Parameters
        ----------
        periods : int
            Number of periods to move; negative moves back and ``0`` returns
            the identifier unchanged.

        Returns
        -------
        PeriodId

            Period ``periods`` steps away, with year rollover. Gregorian daily
            and weekly ids move by 1 or 7 calendar days; fiscal weeks roll
            after week 53.

        Raises
        ------
        ValueError
            If the result's year is out of range, or a fiscal daily identifier
            would leave days ``1..=365`` (the fiscal year's length depends on
            its calendar; use :meth:`next_fiscal` / :meth:`prev_fiscal` with an
            explicit :class:`FiscalConfig`).

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> str(PeriodId.quarter(2025, 4).shift(1))
        '2026Q1'
        >>> str(PeriodId.quarter(2025, 1).shift(-1))
        '2024Q4'
        """
        ...

    def next_fiscal(self, fiscal_config: FiscalConfig) -> PeriodId:
        """
        Next period using fiscal-year week/day capacity.

        Weekly fiscal IDs can advance through a partial week 53 even when the
        same-numbered ISO Gregorian year has only 52 weeks.

        Parameters
        ----------
        fiscal_config : FiscalConfig
            Fiscal-year start month and day used to determine the next fiscal
            week, month, quarter, or year boundary.

        Returns
        -------
        PeriodId
            Following fiscal period of the same kind, with week and day
            capacity determined by ``fiscal_config``.

        Raises
        ------
        ValueError
            If *fiscal_config* does not define a valid fiscal-year start date,
            or the next fiscal boundary is outside the supported date range.

        """
        ...

    def prev_fiscal(self, fiscal_config: FiscalConfig) -> PeriodId:
        """
        Previous period using fiscal-year week/day capacity.

        Parameters
        ----------
        fiscal_config : FiscalConfig
            Fiscal-year start month and day used to determine the preceding
            fiscal week, month, quarter, or year boundary.

        Returns
        -------
        PeriodId
            Preceding fiscal period of the same kind, with week and day
            capacity determined by ``fiscal_config``.

        Raises
        ------
        ValueError
            If *fiscal_config* does not define a valid fiscal-year start date,
            or the preceding fiscal boundary is outside the supported date range.

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __lt__(self, other: PeriodId) -> bool: ...
    def __le__(self, other: PeriodId) -> bool: ...
    def __gt__(self, other: PeriodId) -> bool: ...
    def __ge__(self, other: PeriodId) -> bool: ...

class Period:
    """
    A concrete period with start/end dates and an actual/forecast flag.

    Immutable value type returned by period-building functions.

    Examples
    --------
    >>> from finstack_quant.core.dates import build_periods
    >>> period = build_periods("2024Q1..Q1").periods[0]
    >>> (str(period.id), period.start, period.end)
    ('2024Q1', datetime.date(2024, 1, 1), datetime.date(2024, 4, 1))

    """

    @property
    def id(self) -> PeriodId:
        """
        Stable string identifier for this schedule period.

        Returns
        -------
        PeriodId
            Stable string identifier for this schedule period.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def start(self) -> datetime.date:
        """
        First date included in this schedule period.

        Returns
        -------
        datetime.date
            First date included in this schedule period.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def end(self) -> datetime.date:
        """
        First date after the period; the period does not include it.

        Returns
        -------
        datetime.date
            First date after the period; the period does not include it.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def is_actual(self) -> bool:
        """
        Whether this period is an actual (vs forecast).

        Returns
        -------
        bool
            Whether actual holds for this `Period`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            JSON object with ``id``, ``start``, ``end`` (ISO dates) and
            ``is_actual``.

        Raises
        ------
        ValueError
            If the period cannot be serialized.

        Examples
        --------
        >>> from finstack_quant.core.dates import Period, build_periods
        >>> period = build_periods("2024Q1..Q1").periods[0]
        >>> Period.from_json(period.to_json()) == period
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> Period:
        """
        Deserialize from the canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        Period
            The reconstructed period.

        Raises
        ------
        ValueError
            If *json* is malformed.
        Examples
        --------
        >>> from finstack_quant.core.dates import Period, build_periods
        >>> period = build_periods("2024Q1..Q1").periods[0]
        >>> Period.from_json(period.to_json()) == period
        True

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class PeriodPlan:
    """
    A plan containing a contiguous sequence of periods.

    Returned by :func:`build_periods` and :func:`build_fiscal_periods`.

    Examples
    --------
    >>> from finstack_quant.core.dates import build_periods
    >>> [str(period.id) for period in build_periods("2024Q1..Q2").periods]
    ['2024Q1', '2024Q2']

    """

    @property
    def periods(self) -> list[Period]:
        """
        List of periods in ascending order.

        Returns
        -------
        list[Period]
            List of periods in ascending order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Periods as a pandas DataFrame.

        Returns
        -------
        pandas.DataFrame
            One row per period with columns ``id`` (str), ``start`` and
            ``end`` (``datetime64``) and ``is_actual`` (bool).

        Raises
        ------
        ImportError
            If pandas is not installed.

        Examples
        --------
        >>> from finstack_quant.core.dates import build_periods
        >>> list(build_periods("2024Q1..Q2").to_dataframe().columns)
        ['id', 'start', 'end', 'is_actual']
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            JSON object with a ``periods`` array.

        Raises
        ------
        ValueError
            If the plan cannot be serialized.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodPlan, build_periods
        >>> plan = build_periods("2024Q1..Q2")
        >>> PeriodPlan.from_json(plan.to_json()) == plan
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> PeriodPlan:
        """
        Deserialize from the canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        PeriodPlan
            The reconstructed plan.

        Raises
        ------
        ValueError
            If *json* is malformed.
        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodPlan, build_periods
        >>> plan = build_periods("2024Q1..Q2")
        >>> PeriodPlan.from_json(plan.to_json()) == plan
        True

        """
        ...

    def __iter__(self) -> Iterator[Period]: ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class FiscalConfig:
    """
    Fiscal year configuration.

    Parameters
    ----------
    start_month : int
        Month when the fiscal year starts (1-12).
    start_day : int
        Day when the fiscal year starts (1-31).

    Raises
    ------
    ValueError
        If the month/day combination is invalid.

    Examples
    --------
    >>> from finstack_quant.core.dates import FiscalConfig
    >>> (FiscalConfig.us_federal().start_month, FiscalConfig.uk().start_day)
    (10, 6)

    """

    def __init__(self, start_month: int, start_day: int) -> None:
        """
        Create a fiscal configuration from a start month and day.

        Parameters
        ----------
        start_month : int
            Calendar month number from ``1`` through ``12`` at which each
            fiscal year begins.
        start_day : int
            Calendar day from ``1`` through ``31`` at which each fiscal year
            begins, subject to the selected start month's valid range.

        Raises
        ------
        ValueError
            If the combination is invalid.
        """
        ...

    @classmethod
    def calendar_year(cls) -> FiscalConfig:
        """
        Standard calendar year (January 1).

        Returns
        -------
        FiscalConfig
            Fiscal configuration beginning January 1.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> config = FiscalConfig.calendar_year()
        >>> (config.start_month, config.start_day)
        (1, 1)
        """
        ...

    @classmethod
    def us_federal(cls) -> FiscalConfig:
        """
        US Federal fiscal year (October 1).

        Returns
        -------
        FiscalConfig
            US federal fiscal configuration beginning October 1.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> config = FiscalConfig.us_federal()
        >>> (config.start_month, config.start_day)
        (10, 1)
        """
        ...

    @classmethod
    def uk(cls) -> FiscalConfig:
        """
        UK fiscal year (April 6).

        Returns
        -------
        FiscalConfig
            UK fiscal configuration beginning April 6.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> config = FiscalConfig.uk()
        >>> (config.start_month, config.start_day)
        (4, 6)
        """
        ...

    @classmethod
    def japan(cls) -> FiscalConfig:
        """
        Japanese fiscal year (April 1).

        Returns
        -------
        FiscalConfig
            Japanese fiscal configuration beginning April 1.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> config = FiscalConfig.japan()
        >>> (config.start_month, config.start_day)
        (4, 1)
        """
        ...

    @classmethod
    def australia(cls) -> FiscalConfig:
        """
        Australian fiscal year (July 1).

        Returns
        -------
        FiscalConfig
            Australian fiscal configuration beginning July 1.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> config = FiscalConfig.australia()
        >>> (config.start_month, config.start_day)
        (7, 1)
        """
        ...

    @property
    def start_month(self) -> int:
        """
        Month when the fiscal year starts (1-12).

        Returns
        -------
        int
            Month when the fiscal year starts (1-12).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def start_day(self) -> int:
        """
        Day when the fiscal year starts (1-31).

        Returns
        -------
        int
            Day when the fiscal year starts (1-31).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

def build_periods(
    spec: str,
    actuals_cutoff: Optional[str] = None,
) -> PeriodPlan:
    """
    Build periods from a range expression.

    Parameters
    ----------
    spec : str
        Range expression (e.g. ``"2025Q1..Q4"``, ``"2024M01..M12"``).
    actuals_cutoff : str | None
        Inclusive cutoff for actual/forecast split (e.g. ``"2025Q2"``), using
        the same period kind and fiscal/Gregorian calendar as *spec*.

    Returns
    -------
    PeriodPlan
        Plan containing the generated periods.

    Raises
    ------
    ValueError
        If *spec* or *actuals_cutoff* cannot be parsed, endpoint/cutoff kinds or fiscal
        semantics disagree, any period boundary is outside the supported date
        range, or the range exceeds 100,000 periods. Validation precedes
        allocation of the period list.

    Examples
    --------
    >>> from finstack_quant.core.dates import build_periods
    >>> [(str(period.id), period.is_actual) for period in build_periods("2024Q1..Q4", "2024Q2").periods]
    [('2024Q1', True), ('2024Q2', True), ('2024Q3', False), ('2024Q4', False)]

    """
    ...

def build_fiscal_periods(
    spec: str,
    fiscal_config: FiscalConfig,
    actuals_cutoff: Optional[str] = None,
) -> PeriodPlan:
    """
    Build fiscal periods with a custom fiscal year configuration.

    Parameters
    ----------
    spec : str
        Range expression.
    fiscal_config : FiscalConfig
        Fiscal year configuration.
    actuals_cutoff : str | None
        Inclusive cutoff for actual/forecast split, using the same period kind
        as *spec*. Unprefixed codes are interpreted as fiscal.

    Returns
    -------
    PeriodPlan
        Plan containing the generated fiscal periods.

    Raises
    ------
    ValueError
        If *spec* or *actuals_cutoff* cannot be parsed, endpoint/cutoff kinds disagree,
        any fiscal boundary is outside the supported date range, or the range
        exceeds 100,000 periods. Validation precedes allocation of the period list.

    Examples
    --------
    >>> from finstack_quant.core.dates import FiscalConfig, build_fiscal_periods
    >>> build_fiscal_periods("2024Q1..Q1", FiscalConfig.us_federal()).periods[0].start
    datetime.date(2023, 10, 1)

    """
    ...
