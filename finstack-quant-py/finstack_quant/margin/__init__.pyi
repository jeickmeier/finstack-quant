"""
Margin and collateral: VM/IM calculators, CSA specifications, XVA, metrics.

This module exposes variation and initial margin types, netting-set identifiers,
credit support annex (CSA) specs, eligible collateral schedules, XVA configuration
and results, and margin analytics helpers.

Examples
--------
>>> from finstack_quant.margin import CollateralAssetClass
>>> CollateralAssetClass.cash().standard_haircut()
0.0
"""

from __future__ import annotations

import datetime
from typing import Any, Final

import pandas as pd

from finstack_quant.core.market_data import DiscountCurve, HazardCurve
from finstack_quant.margin import schema as schema

__all__ = [
    "ImMethodology",
    "MarginTenor",
    "MarginCallType",
    "ClearingStatus",
    "CollateralAssetClass",
    "NettingSetId",
    "CsaSpec",
    "EligibleCollateralSchedule",
    "CONSTANTS",
    "VmResult",
    "VmCalculator",
    "ImResult",
    "SimmSensitivities",
    "SimmCalculator",
    "ScheduleImCalculator",
    "HaircutImCalculator",
    "FundingConfig",
    "ExposureDiagnostics",
    "ExposureProfile",
    "XvaResult",
    "ImDecayProfile",
    "ImProfile",
    "MvaResult",
    "compute_bilateral_xva",
    "compute_mva",
    "im_profile_from_simm",
    "MarginUtilization",
    "ExcessCollateral",
    "MarginFundingCost",
    "Haircut01",
    "FrtbSbaResult",
    "EadResult",
    "FrtbSensitivities",
    "FrtbSbaEngine",
    "SaCcrTrade",
    "SaCcrNettingSetConfig",
    "SaCcrEngine",
    "frtb_sba_charge",
    "saccr_ead",
    "schema",
]

# Rust ``finstack_quant_margin::constants`` plus the registry/calculator
# constants needed to interpret results. Keys: ``CALENDAR_DAYS_PER_YEAR``,
# ``DURATION_APPROXIMATION_FACTOR``, ``ONE_BP``, ``STANDARD_CDS_MATURITY_YEARS``,
# ``DEFAULT_BOND_INDEX_DURATION`` (floats), ``tenor_buckets`` (dict of
# ``BUCKET_3M`` .. ``BUCKET_20Y`` -> years), ``BCBS_IOSCO_SCHEDULE_ID`` (str),
# ``HAIRCUT_MPOR_DAYS`` (business days), ``SIMM_TENORS`` (the valid SIMM tenor
# labels, ``"2W"`` .. ``"30Y"``) and ``SIMM_COMMODITY_BUCKET_COUNT`` (17).
CONSTANTS: Final[dict[str, Any]] = ...

class ImMethodology:
    """
    Initial margin calculation methodology.

    Immutable, hashable enum-style type. Constructed via class methods;
    not directly instantiated.

    Examples
    --------
    >>> ImMethodology.from_str("simm")
    ImMethodology(simm)
    """

    @staticmethod
    def haircut() -> ImMethodology:
        """
        Haircut-based IM (repos and securities financing).

        Returns
        -------
        ImMethodology
            Haircut methodology.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> ImMethodology.haircut()
        ImMethodology(haircut)
        """
        ...

    @staticmethod
    def simm() -> ImMethodology:
        """
        ISDA SIMM (sensitivities-based, OTC derivatives).

        Returns
        -------
        ImMethodology
            SIMM methodology.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> ImMethodology.simm()
        ImMethodology(simm)
        """
        ...

    @staticmethod
    def schedule() -> ImMethodology:
        """
        BCBS-IOSCO regulatory schedule approach.

        Returns
        -------
        ImMethodology
            Schedule methodology.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> ImMethodology.schedule()
        ImMethodology(schedule)
        """
        ...

    @staticmethod
    def internal_model() -> ImMethodology:
        """
        Internal model approved by regulator.

        Returns
        -------
        ImMethodology
            Internal model methodology.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> ImMethodology.internal_model()
        ImMethodology(internal_model)
        """
        ...

    @staticmethod
    def clearing_house() -> ImMethodology:
        """
        Clearing house (CCP-specific) methodology.

        Returns
        -------
        ImMethodology
            CCP methodology.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> ImMethodology.clearing_house()
        ImMethodology(clearing_house)
        """
        ...

    @staticmethod
    def from_str(s: str) -> ImMethodology:
        """
        Parse the lower-case wire label (e.g. ``"simm"``, ``"schedule"``).

        Parameters
        ----------
        s : str
            Lower-case wire label: ``"simm"``, ``"schedule"``, ``"haircut"``,
            ``"internal_model"`` or ``"clearing_house"``. Other spellings
            such as ``"SIMM"`` are rejected.

        Returns
        -------
        ImMethodology
            Parsed methodology.

        Raises
        ------
        ValueError
            If the string is not recognized.

        Examples
        --------
        >>> ImMethodology.from_str("schedule")
        ImMethodology(schedule)
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class MarginTenor:
    """
    How often variation margin is exchanged under the CSA.

    Parameters
    ----------
    (Constructed via class methods; not directly instantiated.)

    Returns
    -------
    MarginTenor
        Tenor for margin calls.

    Examples
    --------
    >>> MarginTenor.daily()
    MarginTenor(daily)
    """

    @staticmethod
    def daily() -> MarginTenor:
        """
        Daily margin calls (standard for OTC derivatives post-2016).

        Returns
        -------
        MarginTenor
            Daily tenor.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> str(MarginTenor.daily())
        'daily'
        """
        ...

    @staticmethod
    def weekly() -> MarginTenor:
        """
        Weekly variation-margin call frequency.

        Returns
        -------
        MarginTenor
            Weekly tenor.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> MarginTenor.weekly()
        MarginTenor(weekly)
        """
        ...

    @staticmethod
    def monthly() -> MarginTenor:
        """
        Monthly variation-margin call frequency.

        Returns
        -------
        MarginTenor
            Monthly tenor.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> MarginTenor.monthly()
        MarginTenor(monthly)
        """
        ...

    @staticmethod
    def on_demand() -> MarginTenor:
        """
        Margin calls issued when a threshold breach is observed, not on a calendar.

        Returns
        -------
        MarginTenor
            On-demand tenor.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> MarginTenor.on_demand()
        MarginTenor(on_demand)
        """
        ...

    @staticmethod
    def from_str(s: str) -> MarginTenor:
        """
        Parse this variant from its canonical name string.

        Parameters
        ----------
        s : str
            Lower-case wire label: ``"daily"``, ``"weekly"``, ``"monthly"``
            or ``"on_demand"``. Other spellings are rejected.

        Returns
        -------
        MarginTenor
            Parsed tenor.

        Raises
        ------
        ValueError
            If the string is not recognized.

        Examples
        --------
        >>> MarginTenor.from_str("daily")
        MarginTenor(daily)
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class MarginCallType:
    """
    Kind of margin call (initial margin, VM post/collect, top-up, substitution).

    This is the ``call_type`` column of ``VmCalculator.generate_margin_calls``.
    ``from_str`` parses the lower-case wire label and ``str()`` renders it.

    Parameters
    ----------
    (Constructed via class methods.)

    Returns
    -------
    MarginCallType
        Kind of margin call.

    Examples
    --------
    >>> MarginCallType.initial_margin()
    MarginCallType(initial_margin)
    """

    @staticmethod
    def initial_margin() -> MarginCallType:
        """
        Initial margin posting requirement.

        Returns
        -------
        MarginCallType
            Initial margin call type.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> MarginCallType.initial_margin()
        MarginCallType(initial_margin)
        """
        ...

    @staticmethod
    def variation_margin_post() -> MarginCallType:
        """
        Variation margin paid by the desk, including collateral returned.

        Returns
        -------
        MarginCallType
            VM payment type.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> MarginCallType.variation_margin_post()
        MarginCallType(variation_margin_post)
        """
        ...

    @staticmethod
    def variation_margin_collect() -> MarginCallType:
        """
        Variation margin received by the desk, including collateral returned to it.

        Returns
        -------
        MarginCallType
            VM collection type.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> MarginCallType.variation_margin_collect()
        MarginCallType(variation_margin_collect)
        """
        ...

    @staticmethod
    def top_up() -> MarginCallType:
        """
        Margin call that posts additional collateral to restore the threshold.

        Returns
        -------
        MarginCallType
            Top-up type.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> MarginCallType.top_up()
        MarginCallType(top_up)
        """
        ...

    @staticmethod
    def substitution() -> MarginCallType:
        """
        Collateral substitution request.

        Returns
        -------
        MarginCallType
            Substitution type.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> MarginCallType.substitution()
        MarginCallType(substitution)
        """
        ...

    @staticmethod
    def from_str(s: str) -> MarginCallType:
        """
        Parse the lower-case wire label.

        Parameters
        ----------
        s : str
            ``"initial_margin"``, ``"variation_margin_post"``,
            ``"variation_margin_collect"``, ``"top_up"`` or
            ``"substitution"``. Other spellings are rejected.

        Returns
        -------
        MarginCallType
            Parsed call type.

        Raises
        ------
        ValueError
            If the string is not one of the wire labels.

        Examples
        --------
        >>> MarginCallType.from_str("top_up")
        MarginCallType(top_up)
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class ClearingStatus:
    """
    Clearing status for OTC derivatives.

    Parameters
    ----------
    (Use ``bilateral()`` or ``cleared()``.)

    Returns
    -------
    ClearingStatus
        Bilateral or cleared status.

    Examples
    --------
    >>> ClearingStatus.cleared("LCH").is_cleared
    True
    """

    @staticmethod
    def bilateral() -> ClearingStatus:
        """
        Bilateral (uncleared) trade governed by CSA.

        Returns
        -------
        ClearingStatus
            Bilateral status.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> ClearingStatus.bilateral().is_bilateral
        True
        """
        ...

    @staticmethod
    def cleared(ccp: str) -> ClearingStatus:
        """
        Trade cleared through a CCP.

        Parameters
        ----------
        ccp : str
            Clearing house identifier.

        Returns
        -------
        ClearingStatus
            Cleared status with CCP id.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> ClearingStatus.cleared("LCH").is_cleared
        True
        """
        ...

    @property
    def is_bilateral(self) -> bool:
        """
        Whether this is a bilateral trade.

        Returns
        -------
        bool
            True if bilateral.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ClearingStatus.bilateral().is_bilateral
        True
        """
        ...

    @property
    def is_cleared(self) -> bool:
        """
        Whether this is a cleared trade.

        Returns
        -------
        bool
            True if cleared.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ClearingStatus.cleared("CCP").is_cleared
        True
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class CollateralAssetClass:
    """
    Collateral asset class per BCBS-IOSCO standards.

    Parameters
    ----------
    (Use class factories or ``from_str``.)

    Returns
    -------
    CollateralAssetClass
        Asset class for haircuts and eligibility.

    Examples
    --------
    >>> CollateralAssetClass.cash().standard_haircut()
    0.0
    """

    @staticmethod
    def cash() -> CollateralAssetClass:
        """
        Eligible-collateral class for cash balances.

        Returns
        -------
        CollateralAssetClass
            Cash.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> CollateralAssetClass.cash()
        CollateralAssetClass(cash)
        """
        ...

    @staticmethod
    def government_bonds() -> CollateralAssetClass:
        """
        Eligible-collateral class for government bonds.

        Returns
        -------
        CollateralAssetClass
            Government bonds.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> CollateralAssetClass.government_bonds()
        CollateralAssetClass(government_bonds)
        """
        ...

    @staticmethod
    def agency_bonds() -> CollateralAssetClass:
        """
        Eligible-collateral class for agency (GSE) bonds.

        Returns
        -------
        CollateralAssetClass
            Agency bonds.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> CollateralAssetClass.agency_bonds()
        CollateralAssetClass(agency_bonds)
        """
        ...

    @staticmethod
    def covered_bonds() -> CollateralAssetClass:
        """
        Eligible-collateral class for covered bonds.

        Returns
        -------
        CollateralAssetClass
            Covered bonds.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> CollateralAssetClass.covered_bonds()
        CollateralAssetClass(covered_bonds)
        """
        ...

    @staticmethod
    def corporate_bonds() -> CollateralAssetClass:
        """
        Eligible-collateral class for corporate bonds.

        Returns
        -------
        CollateralAssetClass
            Corporate bonds.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> CollateralAssetClass.corporate_bonds()
        CollateralAssetClass(corporate_bonds)
        """
        ...

    @staticmethod
    def equity() -> CollateralAssetClass:
        """
        Eligible-collateral class for listed equity.

        Returns
        -------
        CollateralAssetClass
            Equity.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> CollateralAssetClass.equity()
        CollateralAssetClass(equity)
        """
        ...

    @staticmethod
    def gold() -> CollateralAssetClass:
        """
        Eligible-collateral class for allocated gold.

        Returns
        -------
        CollateralAssetClass
            Gold.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> CollateralAssetClass.gold()
        CollateralAssetClass(gold)
        """
        ...

    @staticmethod
    def mutual_funds() -> CollateralAssetClass:
        """
        Eligible-collateral class for mutual-fund shares.

        Returns
        -------
        CollateralAssetClass
            Mutual funds.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> CollateralAssetClass.mutual_funds()
        CollateralAssetClass(mutual_funds)
        """
        ...

    @staticmethod
    def from_str(s: str) -> CollateralAssetClass:
        """
        Parse this variant from its canonical name string.

        Parameters
        ----------
        s : str
            Lower-case wire label: ``"cash"``, ``"government_bonds"``,
            ``"agency_bonds"``, ``"covered_bonds"``, ``"corporate_bonds"``,
            ``"equity"``, ``"gold"`` or ``"mutual_funds"``.

        Returns
        -------
        CollateralAssetClass
            Parsed class.

        Raises
        ------
        ValueError
            If not recognized.

        Examples
        --------
        >>> CollateralAssetClass.from_str("cash")
        CollateralAssetClass(cash)
        """
        ...

    def standard_haircut(self) -> float:
        """
        BCBS-IOSCO standard haircut for this asset class.

        Returns
        -------
        float
            Haircut as decimal.

        Raises
        ------
        ValueError
            If the embedded registry has no entry for this asset class.

        Examples
        --------
        >>> CollateralAssetClass.cash().standard_haircut()
        0.0
        """
        ...

    def fx_addon(self) -> float:
        """
        FX haircut add-on for currency mismatch.

        Returns
        -------
        float
            Add-on as decimal.

        Raises
        ------
        ValueError
            If the embedded registry has no entry for this asset class.

        Examples
        --------
        >>> isinstance(CollateralAssetClass.cash().fx_addon(), float)
        True
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class NettingSetId:
    """
    Identifies a margin netting set.

    Immutable, hashable and comparable, so an id can key a dict or group a
    DataFrame; ``to_json`` / ``from_json`` and pickle round-trip the wire form.

    Parameters
    ----------
    (Use ``bilateral`` or ``cleared`` factories.)

    Returns
    -------
    NettingSetId
        Netting set key.

    Examples
    --------
    >>> NettingSetId.bilateral("CPTY", "CSA1").counterparty_id
    'CPTY'
    """

    @staticmethod
    def bilateral(counterparty_id: str, csa_id: str) -> NettingSetId:
        """
        Create a bilateral netting set.

        Parameters
        ----------
        counterparty_id : str
            Counterparty identifier.
        csa_id : str
            CSA agreement identifier.

        Returns
        -------
        NettingSetId
            Bilateral netting set id.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> NettingSetId.bilateral("A", "CSA").is_cleared
        False
        """
        ...

    @staticmethod
    def cleared(ccp_id: str) -> NettingSetId:
        """
        Create a cleared netting set.

        Parameters
        ----------
        ccp_id : str
            Central counterparty identifier.

        Returns
        -------
        NettingSetId
            Cleared netting set id.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> NettingSetId.cleared("LCH").is_cleared
        True
        """
        ...

    @property
    def is_cleared(self) -> bool:
        """
        Whether this is a cleared netting set.

        Returns
        -------
        bool
            True if cleared.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> NettingSetId.cleared("CCP").is_cleared
        True
        """
        ...

    @property
    def counterparty_id(self) -> str:
        """
        Counterparty identifier. For cleared netting sets this returns
        the CCP id; for bilateral, the explicit counterparty id.

        Returns
        -------
        str
            Counterparty id string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> NettingSetId.bilateral("X", "Y").counterparty_id
        'X'
        >>> NettingSetId.cleared("LCH").counterparty_id
        'LCH'
        """
        ...

    @property
    def csa_id(self) -> str | None:
        """
        CSA identifier when bilateral; ``None`` for cleared sets.

        Returns
        -------
        str or None
            CSA id string, or ``None`` for cleared netting sets.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> NettingSetId.bilateral("X", "CSA-001").csa_id
        'CSA-001'
        >>> NettingSetId.cleared("LCH").csa_id is None
        True
        """
        ...

    @property
    def ccp_id(self) -> str | None:
        """
        CCP identifier when cleared; ``None`` for bilateral sets.

        Returns
        -------
        str or None
            CCP id string, or ``None`` for bilateral netting sets.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> NettingSetId.cleared("LCH").ccp_id
        'LCH'
        >>> NettingSetId.bilateral("X", "CSA").ccp_id is None
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> NettingSetId:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical netting-set-id JSON.

        Returns
        -------
        NettingSetId
            Parsed id.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> lch = NettingSetId.cleared("LCH")
        >>> NettingSetId.from_json(lch.to_json()) == lch
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            JSON string accepted by ``from_json``.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(NettingSetId.cleared("LCH").to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

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
            If ``currency`` is unknown or the embedded registry cannot be
            loaded.

        Examples
        --------
        >>> csa = CsaSpec.regulatory("GBP", "GBP-CSA", "GBP-SONIA")
        >>> (csa.base_currency, csa.vm_threshold, csa.collateral_curve_id)
        ('GBP', 0.0, 'GBP-SONIA')
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

    def with_im(
        self,
        methodology: ImMethodology | str,
        mpor_days: int,
        threshold: float,
        mta: float,
        segregated: bool = True,
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
        segregated : bool, default True
            Whether IM must be held with a third-party custodian.

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

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the commercial terms as a single-row pandas ``DataFrame``.

        Columns: ``id``, ``base_currency``, ``calendar_id``,
        ``collateral_curve_id``, ``vm_threshold``, ``vm_mta``, ``vm_rounding``,
        ``vm_independent_amount``, ``vm_frequency``, ``vm_settlement_lag``,
        ``requires_im``, ``im_methodology``, ``im_mpor_days``, ``im_threshold``,
        ``im_mta``, ``im_segregated``. Amount columns are floats in
        ``base_currency``; the ``im_*`` columns are null when no IM applies.

        Returns
        -------
        pd.DataFrame
            One row describing the CSA.

        Raises
        ------
        ValueError
            If the spec cannot be serialized into a pandas object.

        Examples
        --------
        >>> float(CsaSpec.usd_regulatory().to_dataframe().iloc[0]["vm_threshold"])
        0.0
        """
        ...

    def __repr__(self) -> str: ...

class EligibleCollateralSchedule:
    """
    Eligible collateral schedule with haircuts.

    Answers "what can I post and at what haircut": ``to_dataframe`` lists
    every eligible asset class with its haircut and constraints,
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

    def check_concentration_limits(self, allocations: list[tuple[CollateralAssetClass | str, float]]) -> pd.DataFrame:
        """
        Check a proposed collateral mix against the concentration limits.

        Parameters
        ----------
        allocations : list[tuple[CollateralAssetClass | str, float]]
            ``(asset_class, amount)`` pairs in one currency; amounts are
            converted to fractions of their total.

        Returns
        -------
        pd.DataFrame
            Columns ``asset_class``, ``fraction``, ``limit``, ``excess`` (all
            decimal fractions), one row per breached limit; an empty frame
            means the mix is within limits.

        Raises
        ------
        ValueError
            If a string label is not a collateral asset class.

        Examples
        --------
        >>> s = EligibleCollateralSchedule.bcbs_standard()
        >>> s.check_concentration_limits([("cash", 100.0)]).empty
        True
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the eligibility rows as a pandas ``DataFrame``.

        Columns: ``asset_class``, ``min_rating``, ``min_remaining_years``,
        ``max_remaining_years``, ``haircut``, ``fx_haircut_addon``,
        ``concentration_limit``. Haircuts and limits are decimal fractions;
        optional constraints are null when absent. One row per eligible
        entry, in schedule order (the order ``haircut_for`` searches).

        Returns
        -------
        pd.DataFrame
            One row per eligible collateral entry.

        Raises
        ------
        ValueError
            If the schedule cannot be serialized into a pandas object.

        Examples
        --------
        >>> EligibleCollateralSchedule.cash_only().to_dataframe()["asset_class"].tolist()
        ['cash']
        """
        ...

    def __repr__(self) -> str: ...

class VmResult:
    """
    Variation margin calculation result.

    Sign convention: ``gross_exposure`` is the signed mark-to-market from our
    side (positive = the counterparty owes us). ``post_amount`` is what we
    post and ``collect_amount`` what we receive back; at most one is non-zero.
    Amounts are floats in the CSA base currency (``currency``).

    Parameters
    ----------
    (Returned by ``VmCalculator.calculate``; also loadable via ``from_json``.)

    Returns
    -------
    VmResult
        VM amounts and call flag.

    Examples
    --------
    >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
    >>> isinstance(r.net_margin, float)
    True
    """

    @staticmethod
    def from_json(json: str) -> VmResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``VmResult``.

        Returns
        -------
        VmResult
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> VmResult.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15").to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def date(self) -> datetime.date:
        """
        Calculation date.

        Returns
        -------
        datetime.date
            The ``as_of`` passed to ``VmCalculator.calculate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-17")
        >>> r.date
        datetime.date(2024, 6, 17)
        """
        ...

    @property
    def settlement_date(self) -> datetime.date:
        """
        Settlement date of the margin transfer: the calculation date plus the
        CSA settlement lag, adjusted on the CSA calendar.

        Returns
        -------
        datetime.date
            Settlement date, strictly after ``date`` for a T+1 CSA.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-17")
        >>> r.settlement_date
        datetime.date(2024, 6, 18)
        """
        ...

    @property
    def currency(self) -> str:
        """
        CSA base currency of every amount.

        Returns
        -------
        str
            ISO-4217 code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15").currency
        'USD'
        """
        ...

    @property
    def gross_exposure(self) -> float:
        """
        Gross mark-to-market exposure (positive = counterparty owes us).

        Returns
        -------
        float
            Gross exposure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.gross_exposure >= 0
        True
        """
        ...

    @property
    def net_exposure(self) -> float:
        """
        Net exposure after threshold and independent amount.

        Returns
        -------
        float
            Net exposure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> isinstance(r.net_exposure, float)
        True
        """
        ...

    @property
    def post_amount(self) -> float:
        """
        Amount paid by the desk, including new postings and collateral returns.

        Returns
        -------
        float
            Delivery amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.post_amount >= 0
        True
        """
        ...

    @property
    def collect_amount(self) -> float:
        """
        Amount received by the desk, including new collections and collateral returns.

        Returns
        -------
        float
            Return amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.collect_amount >= 0
        True
        """
        ...

    @property
    def net_margin(self) -> float:
        """
        Net desk cash outflow (post minus collect).

        Returns
        -------
        float
            Net margin.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> isinstance(r.net_margin, float)
        True
        """
        ...

    @property
    def requires_call(self) -> bool:
        """
        Whether a margin call is required.

        Returns
        -------
        bool
            Call required flag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> isinstance(r.requires_call, bool)
        True
        """
        ...

    def __repr__(self) -> str: ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the result as a single-row pandas ``DataFrame``.

        Columns: ``date``, ``settlement_date`` (ISO 8601 strings),
        ``gross_exposure``, ``net_exposure``, ``post_amount``,
        ``collect_amount``, ``net_margin``, ``requires_call``, ``currency``.

        All amount columns are floats in the single CSA currency reported by
        ``currency``; positive ``post_amount`` means we post margin and
        positive ``collect_amount`` means cash received by the desk.

        Returns
        -------
        pd.DataFrame
            One row describing the variation-margin result.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class VmCalculator:
    """
    Variation margin calculator following ISDA CSA rules.

    Parameters
    ----------
    csa : CsaSpec
        Credit Support Annex specification.

    Returns
    -------
    VmCalculator
        Calculator bound to ``csa``.

    Examples
    --------
    >>> calc = VmCalculator(CsaSpec.usd_regulatory())
    >>> out = calc.calculate(1e6, 0.0, "USD", "2024-06-15")
    >>> isinstance(out, VmResult)
    True
    """

    def __init__(self, csa: CsaSpec) -> None:
        """
        Bind a variation-margin calculator to one CSA specification.

        Parameters
        ----------
        csa : CsaSpec
            Thresholds, transfer minimums, rounding rules, eligible
            currencies, and calendar terms applied to each margin call.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def csa(self) -> CsaSpec:
        """
        CSA specification this calculator applies.

        Returns
        -------
        CsaSpec
            The spec passed to the constructor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> VmCalculator(CsaSpec.usd_regulatory()).csa.base_currency
        'USD'
        """
        ...

    def calculate(
        self,
        exposure: float,
        posted_collateral: float,
        currency: str,
        as_of: datetime.date | str,
    ) -> VmResult:
        """
        Calculate variation margin.

        Parameters
        ----------
        exposure : float
            Signed mark-to-market in ``currency``: positive means the
            counterparty owes us, negative means we owe them.
        posted_collateral : float
            Signed collateral balance in ``currency``: positive held, negative
            posted, including pending agreed calls.
        currency : str
            ISO currency code; must equal the CSA base currency.
        as_of : datetime.date | str
            Calculation date as ``datetime.date``, ``datetime.datetime``,
            ``pandas.Timestamp`` or ISO ``YYYY-MM-DD`` string; the
            settlement date is derived from it on the CSA calendar.

        Returns
        -------
        VmResult
            VM breakdown with delivery/return amounts and settlement date.

        Raises
        ------
        ValueError
            If the currency is unknown or differs from the CSA base
            currency, an amount is non-finite, or a date string is not ISO
            8601.
        TypeError
            If ``as_of`` is neither a string nor date-like.

        Examples
        --------
        >>> VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-17")
        VmResult(date=2024-06-17, post=0.00, collect=1000000.00, requires_call=True, settlement_date=2024-06-18)
        """
        ...

    def generate_margin_calls(
        self,
        exposures: list[tuple[datetime.date | str, float]] | pd.Series,
        initial_collateral: float,
    ) -> pd.DataFrame:
        """
        Run an exposure time series into a margin-call schedule.

        Parameters
        ----------
        exposures : list[tuple[datetime.date | str, float]] | pd.Series
            Dated signed exposures in the CSA base currency (positive = the
            counterparty owes us), processed in the order given. A ``Series``
            contributes its index as the dates.
        initial_collateral : float
            Collateral posted before the first date, in the CSA base currency.

        Returns
        -------
        pd.DataFrame
            One row per call: ``call_date``, ``settlement_date`` (ISO 8601
            strings), ``call_type`` (``"variation_margin_post"`` or
            ``"variation_margin_collect"``), ``amount``, ``mtm_trigger``,
            ``threshold``, ``mta_applied`` (floats in ``currency``) and
            ``currency``. Dates without a call produce no row.

        Raises
        ------
        ValueError
            If an amount is non-finite or a date string is not ISO 8601.

        Examples
        --------
        >>> calc = VmCalculator(CsaSpec.usd_regulatory())
        >>> calls = calc.generate_margin_calls([("2024-06-17", 1e6), ("2024-06-18", 4e5)], 0.0)
        >>> calls["call_type"].tolist()
        ['variation_margin_collect', 'variation_margin_post']
        """
        ...

    def margin_call_dates(self, start: datetime.date | str, end: datetime.date | str) -> list[datetime.date]:
        """
        Contractual margin-call dates between ``start`` and ``end``.

        Follows the CSA VM frequency on the CSA calendar: daily lists every
        business day, weekly/monthly roll from ``start`` with each date
        adjusted forward, on-demand returns just the adjusted endpoints.

        Parameters
        ----------
        start : datetime.date | str
            First date of the window (inclusive).
        end : datetime.date | str
            Last date of the window (inclusive).

        Returns
        -------
        list[datetime.date]
            Call dates in ascending order.

        Raises
        ------
        ValueError
            If a date string is not ISO 8601 or the CSA calendar is not
            registered.

        Examples
        --------
        >>> calc = VmCalculator(CsaSpec.usd_regulatory())
        >>> calc.margin_call_dates("2024-06-14", "2024-06-17")
        [datetime.date(2024, 6, 14), datetime.date(2024, 6, 17)]
        """
        ...

    def __repr__(self) -> str: ...

class ImResult:
    """
    Initial margin calculation result.

    ``amount`` is a float in ``currency``. ``breakdown_keys`` are
    methodology-specific component labels: SIMM publishes ``IR_Delta``,
    ``IR_Vega``, ``Credit_Qualifying_Delta``, ``Credit_Qualifying_Vega``,
    ``Credit_NonQualifying_Delta``, ``Credit_NonQualifying_Vega``,
    ``Equity_Delta``, ``Equity_Vega``, ``FX_Delta``, ``FX_Vega``,
    ``Commodity_Delta``, ``Commodity_Vega`` and ``Curvature``; the schedule
    calculator publishes the normalised asset class (e.g. ``interest_rate``,
    or ``interest_rate_ngr`` for the NGR path); the haircut calculator the
    collateral asset class.

    Parameters
    ----------
    (Produced by the IM calculators; also loadable via ``from_json``.)

    Returns
    -------
    ImResult
        IM amount and metadata.

    Examples
    --------
    >>> calc = ScheduleImCalculator.bcbs_standard()
    >>> result = calc.calculate_for_notional(1_000_000, "USD", "interest_rate", 5.0, "2025-01-15")
    >>> (result.amount, result.breakdown_keys())
    (40000.0, ['interest_rate'])
    """

    @staticmethod
    def from_json(json: str) -> ImResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``ImResult``.

        Returns
        -------
        ImResult
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = ScheduleImCalculator.bcbs_standard().calculate_for_notional(
        ...     1_000_000, "USD", "interest_rate", 5.0, "2025-01-15"
        ... )
        >>> ImResult.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(
        ...     ScheduleImCalculator
        ...     .bcbs_standard()
        ...     .calculate_for_notional(1_000_000, "USD", "interest_rate", 5.0, "2025-01-15")
        ...     .to_json(),
        ...     str,
        ... )
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def amount(self) -> float:
        """
        Calculated initial margin amount.

        Returns
        -------
        float
            IM notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        Currency of the IM amount.

        Returns
        -------
        str
            ISO currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def methodology(self) -> ImMethodology:
        """
        Methodology used for calculation.

        Returns
        -------
        ImMethodology
            IM methodology.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mpor_days(self) -> int:
        """
        Margin period of risk in business days.

        Returns
        -------
        int
            MPOR in business days (10 for SIMM/schedule, 2 for haircut IM).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def approximation(self) -> bool:
        """
        True when the amount is a conservative proxy, not an exact methodology result.

        Returns
        -------
        bool
            Whether the amount is an approximation (which need not be conservative) rather
            than an exact computation under the named methodology.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_of(self) -> datetime.date:
        """
        Calculation date.

        Returns
        -------
        datetime.date
            The ``as_of`` passed to the calculator.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> calc = ScheduleImCalculator.bcbs_standard()
        >>> calc.calculate_for_notional(1_000_000, "USD", "interest_rate", 5.0, "2025-01-15").as_of
        datetime.date(2025, 1, 15)
        """
        ...

    def breakdown_keys(self) -> list[str]:
        """
        Breakdown component labels present, in canonical sorted order (see
        the class docstring for the SIMM and schedule label sets).

        Returns
        -------
        list[str]
            Keys present in the breakdown map.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def breakdown_amount(self, key: str) -> float | None:
        """
        Get breakdown amount for a risk class.

        Parameters
        ----------
        key : str
            Component label such as ``"IR_Delta"`` (SIMM) or
            ``"interest_rate"`` (schedule).

        Returns
        -------
        float or None
            Amount in ``currency`` if present.

        Notes
        -----
        This method does not raise; a missing result is ``None`` rather than an exception.
        """
        ...

    def __repr__(self) -> str: ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the headline result as a single-row pandas ``DataFrame``.

        Columns: ``amount``, ``currency``, ``methodology``, ``mpor_days``,
        ``as_of``, ``approximation``.

        ``amount`` is a float in ``currency``; ``mpor_days`` is the margin
        period of risk in business days; ``as_of`` is an ISO 8601 date string.
        ``approximation`` is ``True`` when the amount uses a proxy methodology
        rather than an exact computation under the named methodology - do not
        reconcile an approximated figure against an actual margin call.
        Per-risk-class detail lives in ``to_breakdown_dataframe``.

        Returns
        -------
        pd.DataFrame
            One row describing the initial-margin result.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_breakdown_dataframe(self) -> pd.DataFrame:
        """
        Export the per-component breakdown as a pandas ``DataFrame``.

        Columns: ``risk_class``, ``amount``, ``currency``. One row per
        component label (SIMM: ``IR_Delta``, ``IR_Vega``, ``FX_Delta``,
        ``Curvature``, ...; schedule: the asset class such as
        ``interest_rate``), sorted by ``risk_class`` so repeated runs are
        byte-identical. Methodologies that publish no breakdown yield a
        zero-row frame that still carries all three columns.

        Breakdown components do not generally sum to ``amount``: SIMM and
        other methodologies aggregate risk classes with correlations.

        Returns
        -------
        pd.DataFrame
            One row per risk class, sorted by ``risk_class``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

class SimmSensitivities:
    """
    ISDA SIMM sensitivity portfolio.

    Stores signed sensitivity amounts by SIMM risk class and bucket. Amounts
    are currency amounts in ``base_currency``, not percentages or spot
    levels: rate and credit deltas are DV01/CS01-style amounts per 1bp move,
    vegas are currency vega amounts compatible with the SIMM vega weights,
    and curvature is one signed contribution per risk class.

    Tenor labels must be SIMM buckets (``CONSTANTS["SIMM_TENORS"]``:
    ``"2W"``, ``"1M"``, ``"3M"``, ``"6M"``, ``"1Y"``, ``"2Y"``, ``"3Y"``,
    ``"5Y"``, ``"10Y"``, ``"15Y"``, ``"20Y"``, ``"30Y"``) and commodity
    buckets one of the 17 ISDA buckets; ``validate()`` — run automatically by
    ``SimmCalculator.calculate_from_sensitivities`` — rejects anything else so
    a typo cannot price to zero margin.

    Use ``from_json``/``to_json`` for full-fidelity interop with the canonical
    Rust JSON shape, ``from_dataframe``/``to_dataframe`` for CRIF-style bulk
    loading, or the ``add_*`` helpers for notebook-style construction.

    Examples
    --------
    >>> from finstack_quant.margin import SimmSensitivities
    >>> sensitivities = SimmSensitivities("USD")
    >>> sensitivities.is_empty()
    True
    """

    def __init__(self, base_currency: str = "USD") -> None:
        """
        Create an empty SIMM sensitivity set.

        Parameters
        ----------
        base_currency : str, default "USD"
            ISO currency code for the currency in which the sensitivity
            amounts are expressed.

        Raises
        ------
        ValueError
            If ``base_currency`` is not a known currency code.
        """
        ...

    @staticmethod
    def from_json(json: str) -> SimmSensitivities:
        """
        Deserialize SIMM sensitivities from the canonical JSON shape.

        Parameters
        ----------
        json : str
            JSON string produced by ``to_json`` or by Rust
            ``SimmSensitivities::to_json_pretty``. Tuple-keyed Rust maps are
            represented as arrays such as ``[currency, tenor, amount]``.

        Returns
        -------
        SimmSensitivities
            Sensitivity set populated from the JSON payload.

        Raises
        ------
        ValueError
            If the payload is not valid JSON or does not match the SIMM
            sensitivity schema.

        Examples
        --------
        >>> from finstack_quant.margin import SimmSensitivities
        >>> original = SimmSensitivities("USD")
        >>> original.add_ir_delta("USD", "5Y", 1_000.0)
        >>> restored = SimmSensitivities.from_json(original.to_json())
        >>> (restored.base_currency, restored.is_empty())
        ('USD', False)
        """
        ...

    def to_json(self) -> str:
        """
        Serialize sensitivities to the canonical pretty-printed JSON shape.

        Returns
        -------
        str
            JSON string containing all populated buckets and the base currency.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @staticmethod
    def from_dataframe(frame: pd.DataFrame, base_currency: str = "USD") -> SimmSensitivities:
        """
        Bulk-load sensitivities from the long-format frame ``to_dataframe``
        emits (CRIF-style).

        Parameters
        ----------
        frame : pd.DataFrame
            Columns ``risk_class``, ``kind``, ``issuer``, ``bucket``,
            ``tenor``, ``amount`` with the ``to_dataframe`` encoding:
            ``issuer`` is the currency for ``interest_rate``/``fx`` delta,
            the ``"CCY1/CCY2"`` pair for FX vega, the issuer for credit and
            the underlier for equity; ``bucket`` is the credit sector or the
            commodity bucket; ``tenor`` is the SIMM tenor where the risk
            class has one; ``kind`` is ``delta``, ``vega`` or ``curvature``.
        base_currency : str, default "USD"
            Currency in which every ``amount`` is expressed.

        Returns
        -------
        SimmSensitivities
            Container with rows of the same key accumulated.

        Raises
        ------
        ValueError
            If a risk class, kind, sector or currency is unknown or a
            required column is missing.
        TypeError
            If ``frame`` is not a pandas ``DataFrame``.

        Examples
        --------
        >>> original = SimmSensitivities("USD")
        >>> original.add_ir_delta("USD", "5Y", 1_000.0)
        >>> restored = SimmSensitivities.from_dataframe(original.to_dataframe(), "USD")
        >>> restored.total_ir_delta()
        1000.0
        """
        ...

    def add_ir_delta(self, currency: str, tenor: str, amount: float) -> None:
        """
        Add an interest-rate delta bucket.

        Parameters
        ----------
        currency : str
            Currency risk factor, such as ``"USD"``.
        tenor : str
            SIMM tenor bucket, such as ``"2W"``, ``"1Y"``, ``"5Y"``, or
            ``"30Y"`` (see ``CONSTANTS["SIMM_TENORS"]``).
        amount : float
            Signed DV01-style currency amount per 1bp move, in
            ``base_currency``.

        Raises
        ------
        ValueError
            If ``currency`` is not a known currency code.

        Examples
        --------
        >>> sens = SimmSensitivities("USD")
        >>> sens.add_ir_delta("USD", "5Y", 25_000.0)
        >>> sens.total_ir_delta()
        25000.0
        """
        ...

    def add_ir_vega(self, currency: str, tenor: str, amount: float) -> None:
        """
        Add an interest-rate vega bucket.

        Parameters
        ----------
        currency : str
            Currency risk factor, such as ``"USD"``.
        tenor : str
            SIMM tenor bucket (see ``CONSTANTS["SIMM_TENORS"]``).
        amount : float
            Signed currency vega amount in ``base_currency``, compatible with
            the SIMM IR vega weights.

        Raises
        ------
        ValueError
            If ``currency`` is not a known currency code.
        """
        ...

    def add_credit_qualifying_delta(self, sector: str, name: str, tenor: str, amount: float) -> None:
        """
        Add a sector-bucketed credit-qualifying delta sensitivity.

        Parameters
        ----------
        sector : str
            Canonical ISDA SIMM sector, such as ``"sovereign"``,
            ``"financial"``, ``"basic_materials"``,
            ``"high_yield_financial"``, or ``"residual"``.
        name : str
            Issuer, index, or reference-entity identifier.
        tenor : str
            SIMM credit tenor bucket, such as ``"5Y"``.
        amount : float
            Signed CS01-style currency amount per 1bp move, in
            ``base_currency``.

        Raises
        ------
        ValueError
            If ``sector`` is not a canonical SIMM credit sector.
        """
        ...

    def add_credit_qualifying_vega(self, sector: str, name: str, tenor: str, amount: float) -> None:
        """
        Add a sector-bucketed credit-qualifying vega sensitivity.

        Parameters
        ----------
        sector : str
            Canonical ISDA SIMM sector label (see
            ``add_credit_qualifying_delta``).
        name : str
            Issuer, index, or reference-entity identifier.
        tenor : str
            SIMM credit tenor bucket, such as ``"5Y"``.
        amount : float
            Signed currency vega amount in ``base_currency``, compatible with
            the SIMM credit-qualifying vega risk weight.

        Raises
        ------
        ValueError
            If ``sector`` is not a canonical SIMM credit sector.

        Examples
        --------
        >>> sens = SimmSensitivities("USD")
        >>> sens.add_credit_qualifying_vega("financial", "BANK_A", "5Y", 1_000.0)
        >>> sens.is_empty()
        False
        """
        ...

    def add_credit_non_qualifying_delta(self, name: str, tenor: str, amount: float) -> None:
        """
        Add a credit non-qualifying delta sensitivity.

        Parameters
        ----------
        name : str
            Securitization or other explicitly non-qualifying exposure identifier.
        tenor : str
            SIMM credit tenor bucket, such as ``"5Y"``.
        amount : float
            Signed CS01-style currency amount per 1bp move, in
            ``base_currency``.

        Notes
        -----
        This method does not raise; it updates stored state in place.
        """
        ...

    def add_credit_non_qualifying_vega(self, name: str, tenor: str, amount: float) -> None:
        """
        Add a credit non-qualifying vega sensitivity.

        Parameters
        ----------
        name : str
            Securitization or other explicitly non-qualifying exposure identifier.
        tenor : str
            SIMM credit tenor bucket, such as ``"5Y"``.
        amount : float
            Signed currency vega amount in ``base_currency``, compatible with
            the SIMM credit-non-qualifying vega risk weight.

        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = SimmSensitivities("USD")
        >>> sens.add_credit_non_qualifying_vega("RMBS-1", "5Y", 300.0)
        >>> sens.is_empty()
        False
        """
        ...

    def add_equity_delta(self, underlier: str, amount: float) -> None:
        """
        Add an equity delta bucket.

        Parameters
        ----------
        underlier : str
            Equity underlier or index identifier.
        amount : float
            Signed currency sensitivity in ``base_currency`` (not a
            percentage delta).

        Notes
        -----
        This method does not raise; it updates stored state in place.
        """
        ...

    def add_equity_vega(self, underlier: str, amount: float) -> None:
        """
        Add an equity vega bucket.

        Parameters
        ----------
        underlier : str
            Equity underlier or index identifier.
        amount : float
            Signed currency vega amount in ``base_currency``.

        Notes
        -----
        This method does not raise; it updates stored state in place.
        """
        ...

    def add_fx_delta(self, currency: str, amount: float) -> None:
        """
        Add an FX delta sensitivity to a SIMM bucket.

        Parameters
        ----------
        currency : str
            FX risk-factor currency.
        amount : float
            Signed currency sensitivity in ``base_currency`` to the FX risk
            factor (not a spot level or percentage move).

        Raises
        ------
        ValueError
            If ``currency`` is not a known currency code.
        """
        ...

    def add_fx_vega(self, ccy1: str, ccy2: str, amount: float) -> None:
        """
        Add an FX vega bucket for a currency pair.

        Parameters
        ----------
        ccy1 : str
            First currency in the FX pair.
        ccy2 : str
            Second currency in the FX pair.
        amount : float
            Signed currency vega amount in ``base_currency``.

        Raises
        ------
        ValueError
            If either currency code is unknown.
        """
        ...

    def add_commodity_delta(self, bucket: str, amount: float) -> None:
        """
        Add a commodity delta bucket.

        Parameters
        ----------
        bucket : str
            SIMM commodity bucket id (``"1"`` .. ``"17"``) or ISDA bucket
            name in any casing (``"Crude"``, ``"light_ends"``,
            ``"Precious Metals"``, ...). Unknown labels are rejected by
            ``validate()``.
        amount : float
            Signed currency sensitivity in ``base_currency``.

        Notes
        -----
        This method does not raise; it updates stored state in place.
        """
        ...

    def add_commodity_vega(self, bucket: str, amount: float) -> None:
        """
        Add a commodity vega bucket.

        Parameters
        ----------
        bucket : str
            SIMM commodity bucket id or name (see ``add_commodity_delta``).
        amount : float
            Signed currency vega amount in ``base_currency``.

        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = SimmSensitivities("USD")
        >>> sens.add_commodity_vega("Crude", 750.0)
        >>> sens.is_empty()
        False
        """
        ...

    def add_curvature(self, risk_class: str, amount: float) -> None:
        """
        Add a curvature contribution for a SIMM risk class.

        Parameters
        ----------
        risk_class : str
            Lower-case SIMM risk class label: ``"interest_rate"``,
            ``"credit_qualifying"``, ``"credit_non_qualifying"``,
            ``"equity"``, ``"commodity"`` or ``"fx"``.
        amount : float
            Signed curvature contribution in ``base_currency`` before the
            SIMM curvature scale factor is applied.

        Raises
        ------
        ValueError
            If ``risk_class`` is not one of the labels above.
        """
        ...

    def merge(self, other: SimmSensitivities) -> None:
        """
        Add every bucket of ``other`` into this container (amounts sum), so
        offsetting risk nets within a netting set.

        Parameters
        ----------
        other : SimmSensitivities
            Container in the same ``base_currency``; convert with
            ``scaled_to_currency`` first otherwise.

        Raises
        ------
        ValueError
            If the base currencies differ.

        Examples
        --------
        >>> a = SimmSensitivities("USD")
        >>> a.add_ir_delta("USD", "5Y", 1_000.0)
        >>> b = SimmSensitivities("USD")
        >>> b.add_ir_delta("USD", "5Y", -400.0)
        >>> a.merge(b)
        >>> a.total_ir_delta()
        600.0
        """
        ...

    def scaled(self, factor: float) -> SimmSensitivities:
        """
        Return a copy with every amount multiplied by a signed scalar.

        Parameters
        ----------
        factor : float
            Signed multiplier (e.g. position quantity for unit-notional trade
            sensitivities); a negative factor flips every bucket.

        Returns
        -------
        SimmSensitivities
            Scaled copy in the same ``base_currency``.

        Notes
        -----
        This method does not raise; it returns a scaled copy.

        Examples
        --------
        >>> sens = SimmSensitivities("USD")
        >>> sens.add_ir_delta("USD", "5Y", 1_000.0)
        >>> sens.scaled(-2.0).total_ir_delta()
        -2000.0
        """
        ...

    def scaled_to_currency(self, target_currency: str, fx_rate: float) -> SimmSensitivities:
        """
        Return a copy re-expressed in another currency.

        Parameters
        ----------
        target_currency : str
            ISO-4217 code the amounts should be expressed in.
        fx_rate : float
            Value of one unit of the current ``base_currency`` in
            ``target_currency``; every amount is multiplied by it while the
            risk-factor keys are unchanged.

        Returns
        -------
        SimmSensitivities
            Copy with ``base_currency == target_currency``.

        Raises
        ------
        ValueError
            If ``target_currency`` is not a known currency code.

        Examples
        --------
        >>> sens = SimmSensitivities("USD")
        >>> sens.add_ir_delta("USD", "5Y", 1_000.0)
        >>> eur = sens.scaled_to_currency("EUR", 0.9)
        >>> (eur.base_currency, eur.total_ir_delta())
        ('EUR', 900.0)
        """
        ...

    def total_ir_delta(self) -> float:
        """
        Net IR delta summed across all currencies and tenors.

        Returns
        -------
        float
            Signed sum in ``base_currency``.

        Notes
        -----
        This method does not raise; it returns the derived value.

        Examples
        --------
        >>> sens = SimmSensitivities("USD")
        >>> sens.add_ir_delta("USD", "5Y", 1_000.0)
        >>> sens.add_ir_delta("EUR", "2Y", 500.0)
        >>> sens.total_ir_delta()
        1500.0
        """
        ...

    def total_equity_delta(self) -> float:
        """
        Net equity delta summed across all underliers.

        Returns
        -------
        float
            Signed sum in ``base_currency``.

        Notes
        -----
        This method does not raise; it returns the derived value.

        Examples
        --------
        >>> sens = SimmSensitivities("USD")
        >>> sens.add_equity_delta("SPX", 4_000.0)
        >>> sens.total_equity_delta()
        4000.0
        """
        ...

    def validate(self) -> None:
        """
        Validate tenor labels, commodity buckets, identifiers and amounts.

        ``SimmCalculator.calculate_from_sensitivities`` runs this
        automatically; call it directly to check a container built from
        external data.

        Returns
        -------
        None
            Returns ``None`` when every bucket is valid.

        Raises
        ------
        ValueError
            Naming the offending map when a tenor is not a SIMM bucket, a
            commodity bucket is unknown, an identifier is empty or an amount
            is non-finite.

        Examples
        --------
        >>> sens = SimmSensitivities("USD")
        >>> sens.add_ir_delta("USD", "7Y", 1_000.0)
        >>> try:
        ...     sens.validate()
        ... except ValueError as exc:
        ...     print("7Y" in str(exc))
        True
        """
        ...

    def is_empty(self) -> bool:
        """
        Return whether no sensitivity buckets have been populated.

        Returns
        -------
        bool
            ``True`` when every SIMM bucket map is empty. A populated bucket
            with a zero net amount still makes the container non-empty.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

    @property
    def base_currency(self) -> str:
        """
        Currency context in which sensitivity amounts are expressed.

        Returns
        -------
        str
            ISO currency code for sensitivity amounts.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export every populated sensitivity bucket as one long-format pandas
        ``DataFrame``.

        Columns: ``risk_class``, ``bucket``, ``tenor``, ``issuer``, ``kind``,
        ``amount``. One row per populated bucket; an empty container still
        carries all six columns. Long format is used deliberately - a column
        per bucket would give a different schema for every portfolio, and it
        matches ``FrtbSensitivities.to_dataframe``.

        ``risk_class`` uses the SIMM labels ``interest_rate``,
        ``credit_qualifying``, ``credit_non_qualifying``, ``equity``,
        ``commodity`` and ``fx``. ``kind`` is ``delta``, ``vega`` or
        ``curvature``; SIMM curvature is a single signed contribution per risk
        class, not an up/down pair.

        ``issuer`` carries the name axis: a currency code for IR and FX delta,
        a ``"CCY1/CCY2"`` pair for FX vega, an issuer or index for credit, an
        underlier for equity. It is ``None`` for commodity (keyed by bucket
        alone) and for curvature. ``bucket`` holds the SIMM credit sector for
        bucketed credit deltas (e.g. ``"sovereign"``) and the commodity bucket
        label; it is ``None`` elsewhere. ``tenor`` is the SIMM tenor bucket
        (``"2W"``, ``"1M"``, ..., ``"30Y"``) where the risk class has one.

        ``amount`` is a signed currency sensitivity in the container's base
        currency, in whatever convention the caller supplied - SIMM does not
        re-scale these on ingest. ``from_dataframe`` accepts this frame back.

        Rows are sorted by ``(risk_class, kind, issuer, bucket, tenor)`` so
        repeated exports of the same portfolio are identical.

        Returns
        -------
        pd.DataFrame
            One row per populated sensitivity bucket.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class SimmCalculator:
    """
    Indicative historical SIMM v2.6 calculator, with USD concentration thresholds.

    Loads registry-backed SIMM parameters for the requested rule version and
    calculates an approximation from explicit ``SimmSensitivities``. Product-class,
    subcurve and some non-IR dimensions are incomplete. Every result has
    ``approximation=True``; this is not a current regulatory SIMM implementation.

    Examples
    --------
    >>> from finstack_quant.margin import SimmCalculator
    >>> calculator = SimmCalculator("v2_6")
    >>> (calculator.version, calculator.mpor_days)
    ('v2_6', 10)
    """

    def __init__(self, version: str | None = None, mpor_days: int | None = None) -> None:
        """
        Create a SIMM calculator from the embedded margin registry.

        Parameters
        ----------
        version : str or None, optional
            Canonical version label ``"v2_6"``. No other version or alias is
            accepted. When omitted, ``"v2_6"`` is used.
        mpor_days : int | None, optional
            Optional margin period of risk override in business days
            (stamped on results; ISDA SIMM standard is 10). When omitted,
            the registry default for the SIMM version is used.

        Raises
        ------
        ValueError
            If the version is unknown or registry parameters cannot be loaded.
        """
        ...

    @property
    def version(self) -> str:
        """
        Stable supported SIMM version label, ``"v2_6"``.

        Returns
        -------
        str
            Normalized SIMM version label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mpor_days(self) -> int:
        """
        Margin period of risk in business days.

        Returns
        -------
        int
            MPOR in business days.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def calculate_from_sensitivities(
        self,
        sensitivities: SimmSensitivities,
        currency: str,
        as_of: datetime.date | str,
    ) -> ImResult:
        """
        Calculate SIMM initial margin from explicit sensitivities.

        Parameters
        ----------
        sensitivities : SimmSensitivities
            Sensitivity set to aggregate; validated first, so an unknown
            tenor or commodity bucket raises instead of pricing to zero.
        currency : str
            Must be ``"USD"`` and match ``sensitivities.base_currency``.
            Concentration thresholds are denominated in USD. Convert other
            reporting-currency inputs explicitly before calculation.
        as_of : datetime.date | str
            Calculation date stamped on the result (``datetime.date``,
            ``pandas.Timestamp`` or ISO ``YYYY-MM-DD``).

        Returns
        -------
        ImResult
            Initial-margin amount, methodology, MPOR, calculation date, and
            the SIMM component breakdown (``IR_Delta``, ``FX_Delta``, ...).

        Raises
        ------
        ValueError
            If sensitivities fail validation, the input/output currency is not USD, or
            a date string is not ISO 8601.
        TypeError
            If ``as_of`` is neither a string nor date-like.

        Examples
        --------
        >>> sens = SimmSensitivities("USD")
        >>> sens.add_ir_delta("USD", "5Y", 50_000.0)
        >>> result = SimmCalculator("v2_6").calculate_from_sensitivities(sens, "USD", "2025-01-15")
        >>> (result.amount > 0.0, result.breakdown_keys())
        (True, ['IR_Delta'])
        """
        ...

    def __repr__(self) -> str: ...

class ScheduleImCalculator:
    """
    BCBS-IOSCO regulatory schedule initial-margin calculator.

    Applies registry-backed schedule rates to explicit notionals or to a
    heterogeneous netting set with trade-specific rates and the BCBS-IOSCO net-to-gross ratio
    reduction.

    Examples
    --------
    >>> from finstack_quant.margin import ScheduleImCalculator
    >>> ScheduleImCalculator.bcbs_standard().rate("interest_rate", 5.0)
    0.04
    """

    @staticmethod
    def bcbs_standard() -> ScheduleImCalculator:
        """
        Create the embedded BCBS-IOSCO standard schedule calculator.

        Returns
        -------
        ScheduleImCalculator
            Calculator configured with the standard embedded schedule grid.

        Raises
        ------
        ValueError
            If embedded registry data cannot be loaded.

        Examples
        --------
        >>> from finstack_quant.margin import ScheduleImCalculator
        >>> ScheduleImCalculator.bcbs_standard().rate("interest_rate", 5.0)
        0.04
        """
        ...

    @staticmethod
    def from_registry_id(schedule_id: str) -> ScheduleImCalculator:
        """
        Create a schedule calculator from a registry identifier.

        Parameters
        ----------
        schedule_id : str
            Schedule identifier in the embedded margin registry.

        Returns
        -------
        ScheduleImCalculator
            Calculator configured from the matching registry entry.

        Raises
        ------
        ValueError
            If ``schedule_id`` is unknown or registry data is invalid.

        Examples
        --------
        >>> from finstack_quant.margin import ScheduleImCalculator
        >>> ScheduleImCalculator.from_registry_id("bcbs_iosco").rate("interest_rate", 5.0)
        0.04
        """
        ...

    def with_asset_class(self, asset_class: str) -> ScheduleImCalculator:
        """
        Return a copy with a new default schedule asset class.

        Parameters
        ----------
        asset_class : str
            Lower-case schedule asset class label: ``"interest_rate"``,
            ``"credit"``, ``"equity"``, ``"commodity"``, ``"fx"``,
            ``"other"``, or ``"custom_<name>"`` for a registry-defined class.

        Returns
        -------
        ScheduleImCalculator
            Copy of this calculator with the default asset class changed.

        Raises
        ------
        ValueError
            If the asset class is unknown, maturity is negative/non-finite, or
            the selected rate is outside the finite [0, 1] range.
        """
        ...

    def with_maturity(self, years: float) -> ScheduleImCalculator:
        """
        Return a copy with a new default maturity.

        Parameters
        ----------
        years : float
            Representative remaining maturity in years.

        Returns
        -------
        ScheduleImCalculator
            Copy of this calculator with the default maturity changed.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    @property
    def default_asset_class(self) -> str:
        """
        Default asset class label used by trait-based calculations.

        Returns
        -------
        str
            Lower-case label such as ``'interest_rate'`` (``'custom_<name>'`` for registry-defined classes).

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ScheduleImCalculator.bcbs_standard().with_asset_class("credit").default_asset_class
        'credit'
        """
        ...

    @property
    def default_maturity_years(self) -> float:
        """
        Default remaining maturity in years used by trait-based calculations.

        Returns
        -------
        float
            Maturity in years.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ScheduleImCalculator.bcbs_standard().with_maturity(7.0).default_maturity_years
        7.0
        """
        ...

    @property
    def mpor_days(self) -> int:
        """
        Margin period of risk in business days stamped on results.

        Returns
        -------
        int
            MPOR in business days.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ScheduleImCalculator.bcbs_standard().mpor_days
        10
        """
        ...

    def rate(self, asset_class: str, maturity_years: float) -> float:
        """
        Look up a decimal schedule rate.

        Parameters
        ----------
        asset_class : str
            Schedule asset class alias.
        maturity_years : float
            Finite nonnegative remaining maturity in years.

        Returns
        -------
        float
            Decimal IM rate, e.g. ``0.01`` for 1%.

        Raises
        ------
        ValueError
            If the asset class is unknown, maturity is negative/non-finite, or
            the selected rate is outside the finite [0, 1] range.
        """
        ...

    def calculate_for_notional(
        self,
        notional: float,
        currency: str,
        asset_class: str,
        maturity_years: float,
        as_of: datetime.date | str,
    ) -> ImResult:
        """
        Calculate gross schedule IM from an explicit notional.

        Parameters
        ----------
        notional : float
            Regulatory notional or caller-supplied exposure base. The schedule
            formula uses ``abs(notional)``.
        currency : str
            Currency code for the notional and result.
        asset_class : str
            Schedule asset class alias.
        maturity_years : float
            Remaining maturity used for the schedule-rate lookup.
        as_of : datetime.date | str
            Calculation date stamped on the result (``datetime.date``,
            ``pandas.Timestamp`` or ISO ``YYYY-MM-DD``).

        Returns
        -------
        ImResult
            Gross schedule IM with a breakdown key equal to the normalized
            asset class.

        Raises
        ------
        ValueError
            If the currency, asset class, amount, or date is invalid.
        TypeError
            If ``as_of`` is neither a string nor date-like.

        Examples
        --------
        >>> calc = ScheduleImCalculator.bcbs_standard()
        >>> calc.calculate_for_notional(1_000_000, "USD", "interest_rate", 5.0, "2025-01-15").amount
        40000.0
        """
        ...

    def calculate_netting_set_with_ngr(
        self,
        positions: list[tuple[float, float, str, float]],
        currency: str,
        as_of: datetime.date | str,
    ) -> ImResult | None:
        """
        Calculate schedule IM for a netting set using NGR.

        Applies the BCBS-IOSCO reduction ``0.4 + 0.6 * NGR`` to the sum of trade-specific gross IM across the netting set. Each tuple
        specifies ``(signed_mtm, gross_notional, asset_class, maturity_years)``
        in the common reporting currency.

        Parameters
        ----------
        positions : list[tuple[float, float, str, float]]
            Per-trade MTM, notional, schedule class label and finite nonnegative
            residual maturity in years. MTM signs determine NGR; each
            absolute notional receives its own class and maturity rate.
        currency : str
            Reporting currency for every MTM, notional, and result.
        as_of : datetime.date | str
            Calculation date stamped on the result.

        Returns
        -------
        ImResult | None
            NGR-adjusted schedule IM (breakdown key ``"<asset_class>_ngr"``).
            Returns ``None`` for an empty position list or zero gross
            notional.

        Raises
        ------
        ValueError
            If the currency, asset class, amount, or date is invalid.
        TypeError
            If ``as_of`` is neither a string nor date-like.

        Examples
        --------
        >>> calc = ScheduleImCalculator.bcbs_standard()
        >>> netted = calc.calculate_netting_set_with_ngr(
        ...     [(2e6, 1e8, "interest_rate", 5.0), (-1.5e6, 8e7, "interest_rate", 5.0)], "USD", "2025-01-15"
        ... )
        >>> netted.breakdown_keys()
        ['interest_rate_ngr']
        """
        ...

    def __repr__(self) -> str: ...

class HaircutImCalculator:
    """
    Haircut-based initial-margin calculator.

    Applies eligible-collateral haircuts and optional FX add-ons to explicit
    collateral values. This path is intended for repo and securities-financing
    style collateral IM rather than SIMM sensitivities.

    Examples
    --------
    >>> from finstack_quant.margin import CollateralAssetClass, HaircutImCalculator
    >>> HaircutImCalculator.bcbs_standard().haircut_for(CollateralAssetClass.cash())
    0.0
    """

    @staticmethod
    def bcbs_standard() -> HaircutImCalculator:
        """
        Create a haircut calculator with the BCBS-IOSCO schedule.

        Returns
        -------
        HaircutImCalculator
            Calculator using the embedded BCBS-IOSCO collateral haircuts.

        Raises
        ------
        ValueError
            If embedded registry data cannot be loaded.

        Examples
        --------
        >>> from finstack_quant.margin import CollateralAssetClass, HaircutImCalculator
        >>> HaircutImCalculator.bcbs_standard().haircut_for(CollateralAssetClass.cash())
        0.0
        """
        ...

    @staticmethod
    def us_treasuries() -> HaircutImCalculator:
        """
        Create a haircut calculator for US Treasury collateral.

        Returns
        -------
        HaircutImCalculator
            Calculator using the embedded US Treasuries haircut schedule.

        Raises
        ------
        ValueError
            If embedded registry data cannot be loaded.

        Examples
        --------
        >>> from finstack_quant.margin import CollateralAssetClass, HaircutImCalculator
        >>> HaircutImCalculator.us_treasuries().with_collateral_terms(0.5).haircut_for(
        ...     CollateralAssetClass.government_bonds()
        ... )
        0.005
        """
        ...

    @staticmethod
    def from_schedule(schedule: EligibleCollateralSchedule) -> HaircutImCalculator:
        """
        Create a haircut calculator from an eligible-collateral schedule.

        Parameters
        ----------
        schedule : EligibleCollateralSchedule
            Collateral eligibility and haircut schedule.

        Returns
        -------
        HaircutImCalculator
            Calculator backed by ``schedule``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.margin import CollateralAssetClass, EligibleCollateralSchedule, HaircutImCalculator
        >>> schedule = EligibleCollateralSchedule.cash_only()
        >>> HaircutImCalculator.from_schedule(schedule).haircut_for(CollateralAssetClass.cash())
        0.0
        """
        ...

    def with_default_asset_class(self, asset_class: CollateralAssetClass | str) -> HaircutImCalculator:
        """
        Return a copy configured with a default collateral asset class.

        Parameters
        ----------
        asset_class : CollateralAssetClass | str
            Asset class (or its lower-case wire label) used by trait-based
            calculations.

        Returns
        -------
        HaircutImCalculator
            Copy of this calculator with the default asset class changed.

        Raises
        ------
        ValueError
            If a string label is not a collateral asset class.

        Examples
        --------
        >>> calc = HaircutImCalculator.bcbs_standard().with_default_asset_class("government_bonds")
        >>> calc.default_asset_class
        CollateralAssetClass(government_bonds)
        """
        ...

    def with_posted_collateral_currency(self, currency: str) -> HaircutImCalculator:
        """
        Return a copy configured with a posted-collateral currency.

        Parameters
        ----------
        currency : str
            Currency code used to detect FX mismatch in trait-based
            calculations.

        Returns
        -------
        HaircutImCalculator
            Copy of this calculator with the collateral currency configured.

        Raises
        ------
        ValueError
            If ``currency`` is not a known currency code.
        """
        ...

    @property
    def eligible_collateral(self) -> EligibleCollateralSchedule:
        """
        Eligible-collateral schedule the haircuts are read from.

        Returns
        -------
        EligibleCollateralSchedule
            The schedule.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> HaircutImCalculator.bcbs_standard().eligible_collateral.eligible_count > 0
        True
        """
        ...

    @property
    def default_asset_class(self) -> CollateralAssetClass:
        """
        Default collateral asset class assumed by trait-based calculations.

        Returns
        -------
        CollateralAssetClass
            Asset class (cash unless overridden).

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> HaircutImCalculator.bcbs_standard().default_asset_class
        CollateralAssetClass(government_bonds)
        """
        ...

    @property
    def posted_collateral_currency(self) -> str | None:
        """
        Declared posted-collateral currency code, or ``None``.

        Returns
        -------
        str | None
            ISO-4217 code, or ``None`` when not declared.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> HaircutImCalculator.bcbs_standard().with_posted_collateral_currency("EUR").posted_collateral_currency
        'EUR'
        """
        ...

    @property
    def mpor_days(self) -> int:
        """
        Margin period of risk in business days stamped on every result (CONSTANTS HAIRCUT_MPOR_DAYS).

        Returns
        -------
        int
            MPOR in business days.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> HaircutImCalculator.bcbs_standard().mpor_days
        2
        """
        ...

    def with_collateral_terms(self, remaining_years: float, rating: str | None = None) -> HaircutImCalculator:
        """Return a copy configured to select an eligible maturity and rating entry.

        Parameters
        ----------
        remaining_years : float
            Finite, nonnegative residual collateral maturity in years.
        rating : str | None, optional
            Credit rating such as ``"AAA"`` or ``"A-"``. Required when the
            contractual schedule imposes a minimum rating; no rating is inferred.

        Returns
        -------
        HaircutImCalculator
            Copy using these terms for eligibility, haircut and FX add-on lookup.

        Raises
        ------
        ValueError
            If maturity is negative or non-finite, or the rating is unknown.

        Examples
        --------
        >>> calc = HaircutImCalculator.us_treasuries().with_collateral_terms(7.0, "AAA")
        >>> calc.haircut_for("government_bonds")
        0.015
        """
        ...

    def haircut_for(self, asset_class: CollateralAssetClass | str) -> float:
        """
        Look up the decimal haircut for a collateral asset class.

        Parameters
        ----------
        asset_class : CollateralAssetClass | str
            Collateral asset class or its lower-case wire label.

        Returns
        -------
        float
            Decimal haircut including only the base haircut, not the optional
            FX add-on.

        Raises
        ------
        ValueError
            If no schedule or standard haircut exists for ``asset_class``.
        """
        ...

    def calculate_for_collateral(
        self,
        collateral_value: float,
        currency: str,
        asset_class: CollateralAssetClass | str,
        currency_mismatch: bool,
        as_of: datetime.date | str,
    ) -> ImResult:
        """
        Calculate haircut IM from explicit collateral value and asset class.

        Parameters
        ----------
        collateral_value : float
            Collateral market value in ``currency``.
        currency : str
            Currency code for the collateral value and result.
        asset_class : CollateralAssetClass | str
            Collateral asset class (or its wire label) used for the haircut
            lookup and the breakdown key.
        currency_mismatch : bool
            Whether to add the asset-class FX mismatch add-on.
        as_of : datetime.date | str
            Calculation date stamped on the result.

        Returns
        -------
        ImResult
            Haircut IM result. The MPOR is the Rust canonical repo haircut
            horizon, ``CONSTANTS["HAIRCUT_MPOR_DAYS"]`` (2 business days).

        Raises
        ------
        ValueError
            If the currency, amount, date, haircut, or FX add-on cannot be
            resolved.
        TypeError
            If ``as_of`` is neither a string nor date-like.

        Examples
        --------
        >>> calc = HaircutImCalculator.bcbs_standard()
        >>> calc.calculate_for_collateral(1e7, "USD", "cash", True, "2025-01-15").amount
        800000.0
        """
        ...

    def __repr__(self) -> str: ...

class FundingConfig:
    """
    Funding cost/benefit configuration for FVA and MVA calculation.

    Supplying ``im_profile`` turns on MVA: :func:`compute_bilateral_xva` then
    prices the funding cost of posted initial margin and includes it in
    ``XvaResult.total_xva``.

    Parameters
    ----------
    funding_spread_bp : float
        Non-negative finite funding cost spread in basis points.
    funding_benefit_bp : float | None, optional
        Non-negative finite funding benefit in bp, no greater than the cost
        spread; ``None`` for symmetric funding.
    im_profile : ImProfile | None, optional
        Valid expected initial-margin profile driving MVA; ``None`` disables MVA.
    margin_funding_spread_bp : float | None, optional
        Non-negative finite spread applied to posted IM; ``None`` reuses
        ``funding_spread_bp``.

    Returns
    -------
    FundingConfig
        Funding parameters.

    Raises
    ------
    ValueError
        If a spread is negative or non-finite, the benefit exceeds the funding
        cost, or ``im_profile`` is invalid.

    Examples
    --------
    >>> FundingConfig(50.0, None).funding_spread_bp
    50.0
    """

    def __init__(
        self,
        funding_spread_bp: float,
        funding_benefit_bp: float | None = None,
        im_profile: ImProfile | None = None,
        margin_funding_spread_bp: float | None = None,
    ) -> None:
        """
        Initialize FVA and MVA funding parameters.

        Parameters
        ----------
        funding_spread_bp : float
            Non-negative finite funding cost spread in basis points.
        funding_benefit_bp : float | None
            Non-negative finite funding benefit in basis points, no greater
            than the cost spread; ``None`` uses the funding cost spread.
        im_profile : ImProfile | None
            Valid expected initial-margin profile driving MVA, or ``None``.
        margin_funding_spread_bp : float | None
            Non-negative finite IM funding spread in bp, or ``None`` to reuse
            ``funding_spread_bp``.

        Raises
        ------
        ValueError
            If a spread is negative or non-finite, the benefit exceeds the
            funding cost, or ``im_profile`` is invalid.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FundingConfig:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``FundingConfig``.

        Returns
        -------
        FundingConfig
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = FundingConfig(50.0, 30.0)
        >>> FundingConfig.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(FundingConfig(50.0, 30.0).to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def funding_spread_bp(self) -> float:
        """
        Funding spread in basis points.

        Returns
        -------
        float
            Spread in bp.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> FundingConfig(10.0).funding_spread_bp
        10.0
        """
        ...

    @property
    def funding_benefit_bp(self) -> float | None:
        """
        Funding benefit spread in basis points (or None).

        Returns
        -------
        float or None
            Benefit bp if asymmetric.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> FundingConfig(10.0, 8.0).funding_benefit_bp
        8.0
        """
        ...

    @property
    def im_profile(self) -> ImProfile | None:
        """
        Expected initial-margin profile driving MVA (or None).

        Returns
        -------
        ImProfile or None
            The IM profile when MVA is enabled.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> FundingConfig(10.0).im_profile is None
        True
        """
        ...

    @property
    def margin_funding_spread_bp(self) -> float | None:
        """
        IM funding spread in basis points (or None).

        Returns
        -------
        float or None
            Explicit IM funding spread, when overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> FundingConfig(10.0, None, None, 6.0).margin_funding_spread_bp
        6.0
        """
        ...

    def effective_margin_spread_bp(self) -> float:
        """
        Effective IM funding spread in basis points.

        Falls back to ``funding_spread_bp`` when
        ``margin_funding_spread_bp`` is ``None``.

        Returns
        -------
        float
            Effective IM funding spread in bp.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> FundingConfig(10.0).effective_margin_spread_bp()
        10.0
        """
        ...

    def effective_benefit_bp(self) -> float:
        """
        Effective funding benefit spread in basis points.

        Returns
        -------
        float
            Effective benefit bp.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> isinstance(FundingConfig(1.0).effective_benefit_bp(), float)
        True
        """
        ...

    def __repr__(self) -> str: ...

class ExposureDiagnostics:
    """
    Diagnostics from exposure simulation.

    Counters an exposure engine attaches to an ``ExposureProfile`` (via its
    ``diagnostics`` argument): how many market-roll and valuation failures
    occurred over how many time points.

    Parameters
    ----------
    market_roll_failures : int, default 0
        Number of market-roll failures.
    valuation_failures : int, default 0
        Total instrument valuation failures.
    total_time_points : int, default 0
        Total time grid points evaluated.

    Returns
    -------
    ExposureDiagnostics
        Counters for simulation health.

    Examples
    --------
    >>> ExposureDiagnostics(valuation_failures=2, total_time_points=40).valuation_failures
    2
    """

    def __init__(
        self,
        market_roll_failures: int = 0,
        valuation_failures: int = 0,
        total_time_points: int = 0,
    ) -> None:
        """
        Create a diagnostics record from its three counters.

        Parameters
        ----------
        market_roll_failures : int, default 0
            Number of market-roll failures.
        valuation_failures : int, default 0
            Total instrument valuation failures.
        total_time_points : int, default 0
            Total time grid points evaluated.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ExposureDiagnostics:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``ExposureDiagnostics``.

        Returns
        -------
        ExposureDiagnostics
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = ExposureDiagnostics(1, 2, 3)
        >>> ExposureDiagnostics.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(ExposureDiagnostics(1, 2, 3).to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def market_roll_failures(self) -> int:
        """
        Number of market-roll failures.

        Returns
        -------
        int
            Failure count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def valuation_failures(self) -> int:
        """
        Total instrument valuation failures.

        Returns
        -------
        int
            Failure count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_time_points(self) -> int:
        """
        Total time grid points evaluated.

        Returns
        -------
        int
            Point count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...

class ExposureProfile:
    """
    Exposure profile at each time grid point.

    Parameters
    ----------
    times : list[float]
        Exposure or IM observation times in years from the valuation date.
    mtm_values : list[float]
        Portfolio MtM at each time.
    epe : list[float]
        Expected positive exposure series.
    ene : list[float]
        Expected negative exposure series.
    diagnostics : ExposureDiagnostics | None, optional
        Engine failure counters to attach; ``None`` when built by hand.

    Returns
    -------
    ExposureProfile
        Profile vectors.

    Examples
    --------
    >>> p = ExposureProfile([0.0, 1.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0])
    >>> len(p)
    2
    """

    def __init__(
        self,
        times: list[float],
        mtm_values: list[float],
        epe: list[float],
        ene: list[float],
        diagnostics: ExposureDiagnostics | None = None,
    ) -> None:
        """
        Create aligned MtM, EPE, and ENE vectors on an exposure time grid.

        Parameters
        ----------
        times : list[float]
            Finite, nonnegative, strictly increasing times in years. An explicit
            zero-time point is allowed. Before the first point, XVA holds
            its exposure constant; it does not assume zero opening exposure.
        mtm_values : list[float]
            Portfolio mark-to-market amounts at the corresponding times.
        epe : list[float]
            Expected positive exposure amounts at the corresponding times.
        ene : list[float]
            Expected negative exposure amounts at the corresponding times.
        diagnostics : ExposureDiagnostics | None, optional
            Engine failure counters to attach.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied and
        checked by ``validate()`` or the XVA entry points.
        """
        ...

    @staticmethod
    def from_dataframe(frame: pd.DataFrame) -> ExposureProfile:
        """
        Build a profile from the frame ``to_dataframe`` emits.

        Parameters
        ----------
        frame : pd.DataFrame
            Columns ``mtm_values``, ``epe``, ``ene`` indexed by time in
            years.

        Returns
        -------
        ExposureProfile
            Profile with the index as ``times`` and no diagnostics.

        Raises
        ------
        ValueError
            If a column is missing or non-numeric.
        TypeError
            If ``frame`` is not a pandas ``DataFrame``.

        Examples
        --------
        >>> original = ExposureProfile([0.5, 1.0], [1.0, 2.0], [1.0, 2.0], [0.0, 0.0])
        >>> ExposureProfile.from_dataframe(original.to_dataframe()).epe
        [1.0, 2.0]
        """
        ...

    @property
    def diagnostics(self) -> ExposureDiagnostics | None:
        """
        Engine diagnostics attached to the profile.

        Returns
        -------
        ExposureDiagnostics or None
            Counters supplied at construction (or carried through
            ``from_json``), else ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> p = ExposureProfile([1.0], [0.0], [0.0], [0.0], ExposureDiagnostics(0, 1, 1))
        >>> p.diagnostics.valuation_failures
        1
        """
        ...

    @staticmethod
    def from_json(json: str) -> ExposureProfile:
        """
        Parse this object from a JSON object or JSON string.

        Parameters
        ----------
        json : str
            JSON string.

        Returns
        -------
        ExposureProfile
            Parsed profile.

        Raises
        ------
        ValueError
            Invalid JSON.

        Examples
        --------
        >>> original = ExposureProfile([0.0, 1.0], [0.0, 2.0], [0.0, 2.0], [0.0, 0.0])
        >>> restored = ExposureProfile.from_json(original.to_json())
        >>> restored.mtm_values
        [0.0, 2.0]
        """
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
            Serialization error.

        Examples
        --------
        >>> '"times"' in ExposureProfile([1.0], [0.0], [0.0], [0.0]).to_json()
        True
        """
        ...

    def validate(self) -> None:
        """
        Validate internal consistency.

        Returns
        -------
        None
            Returns ``None`` when the vectors are consistent.

        Raises
        ------
        ValueError
            If the profile is empty, lengths differ, times are not strictly
            increasing and positive, or an amount is non-finite.

        Examples
        --------
        >>> ExposureProfile([1.0], [0.0], [0.0], [0.0]).validate()
        """
        ...

    @property
    def times(self) -> list[float]:
        """
        Exposure or IM observation times in years from the valuation date.

        Returns
        -------
        list[float]
            Times.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExposureProfile([0.0, 1.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]).times
        [0.0, 1.0]
        """
        ...

    @property
    def mtm_values(self) -> list[float]:
        """
        Portfolio MtM values at each time point.

        Returns
        -------
        list[float]
            MtM path.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExposureProfile([0.0], [1.0], [0.0], [0.0]).mtm_values
        [1.0]
        """
        ...

    @property
    def epe(self) -> list[float]:
        """
        Expected Positive Exposure at each time point.

        Returns
        -------
        list[float]
            EPE series.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExposureProfile([0.0], [0.0], [2.0], [0.0]).epe
        [2.0]
        """
        ...

    @property
    def ene(self) -> list[float]:
        """
        Expected Negative Exposure at each time point.

        Returns
        -------
        list[float]
            ENE series.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExposureProfile([0.0], [0.0], [0.0], [1.0]).ene
        [1.0]
        """
        ...

    def __len__(self) -> int:
        """Number of time points.

        Returns
        -------
        int
            Length of time grid.

        Examples
        --------
        >>> len(ExposureProfile([0.0, 1.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]))
        2
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a pandas DataFrame with time (years) as index.

        Columns: ``mtm_values``, ``epe``, ``ene``; ``from_dataframe`` accepts
        this frame back.

        Returns
        -------
        pd.DataFrame
            Exposure profile as a DataFrame.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str: ...

class XvaResult:
    """
    Result of XVA calculations (CVA, DVA, FVA, MVA, exposure profiles).

    Adjustments compose as ``total_xva = CVA - DVA + FVA + MVA``; uncomputed
    optional legs contribute zero.

    Parameters
    ----------
    (Produced by XVA engine; also loadable via ``from_json``.)

    Returns
    -------
    XvaResult
        XVA amounts and profiles.

    Examples
    --------
    >>> doc = (
    ...     '{"cva":1.0,"total_xva":1.0,"epe_profile":[[0.0,2.0]],'
    ...     '"ene_profile":[[0.0,0.0]],"pfe_profile":[[0.0,2.0]],"max_pfe":2.0,'
    ...     '"effective_epe_profile":[[0.0,2.0]],"effective_epe":2.0}'
    ... )
    >>> result = XvaResult.from_json(doc)
    >>> (result.cva, result.total_xva)
    (1.0, 1.0)
    """

    @staticmethod
    def from_json(json: str) -> XvaResult:
        """
        Parse this object from a JSON object or JSON string.

        Parameters
        ----------
        json : str
            JSON string.

        Returns
        -------
        XvaResult
            Parsed result.

        Raises
        ------
        ValueError
            Invalid JSON.

        Examples
        --------
        >>> doc = (
        ...     '{"cva":1.0,"total_xva":1.0,"epe_profile":[[0.0,2.0]],'
        ...     '"ene_profile":[[0.0,0.0]],"pfe_profile":[[0.0,2.0]],"max_pfe":2.0,'
        ...     '"effective_epe_profile":[[0.0,2.0]],"effective_epe":2.0}'
        ... )
        >>> result = XvaResult.from_json(doc)
        >>> (result.max_pfe, result.effective_epe)
        (2.0, 2.0)
        """
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
            Serialization error.

        """
        ...

    @property
    def cva(self) -> float:
        """
        Unilateral CVA (positive = cost).

        Returns
        -------
        float
            CVA amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dva(self) -> float | None:
        """
        DVA (own-default benefit, or None).

        Returns
        -------
        float or None
            DVA if computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fva(self) -> float | None:
        """
        FVA (net funding cost/benefit, or None).

        Returns
        -------
        float or None
            FVA if computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mva(self) -> float | None:
        """
        MVA (funding cost of posted initial margin, or None).

        ``None`` unless the run supplied ``FundingConfig.im_profile``.

        Returns
        -------
        float or None
            MVA if computed; positive is a cost to the desk.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_xva(self) -> float:
        """
        All-in adjustment = CVA − DVA + FVA + MVA.

        Uncomputed legs contribute zero. This is the quantity subtracted from
        the risk-free value of the netting set.

        Returns
        -------
        float
            Total XVA.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def max_pfe(self) -> float:
        """
        Maximum PFE across the profile.

        Returns
        -------
        float
            Maximum PFE across the profile.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_epe(self) -> float:
        """
        Effective EPE (time-weighted average, regulatory metric).

        Returns
        -------
        float
            Effective EPE.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def epe_profile(self) -> list[tuple[float, float]]:
        """
        EPE profile as list of (time, value) tuples.

        Returns
        -------
        list[tuple[float, float]]
            (time, EPE) pairs.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ene_profile(self) -> list[tuple[float, float]]:
        """
        ENE profile as list of (time, value) tuples.

        Returns
        -------
        list[tuple[float, float]]
            (time, ENE) pairs.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pfe_profile(self) -> list[tuple[float, float]]:
        """
        PFE profile as list of (time, value) tuples.

        Returns
        -------
        list[tuple[float, float]]
            (time, PFE) pairs.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_epe_profile(self) -> list[tuple[float, float]]:
        """
        Effective EPE profile as list of (time, value) tuples.

        Returns
        -------
        list[tuple[float, float]]
            (time, effective EPE) pairs.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """
        Policy metadata stamped by the computing layer.

        Returns
        -------
        dict[str, Any]
            ``numeric_mode``, ``rounding`` (the active rounding context),
            ``fx_policy_applied`` (or ``None``), ``parallel`` and
            ``timestamp``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> doc = (
        ...     '{"cva":1.0,"total_xva":1.0,"epe_profile":[[0.0,2.0]],'
        ...     '"ene_profile":[[0.0,0.0]],"pfe_profile":[[0.0,2.0]],"max_pfe":2.0,'
        ...     '"effective_epe_profile":[[0.0,2.0]],"effective_epe":2.0}'
        ... )
        >>> "numeric_mode" in XvaResult.from_json(doc).meta
        True
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the XVA components as a single-row pandas DataFrame.

        Columns: ``cva``, ``dva``, ``fva``, ``mva``, ``total_xva``,
        ``max_pfe``, ``effective_epe`` -- all in the netting set's currency
        units, matching the properties of the same name. Uncomputed legs
        (``dva`` / ``fva`` / ``mva``) are ``NaN`` rather than absent, so the
        frame keeps its schema across netting sets.

        This is the default export; the time-indexed exposure profiles are a
        separate table -- see :meth:`to_profiles_dataframe`.

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame, so a portfolio of netting sets stacks with
            ``pd.concat``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_profiles_dataframe(self) -> pd.DataFrame:
        """
        Export exposure profiles as a pandas DataFrame.

        Columns: ``epe``, ``ene``, ``pfe``, ``effective_epe`` -- indexed
        by time in years.

        Returns
        -------
        pd.DataFrame
            Profile DataFrame.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str: ...

class ImDecayProfile:
    """
    Deterministic IM decay profile for MVA (Green 2015, ch. 10).

    Approximates the expected initial-margin path ``E[IM(t)] = IM(0) *
    factor(t)`` used by :func:`im_profile_from_simm`. Constructed via the
    static factories below; there is no direct constructor.

    Parameters
    ----------
    (Use ``constant()``, ``linear_to_maturity()``, or ``sqrt_time()``.)

    Returns
    -------
    ImDecayProfile
        Decay profile applied to today's IM.

    Examples
    --------
    >>> ImDecayProfile.constant().factor(5.0)
    1.0
    """

    @staticmethod
    def constant() -> ImDecayProfile:
        """
        IM stays at today's level for the whole horizon.

        Returns
        -------
        ImDecayProfile
            Decay profile with ``factor(t) == 1`` for all ``t``.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> ImDecayProfile.constant().factor(10.0)
        1.0
        """
        ...

    @staticmethod
    def linear_to_maturity(maturity_years: float) -> ImDecayProfile:
        """
        IM decays linearly to zero at ``maturity_years``.

        Parameters
        ----------
        maturity_years : float
            Portfolio maturity ``T`` in years; must be positive and finite.

        Returns
        -------
        ImDecayProfile
            Decay profile with ``factor(t) = max(1 - t/T, 0)``.

        Raises
        ------
        ValueError
            If ``maturity_years`` is non-positive or non-finite.

        Examples
        --------
        >>> ImDecayProfile.linear_to_maturity(2.0).factor(1.0)
        0.5
        """
        ...

    @staticmethod
    def sqrt_time(maturity_years: float) -> ImDecayProfile:
        """
        IM decays like the square root of remaining time to ``maturity_years``.

        Parameters
        ----------
        maturity_years : float
            Portfolio maturity ``T`` in years; must be positive and finite.

        Returns
        -------
        ImDecayProfile
            Decay profile with ``factor(t) = sqrt(max(1 - t/T, 0))``.

        Raises
        ------
        ValueError
            If ``maturity_years`` is non-positive or non-finite.

        Examples
        --------
        >>> round(ImDecayProfile.sqrt_time(2.0).factor(1.0), 6)
        0.707107
        """
        ...

    def factor(self, t: float) -> float:
        """
        IM decay multiplier applied at time ``t`` in the MVA profile.

        Parameters
        ----------
        t : float
            Time in years from the valuation date.

        Returns
        -------
        float
            Decay factor, always in ``[0, 1]`` for ``t >= 0``.

        Raises
        ------
        (None)
            This is a pure arithmetic evaluation; it never raises.

        Examples
        --------
        >>> ImDecayProfile.constant().factor(3.0)
        1.0
        """
        ...

    @staticmethod
    def from_json(json: str) -> ImDecayProfile:
        """
        Parse this object from a JSON object or JSON string.

        Parameters
        ----------
        json : str
            JSON representation produced by ``to_json``.

        Returns
        -------
        ImDecayProfile
            Parsed decay profile.

        Raises
        ------
        ValueError
            If the payload is not valid JSON or does not match the schema.

        Examples
        --------
        >>> ImDecayProfile.from_json(ImDecayProfile.constant().to_json())
        ImDecayProfile(constant)
        """
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
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> isinstance(ImDecayProfile.constant().to_json(), str)
        True
        """
        ...

    def __repr__(self) -> str: ...

class ImProfile:
    """
    Expected initial-margin profile ``E[IM(t)]`` on a time grid.

    Values are in the aggregation currency chosen when the profile was
    built (e.g. the ``currency`` argument of :func:`im_profile_from_simm`).

    Parameters
    ----------
    times : list[float]
        Time points in years from the valuation date; strictly increasing
        and positive.
    im_values : list[float]
        Expected IM at each time point; non-negative and finite.

    Returns
    -------
    ImProfile
        IM profile as constructed; not validated until ``validate()`` is
        called or the profile is passed to ``compute_mva`` or
        ``im_profile_from_simm``.

    Examples
    --------
    >>> ImProfile([1.0, 2.0], [100.0, 50.0]).times
    [1.0, 2.0]
    """

    def __init__(self, times: list[float], im_values: list[float]) -> None:
        """
        Construct from time and IM vectors.

        Values are stored as given and are not validated at construction
        time. Call ``validate()`` explicitly, or rely on downstream
        functions (``compute_mva``, ``im_profile_from_simm``) to reject an
        inconsistent profile when it is used.

        Parameters
        ----------
        times : list[float]
            Time points in years; strictly increasing and positive.
        im_values : list[float]
            Expected IM at each time point; non-negative and finite.

        Raises
        ------
        (None)
            Construction never raises; values are stored unchecked and
            validated only by ``validate()`` or downstream consumers.

        Examples
        --------
        >>> ImProfile([1.0, 2.0], [100.0, 50.0]).times
        [1.0, 2.0]
        """
        ...

    @staticmethod
    def from_json(json: str) -> ImProfile:
        """
        Parse this object from a JSON object or JSON string.

        Parameters
        ----------
        json : str
            JSON representation produced by ``to_json``.

        Returns
        -------
        ImProfile
            Parsed IM profile.

        Raises
        ------
        ValueError
            If the payload is not valid JSON or does not match the schema.

        Examples
        --------
        >>> ImProfile.from_json(ImProfile([1.0], [1.0]).to_json()).times
        [1.0]
        """
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
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> isinstance(ImProfile([1.0], [1.0]).to_json(), str)
        True
        """
        ...

    def validate(self) -> None:
        """
        Validate internal consistency.

        Raises
        ------
        ValueError
            If the profile is empty, lengths differ, times are not
            strictly increasing and positive, or IM values are
            negative/non-finite.

        Examples
        --------
        >>> ImProfile([1.0], [1.0]).validate()
        """
        ...

    @property
    def times(self) -> list[float]:
        """
        Exposure or IM observation times in years from the valuation date.

        Returns
        -------
        list[float]
            Strictly increasing, positive time points.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ImProfile([1.0, 2.0], [1.0, 2.0]).times
        [1.0, 2.0]
        """
        ...

    @property
    def im_values(self) -> list[float]:
        """
        Expected IM at each time point.

        Returns
        -------
        list[float]
            Non-negative IM values in the profile's aggregation currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ImProfile([1.0], [100.0]).im_values
        [100.0]
        """
        ...

    def __len__(self) -> int:
        """
        Number of time points.

        Returns
        -------
        int
            Length of ``times`` (and ``im_values``).

        Examples
        --------
        >>> len(ImProfile([1.0, 2.0], [1.0, 2.0]))
        2
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a pandas ``DataFrame`` with time (years) as index.

        Returns
        -------
        pandas.DataFrame
            Single column ``im``, indexed by time in years.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.

        Examples
        --------
        >>> df = ImProfile([1.0, 2.0], [100.0, 50.0]).to_dataframe()
        >>> list(df.columns)
        ['im']
        """
        ...

    def __repr__(self) -> str: ...

class MvaResult:
    """
    Result of an MVA computation.

    All monetary quantities are ``float`` in the IM profile's currency.
    Returned by :func:`compute_mva`.

    Parameters
    ----------
    (Returned by ``compute_mva``; not directly instantiated.)

    Returns
    -------
    MvaResult
        MVA value, average IM, and echoed IM profile.

    Examples
    --------
    >>> result = MvaResult.from_json('{"mva":1.0,"average_im":100.0,"im_profile":[[1.0,100.0]]}')
    >>> (result.mva, result.average_im)
    (1.0, 100.0)
    """

    @staticmethod
    def from_json(json: str) -> MvaResult:
        """
        Parse this object from a JSON object or JSON string.

        Parameters
        ----------
        json : str
            JSON representation produced by ``to_json``.

        Returns
        -------
        MvaResult
            Parsed MVA result.

        Raises
        ------
        ValueError
            If the payload is not valid JSON or does not match the schema.

        Examples
        --------
        >>> result = MvaResult.from_json('{"mva":1.0,"average_im":100.0,"im_profile":[[1.0,100.0]]}')
        >>> result.im_profile
        [(1.0, 100.0)]
        """
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
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def mva(self) -> float:
        """
        MVA (positive = lifetime funding cost of posting IM).

        Returns
        -------
        float
            MVA amount in the IM profile's currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def average_im(self) -> float:
        """
        Time-weighted average IM over the profile horizon.

        Returns
        -------
        float
            ``(1/T) * integral_0^T IM(t) dt`` under the same trapezoid
            convention as ``mva``, in the IM profile's currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def im_profile(self) -> list[tuple[float, float]]:
        """
        IM profile used, as ``(time, value)`` tuples.

        Returns
        -------
        list[tuple[float, float]]
            ``(time_years, im_value)`` pairs echoing the input profile.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the IM profile as a pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Single column ``im``, indexed by time in years.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str: ...

class MarginUtilization:
    """
    Margin utilization result (ratio of posted to required margin).

    Parameters
    ----------
    posted_amount : float
        Margin amount actually posted, in the CSA currency.
    required_amount : float
        Margin amount required by the CSA calculation, in the CSA currency.
    currency : str
        ISO currency code (both amounts use this currency).

    Returns
    -------
    MarginUtilization
        Utilization metrics.

    Raises
    ------
    ValueError
        Invalid currency code.

    Examples
    --------
    >>> u = MarginUtilization(100.0, 100.0, "USD")
    >>> u.is_adequate()
    True
    """

    def __init__(
        self,
        posted_amount: float,
        required_amount: float,
        currency: str,
    ) -> None:
        """
        Compare posted and required margin in one reporting currency.

        Parameters
        ----------
        posted_amount : float
            Margin already posted, in ``currency`` units.
        required_amount : float
            Margin requirement, in the same ``currency`` units.
        currency : str
            ISO-4217 code shared by both amounts.

        Raises
        ------
        ValueError
            If ``currency`` is unrecognized, or either amount is non-finite or
            outside the representable monetary range.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MarginUtilization:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``MarginUtilization``.

        Returns
        -------
        MarginUtilization
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = MarginUtilization(100.0, 80.0, "USD")
        >>> MarginUtilization.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(MarginUtilization(100.0, 80.0, "USD").to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def posted(self) -> float:
        """
        Margin amount actually posted, in the CSA currency.

        Returns
        -------
        float
            Posted amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginUtilization(10.0, 20.0, "USD").posted
        10.0
        """
        ...

    @property
    def required(self) -> float:
        """
        Margin amount required by the CSA calculation, in the CSA currency.

        Returns
        -------
        float
            Required amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginUtilization(10.0, 20.0, "USD").required
        20.0
        """
        ...

    @property
    def ratio(self) -> float:
        """
        Utilization ratio (posted / required).

        Returns
        -------
        float
            Ratio.

            Utilization ratio (posted / required).

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginUtilization(50.0, 100.0, "EUR").ratio
        0.5
        """
        ...

    def is_adequate(self) -> bool:
        """
        Whether margin is adequate (ratio >= 1.0).

        Returns
        -------
        bool
            Adequacy flag.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.

        Examples
        --------
        >>> MarginUtilization(100.0, 100.0, "USD").is_adequate()
        True
        """
        ...

    def shortfall(self) -> float:
        """
        Shortfall amount (if any).

        Returns
        -------
        float
            Shortfall in currency units.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> MarginUtilization(0.0, 100.0, "USD").shortfall() >= 0
        True
        """
        ...

    def __repr__(self) -> str: ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the result as a single-row pandas ``DataFrame``.

        Columns: ``posted``, ``required``, ``ratio``, ``shortfall``,
        ``is_adequate``, ``currency``.

        ``posted``, ``required`` and ``shortfall`` are floats in ``currency``.
        ``ratio`` is ``posted / required`` as a decimal fraction (``1.0`` =
        fully covered); it is ``inf`` when nothing is required but margin is
        posted, and ``1.0`` when neither is.

        Returns
        -------
        pd.DataFrame
            One row describing margin utilization.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class ExcessCollateral:
    """
    Excess collateral result.

    Parameters
    ----------
    collateral_value : float
        Collateral mark.
    required_value : float
        Required collateral.
    currency : str
        ISO currency code.

    Returns
    -------
    ExcessCollateral
        Excess or shortfall view.

    Raises
    ------
    ValueError
        Invalid currency.

    Examples
    --------
    >>> ExcessCollateral(120.0, 100.0, "USD").has_excess()
    True
    """

    def __init__(
        self,
        collateral_value: float,
        required_value: float,
        currency: str,
    ) -> None:
        """
        Compare collateral value with the required collateral amount.

        Parameters
        ----------
        collateral_value : float
            Current collateral mark, in ``currency`` units.
        required_value : float
            Required collateral amount in the same currency.
        currency : str
            ISO-4217 code shared by both values.

        Raises
        ------
        ValueError
            If ``currency`` is unrecognized, or either amount is non-finite or
            outside the representable monetary range.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ExcessCollateral:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``ExcessCollateral``.

        Returns
        -------
        ExcessCollateral
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = ExcessCollateral(120.0, 100.0, "USD")
        >>> ExcessCollateral.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(ExcessCollateral(120.0, 100.0, "USD").to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def collateral_value(self) -> float:
        """
        Market value of posted collateral in the CSA currency.

        Returns
        -------
        float
            Collateral mark.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExcessCollateral(10.0, 5.0, "USD").collateral_value
        10.0
        """
        ...

    @property
    def required_value(self) -> float:
        """
        Required collateral amount in the same currency as the mark.

        Returns
        -------
        float
            Requirement.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExcessCollateral(10.0, 5.0, "USD").required_value
        5.0
        """
        ...

    @property
    def excess(self) -> float:
        """
        Excess amount (positive) or shortfall (negative).

        Returns
        -------
        float
            Net excess.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExcessCollateral(10.0, 5.0, "USD").excess > 0
        True
        """
        ...

    def has_excess(self) -> bool:
        """
        Whether there is excess collateral.

        Returns
        -------
        bool
            True if excess > 0.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.

        Examples
        --------
        >>> ExcessCollateral(2.0, 1.0, "USD").has_excess()
        True
        """
        ...

    def has_shortfall(self) -> bool:
        """
        Whether there is a shortfall.

        Returns
        -------
        bool
            True if under-collateralized.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.

        Examples
        --------
        >>> ExcessCollateral(1.0, 2.0, "USD").has_shortfall()
        True
        """
        ...

    def excess_percentage(self) -> float:
        """
        Excess as a percentage of required.

        Returns
        -------
        float
            Fractional excess vs required.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> isinstance(ExcessCollateral(110.0, 100.0, "USD").excess_percentage(), float)
        True
        """
        ...

    def __repr__(self) -> str: ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the result as a single-row pandas ``DataFrame``.

        Columns: ``collateral_value``, ``required_value``, ``excess``,
        ``excess_percentage``, ``has_excess``, ``has_shortfall``,
        ``currency``.

        The three amount columns are floats in ``currency``; ``excess`` is
        ``collateral_value - required_value`` and is negative on a shortfall.
        ``excess_percentage`` is a decimal fraction of ``required_value``
        (``0.1`` = 10% over-collateralised).

        Returns
        -------
        pd.DataFrame
            One row describing the collateral position.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class MarginFundingCost:
    """
    Margin funding cost result.

    Parameters
    ----------
    margin_posted : float
        Margin amount actually posted, in the CSA currency.
    funding_rate : float
        Funding rate (annualized).
    collateral_rate : float
        Rate earned on posted collateral, as a decimal.
    currency : str
        ISO currency code.

    Returns
    -------
    MarginFundingCost
        Annual and periodic funding cost view.

    Raises
    ------
    ValueError
        Invalid currency.

    Examples
    --------
    >>> m = MarginFundingCost(1e6, 0.05, 0.01, "USD")
    >>> m.spread() == 0.04
    True
    """

    def __init__(
        self,
        margin_posted: float,
        funding_rate: float,
        collateral_rate: float,
        currency: str,
    ) -> None:
        """
        Describe the annual funding spread and cost of posted margin.

        Parameters
        ----------
        margin_posted : float
            Posted margin principal, in ``currency`` units.
        funding_rate : float
            Annual funding rate as a decimal, such as ``0.05`` for 5%.
        collateral_rate : float
            Annual collateral remuneration rate as a decimal.
        currency : str
            ISO-4217 code for the margin and calculated costs.

        Raises
        ------
        ValueError
            If ``currency`` is unrecognized, or ``margin_posted`` is non-finite
            or outside the representable monetary range.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MarginFundingCost:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``MarginFundingCost``.

        Returns
        -------
        MarginFundingCost
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = MarginFundingCost(1e6, 0.05, 0.02, "USD")
        >>> MarginFundingCost.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(MarginFundingCost(1e6, 0.05, 0.02, "USD").to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def margin_posted(self) -> float:
        """
        Margin amount actually posted, in the CSA currency.

        Returns
        -------
        float
            Margin posted.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginFundingCost(1.0, 0.1, 0.0, "USD").margin_posted
        1.0
        """
        ...

    @property
    def funding_rate(self) -> float:
        """
        Funding rate (annualized).

        Returns
        -------
        float
            Funding rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginFundingCost(1.0, 0.06, 0.02, "USD").funding_rate
        0.06
        """
        ...

    @property
    def collateral_rate(self) -> float:
        """
        Rate earned on posted collateral, as a decimal.

        Returns
        -------
        float
            Collateral rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginFundingCost(1.0, 0.06, 0.02, "USD").collateral_rate
        0.02
        """
        ...

    @property
    def annual_cost(self) -> float:
        """
        Annualized funding cost.

        Returns
        -------
        float
            Annual cost amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginFundingCost(1e6, 0.05, 0.0, "USD").annual_cost > 0
        True
        """
        ...

    def spread(self) -> float:
        """
        Funding spread (funding rate − collateral rate).

        Returns
        -------
        float
            Net spread.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> round(MarginFundingCost(0.0, 0.05, 0.02, "USD").spread(), 2)
        0.03
        """
        ...

    def cost_for_period(self, year_fraction: float) -> float:
        """
        Cost for a specific period.

        Parameters
        ----------
        year_fraction : float
            Length of period in years.

        Returns
        -------
        float
            Cost over the period.

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.

        Examples
        --------
        >>> MarginFundingCost(1e6, 0.04, 0.0, "USD").cost_for_period(0.5) >= 0
        True
        """
        ...

    def __repr__(self) -> str: ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the result as a single-row pandas ``DataFrame``.

        Columns: ``margin_posted``, ``funding_rate``, ``collateral_rate``,
        ``spread``, ``annual_cost``, ``currency``.

        ``margin_posted`` and ``annual_cost`` are floats in ``currency``; the
        three rate columns are annualized decimal fractions (``0.03`` = 3%).
        ``annual_cost`` is ``margin_posted * spread``, so a collateral rate
        above the funding rate makes it negative (a funding benefit).

        Returns
        -------
        pd.DataFrame
            One row describing the funding cost.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class Haircut01:
    """
    Haircut sensitivity: PV change for +1bp haircut change.

    Parameters
    ----------
    collateral_value : float
        Collateral mark.
    current_haircut : float
        Current haircut as decimal.
    currency : str
        ISO currency code.

    Returns
    -------
    Haircut01
        Sensitivity metrics.

    Raises
    ------
    ValueError
        Invalid currency.

    Examples
    --------
    >>> h = Haircut01(1e6, 0.05, "USD")
    >>> isinstance(h.pv_change, float)
    True
    """

    def __init__(
        self,
        collateral_value: float,
        current_haircut: float,
        currency: str,
    ) -> None:
        """
        Measure collateral-value sensitivity to a one-basis-point haircut increase.

        Parameters
        ----------
        collateral_value : float
            Pre-haircut collateral mark, in ``currency`` units.
        current_haircut : float
            Current haircut as a decimal fraction, such as ``0.05`` for 5%.
        currency : str
            ISO-4217 code for the collateral value and PV change.

        Raises
        ------
        ValueError
            If ``currency`` is unrecognized, or ``collateral_value`` is
            non-finite or outside the representable monetary range.
        """
        ...

    @staticmethod
    def from_json(json: str) -> Haircut01:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``Haircut01``.

        Returns
        -------
        Haircut01
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = Haircut01(1e6, 0.05, "USD")
        >>> Haircut01.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(Haircut01(1e6, 0.05, "USD").to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def collateral_value(self) -> float:
        """
        Market value of posted collateral in the CSA currency.

        Returns
        -------
        float
            Collateral mark.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> Haircut01(100.0, 0.1, "USD").collateral_value
        100.0
        """
        ...

    @property
    def current_haircut(self) -> float:
        """
        Current haircut (decimal).

        Returns
        -------
        float
            Haircut.

            Current haircut (decimal).

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> Haircut01(100.0, 0.1, "USD").current_haircut
        0.1
        """
        ...

    @property
    def pv_change(self) -> float:
        """
        PV change for +1bp haircut.

        Returns
        -------
        float
            Sensitivity amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> isinstance(Haircut01(1e6, 0.05, "USD").pv_change, float)
        True
        """
        ...

    def haircut_bp(self) -> float:
        """
        Current haircut in basis points.

        Returns
        -------
        float
            Haircut in bp.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> Haircut01(1.0, 0.01, "USD").haircut_bp()
        100.0
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the result as a single-row pandas ``DataFrame``.

        Columns: ``collateral_value``, ``current_haircut``, ``haircut_bp``,
        ``pv_change``, ``currency``. ``collateral_value`` and ``pv_change``
        are floats in ``currency``; ``current_haircut`` is a decimal fraction
        and ``haircut_bp`` the same haircut in basis points.

        Returns
        -------
        pd.DataFrame
            One row describing the haircut sensitivity.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.

        Examples
        --------
        >>> float(Haircut01(1.0, 0.01, "USD").to_dataframe()["haircut_bp"].iloc[0])
        100.0
        """
        ...

    def __repr__(self) -> str: ...

class FrtbSensitivities:
    """
    FRTB sensitivity portfolio for the Sensitivity-Based Approach.

    Build up delta / vega / curvature / DRC / RRAO inputs with the ``add_*``
    methods (or ``from_dataframe``), then pass to :func:`frtb_sba_charge` or
    ``FrtbSbaEngine.calculate`` to compute the capital charge under one or
    more correlation scenarios per BCBS d457.

    Units: GIRR deltas are base-currency P&L per **1 percentage point** of
    curve shift (``100 x DV01``); CSR deltas are base-currency P&L per 1 percentage
    point of spread; equity, commodity and FX deltas are base-currency P&L per
    1 percentage point of the underlying; vegas are base-currency P&L per unit
    implied-volatility move; curvature pairs are the up/down shocked P&L
    positions; DRC amounts are signed JTD notionals before LGD; RRAO amounts
    are gross notionals. Bucket numbers are 1-based FRTB buckets.

    Parameters
    ----------
    base_currency : str, default "USD"
        Reporting / base currency ISO code.

    Examples
    --------
    >>> sens = FrtbSensitivities("USD")
    >>> sens.add_girr_delta("5Y", 100_000.0)
    """

    def __init__(self, base_currency: str = "USD") -> None:
        """
        Create an empty FRTB sensitivity set in one reporting currency.

        Parameters
        ----------
        base_currency : str, default "USD"
            Recognized ISO-4217 currency code used for all SBA sensitivities
            and capital amounts.

        Raises
        ------
        ValueError
            If ``base_currency`` is not a recognized ISO currency code.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FrtbSensitivities:
        """
        Construct from a JSON serialization.

        Parameters
        ----------
        json : str
            JSON string produced by ``to_json``.

        Returns
        -------
        FrtbSensitivities
            Sensitivity set populated from the JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized
            ``FrtbSensitivities`` schema.

        Examples
        --------
        >>> from finstack_quant.margin import FrtbSensitivities
        >>> original = FrtbSensitivities("USD")
        >>> original.add_girr_delta("5Y", 100_000.0)
        >>> restored = FrtbSensitivities.from_json(original.to_json())
        >>> restored.base_currency
        'USD'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a JSON string.

        Returns
        -------
        str
            JSON serialization of the sensitivity portfolio.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @staticmethod
    def from_dataframe(frame: pd.DataFrame, base_currency: str = "USD") -> FrtbSensitivities:
        """
        Bulk-load sensitivities from the long-format frame ``to_dataframe``
        emits.

        Parameters
        ----------
        frame : pd.DataFrame
            Columns ``risk_class``, ``kind``, ``issuer``, ``bucket``,
            ``tenor``, ``amount`` encoded as ``to_dataframe`` documents.
            ``curvature_up`` / ``curvature_down`` rows are recombined into
            pairs; ``rrao`` rows carry ``exotic_notional`` /
            ``other_notional``.
        base_currency : str, default "USD"
            Reporting currency of every ``amount``.

        Returns
        -------
        FrtbSensitivities
            Container with rows of the same key accumulated.

        Raises
        ------
        ValueError
            If a risk class or kind is unknown, a required column is missing,
            a currency is unknown, or the frame contains ``drc`` rows (they
            carry no sector/seniority/asset type — use ``add_drc_position``).
        TypeError
            If ``frame`` is not a pandas ``DataFrame``.

        Examples
        --------
        >>> original = FrtbSensitivities("USD")
        >>> original.add_girr_delta("5Y", 100_000.0)
        >>> restored = FrtbSensitivities.from_dataframe(original.to_dataframe())
        >>> restored.to_json() == original.to_json()
        True
        """
        ...

    def validate(self) -> None:
        """
        Validate labels, buckets, identifiers and amounts without pricing.

        The engines run this automatically; call it directly to check a
        container built from external data.

        Returns
        -------
        None
            Returns ``None`` when every input is valid.

        Raises
        ------
        ValueError
            Naming the first invalid field when a tenor or bucket is
            unsupported, an identifier is empty, or a value is non-finite.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_girr_delta("5Y", 100_000.0)
        >>> sens.validate()
        """
        ...

    def add_girr_delta(self, tenor: str, amount: float, currency: str | None = None) -> None:
        """
        Add a GIRR delta sensitivity (currency P&L per 1 percentage-point move).

        Parameters
        ----------
        tenor : str
            GIRR tenor bucket, such as ``"5Y"``.
        amount : float
            Signed sensitivity amount per 1 percentage-point move (``100 * DV01``).
        currency : str, optional
            Currency code; defaults to the base currency.

        Raises
        ------
        ValueError
            If a supplied ``currency`` is not a recognized ISO currency code.
        """
        ...

    def add_girr_inflation_delta(self, amount: float, currency: str | None = None) -> None:
        """
        Add a GIRR inflation delta sensitivity.

        Parameters
        ----------
        amount : float
            Base-currency P&L per 1 percentage point of inflation shift.
        currency : str, optional
            Currency of the inflation curve; defaults to the base currency.
        Raises
        ------
        ValueError
            If a supplied ``currency`` is not a recognized ISO currency code.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_girr_inflation_delta(1_000.0)
        >>> sens.validate()
        """
        ...

    def add_girr_xccy_basis_delta(self, amount: float, currency: str | None = None) -> None:
        """
        Add a GIRR cross-currency basis delta sensitivity.

        Parameters
        ----------
        amount : float
            Base-currency P&L per 1 percentage point of basis shift.
        currency : str, optional
            Currency whose basis moves; defaults to the base currency.
        Raises
        ------
        ValueError
            If a supplied ``currency`` is not a recognized ISO currency code.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_girr_xccy_basis_delta(500.0, "EUR")
        >>> sens.validate()
        """
        ...

    def add_csr_nonsec_delta(self, issuer: str, bucket: int, tenor: str, basis: str, amount: float) -> None:
        """
        Add a CSR non-securitisation delta sensitivity.

        Parameters
        ----------
        issuer : str
            Issuer or reference-entity identifier.
        bucket : int
            1-based CSR non-sec bucket (MAR21.51).
        tenor : str
            Credit-spread tenor label such as ``"5Y"``.
        basis : str
            Explicit spread curve identifier (bond/CDS) or commodity delivery
            location. Equal labels identify the same basis for correlation.
        amount : float
            Base-currency P&L per percentage-point spread move (100 times CS01).
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_csr_nonsec_delta("ACME", 3, "5Y", "basis", 4_000.0)
        >>> sens.validate()
        """
        ...

    def add_csr_nonsec_vega(self, issuer: str, bucket: int, maturity: str, amount: float) -> None:
        """
        Add a CSR non-securitisation vega sensitivity.

        Parameters
        ----------
        issuer : str
            Issuer or reference-entity identifier.
        bucket : int
            1-based CSR non-sec bucket (MAR21.51).
        maturity : str
            Option maturity label such as ``"1Y"``.
        amount : float
            Base-currency P&L as volatility-scaled vega (sigma times dV/dsigma).
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_csr_nonsec_vega("ACME", 3, "1Y", 400.0)
        >>> sens.validate()
        """
        ...

    def add_csr_nonsec_curvature(self, issuer: str, bucket: int, cvr_up: float, cvr_down: float) -> None:
        """
        Add a CSR non-securitisation curvature pair.

        Parameters
        ----------
        issuer : str
            Issuer or reference-entity identifier.
        bucket : int
            1-based CSR non-sec bucket (MAR21.51).
        cvr_up : float
            Curvature risk position under the upward spread shock, in base
            currency.
        cvr_down : float
            Curvature risk position under the downward spread shock, in base
            currency.
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_csr_nonsec_curvature("ACME", 3, 50.0, -40.0)
        >>> sens.validate()
        """
        ...

    def add_csr_sec_ctp_delta(self, tranche: str, bucket: int, tenor: str, basis: str, amount: float) -> None:
        """
        Add a CSR securitisation (correlation trading portfolio) delta sensitivity.

        Parameters
        ----------
        tranche : str
            Tranche or index identifier.
        bucket : int
            1-based CSR sec-CTP bucket (MAR21.59).
        tenor : str
            Credit-spread tenor label such as ``"5Y"``.
        basis : str
            Explicit spread curve identifier (bond/CDS) or commodity delivery
            location. Equal labels identify the same basis for correlation.
        amount : float
            Base-currency P&L per percentage-point spread move (100 times CS01).
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_csr_sec_ctp_delta("CDX-T", 1, "5Y", "basis", 1_000.0)
        >>> sens.validate()
        """
        ...

    def add_csr_sec_ctp_vega(self, tranche: str, bucket: int, maturity: str, amount: float) -> None:
        """
        Add a CSR securitisation (CTP) vega sensitivity.

        Parameters
        ----------
        tranche : str
            Tranche or index identifier.
        bucket : int
            1-based CSR sec-CTP bucket (MAR21.59).
        maturity : str
            Option maturity label such as ``"1Y"``.
        amount : float
            Base-currency P&L as volatility-scaled vega (sigma times dV/dsigma).
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_csr_sec_ctp_vega("CDX-T", 1, "1Y", 100.0)
        >>> sens.validate()
        """
        ...

    def add_csr_sec_ctp_curvature(self, tranche: str, bucket: int, cvr_up: float, cvr_down: float) -> None:
        """
        Add a CSR securitisation (CTP) curvature pair.

        Parameters
        ----------
        tranche : str
            Tranche or index identifier.
        bucket : int
            1-based CSR sec-CTP bucket (MAR21.59).
        cvr_up : float
            Curvature risk position under the upward spread shock.
        cvr_down : float
            Curvature risk position under the downward spread shock.
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_csr_sec_ctp_curvature("CDX-T", 1, 10.0, -8.0)
        >>> sens.validate()
        """
        ...

    def add_csr_sec_nonctp_delta(self, tranche: str, bucket: int, tenor: str, basis: str, amount: float) -> None:
        """
        Add a CSR securitisation (non-CTP) delta sensitivity.

        Parameters
        ----------
        tranche : str
            Tranche identifier.
        bucket : int
            1-based CSR sec non-CTP bucket (MAR21.64).
        tenor : str
            Credit-spread tenor label such as ``"5Y"``.
        basis : str
            Explicit spread curve identifier (bond/CDS) or commodity delivery
            location. Equal labels identify the same basis for correlation.
        amount : float
            Base-currency P&L per percentage-point spread move (100 times CS01).
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_csr_sec_nonctp_delta("ABS-1", 1, "5Y", "basis", 1_000.0)
        >>> sens.validate()
        """
        ...

    def add_csr_sec_nonctp_vega(self, tranche: str, bucket: int, maturity: str, amount: float) -> None:
        """
        Add a CSR securitisation (non-CTP) vega sensitivity.

        Parameters
        ----------
        tranche : str
            Tranche identifier.
        bucket : int
            1-based CSR sec non-CTP bucket (MAR21.64).
        maturity : str
            Option maturity label such as ``"1Y"``.
        amount : float
            Base-currency P&L as volatility-scaled vega (sigma times dV/dsigma).
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_csr_sec_nonctp_vega("ABS-1", 1, "1Y", 100.0)
        >>> sens.validate()
        """
        ...

    def add_csr_sec_nonctp_curvature(self, tranche: str, bucket: int, cvr_up: float, cvr_down: float) -> None:
        """
        Add a CSR securitisation (non-CTP) curvature pair.

        Parameters
        ----------
        tranche : str
            Tranche identifier.
        bucket : int
            1-based CSR sec non-CTP bucket (MAR21.64).
        cvr_up : float
            Curvature risk position under the upward spread shock.
        cvr_down : float
            Curvature risk position under the downward spread shock.
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_csr_sec_nonctp_curvature("ABS-1", 1, 10.0, -8.0)
        >>> sens.validate()
        """
        ...

    def add_equity_delta(self, underlier: str, bucket: int, amount: float) -> None:
        """
        Add an equity delta sensitivity.

        Parameters
        ----------
        underlier : str
            Equity underlier or index identifier.
        bucket : int
            1-based equity bucket (MAR21.72).
        amount : float
            Base-currency P&L per 1 percentage point move in the underlier.
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_equity_delta("ACME", 1, 12_000.0)
        >>> sens.validate()
        """
        ...

    def add_equity_repo_delta(self, underlier: str, bucket: int, amount: float) -> None:
        """
        Add an equity repo-rate delta sensitivity.

        Parameters
        ----------
        underlier : str
            Equity underlier or index identifier.
        bucket : int
            1-based equity bucket (MAR21.72).
        amount : float
            Base-currency P&L per 1 percentage point parallel repo-rate shift.
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_equity_repo_delta("ACME", 1, 12_000.0)
        >>> sens.validate()
        """
        ...

    def add_fx_delta(self, ccy1: str, ccy2: str, amount: float) -> None:
        """
        Add an FX delta sensitivity for the pair (ccy1, ccy2).

        Parameters
        ----------
        ccy1 : str
            First currency in the FX pair.
        ccy2 : str
            Second currency in the FX pair.
        amount : float
            Base-currency P&L per 1 percentage point move in the exchange
            rate.
        Raises
        ------
        ValueError
            If ``ccy1`` or ``ccy2`` is not a recognized ISO currency code.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_fx_delta("EUR", "USD", 9_000.0)
        >>> sens.validate()
        """
        ...

    def add_commodity_delta(self, name: str, bucket: int, tenor: str, basis: str, amount: float) -> None:
        """
        Add a commodity delta sensitivity.

        Parameters
        ----------
        name : str
            Commodity identifier.
        bucket : int
            1-based commodity bucket (MAR21.82).
        tenor : str
            Commodity tenor label such as ``"1Y"``.
        basis : str
            Explicit spread curve identifier (bond/CDS) or commodity delivery
            location. Equal labels identify the same basis for correlation.
        amount : float
            Base-currency P&L per 1 percentage point move in the commodity
            price.
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_commodity_delta("WTI", 2, "1Y", "basis", 3_000.0)
        >>> sens.validate()
        """
        ...

    def add_commodity_vega(self, name: str, bucket: int, maturity: str, amount: float) -> None:
        """
        Add a commodity vega sensitivity.

        Parameters
        ----------
        name : str
            Commodity identifier.
        bucket : int
            1-based commodity bucket (MAR21.82).
        maturity : str
            Option maturity label such as ``"1Y"``.
        amount : float
            Base-currency P&L as volatility-scaled vega (sigma times dV/dsigma).
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_commodity_vega("WTI", 2, "1Y", 300.0)
        >>> sens.validate()
        """
        ...

    def add_commodity_curvature(self, name: str, bucket: int, cvr_up: float, cvr_down: float) -> None:
        """
        Add a commodity curvature pair.

        Parameters
        ----------
        name : str
            Commodity identifier.
        bucket : int
            1-based commodity bucket (MAR21.82).
        cvr_up : float
            Curvature risk position under the upward price shock.
        cvr_down : float
            Curvature risk position under the downward price shock.
        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_commodity_curvature("WTI", 2, 30.0, -20.0)
        >>> sens.validate()
        """
        ...

    def add_girr_vega(
        self,
        option_maturity: str,
        underlying_tenor: str,
        amount: float,
        currency: str | None = None,
    ) -> None:
        """
        Add a GIRR vega sensitivity.

        Parameters
        ----------
        option_maturity : str
            Option maturity label such as ``"1Y"``.
        underlying_tenor : str
            Underlying swap tenor label such as ``"5Y"``.
        amount : float
            Base-currency P&L as volatility-scaled vega (sigma times dV/dsigma).
        currency : str, optional
            Currency code; defaults to the base currency.

        Raises
        ------
        ValueError
            If a supplied ``currency`` is not a recognized ISO currency code.
        """
        ...

    def add_equity_vega(self, underlier: str, bucket: int, maturity: str, amount: float) -> None:
        """
        Add an equity vega sensitivity.

        Parameters
        ----------
        underlier : str
            Equity underlier or index identifier.
        bucket : int
            1-based equity bucket (MAR21.72).
        maturity : str
            Option maturity label such as ``"1Y"``.
        amount : float
            Base-currency P&L as volatility-scaled vega (sigma times dV/dsigma).

        Notes
        -----
        This method does not raise; it updates stored state in place.
        """
        ...

    def add_fx_vega(self, ccy1: str, ccy2: str, maturity: str, amount: float) -> None:
        """
        Add an FX vega sensitivity.

        Parameters
        ----------
        ccy1 : str
            First currency in the FX pair.
        ccy2 : str
            Second currency in the FX pair.
        maturity : str
            Option maturity label such as ``"1Y"``.
        amount : float
            Base-currency P&L as volatility-scaled vega (sigma times dV/dsigma).

        Raises
        ------
        ValueError
            If ``ccy1`` or ``ccy2`` is not a recognized ISO currency code.
        """
        ...

    def add_girr_curvature(self, cvr_up: float, cvr_down: float, currency: str | None = None) -> None:
        """
        Add a GIRR curvature sensitivity.

        Parameters
        ----------
        cvr_up : float
            Curvature risk position under the upward rate shock, in base
            currency.
        cvr_down : float
            Curvature risk position under the downward rate shock, in base
            currency.
        currency : str, optional
            Currency code; defaults to the base currency.

        Raises
        ------
        ValueError
            If a supplied ``currency`` is not a recognized ISO currency code.
        """
        ...

    def add_equity_curvature(self, underlier: str, bucket: int, cvr_up: float, cvr_down: float) -> None:
        """
        Add an equity curvature sensitivity.

        Parameters
        ----------
        underlier : str
            Equity underlier or index identifier.
        bucket : int
            1-based equity bucket (MAR21.72).
        cvr_up : float
            Curvature risk position under the upward price shock.
        cvr_down : float
            Curvature risk position under the downward price shock.

        Notes
        -----
        This method does not raise; it updates stored state in place.
        """
        ...

    def add_fx_curvature(self, ccy1: str, ccy2: str, cvr_up: float, cvr_down: float) -> None:
        """
        Add an FX curvature sensitivity.

        Parameters
        ----------
        ccy1 : str
            First currency in the FX pair.
        ccy2 : str
            Second currency in the FX pair.
        cvr_up : float
            Curvature risk position under the upward FX shock.
        cvr_down : float
            Curvature risk position under the downward FX shock.

        Raises
        ------
        ValueError
            If ``ccy1`` or ``ccy2`` is not a recognized ISO currency code.
        """
        ...

    def add_drc_position(
        self,
        issuer: str,
        jtd_amount: float,
        rating_bucket: int,
        sector: str,
        seniority: str,
        asset_type: str,
        maturity_years: float,
        pnl_adjustment: float = 0.0,
    ) -> None:
        """
        Add a Default Risk Charge position.

        Parameters
        ----------
        issuer : str
            Issuer identifier; long and short JTD net per issuer at charge
            time.
        jtd_amount : float
            Signed jump-to-default **notional** in base currency (positive =
            long, negative = short), before the seniority LGD.
        rating_bucket : int
            Credit-rating bucket, 1 (AAA) to 9 (defaulted) per MAR22.24.
        sector : str
            ``"corporate"``, ``"sovereign"`` or ``"local_government"``.
        seniority : str
            ``"senior_unsecured"``, ``"subordinated"``, ``"equity"`` or
            ``"covered_bond"`` (selects the LGD).
        asset_type : str
            ``"corporate"``, ``"sovereign"``, ``"local_government"`` or
            ``"equity"``.
        maturity_years : float
            Finite nonnegative residual maturity. JTD scales by maturity
            clipped to [0.25, 1.0] years. Securitization DRC is unsupported.
        pnl_adjustment : float, default 0.0
            Mark-to-market adjustment per MAR22.9 (negative for a long
            position carrying an unrealised loss).

        Raises
        ------
        ValueError
            If ``sector``, ``seniority`` or ``asset_type`` is not one of the
            labels above.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_drc_position("ACME", 1e6, 3, "corporate", "senior_unsecured", "corporate", 1.0)
        >>> frtb_sba_charge(sens).drc > 0.0
        True
        """
        ...

    def add_rrao_position(self, instrument_id: str, notional: float, is_exotic: bool = False) -> None:
        """
        Add a Residual Risk Add-On position.

        Parameters
        ----------
        instrument_id : str
            Instrument identifier.
        notional : float
            Gross notional in base currency.
        is_exotic : bool, default False
            ``True`` for an exotic underlying (1.0% weight); ``False`` for
            other residual risk such as gap, correlation or behavioural risk
            (0.1% weight).

        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_rrao_position("EXOTIC-1", 5_000_000.0, True)
        >>> frtb_sba_charge(sens).rrao
        50000.0
        """
        ...

    @property
    def base_currency(self) -> str:
        """
        Base / reporting currency code.

        Returns
        -------
        str
            ISO currency code for FRTB reporting.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export every populated sensitivity bucket as one long-format pandas
        ``DataFrame``.

        Columns: ``risk_class``, ``bucket``, ``tenor``, ``issuer``, ``kind``,
        ``amount``. One row per populated bucket; an empty container still
        carries all six columns. Long format is used deliberately - a column
        per bucket would give a different schema for every portfolio.

        ``risk_class`` uses the same labels as the ``frtb_sba_charge``
        breakdown (``girr``, ``csr_non_sec``, ``csr_sec_ctp``,
        ``csr_sec_non_ctp``, ``equity``, ``commodity``, ``fx``), plus ``drc``
        and ``rrao`` for the two position lists.

        ``kind`` is ``delta``, ``vega``, ``curvature_up``, ``curvature_down``,
        ``inflation_delta``, ``xccy_basis_delta``, ``jtd`` (DRC notional), or
        ``exotic_notional`` / ``other_notional`` (RRAO). A curvature pair is
        split across two rows so ``amount`` stays scalar.

        ``issuer`` carries the name axis: a currency code for GIRR, a
        ``"CCY1/CCY2"`` pair for FX, an issuer, tranche, underlier, commodity
        name, or instrument id elsewhere. ``bucket`` is the FRTB bucket index
        as a **string** (``pd.to_numeric`` if you need it numeric); ``tenor``
        is the tenor or option maturity, and for GIRR vega the
        ``"{option_maturity}/{underlying_tenor}"`` pair. Both are ``None``
        where the risk class has no such axis.

        ``amount`` keeps each bucket's own convention: GIRR deltas are
        base-currency P&L per **1 percentage point** of curve shift (that is,
        ``100 x DV01``), DRC rows are signed JTD notionals before LGD, and
        RRAO rows are gross notionals. ``from_dataframe`` accepts this frame
        back (except ``drc`` rows).

        Rows are sorted by ``(risk_class, kind, issuer, bucket, tenor)`` so
        repeated exports of the same portfolio are identical.

        Returns
        -------
        pd.DataFrame
            One row per populated sensitivity bucket.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class FrtbSbaEngine:
    """
    FRTB SBA engine matching the canonical Rust API.

    Evaluates delta, vega and curvature under each configured correlation
    scenario, takes the maximum, then adds DRC and RRAO (BCBS d457).

    Parameters
    ----------
    scenarios : list[str] or None, optional
        Correlation scenarios to evaluate (``"low"``, ``"medium"``,
        ``"high"``); ``None`` evaluates all three (the regulatory default).
    risk_classes : list[str] or None, optional
        Risk classes whose delta/vega/curvature are included (``"girr"``,
        ``"csr_non_sec"``, ``"csr_sec_ctp"``, ``"csr_sec_non_ctp"``,
        ``"equity"``, ``"commodity"``, ``"fx"``); ``None`` includes all.

    Examples
    --------
    >>> from finstack_quant.margin import FrtbSbaEngine, FrtbSensitivities
    >>> sensitivities = FrtbSensitivities("USD")
    >>> sensitivities.add_girr_delta("5Y", 100_000.0)
    >>> round(FrtbSbaEngine(scenarios=["medium"]).calculate(sensitivities).total, 2)
    110000.0
    """

    def __init__(
        self,
        scenarios: list[str] | None = None,
        risk_classes: list[str] | None = None,
    ) -> None:
        """
        Select the correlation scenarios and risk classes the engine evaluates.

        Parameters
        ----------
        scenarios : list[str] or None, default None
            Lower-case scenario labels (``"low"``, ``"medium"``, ``"high"``);
            the charge is the maximum across them. ``None`` evaluates all
            three.
        risk_classes : list[str] or None, default None
            Lower-case risk-class labels to include; ``None`` includes all
            seven.

        Raises
        ------
        ValueError
            If a label is unknown or either list is empty.
        """
        ...

    @property
    def scenarios(self) -> list[str]:
        """
        Correlation scenario labels evaluated, in configured order.

        Returns
        -------
        list[str]
            Lower-case labels.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> FrtbSbaEngine().scenarios
        ['low', 'medium', 'high']
        """
        ...

    @property
    def risk_classes(self) -> list[str]:
        """
        Risk-class labels included, in configured order.

        Returns
        -------
        list[str]
            Lower-case labels.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> FrtbSbaEngine(risk_classes=["girr", "fx"]).risk_classes
        ['girr', 'fx']
        """
        ...

    def calculate(self, sensitivities: FrtbSensitivities) -> FrtbSbaResult:
        """
        Calculate the FRTB SBA charge for a sensitivity portfolio.

        Parameters
        ----------
        sensitivities : FrtbSensitivities
            Portfolio of FRTB sensitivities (delta, vega, curvature, DRC, RRAO).

        Returns
        -------
        FrtbSbaResult
            Total charge with the per-risk-class delta/vega/curvature
            breakdown, DRC, RRAO, and the per-scenario charges.

        Raises
        ------
        ValueError
            If a sensitivity has an unsupported tenor or bucket, an empty
            required identifier, or a non-finite numeric value.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_girr_delta("5Y", 100_000.0)
        >>> FrtbSbaEngine().calculate(sens).binding_scenario
        'low'
        """
        ...

    def __repr__(self) -> str: ...

class FrtbSbaResult:
    """
    FRTB SBA capital-charge result (BCBS d457).

    Returned by :func:`frtb_sba_charge` and :meth:`FrtbSbaEngine.calculate`.
    Amounts are floats in the sensitivity portfolio's base currency.

    Examples
    --------
    >>> from finstack_quant.margin import FrtbSensitivities, frtb_sba_charge
    >>> sens = FrtbSensitivities("USD")
    >>> sens.add_girr_delta("5Y", 100_000.0)
    >>> result = frtb_sba_charge(sens)
    >>> result.total > 0.0
    True
    >>> result.binding_scenario in {"low", "medium", "high"}
    True
    """

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @staticmethod
    def from_json(json: str) -> FrtbSbaResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``FrtbSbaResult``.

        Returns
        -------
        FrtbSbaResult
            The decoded result.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the ``FrtbSbaResult`` shape.

        Examples
        --------
        >>> from finstack_quant.margin import FrtbSbaResult
        >>> restored = FrtbSbaResult.from_json(result.to_json())  # doctest: +SKIP
        >>> restored.to_json() == result.to_json()  # doctest: +SKIP
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize back to the same JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``FrtbSbaResult``.

        Notes
        -----
        This method does not raise; it derives the value from stored state.
        """
        ...

    @property
    def total(self) -> float:
        """
        Total capital charge, in the portfolio's base currency.

        Returns
        -------
        float
            Total capital charge, in the portfolio's base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def drc(self) -> float:
        """
        Default Risk Charge (credit + equity jump-to-default).

        Returns
        -------
        float
            Default Risk Charge (credit + equity jump-to-default).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rrao(self) -> float:
        """
        Residual Risk Add-On.

        Returns
        -------
        float
            Residual Risk Add-On.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def binding_scenario(self) -> str:
        """
        Correlation scenario that bound: ``"low"``, ``"medium"``, or ``"high"``.

        Returns
        -------
        str
            Correlation scenario that bound: ``"low"``, ``"medium"``, or ``"high"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def delta_by_risk_class(self) -> dict[str, float]:
        """
        Delta risk charge keyed by risk-class wire label (e.g. ``"girr"``).

        Returns
        -------
        dict[str, float]
            Delta risk charge keyed by risk-class wire label (e.g. ``"girr"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def vega_by_risk_class(self) -> dict[str, float]:
        """
        Vega risk charge keyed by risk-class wire label.

        Returns
        -------
        dict[str, float]
            Vega risk charge keyed by risk-class wire label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def curvature_by_risk_class(self) -> dict[str, float]:
        """
        Curvature risk charge keyed by risk-class wire label.

        Returns
        -------
        dict[str, float]
            Curvature risk charge keyed by risk-class wire label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scenario_charges(self) -> dict[str, float]:
        """
        Delta+vega+curvature charge under each evaluated correlation scenario.

        Returns
        -------
        dict[str, float]
            Delta+vega+curvature charge under each evaluated correlation scenario.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """
        Policy metadata stamped by the computing layer.

        Returns
        -------
        dict[str, Any]
            ``numeric_mode``, ``rounding`` (the active rounding context),
            ``fx_policy_applied`` (or ``None``), ``parallel`` and
            ``timestamp``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_girr_delta("5Y", 1.0)
        >>> result = frtb_sba_charge(sens)
        >>> "numeric_mode" in result.meta
        True
        """
        ...

    def to_dataframe(self) -> Any:
        """
        Export the headline charge as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Columns ``total``, ``drc``, ``rrao`` (floats) and
            ``binding_scenario`` (string). One row.

        Notes
        -----
        This method does not raise; it derives the value from stored state.
        """
        ...

    def to_breakdown_dataframe(self) -> Any:
        """
        Export the per-risk-class breakdown as a long-format ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Columns ``component`` (``"delta"``/``"vega"``/``"curvature"``),
            ``risk_class`` and ``charge``. Components do not sum to ``total``:
            SBA aggregates them with prescribed correlations, and ``drc`` /
            ``rrao`` sit outside this frame.

        Notes
        -----
        This method does not raise; it derives the value from stored state.
        """
        ...

    def to_scenario_dataframe(self) -> pd.DataFrame:
        """
        Export the per-scenario SBA charges as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``scenario`` (``"low"``, ``"medium"``, ``"high"``),
            ``charge`` (delta+vega+curvature under that scenario, float in
            the portfolio's base currency) and ``binding`` (``True`` on the
            scenario that produced ``total``). One row per evaluated scenario
            in low/medium/high order.

        Notes
        -----
        This method does not raise; it derives the value from stored state.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_girr_delta("5Y", 100_000.0)
        >>> frtb_sba_charge(sens).to_scenario_dataframe()["scenario"].tolist()
        ['low', 'medium', 'high']
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            Return ``repr(self)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class EadResult:
    """
    SA-CCR Exposure at Default result (BCBS 279).

    Returned by :func:`saccr_ead` and :meth:`SaCcrEngine.calculate_ead`.
    ``pfe == multiplier * add_on_aggregate``; ``ead`` is ``alpha * (rc + pfe)``
    capped at the unmargined EAD for margined sets. RC and PFE remain the
    components before the cap;
    amounts are floats in the netting set's reporting currency.

    Examples
    --------
    >>> from finstack_quant.margin import NettingSetId, SaCcrEngine, SaCcrNettingSetConfig
    >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-15")
    >>> SaCcrEngine().calculate_ead(config, []).ead
    0.0
    """

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @staticmethod
    def from_json(json: str) -> EadResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``EadResult``.

        Returns
        -------
        EadResult
            The decoded result.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the ``EadResult`` shape.

        Examples
        --------
        >>> from finstack_quant.margin import EadResult
        >>> restored = EadResult.from_json(result.to_json())  # doctest: +SKIP
        >>> restored.to_json() == result.to_json()  # doctest: +SKIP
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize back to the same JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``EadResult``.

        Notes
        -----
        This method does not raise; it derives the value from stored state.
        """
        ...

    @property
    def ead(self) -> float:
        """
        Exposure at Default after the unmargined cap for margined sets.

        Returns
        -------
        float
            Exposure at Default after the unmargined cap for margined sets.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rc(self) -> float:
        """
        Replacement cost component.

        Returns
        -------
        float
            Replacement cost component.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pfe(self) -> float:
        """
        Potential future exposure: ``multiplier * add_on_aggregate``.

        Returns
        -------
        float
            Potential future exposure: ``multiplier * add_on_aggregate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def multiplier(self) -> float:
        """
        PFE multiplier recognising over-collateralization (floored at 0.05).

        Returns
        -------
        float
            PFE multiplier recognising over-collateralization (floored at 0.05).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def add_on_aggregate(self) -> float:
        """
        Aggregate add-on across asset classes, before the multiplier.

        Returns
        -------
        float
            Aggregate add-on across asset classes, before the multiplier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def alpha(self) -> float:
        """
        Alpha multiplier (1.4 per BCBS 279 unless overridden on the engine).

        Returns
        -------
        float
            Alpha multiplier (1.4 per BCBS 279 unless overridden on the engine).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def maturity_factor(self) -> float:
        """
        Maturity factor applied to the netting set.

        Returns
        -------
        float
            Maturity factor applied to the netting set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def add_on_by_asset_class(self) -> dict[str, float]:
        """
        Add-on keyed by asset-class wire label (e.g. ``"interest_rate"``).

        Returns
        -------
        dict[str, float]
            Add-on keyed by asset-class wire label (e.g. ``"interest_rate"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """
        Policy metadata stamped by the computing layer.

        Returns
        -------
        dict[str, Any]
            ``numeric_mode``, ``rounding`` (the active rounding context),
            ``fx_policy_applied`` (or ``None``), ``parallel`` and
            ``timestamp``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-15")
        >>> result = SaCcrEngine().calculate_ead(config, [])
        >>> "numeric_mode" in result.meta
        True
        """
        ...

    def to_dataframe(self) -> Any:
        """
        Export the headline exposure as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Columns ``ead``, ``rc``, ``pfe``, ``multiplier``,
            ``add_on_aggregate``, ``alpha``, ``maturity_factor``. One row.

        Notes
        -----
        This method does not raise; it derives the value from stored state.
        """
        ...

    def to_add_on_dataframe(self) -> Any:
        """
        Export the per-asset-class add-on as a pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Columns ``asset_class`` and ``add_on``, one row per asset class
            present. A netting set with no trades yields a zero-row frame that
            still carries both columns with their real dtypes.

        Notes
        -----
        This method does not raise; it derives the value from stored state.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            Return ``repr(self)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class SaCcrTrade:
    """
    A derivative trade for SA-CCR EAD computation per BCBS 279.

    Build with keyword arguments, ``from_json`` or ``from_dataframe``; all
    three validate the direction / supervisory-delta / option-type coherence
    up front. ``notional`` and ``mtm`` are in the netting set's reporting
    currency; ``direction`` is ``+1.0`` (long) or ``-1.0`` (short);
    ``supervisory_delta`` is in ``[-1, 1]`` (exactly ``±1`` for linear
    trades, the signed option delta otherwise).

    Parameters
    ----------
    trade_id : str
        Unique trade identifier.
    asset_class : str
        ``"interest_rate"``, ``"foreign_exchange"``, ``"credit"``,
        ``"equity"`` or ``"commodity"``.
    notional : float
        Reporting-currency notional before supervisory duration for IR/credit;
        CRE52 adjusted notional for other classes.
    start_date : datetime.date | str
        Trade start date (forward-start trades start after the valuation
        date).
    end_date : datetime.date | str
        Underlying end date; option expiry is supplied separately.
    underlier : str
        Underlier reference (currency pair, issuer, equity, commodity).
    hedging_set : str
        Hedging-set identifier within the asset class.
    direction : float
        ``+1.0`` long or ``-1.0`` short.
    supervisory_delta : float
        ``±1`` for linear trades; the signed option delta otherwise,
        sign-consistent with ``option_type`` (BCBS 279 ¶112).
    mtm : float
        Current mark-to-market in the reporting currency.
    is_option : bool, default False
        Whether the trade is an option.
    option_type : str | None, optional
        ``"call_long"``, ``"call_short"``, ``"put_long"`` or
        ``"put_short"``; required when ``is_option`` is ``True``.

    supervisory_category : str | None, optional
        Required for credit, equity and commodity; absent for IR/FX. Labels:
        ``credit_aa``, ``credit_a``, ``credit_bbb``, ``credit_bb``, ``credit_b``,
        ``credit_ccc``, ``credit_index_ig``, ``credit_index_hy``,
        ``equity_single_name``, ``equity_index``, ``commodity_electricity``,
        ``commodity_other``. Selects the supervisory factor and correlation.
    option_maturity_date : datetime.date | str | None, optional
        Required option expiry, at or before the underlying end date. Absent
        for linear trades. Separates option maturity from underlying duration.

    Examples
    --------
    >>> from finstack_quant.margin import SaCcrTrade
    >>> trade = SaCcrTrade(
    ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
    ... )
    >>> (trade.trade_id, trade.asset_class, trade.is_option)
    ('t1', 'interest_rate', False)
    """

    def __init__(
        self,
        trade_id: str,
        asset_class: str,
        notional: float,
        start_date: datetime.date | str,
        end_date: datetime.date | str,
        underlier: str,
        hedging_set: str,
        direction: float,
        supervisory_delta: float,
        mtm: float,
        is_option: bool = False,
        option_type: str | None = None,
        supervisory_category: str | None = None,
        option_maturity_date: datetime.date | str | None = None,
    ) -> None:
        """
        Create and validate a trade (see the class docstring for each field).

        Parameters
        ----------
        trade_id : str
            Unique trade identifier.
        asset_class : str
            Lower-case SA-CCR asset class label.
        notional : float
            Reporting-currency notional before supervisory duration for IR/credit;
        CRE52 adjusted notional for other classes.
        start_date : datetime.date | str
            Trade start date (date-like or ISO ``YYYY-MM-DD``).
        end_date : datetime.date | str
            Trade end date / maturity (date-like or ISO ``YYYY-MM-DD``).
        underlier : str
            Underlier reference.
        hedging_set : str
            Hedging-set identifier within the asset class.
        direction : float
            ``+1.0`` long or ``-1.0`` short.
        supervisory_delta : float
            Supervisory delta in ``[-1, 1]``.
        mtm : float
            Current mark-to-market in the reporting currency.
        is_option : bool, default False
            Whether the trade is an option.
        option_type : str | None, optional
            Option type label, required when ``is_option`` is ``True``.

        supervisory_category : str | None, optional
            Required for credit, equity and commodity; absent for IR/FX. Labels:
            ``credit_aa``, ``credit_a``, ``credit_bbb``, ``credit_bb``, ``credit_b``,
            ``credit_ccc``, ``credit_index_ig``, ``credit_index_hy``,
            ``equity_single_name``, ``equity_index``, ``commodity_electricity``,
            ``commodity_other``. Selects the supervisory factor and correlation.
        option_maturity_date : datetime.date | str | None, optional
            Required option expiry, at or before the underlying end date. Absent
            for linear trades. Separates option maturity from underlying duration.

        Raises
        ------
        ValueError
            If a label is unknown, a date string is not ISO 8601, or the
            trade fails the BCBS 279 coherence checks (zero direction, delta
            outside ``[-1, 1]``, linear delta not ``±1`` or disagreeing with
            direction, option delta sign inconsistent with ``option_type``).
        TypeError
            If a date is neither a string nor date-like.
        """
        ...

    @property
    def supervisory_category(self) -> str | None:
        """Supervisory factor and correlation category, absent for IR and FX.

        Returns
        -------
        str | None
            Supervisory factor and correlation category, absent for IR and FX.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    @property
    def option_maturity_date(self) -> datetime.date | None:
        """Option expiry used for the maturity factor, absent for linear trades.

        Returns
        -------
        datetime.date | None
            Option expiry used for the maturity factor, absent for linear trades.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    @staticmethod
    def from_dataframe(frame: pd.DataFrame) -> list[SaCcrTrade]:
        """
        Build one validated trade per row of a trade tape.

        Parameters
        ----------
        frame : pd.DataFrame
            Columns ``trade_id``, ``asset_class``, ``notional``,
            ``start_date``, ``end_date``, ``underlier``, ``hedging_set``,
            ``direction``, ``supervisory_delta``, ``mtm``; ``is_option`` and
            ``option_type`` are optional and default to a linear trade.
            Dates may be ISO strings or date-like values.

        Returns
        -------
        list[SaCcrTrade]
            One trade per row, in row order.

        Raises
        ------
        ValueError
            Naming the first row that is missing a column, has an unknown
            label, or fails the BCBS 279 coherence checks.
        TypeError
            If ``frame`` is not a pandas ``DataFrame``.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
        ... )
        >>> [t.trade_id for t in SaCcrTrade.from_dataframe(trade.to_dataframe())]
        ['t1']
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the trade as a single-row pandas ``DataFrame``.

        Columns are the constructor fields in order (``trade_id`` ..
        ``option_type``); dates are ISO 8601 strings and ``option_type`` is
        null for linear trades. ``from_dataframe`` accepts this frame back.

        Returns
        -------
        pd.DataFrame
            One row describing the trade.

        Raises
        ------
        ValueError
            If the trade cannot be serialized into a pandas object.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
        ... )
        >>> trade.to_dataframe().iloc[0]["end_date"]
        '2030-01-01'
        """
        ...

    @staticmethod
    def from_json(json: str) -> SaCcrTrade:
        """
        Construct from a JSON serialization.

        Parameters
        ----------
        json : str
            JSON string produced by ``to_json``.

        Returns
        -------
        SaCcrTrade
            Parsed trade.

        Raises
        ------
        ValueError
            If ``json`` is malformed, does not match the canonical schema, or
            violates direction, supervisory-delta, or option-type invariants.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.margin import SaCcrTrade
        >>> payload = {
        ...     "trade_id": "t1",
        ...     "asset_class": "interest_rate",
        ...     "notional": 1_000_000,
        ...     "start_date": "2025-01-01",
        ...     "end_date": "2030-01-01",
        ...     "underlier": "USD-SOFR",
        ...     "hedging_set": "rates",
        ...     "direction": 1.0,
        ...     "supervisory_delta": 1.0,
        ...     "mtm": 0.0,
        ...     "is_option": False,
        ...     "option_type": None,
        ... }
        >>> SaCcrTrade.from_json(json.dumps(payload)).trade_id
        't1'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a JSON string.

        Returns
        -------
        str
            JSON serialization of the trade.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def trade_id(self) -> str:
        """
        Unique trade identifier.

        Returns
        -------
        str
            Trade id string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_class(self) -> str:
        """
        SA-CCR asset-class label used to select supervisory factors.

        Returns
        -------
        str
            One of ``"interest_rate"``, ``"foreign_exchange"``,
            ``"credit"``, ``"equity"``, ``"commodity"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def notional(self) -> float:
        """
        Adjusted notional in reporting currency.

        Returns
        -------
        float
            Notional amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mtm(self) -> float:
        """
        Current mark-to-market value.

        Returns
        -------
        float
            MtM value in the reporting currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def start_date(self) -> datetime.date:
        """
        Trade start date.

        Returns
        -------
        datetime.date
            Start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
        ... )
        >>> trade.start_date
        datetime.date(2025, 1, 1)
        """
        ...

    @property
    def end_date(self) -> datetime.date:
        """
        Underlying end date; option expiry is supplied separately.

        Returns
        -------
        datetime.date
            End date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
        ... )
        >>> trade.end_date
        datetime.date(2030, 1, 1)
        """
        ...

    @property
    def underlier(self) -> str:
        """
        Underlier reference (currency pair, issuer, equity, commodity).

        Returns
        -------
        str
            Underlier identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
        ... )
        >>> trade.underlier
        'USD-SOFR'
        """
        ...

    @property
    def hedging_set(self) -> str:
        """
        Hedging-set identifier within the asset class.

        Returns
        -------
        str
            Hedging-set identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
        ... )
        >>> trade.hedging_set
        'rates'
        """
        ...

    @property
    def direction(self) -> float:
        """
        ``+1.0`` for long, ``-1.0`` for short.

        Returns
        -------
        float
            Signed direction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
        ... )
        >>> trade.direction
        1.0
        """
        ...

    @property
    def supervisory_delta(self) -> float:
        """
        Supervisory delta in ``[-1, 1]``.

        Returns
        -------
        float
            Signed supervisory delta.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
        ... )
        >>> trade.supervisory_delta
        1.0
        """
        ...

    @property
    def is_option(self) -> bool:
        """
        Whether the trade is an option.

        Returns
        -------
        bool
            ``True`` for options.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
        ... )
        >>> trade.is_option
        False
        """
        ...

    @property
    def option_type(self) -> str | None:
        """
        Option type label, or ``None`` for a linear trade.

        Returns
        -------
        str | None
            ``"call_long"``, ``"call_short"``, ``"put_long"``, ``"put_short"`` or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
        ... )
        >>> trade.option_type is None
        True
        """
        ...

    def __repr__(self) -> str: ...

class SaCcrNettingSetConfig:
    """
    SA-CCR netting-set configuration with explicit valuation date.

    Collateral terms that select the margined or unmargined RC/PFE formulas.
    All amounts are in the netting set's reporting currency; ``mpor_days`` is
    the margin period of risk in business days. Keyed by a ``NettingSetId``
    (bilateral or cleared).

    Parameters
    ----------
    (Use the ``unmargined`` or ``margined`` factories.)

    Examples
    --------
    >>> from finstack_quant.margin import NettingSetId, SaCcrNettingSetConfig
    >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-15")
    >>> config.is_margined
    False
    """

    @staticmethod
    def unmargined(
        netting_set_id: NettingSetId,
        collateral: float,
        as_of: datetime.date | str,
    ) -> SaCcrNettingSetConfig:
        """
        Create an unmargined netting-set configuration.

        Threshold, MTA and NICA are zero and ``mpor_days`` is the
        10-business-day bilateral default (only used for the reporting
        maturity factor).

        Parameters
        ----------
        netting_set_id : NettingSetId
            Bilateral (``NettingSetId.bilateral``) or cleared
            (``NettingSetId.cleared``) netting-set key.
        collateral : float
            Net collateral held (positive = bank holds collateral).
        as_of : datetime.date | str
            Valuation date for forward-start and remaining-maturity
            calculations (date-like or ISO ``YYYY-MM-DD``).

        Returns
        -------
        SaCcrNettingSetConfig
            Unmargined netting-set config.

        Raises
        ------
        ValueError
            If ``collateral`` is non-finite or a date string is not ISO 8601.
        TypeError
            If ``netting_set_id`` is not a ``NettingSetId`` or ``as_of`` is
            neither a string nor date-like.

        Examples
        --------
        >>> from finstack_quant.margin import NettingSetId, SaCcrNettingSetConfig
        >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.cleared("LCH"), 0.0, "2025-01-15")
        >>> (config.is_margined, config.netting_set_id.is_cleared)
        (False, True)
        """
        ...

    @staticmethod
    def margined(
        netting_set_id: NettingSetId,
        collateral: float,
        threshold: float,
        mta: float,
        nica: float,
        mpor_days: int,
        as_of: datetime.date | str,
    ) -> SaCcrNettingSetConfig:
        """
        Create a margined netting-set configuration.

        Parameters
        ----------
        netting_set_id : NettingSetId
            Bilateral or cleared netting-set key.
        collateral : float
            Net collateral held (positive = bank holds collateral).
        threshold : float
            CSA threshold (TH), non-negative.
        mta : float
            Minimum transfer amount, non-negative.
        nica : float
            Net independent collateral amount, signed.
        mpor_days : int
            Margin period of risk in business days; at least 10 bilateral or
            5 cleared. Supply any longer period required by liquidity,
            disputes, trade count or remargining frequency.
        as_of : datetime.date | str
            Valuation date for forward-start and remaining-maturity
            calculations.

        Returns
        -------
        SaCcrNettingSetConfig
            Margined netting-set config.

        Raises
        ------
        ValueError
            If an amount is non-finite, threshold or MTA is negative,
            ``mpor_days`` is below the applicable floor, or a date string is not ISO 8601.
        TypeError
            If ``netting_set_id`` is not a ``NettingSetId`` or ``as_of`` is
            neither a string nor date-like.

        Examples
        --------
        >>> from finstack_quant.margin import NettingSetId, SaCcrNettingSetConfig
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.bilateral("CPTY", "CSA"),
        ...     collateral=0.0,
        ...     threshold=0.0,
        ...     mta=0.0,
        ...     nica=0.0,
        ...     mpor_days=10,
        ...     as_of="2025-01-15",
        ... )
        >>> (config.is_margined, config.mpor_days)
        (True, 10)
        """
        ...

    @staticmethod
    def from_json(json: str) -> SaCcrNettingSetConfig:
        """
        Construct from a JSON serialization.

        Parameters
        ----------
        json : str
            JSON string produced by ``to_json``.

        Returns
        -------
        SaCcrNettingSetConfig
            Parsed netting-set config.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized
            ``SaCcrNettingSetConfig`` schema.

        Examples
        --------
        >>> from finstack_quant.margin import NettingSetId, SaCcrNettingSetConfig
        >>> original = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-15")
        >>> SaCcrNettingSetConfig.from_json(original.to_json()).is_margined
        False
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a JSON string.

        Returns
        -------
        str
            JSON serialization of the netting-set config.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def is_margined(self) -> bool:
        """
        Whether the netting set is margined.

        Returns
        -------
        bool
            True if subject to daily margin agreement.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def collateral(self) -> float:
        """
        Net collateral currently held.

        Returns
        -------
        float
            Collateral amount in the reporting currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_of(self) -> datetime.date:
        """
        Valuation date used for forward-start and remaining-maturity calculations.

        Returns
        -------
        datetime.date
            Valuation date the netting set is priced at.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.as_of
        datetime.date(2025, 1, 15)
        """
        ...

    @property
    def netting_set_id(self) -> NettingSetId:
        """
        Netting-set key.

        Returns
        -------
        NettingSetId
            Bilateral or cleared id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.netting_set_id.ccp_id
        'LCH'
        """
        ...

    @property
    def threshold(self) -> float:
        """
        CSA threshold (TH) in the reporting currency.

        Returns
        -------
        float
            Threshold amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.threshold
        100000.0
        """
        ...

    @property
    def mta(self) -> float:
        """
        Minimum transfer amount in the reporting currency.

        Returns
        -------
        float
            MTA amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.mta
        50000.0
        """
        ...

    @property
    def nica(self) -> float:
        """
        Net independent collateral amount in the reporting currency.

        Returns
        -------
        float
            NICA amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.nica
        0.0
        """
        ...

    @property
    def mpor_days(self) -> int:
        """
        Margin period of risk in business days.

        Returns
        -------
        int
            MPOR in business days.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.mpor_days
        5
        """
        ...

    def validate(self) -> None:
        """
        Validate the collateral and margin-agreement terms.

        Returns
        -------
        None
            Returns ``None`` when the terms are valid.

        Raises
        ------
        ValueError
            If an amount is non-finite, threshold or MTA is negative, or a
            margined set has zero MPOR.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.validate()
        """
        ...

    def __repr__(self) -> str: ...

class SaCcrEngine:
    """
    SA-CCR EAD engine matching the canonical Rust API.

    Parameters
    ----------
    alpha : float or None, optional
        Supervisory alpha factor; defaults to 1.4 when ``None``.

    Monetary values in a calculation must already use one consistent currency;
    the engine does not perform currency conversion.

    Examples
    --------
    >>> from finstack_quant.margin import NettingSetId, SaCcrEngine, SaCcrNettingSetConfig
    >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-15")
    >>> SaCcrEngine().calculate_ead(config, []).ead
    0.0
    """

    def __init__(self, alpha: float | None = None) -> None:
        """
        Configure the supervisory multiplier for SA-CCR.

        Parameters
        ----------
        alpha : float or None, default None
            Supervisory alpha multiplier; ``None`` uses the regulatory default
            of ``1.4`` and explicit values must be at least ``1.0``.

        Raises
        ------
        ValueError
            If a supplied ``alpha`` is non-finite or less than ``1.0``.
        """
        ...

    @property
    def alpha(self) -> float:
        """
        Alpha multiplier applied to ``RC + PFE``.

        Returns
        -------
        float
            1.4 unless overridden at construction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> SaCcrEngine(alpha=1.5).alpha
        1.5
        """
        ...

    def calculate_ead(self, config: SaCcrNettingSetConfig, trades: list[SaCcrTrade]) -> EadResult:
        """
        Calculate SA-CCR EAD for a netting set and trade list.

        Parameters
        ----------
        config : SaCcrNettingSetConfig
            Netting-set configuration with valuation date and collateral.
        trades : list[SaCcrTrade]
            Derivative trades in the netting set.

        Returns
        -------
        EadResult
            Exposure at default with replacement cost, PFE, the multiplier,
            the aggregate and per-asset-class add-ons, alpha, and the
            maturity factor.

        Raises
        ------
        ValueError
            If netting-set amounts or trade numeric fields are non-finite, a
            threshold or MTA is negative, a margined MPOR is zero, or a trade's
            direction and supervisory-delta fields are inconsistent.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-01")
        >>> trade = SaCcrTrade(
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
        ... )
        >>> round(SaCcrEngine().calculate_ead(config, [trade]).ead, 2)
        30982.83
        """
        ...

    def __repr__(self) -> str: ...

def im_profile_from_simm(
    calculator: SimmCalculator,
    sensitivities: SimmSensitivities,
    currency: str,
    decay: ImDecayProfile,
    time_grid: list[float],
) -> ImProfile:
    """
    Build a deterministic IM profile from current SIMM sensitivities.

    Computes ``IM(t) = SIMM(sensitivities) * decay(t)`` on ``time_grid``,
    where the base IM is the full cross-risk-class ISDA SIMM aggregate of
    ``sensitivities`` (validated first, so an unknown tenor raises).

    Parameters
    ----------
    calculator : SimmCalculator
        SIMM calculator fixing the version/registry parameters.
    sensitivities : SimmSensitivities
        Current portfolio sensitivities in SIMM buckets.
    currency : str
        Aggregation currency for the SIMM total.
    decay : ImDecayProfile
        Deterministic decay profile applied to the base IM.
    time_grid : list[float]
        Strictly increasing, positive year fractions.

    Returns
    -------
    ImProfile
        Expected IM profile ``E[IM(t)]`` on ``time_grid``.

    Raises
    ------
    ValueError
        If ``currency`` is not a known currency code, or ``sensitivities``,
        ``decay`` or ``time_grid`` fails validation.

    Examples
    --------
    >>> sens = SimmSensitivities("USD")
    >>> sens.add_ir_delta("USD", "5Y", 50_000.0)
    >>> calc = SimmCalculator("v2_6")
    >>> decay = ImDecayProfile.linear_to_maturity(4.0)
    >>> profile = im_profile_from_simm(calc, sens, "USD", decay, [1.0, 2.0, 4.0])
    >>> profile.times
    [1.0, 2.0, 4.0]
    """
    ...

def compute_mva(
    im_profile: ImProfile,
    funding_spread_curve: list[tuple[float, float]] | pd.Series,
    discount_curve: DiscountCurve,
    survival_curve: HazardCurve | None = None,
) -> MvaResult:
    """
    Compute MVA over an expected-IM profile.

    Integrates ``spread(t) * IM(t) * DF(t) * S(t)`` over the profile's time
    grid using the same midpoint/trapezoid convention as the CVA engine,
    with a ``t = 0`` bucket edge (``DF(0) = 1``, ``S(0) = 1``). IM is
    treated as flat (left-constant) before the first grid point.

    Parameters
    ----------
    im_profile : ImProfile
        Expected IM profile ``E[IM(t)]`` (from ``im_profile_from_simm`` or
        the stochastic engine's mean per-path IM).
    funding_spread_curve : list[tuple[float, float]] | pd.Series
        ``(time_years, spread_bp)`` pairs in basis points, or a ``Series`` of
        spreads in bp indexed by time in years; linearly interpolated with
        flat extrapolation, and a single point means a flat spread.
    discount_curve : DiscountCurve
        Risk-free discount curve.
    survival_curve : HazardCurve or None, optional
        Optional bank (own) hazard curve; when ``None``, survival
        probability ``S(t)`` is treated as 1 for all ``t``.

    Returns
    -------
    MvaResult
        MVA, time-weighted average IM, and the echoed IM profile.

    Raises
    ------
    ValueError
        If the profile or spread curve fails validation, or if any curve
        evaluation returns a non-finite value.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> profile = ImProfile([1.0, 2.0], [1_000_000.0, 1_000_000.0])
    >>> result = compute_mva(
    ...     profile,
    ...     [(0.0, 50.0)],
    ...     DiscountCurve.flat("USD-OIS", date(2025, 1, 1), 0.0),
    ... )
    >>> round(result.mva, 2)
    10000.0
    """
    ...

def frtb_sba_charge(sensitivities: FrtbSensitivities, correlation_scenario: str | None = None) -> FrtbSbaResult:
    """
    Compute the FRTB SBA capital charge.

    Parameters
    ----------
    sensitivities : FrtbSensitivities
        Portfolio of FRTB sensitivities (delta, vega, curvature, DRC, RRAO).
    correlation_scenario : str or None, optional
        If provided (``"low"``, ``"medium"``, or ``"high"``), only that scenario
        is evaluated. Otherwise all three are run and the max-binding one is
        reported per BCBS d457.

    Returns
    -------
    FrtbSbaResult
        Total charge with the per-risk-class delta/vega/curvature breakdown,
        DRC, RRAO, and the per-scenario charges with the binding one named.

    Raises
    ------
    ValueError
        If ``correlation_scenario`` is unknown, or a sensitivity has an
        unsupported tenor or bucket, an empty required identifier, or a
        non-finite numeric value.

    Examples
    --------
    >>> sens = FrtbSensitivities("USD")
    >>> sens.add_girr_delta("5Y", 100_000.0)
    >>> result = frtb_sba_charge(sens)
    >>> result.total > 0.0
    True
    """
    ...

def saccr_ead(
    trades: list[SaCcrTrade],
    config: SaCcrNettingSetConfig,
    alpha: float | None = None,
) -> EadResult:
    """
    Compute SA-CCR Exposure at Default per BCBS 279.

    Thin wrapper over ``SaCcrEngine.calculate_ead``: builds the engine with
    the regulatory alpha of 1.4 (or ``alpha``) and prices ``trades`` under
    ``config``.

    Parameters
    ----------
    trades : list[SaCcrTrade]
        Derivative trades making up the netting set (an empty list gives
        zero EAD).
    config : SaCcrNettingSetConfig
        Netting-set collateral, threshold, MTA, NICA, MPoR and valuation date
        from ``SaCcrNettingSetConfig.unmargined`` / ``margined``.
    alpha : float or None, optional
        Supervisory alpha override; must be finite and at least 1.0.

    Returns
    -------
    EadResult
        EAD after applying the unmargined cap for margined sets, together with the multiplier, the
        aggregate and per-asset-class add-ons, and the maturity factor.

    Raises
    ------
    ValueError
        If ``alpha`` is invalid, ``config`` fails validation, or a trade's
        numeric fields are non-finite or its direction, supervisory delta and
        option type are inconsistent.

    Examples
    --------
    >>> from finstack_quant.margin import NettingSetId, SaCcrNettingSetConfig, SaCcrTrade, saccr_ead
    >>> trade = SaCcrTrade(
    ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0
    ... )
    >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-01")
    >>> result = saccr_ead([trade], config)
    >>> (round(result.rc, 2), round(result.pfe, 2), round(result.ead, 2))
    (0.0, 22130.59, 30982.83)
    """
    ...

def compute_bilateral_xva(
    exposure_profile: ExposureProfile,
    counterparty_hazard_curve: HazardCurve,
    own_hazard_curve: HazardCurve,
    discount_curve: DiscountCurve,
    counterparty_recovery_rate: float,
    own_recovery_rate: float,
    funding: FundingConfig | None = None,
) -> XvaResult:
    """
    Compute bilateral XVA: CVA, DVA, FVA, MVA, and the all-in adjustment.

    All legs are weighted by joint (first-to-default) survival, so the credit
    and funding components are not double-counted. MVA is computed only when
    ``funding`` carries an ``im_profile``; that posted IM also reduces ENE for
    bilateral DVA.

    The result reports ``total_xva = CVA - DVA + FVA + MVA``; uncomputed
    optional legs contribute zero.

    Parameters
    ----------
    exposure_profile : ExposureProfile
        EPE/ENE profile from exposure simulation.
    counterparty_hazard_curve : HazardCurve
        Hazard curve for the counterparty's credit.
    own_hazard_curve : HazardCurve
        Hazard curve for the institution's own credit.
    discount_curve : DiscountCurve
        Risk-free discount curve for present-valuing.
    counterparty_recovery_rate : float
        Recovery rate on counterparty default, in ``[0, 1]``.
    own_recovery_rate : float
        Recovery rate on own default, in ``[0, 1]``.
    funding : FundingConfig | None
        Funding configuration driving FVA and, when it carries an
        ``im_profile``, MVA. ``None`` computes the credit legs only.

    Returns
    -------
    XvaResult
        CVA, DVA, FVA, MVA, ``total_xva``, and the exposure/regulatory
        profiles.

    Raises
    ------
    ValueError
        If the exposure profile is empty or inconsistent, a recovery rate is
        outside ``[0, 1]``, funding inputs are invalid, the IM and exposure
        horizons differ, or a curve evaluation is non-finite.

    Examples
    --------
    >>> import datetime as dt
    >>> from finstack_quant.core.market_data.curves import DiscountCurve, HazardCurve
    >>> profile = ExposureProfile([1.0, 2.0], [1e6, 1e6], [1e6, 1e6], [0.0, 0.0])
    >>> df = DiscountCurve(
    ...     "USD-OIS",
    ...     dt.date(2025, 1, 1),
    ...     [(0.5 * i, 1.0) for i in range(9)],
    ...     interp="log_linear",
    ... )
    >>> hz = HazardCurve("CPTY", dt.date(2025, 1, 1), [(0.0, 0.02), (30.0, 0.02)], recovery_rate=0.40)
    >>> result = compute_bilateral_xva(profile, hz, hz, df, 0.40, 0.40)
    >>> result.total_xva == result.cva - result.dva
    True
    """
    ...
