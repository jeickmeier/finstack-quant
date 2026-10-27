"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/ecl.rs``."""

from __future__ import annotations

from typing import Any, ClassVar
import pandas as pd

class EclBucket:
    """
    One integration bucket of an ECL calculation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import EclBucket
    >>> [field for field in ("t_start", "t_end") if hasattr(EclBucket, field)]
    ['t_start', 't_end']
    """
    @property
    def t_start(self) -> float:
        """
        Bucket start in years.

        This property does not raise.

        Returns
        -------
        float
            Bucket start in years.
        """
    @property
    def t_end(self) -> float:
        """
        Bucket end in years.

        This property does not raise.

        Returns
        -------
        float
            Bucket end in years.
        """
    @property
    def marginal_pd(self) -> float:
        """
        Marginal default probability within the bucket (decimal).

        This property does not raise.

        Returns
        -------
        float
            Marginal default probability within the bucket (decimal).
        """
    @property
    def lgd(self) -> float:
        """
        Loss given default applied in the bucket (decimal).

        This property does not raise.

        Returns
        -------
        float
            Loss given default applied in the bucket (decimal).
        """
    @property
    def ead(self) -> float:
        """
        Exposure at default in the bucket, in base currency.

        This property does not raise.

        Returns
        -------
        float
            Exposure at default in the bucket, in base currency.
        """
    @property
    def discount_factor(self) -> float:
        """
        Discount factor at the bucket midpoint, or at recovery for Stage 3.

        This property does not raise.

        Returns
        -------
        float
            Discount factor at the bucket midpoint, or at recovery for Stage 3.
        """
    @property
    def ecl(self) -> float:
        """
        Bucket ECL contribution in base currency.

        This property does not raise.

        Returns
        -------
        float
            Bucket ECL contribution in base currency.
        """

class EclResult:
    """
    ECL for one exposure under one PD scenario, with bucket detail.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import EclResult
    >>> EclResult.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @property
    def exposure_id(self) -> str:
        """
        Exposure identifier.

        This property does not raise.

        Returns
        -------
        str
            Exposure identifier.
        """
    @property
    def stage(self) -> Stage:
        """
        Stage used for the measurement horizon.

        This property does not raise.

        Returns
        -------
        Stage
            Stage used for the measurement horizon.
        """
    @property
    def ecl(self) -> float:
        """
        Total ECL in the exposure's base currency.

        This property does not raise.

        Returns
        -------
        float
            Total ECL in the exposure's base currency.
        """
    @property
    def horizon(self) -> float:
        """
        Measurement horizon in years.

        This property does not raise.

        Returns
        -------
        float
            Measurement horizon in years.
        """
    @property
    def buckets(self) -> list[EclBucket]:
        """
        Bucket-level contributions in time order.

        This property does not raise.

        Returns
        -------
        list[EclBucket]
            Bucket-level contributions in time order.
        """
    @property
    def meta(self) -> Any:
        """
        Result metadata (numeric mode, rounding context) as a dict.

        This property does not raise.

        Returns
        -------
        Any
            Result metadata (numeric mode, rounding context) as a dict.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the buckets as a pandas ``DataFrame``.

        Columns: ``t_start``, ``t_end`` (years), ``marginal_pd``, ``lgd``
        (decimals), ``ead``, ``ecl`` (base currency), ``discount_factor``.
        One row per bucket in time order.

        Returns
        -------
        pd.DataFrame
            Export the buckets as a pandas ``DataFrame``. Columns: ``t_start``, ``t_end`` (years), ``marginal_pd``, ``lgd`` (decimals), ``ead``, ``ecl`` (base currency), ``discount_factor``. One row per bucket in time order.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> EclResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        EclResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``EclResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import EclResult
        >>> EclResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class Exposure:
    """
    A single credit exposure at a reporting date.

    Wraps the Rust ``Exposure`` and carries the two remaining-window PDs the
    simplified SICR test compares. ``classify_stage`` reads days past due,
    qualitative flags, rating labels, previous stage and performing periods
    (``ead``, ``lgd`` and ``eir`` do not affect staging); ``compute_ecl`` prices
    ``ead + undrawn * ccf`` with ``lgd``, ``eir``, ``remaining_maturity`` and
    any ``ead_schedule``.

    Parameters
    ----------
    id : str
        Unique identifier for the exposure.
    ead : float
        Drawn outstanding balance at the reporting date, in base currency.
    lgd : float
        Loss given default as a decimal fraction in ``[0, 1]``. For Stage 3,
        this is the undiscounted loss fraction; allowance is current EAD less
        discounted recovery ``(1 - lgd) * EAD``.
    eir : float
        Effective interest rate as a decimal annual rate, used as the IFRS 9
        discount rate.
    remaining_maturity : float
        Remaining maturity in years.
    current_pd : float
        Current probability of default from reporting over
        ``h = min(remaining_maturity, 30.0)`` years, as a decimal in ``[0, 1]``.
    origination_pd : float
        Initial-recognition expected PD for that same remaining window,
        conditional on survival to the reporting date, as a decimal in ``[0, 1]``.
        For original cumulative curve ``F``, elapsed years ``a`` and the capped
        horizon ``h = min(remaining_maturity, 30.0)``, supply
        ``(F(a + h) - F(a)) / (1 - F(a))``.
    dpd : int | None
        Days past due. ``None`` uses the Rust ``Exposure`` default of ``0``
        (performing).
    undrawn : float
        Undrawn commitment in the same currency as ``ead``. Default ``0.0``.
    ccf : float
        Credit-conversion factor applied to ``undrawn``, decimal in ``[0, 1]``.
        Default ``0.75`` (Basel IRB revolver).
    current_rating : str | None
        Current rating label, used with ``origination_rating`` for the
        rating-downgrade trigger. Default ``None``.
    origination_rating : str | None
        Rating label at initial recognition. Default ``None``.
    qualitative_flags : QualitativeFlags | None
        SICR and default-evidence flags. Default: no flags.
    previous_stage : Stage | str | None
        Stage assigned at the previous reporting date, enabling the curing
        rules. Accepts a ``Stage`` or ``"stage1"``, ``"stage2"``, or
        ``"stage3"``. Default ``None``.
    consecutive_performing_periods : int | None
        Performing periods since the last trigger, for curing. ``None`` uses
        the Rust ``Exposure`` default of ``0``.
    ead_schedule : list[tuple[float, float]] | None
        Optional EAD amortisation profile as ``(time_years, ead)`` knots.
    segments : list[str] | None
        Portfolio segment keys. Default ``[]``.

    Raises
    ------
    ValueError
        If a non-``None`` ``previous_stage`` is neither a ``Stage`` nor a
        recognized stage string. Financial inputs are validated by staging and
        ECL calculations.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure
    >>> Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015).dpd
    0
    """
    def __init__(
        self,
        id: str,
        ead: float,
        lgd: float,
        eir: float,
        remaining_maturity: float,
        current_pd: float,
        origination_pd: float,
        dpd: int | None = None,
        undrawn: float = 0.0,
        ccf: float = 0.75,
        current_rating: str | None = None,
        origination_rating: str | None = None,
        qualitative_flags: QualitativeFlags | None = None,
        previous_stage: Any | None = None,
        consecutive_performing_periods: int | None = None,
        ead_schedule: list[tuple[float, float]] | None = None,
        segments: list[str] | None = None,
    ) -> None: ...
    @property
    def current_pd(self) -> float:
        """Current reporting-date PD over the capped SICR window.

        This property does not raise.

        Returns
        -------
        float
            Decimal probability in ``[0, 1]`` over
            ``min(remaining_maturity, 30.0)`` years, covering the same window
            as ``origination_pd``; validated when staging runs.
        """
    @current_pd.setter
    def current_pd(self, value: float) -> None:
        """Replace the current PD used by subsequent staging calls.

        Parameters
        ----------
        value : float
            Decimal PD from reporting over ``min(remaining_maturity, 30.0)`` years.
            Staging rejects non-finite values or probabilities outside ``[0, 1]``.

        This setter stores the value, does not raise, and leaves validation to staging.
        """
    @property
    def origination_pd(self) -> float:
        """Initial-recognition expected PD for the same capped SICR window.

        This property does not raise.

        Returns
        -------
        float
            Decimal PD conditional on survival to reporting. For original
            cumulative curve ``F``, age ``a`` and
            ``h = min(remaining_maturity, 30.0)``, this is
            ``(F(a + h) - F(a)) / (1 - F(a))``.
        """
    @origination_pd.setter
    def origination_pd(self, value: float) -> None:
        """Replace the original expected PD used by subsequent staging calls.

        Parameters
        ----------
        value : float
            Decimal PD over the same capped SICR window as ``current_pd``,
            conditional on survival to reporting, in ``[0, 1]``.

        This setter stores the value, does not raise, and leaves validation to staging.
        """
    @property
    def id(self) -> str:
        """
        Unique identifier for the exposure.

        This property does not raise.

        Returns
        -------
        str
            Unique identifier for the exposure.
        """
    @property
    def ead(self) -> float:
        """
        Drawn outstanding balance in base currency.

        This property does not raise.

        Returns
        -------
        float
            Drawn outstanding balance in base currency.
        """
    @property
    def undrawn(self) -> float:
        """
        Undrawn commitment in the same currency as ``ead``.

        This property does not raise.

        Returns
        -------
        float
            Undrawn commitment in the same currency as ``ead``.
        """
    @property
    def ccf(self) -> float:
        """
        Credit-conversion factor applied to ``undrawn`` (decimal in ``[0, 1]``).

        This property does not raise.

        Returns
        -------
        float
            Credit-conversion factor applied to ``undrawn`` (decimal in ``[0, 1]``).
        """
    @property
    def lgd(self) -> float:
        """
        Loss given default as a decimal fraction.

        This property does not raise.

        Returns
        -------
        float
            Loss given default as a decimal fraction.
        """
    @property
    def eir(self) -> float:
        """
        Effective interest rate as a decimal annual rate.

        This property does not raise.

        Returns
        -------
        float
            Effective interest rate as a decimal annual rate.
        """
    @property
    def remaining_maturity(self) -> float:
        """
        Remaining maturity in years.

        This property does not raise.

        Returns
        -------
        float
            Remaining maturity in years.
        """
    @property
    def dpd(self) -> int:
        """
        Days the exposure is past due at the reporting date.

        This property does not raise.

        Returns
        -------
        int
            Whole days past due; ``0`` for a performing exposure.
        """
    @property
    def current_rating(self) -> str | None:
        """
        Current rating label, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Current rating label, or ``None``.
        """
    @property
    def origination_rating(self) -> str | None:
        """
        Rating label at initial recognition, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Rating label at initial recognition, or ``None``.
        """
    @property
    def qualitative_flags(self) -> QualitativeFlags:
        """
        SICR and default-evidence flags.

        This property does not raise.

        Returns
        -------
        QualitativeFlags
            SICR and default-evidence flags.
        """
    @property
    def previous_stage(self) -> Stage | None:
        """
        Stage at the previous reporting date, or ``None``.

        This property does not raise.

        Returns
        -------
        Stage | None
            Stage at the previous reporting date, or ``None``.
        """
    @property
    def consecutive_performing_periods(self) -> int:
        """
        Performing periods since the last trigger.

        This property does not raise.

        Returns
        -------
        int
            Performing periods since the last trigger.
        """
    @property
    def ead_schedule(self) -> list[tuple[float, float]] | None:
        """
        EAD amortisation profile as ``(time_years, ead)`` knots, or ``None``.

        This property does not raise.

        Returns
        -------
        list[tuple[float, float]] | None
            EAD amortisation profile as ``(time_years, ead)`` knots, or ``None``.
        """
    @property
    def segments(self) -> list[str]:
        """
        Portfolio segment keys.

        This property does not raise.

        Returns
        -------
        list[str]
            Portfolio segment keys.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the exposure as a single-row pandas ``DataFrame``.

        Columns: ``id``, ``ead``, ``undrawn``, ``ccf``, ``lgd``, ``eir``,
        ``remaining_maturity``, ``current_pd``, ``origination_pd``, ``dpd``,
        ``current_rating``, ``origination_rating``. Amounts are in the
        exposure's base currency; ``ccf``, ``lgd`` and the PDs are decimal
        fractions; ``eir`` is a decimal annual rate; ``remaining_maturity`` is
        in years; ``dpd`` is whole days.

        Returns
        -------
        pd.DataFrame
            Export the exposure as a single-row pandas ``DataFrame``. Columns: ``id``, ``ead``, ``undrawn``, ``ccf``, ``lgd``, ``eir``, ``remaining_maturity``, ``current_pd``, ``origination_pd``, ``dpd``, ``current_rating``, ``origination_rating``. Amounts are in the exposure's base currency; ``ccf``, ``lgd`` and the PDs are decimal fractions; ``eir`` is a decimal annual rate; ``remaining_maturity`` is in years; ``dpd`` is whole days.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """

class QualitativeFlags:
    """
    Qualitative SICR and default-evidence flags for staging (IFRS 9 B5.5.17 / B5.5.37).

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    watchlist : bool
        Exposure is on an internal watchlist (SICR indicator). Default ``False``.
    forbearance : bool
        Forbearance measures were granted (SICR indicator). Default ``False``.
    adverse_conditions : bool
        Adverse business, financial or economic conditions (SICR indicator).
        Default ``False``.
    custom : list[str]
        Additional caller-defined SICR flags; any non-empty entry counts as an
        active flag. Default ``[]``.
    bankruptcy : bool
        Objective evidence of default: bankruptcy or similar proceedings.
        Default ``False``.
    distressed_modification : bool
        Objective evidence of default: distressed restructuring. Default ``False``.
    cross_default : bool
        Objective evidence of default: cross-default triggered. Default ``False``.
    other_default_evidence : list[str]
        Additional caller-defined default-evidence flags. Default ``[]``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import QualitativeFlags
    >>> QualitativeFlags(watchlist=True).active_flags
    ['watchlist']
    """
    def __init__(
        self,
        watchlist: bool = False,
        forbearance: bool = False,
        adverse_conditions: bool = False,
        custom: list[str] = ...,
        bankruptcy: bool = False,
        distressed_modification: bool = False,
        cross_default: bool = False,
        other_default_evidence: list[str] = ...,
    ) -> None: ...
    @property
    def watchlist(self) -> bool:
        """
        Internal watchlist flag (SICR indicator).

        This property does not raise.

        Returns
        -------
        bool
            Internal watchlist flag (SICR indicator).
        """
    @property
    def forbearance(self) -> bool:
        """
        Forbearance flag (SICR indicator).

        This property does not raise.

        Returns
        -------
        bool
            Forbearance flag (SICR indicator).
        """
    @property
    def adverse_conditions(self) -> bool:
        """
        Adverse-conditions flag (SICR indicator).

        This property does not raise.

        Returns
        -------
        bool
            Adverse-conditions flag (SICR indicator).
        """
    @property
    def custom(self) -> list[str]:
        """
        Caller-defined SICR flags.

        This property does not raise.

        Returns
        -------
        list[str]
            Caller-defined SICR flags.
        """
    @property
    def bankruptcy(self) -> bool:
        """
        Bankruptcy flag (objective evidence of default).

        This property does not raise.

        Returns
        -------
        bool
            Bankruptcy flag (objective evidence of default).
        """
    @property
    def distressed_modification(self) -> bool:
        """
        Distressed-modification flag (objective evidence of default).

        This property does not raise.

        Returns
        -------
        bool
            Distressed-modification flag (objective evidence of default).
        """
    @property
    def cross_default(self) -> bool:
        """
        Cross-default flag (objective evidence of default).

        This property does not raise.

        Returns
        -------
        bool
            Cross-default flag (objective evidence of default).
        """
    @property
    def other_default_evidence(self) -> list[str]:
        """
        Caller-defined default-evidence flags.

        This property does not raise.

        Returns
        -------
        list[str]
            Caller-defined default-evidence flags.
        """
    @property
    def active_flags(self) -> list[str]:
        """
        Names of the active SICR flags, in waterfall order.

        This property does not raise.

        Returns
        -------
        list[str]
            Names of the active SICR flags, in waterfall order.
        """
    @property
    def active_default_evidence(self) -> list[str]:
        """
        Names of the active default-evidence flags, in waterfall order.

        This property does not raise.

        Returns
        -------
        list[str]
            Names of the active default-evidence flags, in waterfall order.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> QualitativeFlags:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        QualitativeFlags
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``QualitativeFlags`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import QualitativeFlags
        >>> QualitativeFlags.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class Stage:
    """IFRS 9 impairment stage.

    ``Stage.Stage1`` measures a 12-month ECL; ``Stage.Stage2`` and
    ``Stage.Stage3`` measure lifetime ECL (Stage 3 is credit-impaired).
    ``value`` is the serde name (``"stage1"``, ``"stage2"``, ``"stage3"``)
    accepted wherever a stage string is taken.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Stage
    >>> Stage.from_str("stage2").value
    'stage2'
    >>> Stage.Stage2 == Stage.from_str("stage2")
    True
    """

    Stage1: ClassVar[Stage]
    """Performing: 12-month ECL.
    """
    Stage2: ClassVar[Stage]
    """Significant increase in credit risk: lifetime ECL.
    """
    Stage3: ClassVar[Stage]
    """Credit-impaired: lifetime ECL with PD = 1.
    """
    @staticmethod
    def from_str(value: str) -> Stage:
        """
        Parse the serde name (``"stage1"``, ``"stage2"`` or ``"stage3"``).

        Parameters
        ----------
        value : str
            Serde stage name; one of ``"stage1"``, ``"stage2"`` or
            ``"stage3"``, matching the wire representation exactly.

        Returns
        -------
        Stage
            The impairment stage the name denotes.

        Raises
        ------
        ValueError
            If ``value`` is not one of the three serde names.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import Stage
        >>> Stage.from_str("stage2").value
        'stage2'
        """
    @property
    def value(self) -> str:
        """
        Serde name used in JSON and accepted by ``compute_ecl(stage=...)``.

        This property does not raise.

        Returns
        -------
        str
            Serde name used in JSON and accepted by ``compute_ecl(stage=...)``.
        """

class StageResult:
    """Outcome of IFRS 9 stage classification with its trigger audit trail.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import StageResult
    >>> StageResult.from_json('{"stage":"stage1","triggers":["no_trigger"],"cured":false}').stage.value
    'stage1'
    """
    @property
    def stage(self) -> Stage:
        """
        IFRS 9 impairment stage assigned to the exposure.

        This property does not raise.

        Returns
        -------
        Stage
            Stage 1, 2 or 3 as decided by the staging rules.
        """
    @property
    def triggers(self) -> list[str]:
        """
                Ordered trigger audit trail rendered by the canonical Rust display
        (``["no_trigger"]`` for a clean Stage 1).

                This property does not raise.

                Returns
                -------
                list[str]
                    Ordered trigger audit trail rendered by the canonical Rust display (``["no_trigger"]`` for a clean Stage 1).
        """
    @property
    def cured(self) -> bool:
        """
        Whether the exposure was cured down from a higher previous stage.

        This property does not raise.

        Returns
        -------
        bool
            Whether the exposure was cured down from a higher previous stage.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> StageResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        StageResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``StageResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import StageResult
        >>> StageResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class StagingConfig:
    """
    IFRS 9 staging policy: SICR thresholds, days-past-due backstops, qualitative
    switches and curing windows.

    Omitted parameters use the canonical Rust ``StagingConfig::default()``
    values, so ``StagingConfig()`` is the standard policy. Explicit ``None``
    disables the relative PD trigger.

    Raises
    ------
    ValueError
        If the absolute or enabled relative threshold is negative or non-finite.

    Parameters
    ----------
    pd_delta_absolute : float | None
        Absolute remaining-window PD increase (decimal, ``0.01`` = 1pp) that fires the
        Stage 2 SICR trigger.
    pd_delta_relative : float | None
        Finite non-negative relative remaining-window PD multiple that fires
        Stage 2. Omitted uses ``2.0`` (PD doubled); ``None`` disables the trigger.
    rating_downgrade_notches : int | None
        Downgrade notches from origination that fire Stage 2; ``0`` disables
        the trigger.
    rating_scale_labels : list[str] | None
        Ordered best-to-worst rating labels used to count notches; ``None``
        uses the 10-state S&P/Fitch scale.
    dpd_stage2_threshold : int | None
        Days past due at or above which the Stage 2 backstop fires (default 30).
    dpd_stage3_threshold : int | None
        Days past due at or above which Stage 3 is forced (default 90).
    qualitative_triggers_enabled : bool | None
        Whether any active SICR flag fires Stage 2.
    stage3_qualitative_triggers_enabled : bool | None
        Whether default-evidence flags force Stage 3.
    cure_periods_stage2_to_1 : int | None
        Consecutive performing periods required to cure Stage 2 to Stage 1.
    cure_periods_stage3_to_2 : int | None
        Consecutive performing periods required to cure Stage 3 to Stage 2.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import StagingConfig
    >>> StagingConfig(pd_delta_absolute=0.02).pd_delta_absolute
    0.02
    """
    def __init__(
        self,
        pd_delta_absolute: float | None = None,
        pd_delta_relative: float | None = 2.0,
        rating_downgrade_notches: int | None = None,
        rating_scale_labels: list[str] | None = None,
        dpd_stage2_threshold: int | None = None,
        dpd_stage3_threshold: int | None = None,
        qualitative_triggers_enabled: bool | None = None,
        stage3_qualitative_triggers_enabled: bool | None = None,
        cure_periods_stage2_to_1: int | None = None,
        cure_periods_stage3_to_2: int | None = None,
    ) -> None: ...
    @property
    def pd_delta_absolute(self) -> float:
        """
        Absolute remaining-window PD increase (decimal) that fires Stage 2.

        This property does not raise.

        Returns
        -------
        float
            Absolute remaining-window PD increase (decimal) that fires Stage 2.
        """
    @property
    def pd_delta_relative(self) -> float | None:
        """
        Relative remaining-window PD multiple that fires Stage 2.

        This property does not raise.

        Returns
        -------
        float | None
            Finite non-negative relative threshold, or ``None`` when disabled.
        """
    @property
    def rating_downgrade_notches(self) -> int:
        """
        Downgrade notches that fire Stage 2 (``0`` = disabled).

        This property does not raise.

        Returns
        -------
        int
            Downgrade notches that fire Stage 2 (``0`` = disabled).
        """
    @property
    def rating_scale_labels(self) -> list[str] | None:
        """
        Ordered best-to-worst rating labels, or ``None`` for the default scale.

        This property does not raise.

        Returns
        -------
        list[str] | None
            Ordered best-to-worst rating labels, or ``None`` for the default scale.
        """
    @property
    def dpd_stage2_threshold(self) -> int:
        """
        Days past due at or above which the Stage 2 backstop fires.

        This property does not raise.

        Returns
        -------
        int
            Days past due at or above which the Stage 2 backstop fires.
        """
    @property
    def dpd_stage3_threshold(self) -> int:
        """
        Days past due at or above which Stage 3 is forced.

        This property does not raise.

        Returns
        -------
        int
            Days past due at or above which Stage 3 is forced.
        """
    @property
    def qualitative_triggers_enabled(self) -> bool:
        """
        Whether active SICR flags fire Stage 2.

        This property does not raise.

        Returns
        -------
        bool
            Whether active SICR flags fire Stage 2.
        """
    @property
    def stage3_qualitative_triggers_enabled(self) -> bool:
        """
        Whether default-evidence flags force Stage 3.

        This property does not raise.

        Returns
        -------
        bool
            Whether default-evidence flags force Stage 3.
        """
    @property
    def cure_periods_stage2_to_1(self) -> int:
        """
        Performing periods required to cure Stage 2 to Stage 1.

        This property does not raise.

        Returns
        -------
        int
            Performing periods required to cure Stage 2 to Stage 1.
        """
    @property
    def cure_periods_stage3_to_2(self) -> int:
        """
        Performing periods required to cure Stage 3 to Stage 2.

        This property does not raise.

        Returns
        -------
        int
            Performing periods required to cure Stage 3 to Stage 2.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If a threshold is negative/non-finite or the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> StagingConfig:
        """
        Deserialize from canonical JSON (unknown fields are rejected).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        StagingConfig
            Deserialize from canonical JSON (unknown fields are rejected).

        Raises
        ------
        ValueError
            If ``json`` is malformed or a threshold is negative/non-finite.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import StagingConfig
        >>> StagingConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class WeightedEclResult:
    """Probability-weighted ECL across macro scenarios with the per-scenario
    audit trail.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure, compute_ecl
    >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015)
    >>> result = compute_ecl(exp, [(1.0, 0.02), (3.0, 0.06)], stage="stage2")
    >>> result.stage.value
    'stage2'
    >>> list(result.to_dataframe().columns)[:2]
    ['scenario', 'weight']
    """
    @property
    def exposure_id(self) -> str:
        """
        Exposure identifier.

        This property does not raise.

        Returns
        -------
        str
            Exposure identifier.
        """
    @property
    def stage(self) -> Stage:
        """
        Stage used for the measurement horizon.

        This property does not raise.

        Returns
        -------
        Stage
            Stage used for the measurement horizon.
        """
    @property
    def ecl(self) -> float:
        """
        Probability-weighted ECL in the exposure's base currency.

        This property does not raise.

        Returns
        -------
        float
            Probability-weighted ECL in the exposure's base currency.
        """
    @property
    def scenario_breakdown(self) -> list[tuple[str, float, EclResult]]:
        """
        Per-scenario ``(scenario_id, weight, EclResult)`` triples.

        This property does not raise.

        Returns
        -------
        list[tuple[str, float, EclResult]]
            Per-scenario ``(scenario_id, weight, EclResult)`` triples.
        """
    @property
    def meta(self) -> Any:
        """
        Result metadata (numeric mode, rounding context) as a dict.

        This property does not raise.

        Returns
        -------
        Any
            Result metadata (numeric mode, rounding context) as a dict.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the scenario x bucket audit trail as a pandas ``DataFrame``.

        Columns: ``scenario`` (scenario id), ``weight`` (decimal probability),
        ``t_start``, ``t_end`` (years), ``marginal_pd``, ``lgd`` (decimals),
        ``ead``, ``ecl`` (base currency, unweighted per scenario),
        ``discount_factor``. One row per (scenario, bucket).

        Returns
        -------
        pd.DataFrame
            Export the scenario x bucket audit trail as a pandas ``DataFrame``. Columns: ``scenario`` (scenario id), ``weight`` (decimal probability), ``t_start``, ``t_end`` (years), ``marginal_pd``, ``lgd`` (decimals), ``ead``, ``ecl`` (base currency, unweighted per scenario), ``discount_factor``. One row per (scenario, bucket).

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> WeightedEclResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        WeightedEclResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``WeightedEclResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import WeightedEclResult
        >>> WeightedEclResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

def classify_stage(exposure: Exposure, config: StagingConfig | None = None) -> StageResult:
    """Classify an exposure into an IFRS 9 stage.

    Runs the full Rust staging waterfall: the Stage 3 days-past-due backstop,
    default-evidence flags, the absolute and relative PD-delta SICR tests
    (``current_pd`` versus ``origination_pd``), the rating-downgrade notch test,
    qualitative SICR flags, the Stage 2 days-past-due backstop and curing.
    Both supplied PDs must cover ``min(remaining_maturity, 30.0)`` years from
    reporting; the original PD must be conditional on survival to reporting.
    The caller aligns these scalar risks to that window before classification.

    Parameters
    ----------
    exposure : Exposure
        The credit exposure; ``ead``, ``lgd`` and ``eir`` do not affect staging.
    config : StagingConfig | None
        Staging policy. ``None`` uses the canonical Rust defaults
        (1pp absolute PD delta, 30/90 DPD backstops, qualitative triggers on).

    Returns
    -------
    StageResult
        Assigned ``stage`` plus the ordered ``triggers`` audit trail and the
        ``cured`` flag.

    Raises
    ------
    ValueError
        If PDs are non-finite or outside [0, 1], maturity or staging thresholds
        are invalid, or an enabled relative PD ratio is not finite. Zero
        origination PD with positive current PD raises when PD-delta testing
        runs; both zero PDs are stable. Stage 3 backstops skip PD-delta testing.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure, classify_stage
    >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015, dpd=35)
    >>> classify_stage(exp).stage.value
    'stage2'
    """

def compute_ecl(
    exposure: Exposure,
    pd_schedule: list[tuple[float, float]],
    stage: Stage | str,
    bucket_width_years: float | None = None,
    stage3_time_to_recovery_years: float | None = None,
) -> WeightedEclResult:
    """Compute single-scenario ECL for one exposure.

    The priced exposure at default is ``ead + undrawn * ccf``; ``lgd``, ``eir``,
    ``remaining_maturity`` and any ``ead_schedule`` are read from the exposure.

    Parameters
    ----------
    exposure : Exposure
        The credit exposure to measure.
    pd_schedule : list[tuple[float, float]]
        Cumulative PD curve as ``[(time_years, cumulative_pd), ...]``, ascending
        in time and non-decreasing in PD. A ``(0.0, 0.0)`` knot is inserted
        when absent.
    stage : Stage | str
        Measurement stage (``Stage`` or serde name ``"stage1"``/``"stage2"``/
        ``"stage3"``). Required: classify first with ``classify_stage`` under
        the desk's ``StagingConfig`` and pass ``result.stage``.
    bucket_width_years : float | None
        Finite integration width of at least 0.0001 years (``0.25`` = quarterly); ``None`` uses
        the canonical policy default.
    stage3_time_to_recovery_years : float | None
        Stage 3 discounting horizon to expected recovery, in years; ``None``
        uses the canonical policy default.

    Returns
    -------
    WeightedEclResult
        ``ecl`` in base currency, the ``stage`` used, and a one-scenario
        ``scenario_breakdown`` with bucket detail (``to_dataframe()``).

    Raises
    ------
    ValueError
        If ``stage`` is unknown, the PD or EAD schedule is invalid, or an
        exposure input is outside its accepted range.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure, classify_stage, compute_ecl
    >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015)
    >>> stage = classify_stage(exp).stage
    >>> compute_ecl(exp, [(1.0, 0.02), (3.0, 0.06)], stage).ecl > 0
    True
    """

def compute_ecl_weighted(
    exposure: Exposure,
    scenarios: list[tuple[float, list[tuple[float, float]]]],
    stage: Stage | str,
    bucket_width_years: float | None = None,
    stage3_time_to_recovery_years: float | None = None,
) -> WeightedEclResult:
    """Compute probability-weighted ECL across macro scenarios.

    Parameters
    ----------
    exposure : Exposure
        The credit exposure to measure (EAD is ``ead + undrawn * ccf``).
    scenarios : list[tuple[float, list[tuple[float, float]]]]
        ``(weight, pd_schedule)`` pairs; weights must sum to ``1.0`` and each
        schedule follows the ``compute_ecl`` conventions.
    stage : Stage | str
        Measurement stage (required); classify first with ``classify_stage``
        under the desk's ``StagingConfig`` and pass ``result.stage``.
    bucket_width_years : float | None
        Finite integration width of at least 0.0001 years; ``None`` uses the canonical default.
    stage3_time_to_recovery_years : float | None
        Stage 3 discounting horizon in years; ``None`` uses the canonical default.

    Returns
    -------
    WeightedEclResult
        Probability-weighted ``ecl`` with one ``scenario_breakdown`` entry per
        scenario.

    Raises
    ------
    ValueError
        If ``scenarios`` is empty, weights do not sum to ``1.0``, ``stage`` is
        unknown, a schedule is invalid, or an exposure input is out of range.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure, compute_ecl_weighted
    >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 1.0, 0.02, 0.015)
    >>> scenarios = [(0.7, [(1.0, 0.02)]), (0.3, [(1.0, 0.05)])]
    >>> len(compute_ecl_weighted(exp, scenarios, stage="stage1").scenario_breakdown)
    2
    """
