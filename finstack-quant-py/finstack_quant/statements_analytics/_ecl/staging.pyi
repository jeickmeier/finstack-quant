"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/ecl/staging.rs``."""

from __future__ import annotations

from collections.abc import Sequence
from decimal import Decimal
from datetime import date
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from typing import Any, Literal, ClassVar, NotRequired, TypedDict
import pandas as pd
import polars as pl
from finstack_quant.statements_analytics._ecl.model import Exposure, compute_ecl

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

class QualitativeFlags:
    """
    Qualitative SICR and default-evidence flags for staging (IFRS 9 B5.5.17 / B5.5.37).

    Every parameter is keyword-only: Rust reads the keywords as the
    ``QualitativeFlags`` serde struct, in which every flag defaults to
    ``False`` and every list to ``[]``.

    Raises
    ------
    ValueError
        If a keyword is unknown or a value does not have its wire form.
    TypeError
        If a positional argument is passed.

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
        *,
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

class StagingConfig:
    """
    IFRS 9 staging policy: SICR thresholds, days-past-due backstops, qualitative
    switches and curing windows.

    Every parameter is keyword-only: Rust reads the keywords as the
    ``StagingConfig`` serde struct, and an omitted keyword takes the Rust
    ``StagingConfig::default()`` value (the embedded default IFRS 9 policy), so
    ``StagingConfig()`` is the standard policy. Explicit ``None`` disables the
    relative PD trigger; no other field takes ``None``.

    Raises
    ------
    ValueError
        If a keyword is unknown, a value does not have its wire form (``None``
        for a non-optional field, a non-finite float), or the absolute or
        enabled relative threshold is negative.
    TypeError
        If a positional argument is passed.

    Parameters
    ----------
    pd_delta_absolute : float
        Absolute remaining-window PD increase (decimal, ``0.01`` = 1pp) that fires the
        Stage 2 SICR trigger. Default ``0.01``.
    pd_delta_relative : float | None
        Finite non-negative relative remaining-window PD multiple that fires
        Stage 2. Omitted uses ``2.0`` (PD doubled); ``None`` disables the trigger.
    rating_downgrade_notches : int
        Downgrade notches from origination that fire Stage 2; ``0`` disables
        the trigger. Default ``3``.
    rating_scale_labels : list[str] | None
        Ordered best-to-worst rating labels used to count notches; ``None``
        (default) uses the 10-state S&P/Fitch scale.
    dpd_stage2_threshold : int
        Days past due at or above which the Stage 2 backstop fires (default 30).
    dpd_stage3_threshold : int
        Days past due at or above which Stage 3 is forced (default 90).
    qualitative_triggers_enabled : bool
        Whether any active SICR flag fires Stage 2. Default ``True``.
    stage3_qualitative_triggers_enabled : bool
        Whether default-evidence flags force Stage 3. Default ``True``.
    cure_periods_stage2_to_1 : int
        Consecutive performing periods required to cure Stage 2 to Stage 1.
        Default ``3``.
    cure_periods_stage3_to_2 : int
        Consecutive performing periods required to cure Stage 3 to Stage 2.
        Default ``12``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import StagingConfig
    >>> StagingConfig(pd_delta_absolute=0.02).pd_delta_absolute
    0.02
    """
    def __init__(
        self,
        *,
        pd_delta_absolute: float = 0.01,
        pd_delta_relative: float | None = 2.0,
        rating_downgrade_notches: int = 3,
        rating_scale_labels: list[str] | None = None,
        dpd_stage2_threshold: int = 30,
        dpd_stage3_threshold: int = 90,
        qualitative_triggers_enabled: bool = True,
        stage3_qualitative_triggers_enabled: bool = True,
        cure_periods_stage2_to_1: int = 3,
        cure_periods_stage3_to_2: int = 12,
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

def classify_stage(exposure: Exposure, config: StagingConfig | None = None) -> StageResult:
    """Classify an exposure into an IFRS 9 stage.

    Runs the full Rust staging waterfall: the Stage 3 days-past-due backstop,
    default-evidence flags, the absolute and relative PD-delta SICR tests
    (``current_pd`` versus ``origination_pd``), the rating-downgrade notch test,
    qualitative SICR flags, the Stage 2 days-past-due backstop and curing.
    Both supplied PDs must cover ``min(remaining_maturity_years, 30.0)`` years from
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
    >>> exp = Exposure(
    ...     id="loan",
    ...     ead=1_000_000.0,
    ...     lgd=0.45,
    ...     eir=0.06,
    ...     remaining_maturity_years=3.0,
    ...     current_pd=0.02,
    ...     origination_pd=0.015,
    ...     days_past_due=35,
    ... )
    >>> classify_stage(exp).stage.value
    'stage2'
    """
