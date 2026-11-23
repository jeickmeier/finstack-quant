"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/types/csa.rs``."""

from __future__ import annotations

from finstack_quant.core.currency import Currency
from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.config import FinstackConfig
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from finstack_quant.margin._types.enums import CollateralAssetClass, ImCollateralResult, ImMethodology, MarginTenor

class CsaSpec:
    """
    Credit Support Annex specification (ISDA standard).

    Build from a registry preset (``usd_regulatory``, ``eur_regulatory``,
    ``regulatory(currency, id, collateral_curve)``) then adjust legacy
    bilateral terms with ``with_vm_threshold`` / ``with_im``. Every commercial
    term is readable through typed getters; amounts are floats in
    ``base_currency``.

    Parameters
    ----------
    (Use regulatory factories, the ``with_*`` builders, or ``from_json``.)

    Returns
    -------
    CsaSpec
        CSA terms for margin calculation.

    Examples
    --------
    >>> CsaSpec.usd_regulatory().base_currency
    'USD'
    """

    @staticmethod
    def builder(id: str, base_currency: Currency | str, collateral_curve_id: str) -> CsaSpecBuilder:
        """Start authoring an agreement with an explicit reporting currency.

        Parameters
        ----------
        id : str
            Stable agreement identifier for trade and collateral-account joins.
        base_currency : Currency or str
            Contractual currency of margin balances, thresholds and transfers.
        collateral_curve_id : str
            Market identifier of the curve used to discount cash collateral.

        Returns
        -------
        CsaSpecBuilder
            Builder requiring explicit variation-margin, eligibility, call timing and calendar.

        Raises
        ------
        ValueError
            If the currency code is unrecognized.

        Examples
        --------
        >>> from finstack_quant.margin import CsaSpec
        >>> type(CsaSpec.builder("CSA-A", "USD", "USD-OIS")).__name__
        'CsaSpecBuilder'
        """
        ...

    def to_builder(self) -> CsaSpecBuilder:
        """Copy the agreement's complete terms into an independent builder.

        Returns
        -------
        CsaSpecBuilder
            Builder carrying the currency, market references and agreement terms.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def usd_regulatory() -> CsaSpec:
        """
        Standard regulatory CSA for USD derivatives.

        Returns
        -------
        CsaSpec
            USD regulatory CSA.

        Raises
        ------
        ValueError
            If the embedded margin registry cannot be loaded.

        Examples
        --------
        >>> csa = CsaSpec.usd_regulatory()
        >>> (csa.id, csa.base_currency)
        ('USD-REGULATORY-CSA', 'USD')
        """
        ...

    @staticmethod
    def eur_regulatory() -> CsaSpec:
        """
        Standard regulatory CSA for EUR derivatives.

        Returns
        -------
        CsaSpec
            EUR regulatory CSA.

        Raises
        ------
        ValueError
            If the embedded margin registry cannot be loaded.

        Examples
        --------
        >>> CsaSpec.eur_regulatory().base_currency
        'EUR'
        """
        ...

    @staticmethod
    def regulatory(currency: str, id: str, collateral_curve: str) -> CsaSpec:
        """
        Standard regulatory CSA for any currency (zero VM threshold, daily
        exchange, SIMM IM, BCBS-IOSCO collateral, the currency's default
        margin calendar).

        Parameters
        ----------
        currency : str
            ISO-4217 base currency for thresholds, MTA and collateral values.
        id : str
            CSA identifier used in margin lookups; must be non-empty.
        collateral_curve : str
            Discount-curve id for collateral valuation, typically the
            currency's OIS/RFR curve (e.g. ``"GBP-SONIA"``).

        Returns
        -------
        CsaSpec
            Regulatory CSA in ``currency``.

        Raises
        ------
        ValueError
            If ``currency`` is unknown, ``id`` is empty or whitespace-only,
            ``collateral_curve`` is empty, or the embedded registry cannot be
            loaded.

        Examples
        --------
        >>> csa = CsaSpec.regulatory("GBP", "GBP-CSA", "GBP-SONIA")
        >>> (csa.base_currency, csa.vm_threshold, csa.collateral_curve_id)
        ('GBP', 0.0, 'GBP-SONIA')
        """
        ...

    @staticmethod
    def regulatory_from_config(config: FinstackConfig, currency: str, id: str, collateral_curve: str) -> CsaSpec:
        """
        Standard regulatory CSA built from a config's margin-registry overlay.

        Same terms as ``regulatory``, except that a ``"margin.registry.v1"``
        extension in ``config`` takes precedence over the embedded margin
        registry (a deep object merge; see the margin crate README).

        Parameters
        ----------
        config : FinstackConfig
            Config whose ``"margin.registry.v1"`` extension (if any) overlays
            the embedded registry; without one the result equals
            ``CsaSpec.regulatory(currency, id, collateral_curve)``.
        currency : str
            ISO-4217 base currency for thresholds, MTA and collateral values.
        id : str
            CSA identifier used in margin lookups; must be non-empty.
        collateral_curve : str
            Discount-curve id for collateral valuation (e.g. ``"USD-OIS"``).

        Returns
        -------
        CsaSpec
            Validated regulatory CSA using the merged registry terms.

        Raises
        ------
        ValueError
            If ``currency`` is unknown, ``id`` is empty, or the overlay is
            malformed or fails registry validation.

        Examples
        --------
        >>> from finstack_quant.core.config import FinstackConfig
        >>> config = FinstackConfig()
        >>> config.set_extension("margin.registry.v1", {"defaults": {"defaults": {"vm": {"mta": 250_000.0}}}})
        >>> CsaSpec.regulatory_from_config(config, "USD", "CSA-1", "USD-OIS").vm_mta
        250000.0
        """
        ...

    def with_vm_threshold(
        self,
        threshold: float,
        mta: float,
        rounding: float | None = None,
        independent_amount: float | None = None,
    ) -> CsaSpec:
        """
        Return a copy with bilateral (legacy, non-zero) VM threshold terms.

        Parameters
        ----------
        threshold : float
            VM threshold in ``base_currency`` below which no margin is
            exchanged.
        mta : float
            Minimum transfer amount in ``base_currency``.
        rounding : float | None, optional
            Transfer rounding increment in ``base_currency``; ``None`` keeps
            the Rust default of 10,000.
        independent_amount : float | None, optional
            Independent amount in ``base_currency``; ``None`` keeps zero.

        Returns
        -------
        CsaSpec
            Copy with the VM terms replaced; frequency and settlement lag are
            unchanged.

        Raises
        ------
        ValueError
            If an amount is non-finite or outside the representable range.

        Examples
        --------
        >>> csa = CsaSpec.usd_regulatory().with_vm_threshold(300_000.0, 50_000.0)
        >>> (csa.vm_threshold, csa.vm_mta, csa.vm_rounding)
        (300000.0, 50000.0, 10000.0)
        """
        ...

    def apply_im_terms(self, gross_initial_margin: float, current_collateral: float) -> ImCollateralResult:
        """Apply the allocated CSA threshold and MTA once to its one-way IM account.

        Parameters
        ----------
        gross_initial_margin : float
            Nonnegative gross IM summed over all this CSA's netting sets in
            base_currency major units, already calculated for the elected MPOR.
        current_collateral : float
            Nonnegative existing IM balance in base_currency, excluding VM and
            the opposite party's separately collateralized IM account.

        Returns
        -------
        ImCollateralResult
            Gross IM, target balance, signed transfer and segregation requirement.
            MTA equality triggers a transfer; smaller absolute transfers are zero.

        Raises
        ------
        ValueError
            If amounts are negative or non-finite, CSA terms are invalid or IM is absent.
        """
        ...

    def with_im(
        self,
        methodology: ImMethodology | str,
        mpor_days: int,
        threshold: float,
        mta: float,
        segregated: bool | None = None,
    ) -> CsaSpec:
        """
        Return a copy with explicit initial-margin terms.

        Parameters
        ----------
        methodology : ImMethodology | str
            IM regime, as an ``ImMethodology`` or its lower-case wire label
            (``"simm"``, ``"schedule"``, ``"haircut"``, ``"internal_model"``,
            ``"clearing_house"``).
        mpor_days : int
            Margin period of risk in business days; must be positive.
        threshold : float
            IM threshold in ``base_currency``.
        mta : float
            IM minimum transfer amount in ``base_currency``.
        segregated : bool | None, default None
            Whether IM must be held with a third-party custodian. ``None``
            keeps the margin-registry default for ``methodology`` (``True``
            for SIMM and schedule IM, ``False`` for clearing-house and
            haircut IM).

        Returns
        -------
        CsaSpec
            Copy with ``requires_im`` true and the IM terms replaced.

        Raises
        ------
        ValueError
            If ``mpor_days`` is zero, an amount is non-finite, or the
            methodology label is unknown.

        Examples
        --------
        >>> csa = CsaSpec.usd_regulatory().with_im("schedule", 5, 1_000_000.0, 0.0)
        >>> (str(csa.im_methodology), csa.im_mpor_days, csa.im_threshold)
        ('schedule', 5, 1000000.0)
        >>> CsaSpec.usd_regulatory().with_im("clearing_house", 5, 0.0, 0.0).im_segregated
        False
        """
        ...

    @staticmethod
    def from_json(json: str) -> CsaSpec:
        """
        Deserialize from a JSON string.

        Parameters
        ----------
        json : str
            JSON representation.

        Returns
        -------
        CsaSpec
            Parsed CSA.

        Raises
        ------
        ValueError
            If JSON is invalid.

        Examples
        --------
        >>> csa = CsaSpec.from_json(CsaSpec.usd_regulatory().to_json())
        >>> csa.base_currency
        'USD'
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    def to_json(self) -> str:
        """
        Serialize to a JSON string.

        Returns
        -------
        str
            Pretty-printed JSON.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(CsaSpec.usd_regulatory().to_json(), str)
        True
        """
        ...

    @property
    def id(self) -> str:
        """
        Stable CSA identifier used in margin lookups.

        Returns
        -------
        str
            Identifier of this CSA specification.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> isinstance(CsaSpec.usd_regulatory().id, str)
        True
        """
        ...

    @property
    def base_currency(self) -> str:
        """
        ISO currency code in which CSA amounts are expressed.

        Returns
        -------
        str
            ISO currency code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().base_currency
        'USD'
        """
        ...

    @property
    def calendar_id(self) -> str:
        """
        Contractual business-day calendar identifier.

        Returns
        -------
        str
            Contractual business-day calendar identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def collateral_curve_id(self) -> str:
        """
        Discount-curve id used to value collateral.

        Returns
        -------
        str
            Curve identifier (e.g. ``'USD-OIS'``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().collateral_curve_id
        'USD-OIS'
        """
        ...

    @property
    def vm_threshold(self) -> float:
        """
        VM threshold in ``base_currency`` below which no margin is exchanged.

        Returns
        -------
        float
            Threshold amount (zero for regulatory CSAs).

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().vm_threshold
        0.0
        """
        ...

    @property
    def vm_mta(self) -> float:
        """
        VM minimum transfer amount in ``base_currency``.

        Returns
        -------
        float
            MTA amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().vm_mta >= 0.0
        True
        """
        ...

    @property
    def vm_rounding(self) -> float:
        """
        VM transfer rounding increment in ``base_currency``.

        Returns
        -------
        float
            Rounding increment.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().vm_rounding > 0.0
        True
        """
        ...

    @property
    def vm_independent_amount(self) -> float:
        """
        VM independent amount in ``base_currency``.

        Returns
        -------
        float
            Independent amount (zero for regulatory CSAs).

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().vm_independent_amount
        0.0
        """
        ...

    @property
    def vm_frequency(self) -> MarginTenor:
        """
        VM call frequency.

        Returns
        -------
        MarginTenor
            Call frequency (daily for regulatory CSAs).

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().vm_frequency
        MarginTenor(daily)
        """
        ...

    @property
    def vm_settlement_lag(self) -> int:
        """
        VM settlement lag in business days (T+n).

        Returns
        -------
        int
            Settlement lag in business days.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().vm_settlement_lag
        1
        """
        ...

    @property
    def im_methodology(self) -> ImMethodology | None:
        """
        IM methodology, or ``None`` when no IM is exchanged.

        Returns
        -------
        ImMethodology | None
            Methodology, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().im_methodology
        ImMethodology(simm)
        """
        ...

    @property
    def im_mpor_days(self) -> int | None:
        """
        IM margin period of risk in business days, or ``None`` without IM.

        Returns
        -------
        int | None
            MPOR in business days, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().im_mpor_days
        10
        """
        ...

    @property
    def im_threshold(self) -> float | None:
        """
        IM threshold in ``base_currency``, or ``None`` without IM.

        Returns
        -------
        float | None
            IM threshold, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().im_threshold is not None
        True
        """
        ...

    @property
    def im_mta(self) -> float | None:
        """
        IM minimum transfer amount in ``base_currency``, or ``None`` without IM.

        Returns
        -------
        float | None
            IM MTA, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().im_mta is not None
        True
        """
        ...

    @property
    def im_segregated(self) -> bool | None:
        """
        Whether IM must be segregated, or ``None`` without IM.

        Returns
        -------
        bool | None
            Segregation flag, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().im_segregated
        True
        """
        ...

    @property
    def eligible_collateral(self) -> EligibleCollateralSchedule:
        """
        Eligible-collateral schedule governing what can be posted.

        Returns
        -------
        EligibleCollateralSchedule
            Schedule with haircuts.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().eligible_collateral.eligible_count > 0
        True
        """
        ...

    @property
    def call_timing(self) -> dict[str, int]:
        """
        Margin-call timing terms.

        Returns
        -------
        dict[str, int]
            Keys ``notification_deadline_hours``, ``response_deadline_hours``, ``dispute_resolution_days``, ``delivery_grace_days``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> sorted(CsaSpec.usd_regulatory().call_timing)[:2]
        ['delivery_grace_days', 'dispute_resolution_days']
        """
        ...

    @property
    def requires_im(self) -> bool:
        """
        Whether this CSA requires initial margin.

        Returns
        -------
        bool
            True if IM required.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> isinstance(CsaSpec.usd_regulatory().requires_im, bool)
        True
        """
        ...

    def validate(self) -> None:
        """
        Validate CSA identifiers and the contractual holiday calendar.

        Runs the same Rust-side validation that ``from_json`` applies on
        ingest, so an already-constructed spec can be re-checked in place.

        Returns
        -------
        None
            Returns ``None`` when the spec is valid.

        Raises
        ------
        ValueError
            If an identifier is empty or the calendar id is unknown.

        Examples
        --------
        >>> from finstack_quant.margin import CsaSpec
        >>> CsaSpec.usd_regulatory().validate() is None
        True
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export the commercial terms as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``id``, ``base_currency``, ``calendar_id``,
        ``collateral_curve_id``, ``vm_threshold``, ``vm_mta``, ``vm_rounding``,
        ``vm_independent_amount``, ``vm_frequency``, ``vm_settlement_lag``,
        ``requires_im``, ``im_methodology``, ``im_mpor_days``, ``im_threshold``,
        ``im_mta``, ``im_segregated``. Amount columns are floats in
        ``base_currency``; the ``im_*`` columns are null when no IM applies.

        Returns
        -------
        ArrowTable
            One row describing the CSA.

        Raises
        ------
        ValueError
            If the spec cannot be serialized into an Arrow table.

        Examples
        --------
        >>> float(CsaSpec.usd_regulatory().to_arrow().to_dict()["vm_threshold"][0])
        0.0
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export the commercial terms as a single-row pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            One row describing the CSA.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.

        Examples
        --------
        >>> float(CsaSpec.usd_regulatory().to_pandas().iloc[0]["vm_threshold"])
        0.0
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export the commercial terms as a single-row Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            One row describing the CSA.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export the commercial terms as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row describing the CSA.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def __repr__(self) -> str: ...

class CsaSpecBuilder:
    """Author and validate the negotiated terms of a Credit Support Annex.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.margin import CsaSpec
    >>> base = CsaSpec.usd_regulatory()
    >>> agreement = base.to_builder().vm_threshold(Money(250_000, "USD"), Money(50_000, "USD")).build()
    >>> agreement.base_currency
    'USD'
    """

    def build(self) -> CsaSpec:
        """Consume the builder and validate the agreement.

        Returns
        -------
        CsaSpec
            Validated terms with explicit market references and margin currency.

        Raises
        ------
        ValueError
            If the builder was consumed, required terms are absent or a term violates the CSA contract.
        """
        ...

    def im_simm(self) -> CsaSpecBuilder:
        """Elect the registry's SIMM initial-margin terms in the agreement currency.

        Returns
        -------
        CsaSpecBuilder
            Active builder carrying the elected SIMM terms.

        Raises
        ------
        ValueError
            If the builder was consumed, the registry is unavailable or it has no eligible terms.
        """
        ...

    def vm_threshold(self, threshold: Money, mta: Money) -> CsaSpecBuilder:
        """Set the variation-margin allowance and transfer minimum.

        Parameters
        ----------
        threshold : Money
            Uncollateralized allowance in the CSA currency.
        mta : Money
            Minimum transfer in the same currency.

        Returns
        -------
        CsaSpecBuilder
            Active builder retaining previously elected VM rounding,
            independent amount, call frequency and settlement lag.

        Raises
        ------
        ValueError
            If the builder was consumed. Invalid money terms fail at build.
        """
        ...

    def calendar_id(self, value: str) -> CsaSpecBuilder:
        """Set the agreement's calendar id.

        Parameters
        ----------
        value : str
            Registered business-day calendar for calls and settlements.

        Returns
        -------
        CsaSpecBuilder
            Active builder carrying the specified terms.

        Raises
        ------
        ValueError
            If the supplied term is malformed or the builder was consumed.
        """
        ...

    def vm_params(self, value: dict[str, object]) -> CsaSpecBuilder:
        """Set the agreement's vm params.

        Parameters
        ----------
        value : dict[str, object]
            Threshold, minimum transfer, rounding and frequency under the canonical variation-margin contract.

        Returns
        -------
        CsaSpecBuilder
            Active builder carrying the specified terms.

        Raises
        ------
        ValueError
            If the supplied term is malformed or the builder was consumed.
        """
        ...

    def im_params(self, value: dict[str, object] | None) -> CsaSpecBuilder:
        """Set the agreement's im params.

        Parameters
        ----------
        value : dict[str, object] | None
            Method, MPOR, threshold, transfer minimum and segregation terms. None explicitly clears IM.

        Returns
        -------
        CsaSpecBuilder
            Active builder carrying the specified terms.

        Raises
        ------
        ValueError
            If the supplied term is malformed or the builder was consumed.
        """
        ...

    def eligible_collateral(self, value: EligibleCollateralSchedule) -> CsaSpecBuilder:
        """Set the agreement's eligible collateral.

        Parameters
        ----------
        value : EligibleCollateralSchedule
            Schedule of eligible instruments and applied collateral valuation haircuts.

        Returns
        -------
        CsaSpecBuilder
            Active builder carrying the specified terms.

        Raises
        ------
        ValueError
            If the supplied term is malformed or the builder was consumed.
        """
        ...

    def call_timing(self, value: dict[str, int]) -> CsaSpecBuilder:
        """Set the agreement's call timing.

        Parameters
        ----------
        value : dict[str, int]
            Hour and business-day fields for the canonical margin call and dispute windows.

        Returns
        -------
        CsaSpecBuilder
            Active builder carrying the specified terms.

        Raises
        ------
        ValueError
            If the supplied term is malformed or the builder was consumed.
        """
        ...

class EligibleCollateralSchedule:
    """
    Eligible collateral schedule with haircuts.

    Answers "what can I post and at what haircut": ``to_arrow`` / ``to_pandas``
    lists every eligible asset class with its haircut and constraints,
    ``haircut_for_maturity`` resolves the maturity-bucketed haircut for a
    bond, and ``check_concentration_limits`` flags a proposed collateral mix
    that breaches a concentration limit. Haircuts are decimal fractions
    (``0.02`` = 2%).

    Parameters
    ----------
    (Use factories or ``from_json``.)

    Returns
    -------
    EligibleCollateralSchedule
        Schedule of eligible assets and haircuts.

    Examples
    --------
    >>> EligibleCollateralSchedule.cash_only().eligible_count >= 1
    True
    """

    @staticmethod
    def cash_only() -> EligibleCollateralSchedule:
        """
        Eligible-collateral schedule that accepts cash only.

        Returns
        -------
        EligibleCollateralSchedule
            Schedule with cash only.

        Raises
        ------
        ValueError
            If the embedded margin registry cannot be loaded.

        Examples
        --------
        >>> schedule = EligibleCollateralSchedule.cash_only()
        >>> (schedule.eligible_count, schedule.rehypothecation_allowed)
        (1, False)
        """
        ...

    @staticmethod
    def bcbs_standard() -> EligibleCollateralSchedule:
        """
        Standard BCBS-IOSCO compliant schedule.

        Returns
        -------
        EligibleCollateralSchedule
            BCBS schedule.

        Raises
        ------
        ValueError
            If the embedded margin registry cannot be loaded.

        Examples
        --------
        >>> EligibleCollateralSchedule.bcbs_standard().eligible_count > 0
        True
        """
        ...

    @staticmethod
    def us_treasuries() -> EligibleCollateralSchedule:
        """
        US Treasuries repo schedule.

        Returns
        -------
        EligibleCollateralSchedule
            Treasury-focused schedule.

        Raises
        ------
        ValueError
            If the embedded margin registry cannot be loaded.

        Examples
        --------
        >>> EligibleCollateralSchedule.us_treasuries().eligible_count > 0
        True
        """
        ...

    @staticmethod
    def from_finstack_config(config: FinstackConfig, schedule_id: str) -> EligibleCollateralSchedule:
        """
        Load a named schedule from a config's margin-registry overlay.

        Parameters
        ----------
        config : FinstackConfig
            Config whose ``"margin.registry.v1"`` extension (if any) overlays
            the embedded margin registry; configured schedules take
            precedence.
        schedule_id : str
            Registry id of the schedule, e.g. ``"bcbs_standard"``,
            ``"cash_only"``, ``"us_treasuries"`` or an id the overlay adds.

        Returns
        -------
        EligibleCollateralSchedule
            A copy of the merged registry's schedule.

        Raises
        ------
        ValueError
            If the overlay is malformed or ``schedule_id`` names no schedule.

        Examples
        --------
        >>> from finstack_quant.core.config import FinstackConfig
        >>> schedule = EligibleCollateralSchedule.from_finstack_config(FinstackConfig(), "bcbs_standard")
        >>> schedule.eligible_count == EligibleCollateralSchedule.bcbs_standard().eligible_count
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> EligibleCollateralSchedule:
        """
        Parse this object from a JSON object or JSON string.

        Parameters
        ----------
        json : str
            JSON representation.

        Returns
        -------
        EligibleCollateralSchedule
            Parsed schedule.

        Raises
        ------
        ValueError
            If JSON is invalid.

        Examples
        --------
        >>> original = EligibleCollateralSchedule.cash_only()
        >>> restored = EligibleCollateralSchedule.from_json(original.to_json())
        >>> restored.eligible_count
        1
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    def to_json(self) -> str:
        """
        Serialize this object to a JSON-compatible dict.

        Returns
        -------
        str
            Pretty-printed JSON.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(EligibleCollateralSchedule.cash_only().to_json(), str)
        True
        """
        ...

    @property
    def rehypothecation_allowed(self) -> bool:
        """
        Whether rehypothecation is allowed.

        Returns
        -------
        bool
            Rehypothecation flag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> isinstance(EligibleCollateralSchedule.cash_only().rehypothecation_allowed, bool)
        True
        """
        ...

    @property
    def eligible_count(self) -> int:
        """
        Number of eligible collateral types.

        Returns
        -------
        int
            Count of eligible entries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> EligibleCollateralSchedule.cash_only().eligible_count >= 1
        True
        """
        ...

    @property
    def default_haircut(self) -> float | None:
        """
        Haircut (decimal) for collateral types not listed explicitly.

        Returns
        -------
        float | None
            Default haircut, or ``None`` when only listed types are accepted.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> EligibleCollateralSchedule.cash_only().default_haircut is None
        True
        """
        ...

    def is_eligible(self, asset_class: CollateralAssetClass | str) -> bool:
        """
        Check if an asset class is eligible.

        Parameters
        ----------
        asset_class : CollateralAssetClass | str
            Asset class to test, or its lower-case wire label.

        Returns
        -------
        bool
            True if eligible under this schedule.

        Raises
        ------
        ValueError
            If a string label is not a collateral asset class.

        Examples
        --------
        >>> s = EligibleCollateralSchedule.cash_only()
        >>> s.is_eligible(CollateralAssetClass.cash())
        True
        """
        ...

    def haircut_for(self, asset_class: CollateralAssetClass | str) -> float | None:
        """
        Get the haircut (decimal) for an asset class, ignoring maturity
        constraints.

        Parameters
        ----------
        asset_class : CollateralAssetClass | str
            Asset class or its lower-case wire label.

        Returns
        -------
        float or None
            First matching entry's haircut, else ``default_haircut``, else
            ``None``.

        Raises
        ------
        ValueError
            If a string label is not a collateral asset class.

        Examples
        --------
        >>> s = EligibleCollateralSchedule.cash_only()
        >>> s.haircut_for(CollateralAssetClass.cash()) is not None
        True
        """
        ...

    def haircut_for_maturity(self, asset_class: CollateralAssetClass | str, remaining_years: float) -> float | None:
        """
        Get the haircut (decimal) for an asset class at a remaining maturity.

        Parameters
        ----------
        asset_class : CollateralAssetClass | str
            Asset class or its lower-case wire label.
        remaining_years : float
            Remaining maturity in years, matched against each entry's
            maturity constraints.

        Returns
        -------
        float or None
            Haircut of the first entry whose constraints admit
            ``remaining_years``, else ``default_haircut``, else ``None``.

        Raises
        ------
        ValueError
            If a string label is not a collateral asset class.

        Examples
        --------
        >>> s = EligibleCollateralSchedule.bcbs_standard()
        >>> s.haircut_for_maturity("government_bonds", 0.5)
        0.005
        """
        ...

    def check_concentration_limits(self, allocations: list[tuple[CollateralAssetClass | str, float]]) -> ArrowTable:
        """
        Check a proposed collateral mix against the concentration limits.

        Parameters
        ----------
        allocations : list[tuple[CollateralAssetClass | str, float]]
            ``(asset_class, amount)`` pairs in one currency; amounts are
            converted to fractions of their total.

        Returns
        -------
        ArrowTable
            Columns ``asset_class``, ``fraction``, ``limit``, ``excess`` (all
            decimal fractions), one row per breached limit; an empty table
            means the mix is within limits.

        Raises
        ------
        ValueError
            If a string label is not a collateral asset class.

        Examples
        --------
        >>> s = EligibleCollateralSchedule.bcbs_standard()
        >>> len(s.check_concentration_limits([("cash", 100.0)])) == 0
        True
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export the eligibility rows as an :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``asset_class``, ``min_rating``, ``min_remaining_years``,
        ``max_remaining_years``, ``haircut``, ``fx_haircut_addon``,
        ``concentration_limit``. Haircuts and limits are decimal fractions;
        optional constraints are null when absent. One row per eligible
        entry, in schedule order (the order ``haircut_for`` searches).

        Returns
        -------
        ArrowTable
            One row per eligible collateral entry.

        Raises
        ------
        ValueError
            If the schedule cannot be serialized into an Arrow table.

        Examples
        --------
        >>> EligibleCollateralSchedule.cash_only().to_arrow().to_dict()["asset_class"]
        ['cash']
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export the eligibility rows as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            One row per eligible collateral entry.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.

        Examples
        --------
        >>> EligibleCollateralSchedule.cash_only().to_pandas()["asset_class"].tolist()
        ['cash']
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export the eligibility rows as a Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            One row per eligible collateral entry.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export the eligibility rows as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row per eligible collateral entry.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def __repr__(self) -> str: ...

class SimmCreditClassification:
    """Select the qualifying or non-qualifying native SIMM credit risk class.

    Examples
    --------
    >>> from finstack_quant.margin import SimmCreditClassification
    >>> SimmCreditClassification.qualifying("financial").to_dict()["risk_class"]
    'qualifying'
    """

    @staticmethod
    def qualifying(sector: str) -> SimmCreditClassification:
        """Classify a credit-qualifying position by its SIMM sector.

        Parameters
        ----------
        sector : str
            Native SIMM sector label, such as financial or sovereign.

        Returns
        -------
        SimmCreditClassification
            Qualifying classification with explicit sector.

        Raises
        ------
        ValueError
            If the native sector label is unrecognized.

        Examples
        --------
        >>> from finstack_quant.margin import SimmCreditClassification
        >>> SimmCreditClassification.qualifying("financial").to_dict()["sector"]
        'financial'
        """
        ...

    @staticmethod
    def non_qualifying() -> SimmCreditClassification:
        """Classify an applicable exposure as credit non-qualifying.

        Returns
        -------
        SimmCreditClassification
            Non-qualifying classification without a sector assignment.

        Notes
        -----
        This constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.margin import SimmCreditClassification
        >>> SimmCreditClassification.non_qualifying().to_dict()
        {'risk_class': 'non_qualifying'}
        """
        ...

    def to_dict(self) -> dict[str, str]:
        """Return the native risk-class and applicable sector labels.

        Returns
        -------
        dict[str, str]
            Independent wire mapping.

        Raises
        ------
        ValueError
            If the classification cannot be exported.
        """
        ...

class OtcMarginSpec:
    """Explicit clearing status and CSA terms for a marginable OTC trade.

    Examples
    --------
    >>> from finstack_quant.margin import CsaSpec, OtcMarginSpec
    >>> spec = OtcMarginSpec.bilateral_simm(CsaSpec.usd_regulatory())
    >>> spec.to_dict()["csa"]["base_currency"]
    'USD'
    """

    @staticmethod
    def bilateral_simm(csa: CsaSpec) -> OtcMarginSpec:
        """Use ISDA SIMM for the supplied bilateral CSA.

        Parameters
        ----------
        csa : CsaSpec
            Existing agreement. Its collateral, VM and timing terms are copied.
            An agreement with no IM retains that explicit election.

        Returns
        -------
        OtcMarginSpec
            Bilateral terms with the elected initial margin convention.

        Notes
        -----
        This constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.margin import CsaSpec, OtcMarginSpec
        >>> OtcMarginSpec.bilateral_simm(CsaSpec.usd_regulatory()).to_dict()["csa"]["base_currency"]
        'USD'
        """
        ...

    @staticmethod
    def bilateral_schedule(csa: CsaSpec) -> OtcMarginSpec:
        """Use schedule-based IM for the supplied bilateral CSA.

        Parameters
        ----------
        csa : CsaSpec
            Existing agreement. Its collateral, VM and timing terms are copied.
            An agreement with no IM retains that explicit election.

        Returns
        -------
        OtcMarginSpec
            Bilateral terms with the elected initial margin convention.

        Notes
        -----
        This constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.margin import CsaSpec, OtcMarginSpec
        >>> OtcMarginSpec.bilateral_schedule(CsaSpec.usd_regulatory()).to_dict()["csa"]["base_currency"]
        'USD'
        """
        ...

    @staticmethod
    def cleared(ccp: str, currency: str) -> OtcMarginSpec:
        """Apply the native cleared-margin defaults for an identified CCP.

        Parameters
        ----------
        ccp : str
            Clearinghouse identifier, such as LCH.
        currency : str
            ISO-4217 settlement and collateral reporting currency.

        Returns
        -------
        OtcMarginSpec
            Cleared terms with the embedded collateral schedule and timing.

        Raises
        ------
        ValueError
            If the currency is invalid or the native registry cannot be resolved.

        Examples
        --------
        >>> from finstack_quant.margin import OtcMarginSpec
        >>> OtcMarginSpec.cleared("LCH", "USD").to_dict()["csa"]["base_currency"]
        'USD'
        """
        ...

    def with_simm_credit_classification(self, classification: SimmCreditClassification) -> OtcMarginSpec:
        """Copy the trade's terms with a selected SIMM credit classification.

        Parameters
        ----------
        classification : SimmCreditClassification
            Explicit qualifying sector or non-qualifying election for the trade.

        Returns
        -------
        OtcMarginSpec
            Independent copy with the elected credit class.

        Notes
        -----
        This method does not raise.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """Export the native margin terms for an instrument specification.

        Returns
        -------
        dict[str, Any]
            Independent copy of CSA, clearing status and any credit class.

        Raises
        ------
        ValueError
            If the terms cannot be exported.
        """
        ...
