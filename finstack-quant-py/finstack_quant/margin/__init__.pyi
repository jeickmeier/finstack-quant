"""Margin and collateral: VM/IM calculators, CSA specifications, XVA, metrics.

This module exposes variation and initial margin types, netting-set identifiers,
credit support annex (CSA) specs, eligible collateral schedules, XVA configuration
and results, and margin analytics helpers.
"""

from __future__ import annotations

from typing import Any, Final

import pandas as pd

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
    "XvaConfig",
    "ExposureDiagnostics",
    "ExposureProfile",
    "XvaResult",
    "CsaTerms",
    "XvaNettingSet",
    "MarginUtilization",
    "ExcessCollateral",
    "MarginFundingCost",
    "Haircut01",
    "FrtbSensitivities",
    "FrtbSbaEngine",
    "SaCcrTrade",
    "SaCcrNettingSetConfig",
    "SaCcrEngine",
    "frtb_sba_charge",
    "saccr_ead",
]

CONSTANTS: Final[dict[str, str]] = ...

class ImMethodology:
    """Initial margin calculation methodology.

    Parameters
    ----------
    (Constructed via class methods; not directly instantiated.)

    Returns
    -------
    ImMethodology
        Enum-like value for IM approach.

    Examples
    --------
    >>> ImMethodology.from_str("simm")
    ImMethodology(Simm)
    """

    @staticmethod
    def haircut() -> ImMethodology:
        """Haircut-based IM (repos and securities financing).

        Returns
        -------
        ImMethodology
            Haircut methodology.

        Examples
        --------
        >>> ImMethodology.haircut()
        ImMethodology(Haircut)
        """
        ...

    @staticmethod
    def simm() -> ImMethodology:
        """ISDA SIMM (sensitivities-based, OTC derivatives).

        Returns
        -------
        ImMethodology
            SIMM methodology.

        Examples
        --------
        >>> ImMethodology.simm()
        ImMethodology(Simm)
        """
        ...

    @staticmethod
    def schedule() -> ImMethodology:
        """BCBS-IOSCO regulatory schedule approach.

        Returns
        -------
        ImMethodology
            Schedule methodology.

        Examples
        --------
        >>> ImMethodology.schedule()
        ImMethodology(Schedule)
        """
        ...

    @staticmethod
    def internal_model() -> ImMethodology:
        """Internal model approved by regulator.

        Returns
        -------
        ImMethodology
            Internal model methodology.

        Examples
        --------
        >>> ImMethodology.internal_model()
        ImMethodology(InternalModel)
        """
        ...

    @staticmethod
    def clearing_house() -> ImMethodology:
        """Clearing house (CCP-specific) methodology.

        Returns
        -------
        ImMethodology
            CCP methodology.

        Examples
        --------
        >>> ImMethodology.clearing_house()
        ImMethodology(ClearingHouse)
        """
        ...

    @staticmethod
    def from_str(s: str) -> ImMethodology:
        """Parse from a string (e.g. ``"simm"``, ``"schedule"``).

        Parameters
        ----------
        s : str
            Methodology name.

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
        ImMethodology(Schedule)
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class MarginTenor:
    """Margin call frequency.

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
    MarginTenor(Daily)
    """

    @staticmethod
    def daily() -> MarginTenor:
        """Daily margin calls (standard for OTC derivatives post-2016).

        Returns
        -------
        MarginTenor
            Daily tenor.

        Examples
        --------
        >>> str(MarginTenor.daily())
        'Daily'
        """
        ...

    @staticmethod
    def weekly() -> MarginTenor:
        """Weekly margin calls.

        Returns
        -------
        MarginTenor
            Weekly tenor.

        Examples
        --------
        >>> MarginTenor.weekly()
        MarginTenor(Weekly)
        """
        ...

    @staticmethod
    def monthly() -> MarginTenor:
        """Monthly margin calls.

        Returns
        -------
        MarginTenor
            Monthly tenor.

        Examples
        --------
        >>> MarginTenor.monthly()
        MarginTenor(Monthly)
        """
        ...

    @staticmethod
    def on_demand() -> MarginTenor:
        """On-demand margin calls.

        Returns
        -------
        MarginTenor
            On-demand tenor.

        Examples
        --------
        >>> MarginTenor.on_demand()
        MarginTenor(OnDemand)
        """
        ...

    @staticmethod
    def from_str(s: str) -> MarginTenor:
        """Parse from string.

        Parameters
        ----------
        s : str
            Tenor name.

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
        MarginTenor(Daily)
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class MarginCallType:
    """Type of margin call.

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
    MarginCallType(...)
    """

    @staticmethod
    def initial_margin() -> MarginCallType:
        """Initial margin posting requirement.

        Returns
        -------
        MarginCallType
            Initial margin call type.

        Examples
        --------
        >>> MarginCallType.initial_margin()
        MarginCallType(...)
        """
        ...

    @staticmethod
    def variation_margin_delivery() -> MarginCallType:
        """Variation margin delivery (margin to be posted).

        Returns
        -------
        MarginCallType
            VM delivery type.

        Examples
        --------
        >>> MarginCallType.variation_margin_delivery()
        MarginCallType(...)
        """
        ...

    @staticmethod
    def variation_margin_return() -> MarginCallType:
        """Variation margin return (margin to be received back).

        Returns
        -------
        MarginCallType
            VM return type.

        Examples
        --------
        >>> MarginCallType.variation_margin_return()
        MarginCallType(...)
        """
        ...

    @staticmethod
    def top_up() -> MarginCallType:
        """Top-up margin call.

        Returns
        -------
        MarginCallType
            Top-up type.

        Examples
        --------
        >>> MarginCallType.top_up()
        MarginCallType(...)
        """
        ...

    @staticmethod
    def substitution() -> MarginCallType:
        """Collateral substitution request.

        Returns
        -------
        MarginCallType
            Substitution type.

        Examples
        --------
        >>> MarginCallType.substitution()
        MarginCallType(...)
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...

class ClearingStatus:
    """Clearing status for OTC derivatives.

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
        """Bilateral (uncleared) trade governed by CSA.

        Returns
        -------
        ClearingStatus
            Bilateral status.

        Examples
        --------
        >>> ClearingStatus.bilateral().is_bilateral
        True
        """
        ...

    @staticmethod
    def cleared(ccp: str) -> ClearingStatus:
        """Trade cleared through a CCP.

        Parameters
        ----------
        ccp : str
            Clearing house identifier.

        Returns
        -------
        ClearingStatus
            Cleared status with CCP id.

        Examples
        --------
        >>> ClearingStatus.cleared("LCH").is_cleared
        True
        """
        ...

    @property
    def is_bilateral(self) -> bool:
        """Whether this is a bilateral trade.

        Returns
        -------
        bool
            True if bilateral.

        Examples
        --------
        >>> ClearingStatus.bilateral().is_bilateral
        True
        """
        ...

    @property
    def is_cleared(self) -> bool:
        """Whether this is a cleared trade.

        Returns
        -------
        bool
            True if cleared.

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
    """Collateral asset class per BCBS-IOSCO standards.

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
        """Cash collateral class.

        Returns
        -------
        CollateralAssetClass
            Cash.

        Examples
        --------
        >>> CollateralAssetClass.cash()
        CollateralAssetClass(Cash)
        """
        ...

    @staticmethod
    def government_bonds() -> CollateralAssetClass:
        """Government bonds class.

        Returns
        -------
        CollateralAssetClass
            Government bonds.

        Examples
        --------
        >>> CollateralAssetClass.government_bonds()
        CollateralAssetClass(GovernmentBonds)
        """
        ...

    @staticmethod
    def agency_bonds() -> CollateralAssetClass:
        """Agency bonds class.

        Returns
        -------
        CollateralAssetClass
            Agency bonds.

        Examples
        --------
        >>> CollateralAssetClass.agency_bonds()
        CollateralAssetClass(AgencyBonds)
        """
        ...

    @staticmethod
    def covered_bonds() -> CollateralAssetClass:
        """Covered bonds class.

        Returns
        -------
        CollateralAssetClass
            Covered bonds.

        Examples
        --------
        >>> CollateralAssetClass.covered_bonds()
        CollateralAssetClass(CoveredBonds)
        """
        ...

    @staticmethod
    def corporate_bonds() -> CollateralAssetClass:
        """Corporate bonds class.

        Returns
        -------
        CollateralAssetClass
            Corporate bonds.

        Examples
        --------
        >>> CollateralAssetClass.corporate_bonds()
        CollateralAssetClass(CorporateBonds)
        """
        ...

    @staticmethod
    def equity() -> CollateralAssetClass:
        """Equity class.

        Returns
        -------
        CollateralAssetClass
            Equity.

        Examples
        --------
        >>> CollateralAssetClass.equity()
        CollateralAssetClass(Equity)
        """
        ...

    @staticmethod
    def gold() -> CollateralAssetClass:
        """Gold class.

        Returns
        -------
        CollateralAssetClass
            Gold.

        Examples
        --------
        >>> CollateralAssetClass.gold()
        CollateralAssetClass(Gold)
        """
        ...

    @staticmethod
    def mutual_funds() -> CollateralAssetClass:
        """Mutual funds class.

        Returns
        -------
        CollateralAssetClass
            Mutual funds.

        Examples
        --------
        >>> CollateralAssetClass.mutual_funds()
        CollateralAssetClass(MutualFunds)
        """
        ...

    @staticmethod
    def from_str(s: str) -> CollateralAssetClass:
        """Parse from string.

        Parameters
        ----------
        s : str
            Asset class name.

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
        CollateralAssetClass(Cash)
        """
        ...

    def standard_haircut(self) -> float:
        """BCBS-IOSCO standard haircut for this asset class.

        Returns
        -------
        float
            Haircut as decimal.

        Raises
        ------
        Exception
            If the core library returns an error.

        Examples
        --------
        >>> CollateralAssetClass.cash().standard_haircut()
        0.0
        """
        ...

    def fx_addon(self) -> float:
        """FX haircut add-on for currency mismatch.

        Returns
        -------
        float
            Add-on as decimal.

        Raises
        ------
        Exception
            If the core library returns an error.

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
    """Identifies a margin netting set.

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
        """Create a bilateral netting set.

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

        Examples
        --------
        >>> NettingSetId.bilateral("A", "CSA").is_cleared
        False
        """
        ...

    @staticmethod
    def cleared(ccp_id: str) -> NettingSetId:
        """Create a cleared netting set.

        Parameters
        ----------
        ccp_id : str
            Central counterparty identifier.

        Returns
        -------
        NettingSetId
            Cleared netting set id.

        Examples
        --------
        >>> NettingSetId.cleared("LCH").is_cleared
        True
        """
        ...

    @property
    def is_cleared(self) -> bool:
        """Whether this is a cleared netting set.

        Returns
        -------
        bool
            True if cleared.

        Examples
        --------
        >>> NettingSetId.cleared("CCP").is_cleared
        True
        """
        ...

    @property
    def counterparty_id(self) -> str:
        """Counterparty identifier. For cleared netting sets this returns
        the CCP id; for bilateral, the explicit counterparty id.

        Returns
        -------
        str
            Counterparty id string.

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
        """CSA identifier when bilateral; ``None`` for cleared sets.

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
        """CCP identifier when cleared; ``None`` for bilateral sets.

        Examples
        --------
        >>> NettingSetId.cleared("LCH").ccp_id
        'LCH'
        >>> NettingSetId.bilateral("X", "CSA").ccp_id is None
        True
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class CsaSpec:
    """Credit Support Annex specification (ISDA standard).

    Parameters
    ----------
    (Use regulatory factories or ``from_json``.)

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
        """Standard regulatory CSA for USD derivatives.

        Returns
        -------
        CsaSpec
            USD regulatory CSA.

        Raises
        ------
        Exception
            If construction fails in the core library.

        Examples
        --------
        >>> csa = CsaSpec.usd_regulatory()
        >>> csa.id  # doctest: +SKIP
        """
        ...

    @staticmethod
    def eur_regulatory() -> CsaSpec:
        """Standard regulatory CSA for EUR derivatives.

        Returns
        -------
        CsaSpec
            EUR regulatory CSA.

        Raises
        ------
        Exception
            If construction fails in the core library.

        Examples
        --------
        >>> CsaSpec.eur_regulatory().base_currency
        'EUR'
        """
        ...

    @staticmethod
    def from_json(json: str) -> CsaSpec:
        """Deserialize from a JSON string.

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
        >>> CsaSpec.from_json("{}")  # doctest: +SKIP
        """
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string.

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
        """CSA identifier.

        Returns
        -------
        str
            CSA id.

        Examples
        --------
        >>> isinstance(CsaSpec.usd_regulatory().id, str)
        True
        """
        ...

    @property
    def base_currency(self) -> str:
        """Base currency code.

        Returns
        -------
        str
            ISO currency code.

        Examples
        --------
        >>> CsaSpec.usd_regulatory().base_currency
        'USD'
        """
        ...

    @property
    def requires_im(self) -> bool:
        """Whether this CSA requires initial margin.

        Returns
        -------
        bool
            True if IM required.

        Examples
        --------
        >>> isinstance(CsaSpec.usd_regulatory().requires_im, bool)
        True
        """
        ...

    def __repr__(self) -> str: ...

class EligibleCollateralSchedule:
    """Eligible collateral schedule with haircuts.

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
        """Cash-only schedule.

        Returns
        -------
        EligibleCollateralSchedule
            Schedule with cash only.

        Raises
        ------
        Exception
            If construction fails.

        Examples
        --------
        >>> EligibleCollateralSchedule.cash_only().rehypothecation_allowed  # doctest: +SKIP
        """
        ...

    @staticmethod
    def bcbs_standard() -> EligibleCollateralSchedule:
        """Standard BCBS-IOSCO compliant schedule.

        Returns
        -------
        EligibleCollateralSchedule
            BCBS schedule.

        Raises
        ------
        Exception
            If construction fails.

        Examples
        --------
        >>> EligibleCollateralSchedule.bcbs_standard().eligible_count > 0
        True
        """
        ...

    @staticmethod
    def us_treasuries() -> EligibleCollateralSchedule:
        """US Treasuries repo schedule.

        Returns
        -------
        EligibleCollateralSchedule
            Treasury-focused schedule.

        Raises
        ------
        Exception
            If construction fails.

        Examples
        --------
        >>> EligibleCollateralSchedule.us_treasuries().eligible_count > 0
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> EligibleCollateralSchedule:
        """Deserialize from JSON.

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
        >>> EligibleCollateralSchedule.from_json("{}")  # doctest: +SKIP
        """
        ...

    def to_json(self) -> str:
        """Serialize to JSON.

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
        """Whether rehypothecation is allowed.

        Returns
        -------
        bool
            Rehypothecation flag.

        Examples
        --------
        >>> isinstance(EligibleCollateralSchedule.cash_only().rehypothecation_allowed, bool)
        True
        """
        ...

    @property
    def eligible_count(self) -> int:
        """Number of eligible collateral types.

        Returns
        -------
        int
            Count of eligible entries.

        Examples
        --------
        >>> EligibleCollateralSchedule.cash_only().eligible_count >= 1
        True
        """
        ...

    def is_eligible(self, asset_class: CollateralAssetClass) -> bool:
        """Check if an asset class is eligible.

        Parameters
        ----------
        asset_class : CollateralAssetClass
            Asset class to test.

        Returns
        -------
        bool
            True if eligible under this schedule.

        Examples
        --------
        >>> s = EligibleCollateralSchedule.cash_only()
        >>> s.is_eligible(CollateralAssetClass.cash())
        True
        """
        ...

    def haircut_for(self, asset_class: CollateralAssetClass) -> float | None:
        """Get the haircut for an asset class.

        Parameters
        ----------
        asset_class : CollateralAssetClass
            Asset class.

        Returns
        -------
        float or None
            Haircut if defined, else None.

        Examples
        --------
        >>> s = EligibleCollateralSchedule.cash_only()
        >>> s.haircut_for(CollateralAssetClass.cash()) is not None
        True
        """
        ...

    def __repr__(self) -> str: ...

class VmResult:
    """Variation margin calculation result.

    Parameters
    ----------
    (Returned by ``VmCalculator.calculate``.)

    Returns
    -------
    VmResult
        VM amounts and call flag.

    Examples
    --------
    >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", 2024, 6, 15)
    >>> isinstance(r.net_margin, float)
    True
    """

    @property
    def gross_exposure(self) -> float:
        """Gross mark-to-market exposure amount.

        Returns
        -------
        float
            Gross exposure.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", 2024, 6, 15)
        >>> r.gross_exposure >= 0
        True
        """
        ...

    @property
    def net_exposure(self) -> float:
        """Net exposure after threshold and independent amount.

        Returns
        -------
        float
            Net exposure.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", 2024, 6, 15)
        >>> isinstance(r.net_exposure, float)
        True
        """
        ...

    @property
    def delivery_amount(self) -> float:
        """Delivery amount (positive = we post margin).

        Returns
        -------
        float
            Delivery amount.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", 2024, 6, 15)
        >>> r.delivery_amount >= 0
        True
        """
        ...

    @property
    def return_amount(self) -> float:
        """Return amount (positive = we receive margin back).

        Returns
        -------
        float
            Return amount.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", 2024, 6, 15)
        >>> r.return_amount >= 0
        True
        """
        ...

    @property
    def net_margin(self) -> float:
        """Net margin amount (delivery − return).

        Returns
        -------
        float
            Net margin.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", 2024, 6, 15)
        >>> isinstance(r.net_margin, float)
        True
        """
        ...

    @property
    def requires_call(self) -> bool:
        """Whether a margin call is required.

        Returns
        -------
        bool
            Call required flag.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", 2024, 6, 15)
        >>> isinstance(r.requires_call, bool)
        True
        """
        ...

    def __repr__(self) -> str: ...

class VmCalculator:
    """Variation margin calculator following ISDA CSA rules.

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
    >>> out = calc.calculate(1e6, 0.0, "USD", 2024, 6, 15)
    >>> isinstance(out, VmResult)
    True
    """

    def __init__(self, csa: CsaSpec) -> None: ...
    def calculate(
        self,
        exposure: float,
        posted_collateral: float,
        currency: str,
        year: int,
        month: int,
        day: int,
    ) -> VmResult:
        """Calculate variation margin.

        Parameters
        ----------
        exposure : float
            Mark-to-market exposure.
        posted_collateral : float
            Posted collateral amount.
        currency : str
            ISO currency code.
        year : int
            As-of year.
        month : int
            As-of month (1–12).
        day : int
            As-of day.

        Returns
        -------
        VmResult
            VM breakdown.

        Raises
        ------
        ValueError
            Invalid currency, month, or calendar date.
        Exception
            Core calculation error.

        Examples
        --------
        >>> VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", 2024, 6, 15)
        VmResult(...)
        """
        ...

class ImResult:
    """Initial margin calculation result.

    Parameters
    ----------
    (Produced by IM workflows in the margin crate; exposed for typing.)

    Returns
    -------
    ImResult
        IM amount and metadata.

    Examples
    --------
    >>> isinstance(ImResult, type)
    True
    """

    @property
    def amount(self) -> float:
        """Calculated initial margin amount.

        Returns
        -------
        float
            IM notional.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def currency(self) -> str:
        """Currency of the IM amount.

        Returns
        -------
        str
            ISO currency.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def methodology(self) -> ImMethodology:
        """Methodology used for calculation.

        Returns
        -------
        ImMethodology
            IM methodology.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def mpor_days(self) -> int:
        """Margin Period of Risk (days).

        Returns
        -------
        int
            MPOR in days.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def as_of(self) -> str:
        """Calculation date as an ISO 8601 string."""
        ...

    def breakdown_keys(self) -> list[str]:
        """Risk-class breakdown keys (if available).

        Returns
        -------
        list[str]
            Keys present in the breakdown map.

        Examples
        --------
        >>> # Depends on instance data
        """
        ...

    def breakdown_amount(self, key: str) -> float | None:
        """Get breakdown amount for a risk class.

        Parameters
        ----------
        key : str
            Risk class key.

        Returns
        -------
        float or None
            Amount if present.

        Examples
        --------
        >>> # Depends on instance data
        """
        ...

    def __repr__(self) -> str: ...

class SimmSensitivities:
    """ISDA SIMM sensitivity portfolio.

    Stores signed sensitivity amounts by SIMM risk class and bucket. Amounts
    are currency amounts, not percentages or spot levels. Rate and credit
    delta inputs are DV01/CS01-style amounts per 1bp move, and the
    ``base_currency`` records the currency context in which those amounts were
    produced.

    Use ``from_json``/``to_json`` for full-fidelity interop with the canonical
    Rust JSON shape, or the ``add_*`` helpers for notebook-style construction.
    """

    def __init__(self, base_currency: str = "USD") -> None:
        """Create an empty SIMM sensitivity set.

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
        """Deserialize SIMM sensitivities from the canonical JSON shape.

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
        """
        ...

    def to_json(self) -> str:
        """Serialize sensitivities to the canonical pretty-printed JSON shape.

        Returns
        -------
        str
            JSON string containing all populated buckets and the base currency.
        """
        ...

    def add_ir_delta(self, currency: str, tenor: str, amount: float) -> None:
        """Add an interest-rate delta bucket.

        Parameters
        ----------
        currency : str
            Currency risk factor, such as ``"USD"``.
        tenor : str
            SIMM tenor bucket, such as ``"2W"``, ``"1Y"``, ``"5Y"``, or
            ``"30Y"``.
        amount : float
            Signed DV01-style currency amount per 1bp move.

        Raises
        ------
        ValueError
            If ``currency`` is not a known currency code.
        """
        ...

    def add_ir_vega(self, currency: str, tenor: str, amount: float) -> None:
        """Add an interest-rate vega bucket.

        Parameters
        ----------
        currency : str
            Currency risk factor, such as ``"USD"``.
        tenor : str
            SIMM tenor bucket.
        amount : float
            Signed currency vega amount compatible with SIMM vega weights.

        Raises
        ------
        ValueError
            If ``currency`` is not a known currency code.
        """
        ...

    def add_credit_delta(self, name: str, qualifying: bool, tenor: str, amount: float) -> None:
        """Add a credit delta bucket.

        Parameters
        ----------
        name : str
            Issuer, index, or reference-entity identifier.
        qualifying : bool
            ``True`` for SIMM credit qualifying, ``False`` for credit
            non-qualifying.
        tenor : str
            Credit tenor bucket, such as ``"5Y"``.
        amount : float
            Signed CS01-style currency amount per 1bp move.
        """
        ...

    def add_credit_delta_bucketed(self, sector: str, name: str, tenor: str, amount: float) -> None:
        """Add a bucketed credit-qualifying delta sensitivity.

        Parameters
        ----------
        sector : str
            SIMM credit sector alias. Supported aliases include
            ``"sovereign"``, ``"financial"``, ``"basic_materials"``,
            ``"technology_media"``, ``"health_care"``, ``"hy_financial"``,
            ``"index"``, ``"securitized"``, and ``"residual"``.
        name : str
            Issuer, index, or reference-entity identifier.
        tenor : str
            Credit tenor bucket, such as ``"5Y"``.
        amount : float
            Signed CS01-style currency amount per 1bp move.

        Raises
        ------
        ValueError
            If ``sector`` is not recognized.
        """
        ...

    def add_equity_delta(self, underlier: str, amount: float) -> None:
        """Add an equity delta bucket.

        Parameters
        ----------
        underlier : str
            Equity underlier or index identifier.
        amount : float
            Signed currency sensitivity amount.
        """
        ...

    def add_equity_vega(self, underlier: str, amount: float) -> None:
        """Add an equity vega bucket.

        Parameters
        ----------
        underlier : str
            Equity underlier or index identifier.
        amount : float
            Signed currency vega amount.
        """
        ...

    def add_fx_delta(self, currency: str, amount: float) -> None:
        """Add an FX delta bucket.

        Parameters
        ----------
        currency : str
            FX risk-factor currency.
        amount : float
            Signed currency sensitivity amount.

        Raises
        ------
        ValueError
            If ``currency`` is not a known currency code.
        """
        ...

    def add_fx_vega(self, ccy1: str, ccy2: str, amount: float) -> None:
        """Add an FX vega bucket for a currency pair.

        Parameters
        ----------
        ccy1 : str
            First currency in the FX pair.
        ccy2 : str
            Second currency in the FX pair.
        amount : float
            Signed currency vega amount.

        Raises
        ------
        ValueError
            If either currency code is unknown.
        """
        ...

    def add_commodity_delta(self, bucket: str, amount: float) -> None:
        """Add a commodity delta bucket.

        Parameters
        ----------
        bucket : str
            Commodity bucket label expected by the configured SIMM registry,
            such as ``"energy"``.
        amount : float
            Signed currency sensitivity amount.
        """
        ...

    def add_curvature(self, risk_class: str, amount: float) -> None:
        """Add a curvature contribution for a SIMM risk class.

        Parameters
        ----------
        risk_class : str
            SIMM risk class alias. Supported aliases include
            ``"interest_rate"``, ``"rates"``, ``"credit_qualifying"``,
            ``"credit_non_qualifying"``, ``"equity"``, ``"commodity"``,
            and ``"fx"``.
        amount : float
            Signed curvature contribution in currency units before the SIMM
            curvature scale factor is applied.

        Raises
        ------
        ValueError
            If ``risk_class`` is not recognized.
        """
        ...

    def is_empty(self) -> bool:
        """Return whether no sensitivity buckets have been populated.

        Returns
        -------
        bool
            ``True`` when every SIMM bucket map is empty. A populated bucket
            with a zero net amount still makes the container non-empty.
        """
        ...

    @property
    def base_currency(self) -> str:
        """Currency context in which sensitivity amounts are expressed."""
        ...

class SimmCalculator:
    """ISDA SIMM initial-margin calculator.

    Loads registry-backed SIMM parameters for the requested rule version and
    calculates initial margin from explicit ``SimmSensitivities``.
    """

    def __init__(self, version: str = "v2_6", mpor_days: int | None = None) -> None:
        """Create a SIMM calculator from the embedded margin registry.

        Parameters
        ----------
        version : str, default "v2_6"
            SIMM version alias. Supported values include ``"v2_5"``,
            ``"2.5"``, ``"SIMM 2.5"``, ``"v2_6"``, ``"2.6"``, and
            ``"SIMM 2.6"``.
        mpor_days : int | None, optional
            Optional margin period of risk override in calendar days. When
            omitted, the registry default for the SIMM version is used.

        Raises
        ------
        ValueError
            If the version is unknown or registry parameters cannot be loaded.
        """
        ...

    @property
    def version(self) -> str:
        """Stable SIMM version label, either ``"v2_5"`` or ``"v2_6"``."""
        ...

    @property
    def mpor_days(self) -> int:
        """Margin period of risk in calendar days."""
        ...

    def calculate_from_sensitivities(
        self,
        sensitivities: SimmSensitivities,
        currency: str,
        year: int,
        month: int,
        day: int,
    ) -> ImResult:
        """Calculate SIMM from explicit sensitivities.

        Parameters
        ----------
        sensitivities : SimmSensitivities
            Sensitivity set to aggregate.
        currency : str
            Reporting currency for the resulting margin amount.
        year : int
            Calculation year.
        month : int
            Calculation month, from 1 to 12.
        day : int
            Calculation day of month.

        Returns
        -------
        ImResult
            Initial-margin amount, methodology, MPOR, calculation date, and
            risk-class breakdown.

        Raises
        ------
        ValueError
            If the reporting currency or date is invalid.
        """
        ...

class ScheduleImCalculator:
    """BCBS-IOSCO regulatory schedule initial-margin calculator.

    Applies registry-backed schedule rates to explicit notionals or to a
    single-asset-class netting set with the BCBS-IOSCO net-to-gross ratio
    reduction.
    """

    @staticmethod
    def bcbs_standard() -> ScheduleImCalculator:
        """Create the embedded BCBS-IOSCO standard schedule calculator.

        Returns
        -------
        ScheduleImCalculator
            Calculator configured with the standard embedded schedule grid.

        Raises
        ------
        ValueError
            If embedded registry data cannot be loaded.
        """
        ...

    @staticmethod
    def from_registry_id(schedule_id: str) -> ScheduleImCalculator:
        """Create a schedule calculator from a registry identifier.

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
        """
        ...

    def with_asset_class(self, asset_class: str) -> ScheduleImCalculator:
        """Return a copy with a new default schedule asset class.

        Parameters
        ----------
        asset_class : str
            Schedule asset class alias such as ``"interest_rate"``,
            ``"credit"``, ``"equity"``, ``"commodity"``, ``"fx"``, or
            ``"other"``.

        Returns
        -------
        ScheduleImCalculator
            Copy of this calculator with the default asset class changed.

        Raises
        ------
        ValueError
            If ``asset_class`` is not recognized.
        """
        ...

    def with_maturity(self, years: float) -> ScheduleImCalculator:
        """Return a copy with a new default maturity.

        Parameters
        ----------
        years : float
            Representative remaining maturity in years.

        Returns
        -------
        ScheduleImCalculator
            Copy of this calculator with the default maturity changed.
        """
        ...

    def rate(self, asset_class: str, maturity_years: float) -> float:
        """Look up a decimal schedule rate.

        Parameters
        ----------
        asset_class : str
            Schedule asset class alias.
        maturity_years : float
            Remaining maturity in years.

        Returns
        -------
        float
            Decimal IM rate, e.g. ``0.01`` for 1%.

        Raises
        ------
        ValueError
            If ``asset_class`` is not recognized.
        """
        ...

    def calculate_for_notional(
        self,
        notional: float,
        currency: str,
        asset_class: str,
        maturity_years: float,
        year: int,
        month: int,
        day: int,
    ) -> ImResult:
        """Calculate gross schedule IM from an explicit notional.

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
        year : int
            Calculation year.
        month : int
            Calculation month, from 1 to 12.
        day : int
            Calculation day of month.

        Returns
        -------
        ImResult
            Gross schedule IM with a breakdown key equal to the normalized
            asset class.

        Raises
        ------
        ValueError
            If the currency, asset class, amount, or date is invalid.
        """
        ...

    def calculate_netting_set_with_ngr(
        self,
        positions: list[tuple[float, float]],
        currency: str,
        asset_class: str,
        maturity_years: float,
        year: int,
        month: int,
        day: int,
    ) -> ImResult | None:
        """Calculate schedule IM for a netting set using NGR.

        Applies the BCBS-IOSCO reduction ``0.4 + 0.6 * NGR`` to a
        single-asset-class set of ``(signed_mtm, gross_notional)`` positions.
        The binding assumes every tuple is in ``currency`` and that the set has
        already been partitioned by asset class.

        Parameters
        ----------
        positions : list[tuple[float, float]]
            ``(signed_mtm, gross_notional)`` pairs. MTM signs drive the NGR
            numerator; gross notionals are summed as absolute values.
        currency : str
            Reporting currency for every MTM, notional, and result.
        asset_class : str
            Schedule asset class applied uniformly to all positions.
        maturity_years : float
            Representative remaining maturity used for the rate lookup.
        year : int
            Calculation year.
        month : int
            Calculation month, from 1 to 12.
        day : int
            Calculation day of month.

        Returns
        -------
        ImResult | None
            NGR-adjusted schedule IM. Returns ``None`` for an empty position
            list, zero gross notionals, or inconsistent currencies after
            conversion to Rust money values.

        Raises
        ------
        ValueError
            If the currency, asset class, amount, or date is invalid.
        """
        ...

class HaircutImCalculator:
    """Haircut-based initial-margin calculator.

    Applies eligible-collateral haircuts and optional FX add-ons to explicit
    collateral values. This path is intended for repo and securities-financing
    style collateral IM rather than SIMM sensitivities.
    """

    @staticmethod
    def bcbs_standard() -> HaircutImCalculator:
        """Create a haircut calculator with the BCBS-IOSCO schedule.

        Returns
        -------
        HaircutImCalculator
            Calculator using the embedded BCBS-IOSCO collateral haircuts.

        Raises
        ------
        ValueError
            If embedded registry data cannot be loaded.
        """
        ...

    @staticmethod
    def us_treasuries() -> HaircutImCalculator:
        """Create a haircut calculator for US Treasury collateral.

        Returns
        -------
        HaircutImCalculator
            Calculator using the embedded US Treasuries haircut schedule.

        Raises
        ------
        ValueError
            If embedded registry data cannot be loaded.
        """
        ...

    @staticmethod
    def from_schedule(schedule: EligibleCollateralSchedule) -> HaircutImCalculator:
        """Create a haircut calculator from an eligible-collateral schedule.

        Parameters
        ----------
        schedule : EligibleCollateralSchedule
            Collateral eligibility and haircut schedule.

        Returns
        -------
        HaircutImCalculator
            Calculator backed by ``schedule``.
        """
        ...

    def with_default_asset_class(self, asset_class: CollateralAssetClass) -> HaircutImCalculator:
        """Return a copy configured with a default collateral asset class.

        Parameters
        ----------
        asset_class : CollateralAssetClass
            Asset class used by trait-based calculations.

        Returns
        -------
        HaircutImCalculator
            Copy of this calculator with the default asset class changed.
        """
        ...

    def with_posted_collateral_currency(self, currency: str) -> HaircutImCalculator:
        """Return a copy configured with a posted-collateral currency.

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

    def haircut_for(self, asset_class: CollateralAssetClass) -> float:
        """Look up the decimal haircut for a collateral asset class.

        Parameters
        ----------
        asset_class : CollateralAssetClass
            Collateral asset class.

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
        asset_class: CollateralAssetClass,
        currency_mismatch: bool,
        year: int,
        month: int,
        day: int,
    ) -> ImResult:
        """Calculate haircut IM from explicit collateral value and asset class.

        Parameters
        ----------
        collateral_value : float
            Collateral market value in ``currency``.
        currency : str
            Currency code for the collateral value and result.
        asset_class : CollateralAssetClass
            Collateral asset class used for the haircut lookup.
        currency_mismatch : bool
            Whether to add the asset-class FX mismatch add-on.
        year : int
            Calculation year.
        month : int
            Calculation month, from 1 to 12.
        day : int
            Calculation day of month.

        Returns
        -------
        ImResult
            Haircut IM result. The MPOR is the Rust canonical repo haircut
            horizon, currently 2 calendar days.

        Raises
        ------
        ValueError
            If the currency, amount, date, haircut, or FX add-on cannot be
            resolved.
        """
        ...

class FundingConfig:
    """Funding cost/benefit configuration for FVA calculation.

    Parameters
    ----------
    funding_spread_bps : float
        Funding spread in basis points.
    funding_benefit_bps : float | None, optional
        Funding benefit in bps; ``None`` for symmetric funding.

    Returns
    -------
    FundingConfig
        Funding parameters.

    Examples
    --------
    >>> FundingConfig(50.0, None).funding_spread_bps
    50.0
    """

    def __init__(
        self,
        funding_spread_bps: float,
        funding_benefit_bps: float | None = None,
    ) -> None: ...
    @property
    def funding_spread_bps(self) -> float:
        """Funding spread in basis points.

        Returns
        -------
        float
            Spread in bps.

        Examples
        --------
        >>> FundingConfig(10.0).funding_spread_bps
        10.0
        """
        ...

    @property
    def funding_benefit_bps(self) -> float | None:
        """Funding benefit spread in basis points (or None).

        Returns
        -------
        float or None
            Benefit bps if asymmetric.

        Examples
        --------
        >>> FundingConfig(10.0, 8.0).funding_benefit_bps
        8.0
        """
        ...

    def effective_benefit_bps(self) -> float:
        """Effective funding benefit spread in basis points.

        Returns
        -------
        float
            Effective benefit bps.

        Examples
        --------
        >>> isinstance(FundingConfig(1.0).effective_benefit_bps(), float)
        True
        """
        ...

    def __repr__(self) -> str: ...

class XvaConfig:
    """XVA calculation configuration.

    Parameters
    ----------
    time_grid : list[float] | None, optional
        Time grid in years; defaults to library default.
    recovery_rate : float | None, optional
        Counterparty recovery; defaults to library default.
    own_recovery_rate : float | None, optional
        Own recovery; optional.
    funding : FundingConfig | None, optional
        FVA funding configuration.

    Returns
    -------
    XvaConfig
        Configuration for XVA runs.

    Examples
    --------
    >>> cfg = XvaConfig()
    >>> cfg.recovery_rate > 0
    True
    """

    def __init__(
        self,
        time_grid: list[float] | None = None,
        recovery_rate: float | None = None,
        own_recovery_rate: float | None = None,
        funding: FundingConfig | None = None,
    ) -> None: ...
    @staticmethod
    def from_json(json: str) -> XvaConfig:
        """Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON string.

        Returns
        -------
        XvaConfig
            Parsed config.

        Raises
        ------
        ValueError
            Invalid JSON.

        Examples
        --------
        >>> XvaConfig.from_json("{}")  # doctest: +SKIP
        """
        ...

    def to_json(self) -> str:
        """Serialize to JSON.

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
        >>> isinstance(XvaConfig().to_json(), str)
        True
        """
        ...

    def validate(self) -> None:
        """Validate configuration parameters.

        Raises
        ------
        Exception
            If parameters are invalid.

        Examples
        --------
        >>> XvaConfig().validate()
        """
        ...

    @property
    def time_grid(self) -> list[float]:
        """Time grid for exposure simulation (years from today).

        Returns
        -------
        list[float]
            Time points in years.

        Examples
        --------
        >>> len(XvaConfig().time_grid) > 0
        True
        """
        ...

    @property
    def recovery_rate(self) -> float:
        """Recovery rate for counterparty default.

        Returns
        -------
        float
            Recovery fraction.

        Examples
        --------
        >>> 0 <= XvaConfig().recovery_rate <= 1
        True
        """
        ...

    @property
    def own_recovery_rate(self) -> float | None:
        """Recovery rate for own default (or None).

        Returns
        -------
        float or None
            Own recovery if set.

        Examples
        --------
        >>> XvaConfig(own_recovery_rate=0.4).own_recovery_rate
        0.4
        """
        ...

    def __repr__(self) -> str: ...

class ExposureDiagnostics:
    """Diagnostics from exposure simulation.

    Parameters
    ----------
    (Embedded in exposure results when provided by the engine.)

    Returns
    -------
    ExposureDiagnostics
        Counters for simulation health.

    Examples
    --------
    >>> isinstance(ExposureDiagnostics, type)
    True
    """

    @property
    def market_roll_failures(self) -> int:
        """Number of market-roll failures.

        Returns
        -------
        int
            Failure count.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def valuation_failures(self) -> int:
        """Total instrument valuation failures.

        Returns
        -------
        int
            Failure count.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def total_time_points(self) -> int:
        """Total time grid points evaluated.

        Returns
        -------
        int
            Point count.

        Examples
        --------
        >>> # Instance field
        """
        ...

    def __repr__(self) -> str: ...

class ExposureProfile:
    """Exposure profile at each time grid point.

    Parameters
    ----------
    times : list[float]
        Time points in years.
    mtm_values : list[float]
        Portfolio MtM at each time.
    epe : list[float]
        Expected positive exposure series.
    ene : list[float]
        Expected negative exposure series.

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
    ) -> None: ...
    @staticmethod
    def from_json(json: str) -> ExposureProfile:
        """Deserialize from JSON.

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
        >>> ExposureProfile.from_json("{}")  # doctest: +SKIP
        """
        ...

    def to_json(self) -> str:
        """Serialize to JSON.

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
        >>> ExposureProfile([0.0], [0.0], [0.0], [0.0]).to_json()
        """
        ...

    def validate(self) -> None:
        """Validate internal consistency.

        Raises
        ------
        Exception
            If vectors are inconsistent.

        Examples
        --------
        >>> ExposureProfile([0.0], [0.0], [0.0], [0.0]).validate()
        """
        ...

    @property
    def times(self) -> list[float]:
        """Time points in years.

        Returns
        -------
        list[float]
            Times.

        Examples
        --------
        >>> ExposureProfile([0.0, 1.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]).times
        [0.0, 1.0]
        """
        ...

    @property
    def mtm_values(self) -> list[float]:
        """Portfolio MtM values at each time point.

        Returns
        -------
        list[float]
            MtM path.

        Examples
        --------
        >>> ExposureProfile([0.0], [1.0], [0.0], [0.0]).mtm_values
        [1.0]
        """
        ...

    @property
    def epe(self) -> list[float]:
        """Expected Positive Exposure at each time point.

        Returns
        -------
        list[float]
            EPE series.

        Examples
        --------
        >>> ExposureProfile([0.0], [0.0], [2.0], [0.0]).epe
        [2.0]
        """
        ...

    @property
    def ene(self) -> list[float]:
        """Expected Negative Exposure at each time point.

        Returns
        -------
        list[float]
            ENE series.

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
        """Export as a pandas DataFrame with time (years) as index.

        Columns: ``mtm_values``, ``epe``, ``ene``.

        Returns
        -------
        pd.DataFrame
            Exposure profile as a DataFrame.
        """
        ...

    def __repr__(self) -> str: ...

class XvaResult:
    """Result of XVA calculations (CVA, DVA, FVA, exposure profiles).

    Parameters
    ----------
    (Produced by XVA engine; also loadable via ``from_json``.)

    Returns
    -------
    XvaResult
        XVA amounts and profiles.

    Examples
    --------
    >>> isinstance(XvaResult, type)
    True
    """

    @staticmethod
    def from_json(json: str) -> XvaResult:
        """Deserialize from JSON.

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
        >>> XvaResult.from_json("{}")  # doctest: +SKIP
        """
        ...

    def to_json(self) -> str:
        """Serialize to JSON.

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
        >>> # Round-trip when instance available
        """
        ...

    @property
    def cva(self) -> float:
        """Unilateral CVA (positive = cost).

        Returns
        -------
        float
            CVA amount.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def dva(self) -> float | None:
        """DVA (own-default benefit, or None).

        Returns
        -------
        float or None
            DVA if computed.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def fva(self) -> float | None:
        """FVA (net funding cost/benefit, or None).

        Returns
        -------
        float or None
            FVA if computed.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def bilateral_cva(self) -> float | None:
        """Bilateral CVA = CVA − DVA (or None).

        Returns
        -------
        float or None
            Bilateral CVA if defined.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def max_pfe(self) -> float:
        """Maximum PFE across the profile.

        Returns
        -------
        float
            Peak PFE.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def effective_epe(self) -> float:
        """Effective EPE (time-weighted average, regulatory metric).

        Returns
        -------
        float
            Effective EPE.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def epe_profile(self) -> list[tuple[float, float]]:
        """EPE profile as list of (time, value) tuples.

        Returns
        -------
        list[tuple[float, float]]
            (time, EPE) pairs.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def ene_profile(self) -> list[tuple[float, float]]:
        """ENE profile as list of (time, value) tuples.

        Returns
        -------
        list[tuple[float, float]]
            (time, ENE) pairs.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def pfe_profile(self) -> list[tuple[float, float]]:
        """PFE profile as list of (time, value) tuples.

        Returns
        -------
        list[tuple[float, float]]
            (time, PFE) pairs.

        Examples
        --------
        >>> # Instance field
        """
        ...

    @property
    def effective_epe_profile(self) -> list[tuple[float, float]]:
        """Effective EPE profile as list of (time, value) tuples.

        Returns
        -------
        list[tuple[float, float]]
            (time, effective EPE) pairs.

        Examples
        --------
        >>> # Instance field
        """
        ...

    def profiles_to_dataframe(self) -> pd.DataFrame:
        """Export exposure profiles as a pandas DataFrame.

        Columns: ``epe``, ``ene``, ``pfe``, ``effective_epe`` -- indexed
        by time in years.

        Returns
        -------
        pd.DataFrame
            Profile DataFrame.
        """
        ...

    def __repr__(self) -> str: ...

class CsaTerms:
    """Credit Support Annex terms for XVA collateralization.

    Parameters
    ----------
    threshold : float
        Threshold below which no collateral is required.
    mta : float
        Minimum transfer amount.
    mpor_days : int
        Margin period of risk in calendar days.
    independent_amount : float
        Independent amount (initial margin).

    Returns
    -------
    CsaTerms
        Collateral terms for XVA.

    Examples
    --------
    >>> CsaTerms(0.0, 0.0, 10, 0.0).mpor_days
    10
    """

    def __init__(
        self,
        threshold: float,
        mta: float,
        mpor_days: int,
        independent_amount: float,
    ) -> None: ...
    @property
    def threshold(self) -> float:
        """Threshold below which no collateral is required.

        Returns
        -------
        float
            Threshold amount.

        Examples
        --------
        >>> CsaTerms(1e6, 0.0, 5, 0.0).threshold
        1000000.0
        """
        ...

    @property
    def mta(self) -> float:
        """Minimum transfer amount.

        Returns
        -------
        float
            MTA.

        Examples
        --------
        >>> CsaTerms(0.0, 5e4, 5, 0.0).mta
        50000.0
        """
        ...

    @property
    def mpor_days(self) -> int:
        """Margin period of risk in calendar days.

        Returns
        -------
        int
            MPOR days.

        Examples
        --------
        >>> CsaTerms(0.0, 0.0, 14, 0.0).mpor_days
        14
        """
        ...

    @property
    def independent_amount(self) -> float:
        """Independent amount (initial margin).

        Returns
        -------
        float
            IA amount.

        Examples
        --------
        >>> CsaTerms(0.0, 0.0, 5, 1e5).independent_amount
        100000.0
        """
        ...

    def __repr__(self) -> str: ...

class XvaNettingSet:
    """XVA netting set: trades under a single ISDA master agreement.

    Parameters
    ----------
    id : str
        Netting set identifier.
    counterparty_id : str
        Counterparty identifier.
    csa : CsaTerms | None, optional
        Collateral terms if collateralized.
    reporting_currency : str | None, optional
        ISO currency for reporting.

    Returns
    -------
    XvaNettingSet
        Netting set descriptor.

    Raises
    ------
    ValueError
        If ``reporting_currency`` is not a valid currency code.

    Examples
    --------
    >>> XvaNettingSet("NS1", "CPTY").is_collateralized
    False
    """

    def __init__(
        self,
        id: str,
        counterparty_id: str,
        csa: CsaTerms | None = None,
        reporting_currency: str | None = None,
    ) -> None: ...
    @property
    def id(self) -> str:
        """Netting set identifier.

        Returns
        -------
        str
            Id string.

        Examples
        --------
        >>> XvaNettingSet("A", "B").id
        'A'
        """
        ...

    @property
    def counterparty_id(self) -> str:
        """Counterparty identifier.

        Returns
        -------
        str
            Counterparty id.

        Examples
        --------
        >>> XvaNettingSet("A", "CP").counterparty_id
        'CP'
        """
        ...

    @property
    def is_collateralized(self) -> bool:
        """Whether this netting set is collateralized.

        Returns
        -------
        bool
            True if CSA terms are set.

        Examples
        --------
        >>> XvaNettingSet("A", "B", CsaTerms(0, 0, 5, 0)).is_collateralized
        True
        """
        ...

    def __repr__(self) -> str: ...

class MarginUtilization:
    """Margin utilization result (ratio of posted to required margin).

    Parameters
    ----------
    posted_amount : float
        Posted margin amount.
    required_amount : float
        Required margin amount.
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
    ) -> None: ...
    @property
    def posted(self) -> float:
        """Posted margin amount.

        Returns
        -------
        float
            Posted amount.

        Examples
        --------
        >>> MarginUtilization(10.0, 20.0, "USD").posted
        10.0
        """
        ...

    @property
    def required(self) -> float:
        """Required margin amount.

        Returns
        -------
        float
            Required amount.

        Examples
        --------
        >>> MarginUtilization(10.0, 20.0, "USD").required
        20.0
        """
        ...

    @property
    def ratio(self) -> float:
        """Utilization ratio (posted / required).

        Returns
        -------
        float
            Ratio.

        Examples
        --------
        >>> MarginUtilization(50.0, 100.0, "EUR").ratio
        0.5
        """
        ...

    def is_adequate(self) -> bool:
        """Whether margin is adequate (ratio >= 1.0).

        Returns
        -------
        bool
            Adequacy flag.

        Examples
        --------
        >>> MarginUtilization(100.0, 100.0, "USD").is_adequate()
        True
        """
        ...

    def shortfall(self) -> float:
        """Shortfall amount (if any).

        Returns
        -------
        float
            Shortfall in currency units.

        Examples
        --------
        >>> MarginUtilization(0.0, 100.0, "USD").shortfall() >= 0
        True
        """
        ...

    def __repr__(self) -> str: ...

class ExcessCollateral:
    """Excess collateral result.

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
    ) -> None: ...
    @property
    def collateral_value(self) -> float:
        """Collateral value.

        Returns
        -------
        float
            Collateral mark.

        Examples
        --------
        >>> ExcessCollateral(10.0, 5.0, "USD").collateral_value
        10.0
        """
        ...

    @property
    def required_value(self) -> float:
        """Required value.

        Returns
        -------
        float
            Requirement.

        Examples
        --------
        >>> ExcessCollateral(10.0, 5.0, "USD").required_value
        5.0
        """
        ...

    @property
    def excess(self) -> float:
        """Excess amount (positive) or shortfall (negative).

        Returns
        -------
        float
            Net excess.

        Examples
        --------
        >>> ExcessCollateral(10.0, 5.0, "USD").excess > 0
        True
        """
        ...

    def has_excess(self) -> bool:
        """Whether there is excess collateral.

        Returns
        -------
        bool
            True if excess > 0.

        Examples
        --------
        >>> ExcessCollateral(2.0, 1.0, "USD").has_excess()
        True
        """
        ...

    def has_shortfall(self) -> bool:
        """Whether there is a shortfall.

        Returns
        -------
        bool
            True if under-collateralized.

        Examples
        --------
        >>> ExcessCollateral(1.0, 2.0, "USD").has_shortfall()
        True
        """
        ...

    def excess_percentage(self) -> float:
        """Excess as a percentage of required.

        Returns
        -------
        float
            Fractional excess vs required.

        Examples
        --------
        >>> isinstance(ExcessCollateral(110.0, 100.0, "USD").excess_percentage(), float)
        True
        """
        ...

    def __repr__(self) -> str: ...

class MarginFundingCost:
    """Margin funding cost result.

    Parameters
    ----------
    margin_posted : float
        Posted margin amount.
    funding_rate : float
        Funding rate (annualized).
    collateral_rate : float
        Collateral return rate.
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
    ) -> None: ...
    @property
    def margin_posted(self) -> float:
        """Posted margin amount.

        Returns
        -------
        float
            Margin posted.

        Examples
        --------
        >>> MarginFundingCost(1.0, 0.1, 0.0, "USD").margin_posted
        1.0
        """
        ...

    @property
    def funding_rate(self) -> float:
        """Funding rate (annualized).

        Returns
        -------
        float
            Funding rate.

        Examples
        --------
        >>> MarginFundingCost(1.0, 0.06, 0.02, "USD").funding_rate
        0.06
        """
        ...

    @property
    def collateral_rate(self) -> float:
        """Collateral return rate.

        Returns
        -------
        float
            Collateral rate.

        Examples
        --------
        >>> MarginFundingCost(1.0, 0.06, 0.02, "USD").collateral_rate
        0.02
        """
        ...

    @property
    def annual_cost(self) -> float:
        """Annualized funding cost.

        Returns
        -------
        float
            Annual cost amount.

        Examples
        --------
        >>> MarginFundingCost(1e6, 0.05, 0.0, "USD").annual_cost > 0
        True
        """
        ...

    def spread(self) -> float:
        """Funding spread (funding rate − collateral rate).

        Returns
        -------
        float
            Net spread.

        Examples
        --------
        >>> MarginFundingCost(0.0, 0.05, 0.02, "USD").spread()
        0.03
        """
        ...

    def cost_for_period(self, year_fraction: float) -> float:
        """Cost for a specific period.

        Parameters
        ----------
        year_fraction : float
            Length of period in years.

        Returns
        -------
        float
            Cost over the period.

        Examples
        --------
        >>> MarginFundingCost(1e6, 0.04, 0.0, "USD").cost_for_period(0.5) >= 0
        True
        """
        ...

    def __repr__(self) -> str: ...

class Haircut01:
    """Haircut sensitivity: PV change for +1bp haircut change.

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
    ) -> None: ...
    @property
    def collateral_value(self) -> float:
        """Collateral value.

        Returns
        -------
        float
            Collateral mark.

        Examples
        --------
        >>> Haircut01(100.0, 0.1, "USD").collateral_value
        100.0
        """
        ...

    @property
    def current_haircut(self) -> float:
        """Current haircut (decimal).

        Returns
        -------
        float
            Haircut.

        Examples
        --------
        >>> Haircut01(100.0, 0.1, "USD").current_haircut
        0.1
        """
        ...

    @property
    def pv_change(self) -> float:
        """PV change for +1bp haircut.

        Returns
        -------
        float
            Sensitivity amount.

        Examples
        --------
        >>> isinstance(Haircut01(1e6, 0.05, "USD").pv_change, float)
        True
        """
        ...

    def haircut_bps(self) -> float:
        """Current haircut in basis points.

        Returns
        -------
        float
            Haircut in bps.

        Examples
        --------
        >>> Haircut01(1.0, 0.01, "USD").haircut_bps()
        100.0
        """
        ...

    def __repr__(self) -> str: ...

class FrtbSensitivities:
    """FRTB sensitivity portfolio for the Sensitivity-Based Approach.

    Build up delta / vega / curvature inputs with the ``add_*`` methods, then
    pass to :func:`frtb_sba_charge` to compute the capital charge under one or
    more correlation scenarios per BCBS d457.

    Parameters
    ----------
    base_currency : str, default "USD"
        Reporting / base currency ISO code.

    Examples
    --------
    >>> sens = FrtbSensitivities("USD")
    >>> sens.add_girr_delta("5Y", 100_000.0)
    """

    def __init__(self, base_currency: str = "USD") -> None: ...
    @staticmethod
    def from_json(json: str) -> FrtbSensitivities:
        """Construct from a JSON serialization."""
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        ...

    def add_girr_delta(self, tenor: str, amount: float, currency: str | None = None) -> None:
        """Add a GIRR delta sensitivity (currency per 1bp)."""
        ...

    def add_csr_delta(self, issuer: str, bucket: int, tenor: str, amount: float) -> None:
        """Add a CSR (non-securitization) delta sensitivity."""
        ...

    def add_equity_delta(self, underlier: str, bucket: int, amount: float) -> None:
        """Add an equity delta sensitivity."""
        ...

    def add_fx_delta(self, ccy1: str, ccy2: str, amount: float) -> None:
        """Add an FX delta sensitivity for the pair (ccy1, ccy2)."""
        ...

    def add_commodity_delta(self, name: str, bucket: int, tenor: str, amount: float) -> None:
        """Add a commodity delta sensitivity."""
        ...

    def add_girr_vega(
        self,
        option_maturity: str,
        underlying_tenor: str,
        amount: float,
        currency: str | None = None,
    ) -> None:
        """Add a GIRR vega sensitivity."""
        ...

    def add_equity_vega(self, underlier: str, bucket: int, maturity: str, amount: float) -> None:
        """Add an equity vega sensitivity."""
        ...

    def add_fx_vega(self, ccy1: str, ccy2: str, maturity: str, amount: float) -> None:
        """Add an FX vega sensitivity."""
        ...

    def add_girr_curvature(self, cvr_up: float, cvr_down: float, currency: str | None = None) -> None:
        """Add a GIRR curvature sensitivity."""
        ...

    def add_equity_curvature(self, underlier: str, bucket: int, cvr_up: float, cvr_down: float) -> None:
        """Add an equity curvature sensitivity."""
        ...

    def add_fx_curvature(self, ccy1: str, ccy2: str, cvr_up: float, cvr_down: float) -> None:
        """Add an FX curvature sensitivity."""
        ...

    def add_rrao_position(self, instrument_id: str, notional: float, is_exotic: bool = False) -> None:
        """Add a Residual Risk Add-On position."""
        ...

    @property
    def base_currency(self) -> str:
        """Base / reporting currency code."""
        ...

    def __repr__(self) -> str: ...

class FrtbSbaEngine:
    """FRTB SBA engine matching the canonical Rust API."""

    def __init__(self, correlation_scenario: str | None = None) -> None: ...
    def calculate(self, sensitivities: FrtbSensitivities) -> tuple[float, dict[str, Any]]:
        """Calculate the FRTB SBA charge for a sensitivity portfolio."""
        ...

class SaCcrTrade:
    """A derivative trade for SA-CCR EAD computation per BCBS 279.

    Parameters
    ----------
    trade_id : str
        Unique trade identifier.
    asset_class : str
        One of ``"ir"``, ``"fx"``, ``"credit"``, ``"equity"``, ``"commodity"``.
    notional : float
        Adjusted notional in reporting currency.
    start_year, start_month, start_day : int
        Trade start date.
    end_year, end_month, end_day : int
        Trade end / maturity date.
    underlier : str
        Underlier reference (e.g., currency pair, issuer, equity name).
    hedging_set : str
        Hedging-set identifier used for within-class offsetting.
    direction : float, default 1.0
        ``+1.0`` for long, ``-1.0`` for short.
    mtm : float, default 0.0
        Current mark-to-market value.
    """

    def __init__(
        self,
        trade_id: str,
        asset_class: str,
        notional: float,
        start_year: int,
        start_month: int,
        start_day: int,
        end_year: int,
        end_month: int,
        end_day: int,
        underlier: str,
        hedging_set: str,
        direction: float = 1.0,
        mtm: float = 0.0,
    ) -> None: ...
    @staticmethod
    def from_json(json: str) -> SaCcrTrade:
        """Construct from a JSON serialization."""
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        ...

    @property
    def trade_id(self) -> str: ...
    @property
    def asset_class(self) -> str: ...
    @property
    def notional(self) -> float: ...
    @property
    def mtm(self) -> float: ...
    def __repr__(self) -> str: ...

class SaCcrNettingSetConfig:
    """SA-CCR netting-set configuration with explicit valuation date."""

    @staticmethod
    def unmargined(
        counterparty_id: str,
        csa_id: str,
        collateral: float,
        as_of_year: int,
        as_of_month: int,
        as_of_day: int,
    ) -> SaCcrNettingSetConfig: ...
    @staticmethod
    def margined(
        counterparty_id: str,
        csa_id: str,
        collateral: float,
        threshold: float,
        mta: float,
        nica: float,
        mpor_days: int,
        as_of_year: int,
        as_of_month: int,
        as_of_day: int,
    ) -> SaCcrNettingSetConfig: ...
    @staticmethod
    def from_json(json: str) -> SaCcrNettingSetConfig:
        """Construct from a JSON serialization."""
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string."""
        ...

    @property
    def is_margined(self) -> bool: ...
    @property
    def collateral(self) -> float: ...

class SaCcrEngine:
    """SA-CCR EAD engine matching the canonical Rust API."""

    def __init__(self, alpha: float | None = None, reporting_currency: str = "USD") -> None: ...
    def calculate_ead(self, config: SaCcrNettingSetConfig, trades: list[SaCcrTrade]) -> dict[str, Any]:
        """Calculate SA-CCR EAD for a netting set and trade list."""
        ...

def frtb_sba_charge(
    sensitivities: FrtbSensitivities, correlation_scenario: str | None = None
) -> tuple[float, dict[str, Any]]:
    """Compute the FRTB SBA capital charge.

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
    tuple[float, dict]
        ``(total_charge, breakdown)`` where ``breakdown`` has keys
        ``delta``, ``vega``, ``curvature`` (each dict of risk class -> charge),
        plus ``drc``, ``rrao``, ``binding_scenario``, and
        ``scenario_charges``.

    Examples
    --------
    >>> sens = FrtbSensitivities("USD")
    >>> sens.add_girr_delta("5Y", 100_000.0)
    >>> total, breakdown = frtb_sba_charge(sens)
    >>> total > 0.0
    True
    """
    ...

def saccr_ead(trades: list[SaCcrTrade], margined: bool = False, collateral: float = 0.0) -> tuple[float, float, float]:
    """Compute SA-CCR Exposure at Default per BCBS 279.

    Parameters
    ----------
    trades : list[SaCcrTrade]
        Derivative trades making up the netting set.
    margined : bool, default False
        Whether the netting set is subject to a daily margin agreement.
    collateral : float, default 0.0
        Net collateral currently held (positive = bank holds collateral).

    Returns
    -------
    tuple[float, float, float]
        ``(rc, pfe, ead)`` where ``ead = alpha * (rc + pfe)`` with alpha = 1.4.
    """
    ...
