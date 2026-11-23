"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/types/enums.rs``."""

from __future__ import annotations

from finstack_quant.core.currency import Currency
from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.config import FinstackConfig
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from finstack_quant.margin._types.csa import CsaSpec

class ImCollateralResult:
    """One-way IM account with gross model risk and contractual transfer separated.

    Obtain this immutable result from CsaSpec.apply_im_terms. IM remains separate
    from VM; segregation requires custody without reuse to meet VM obligations.

    Examples
    --------
    >>> from finstack_quant.margin import CsaSpec
    >>> csa = CsaSpec.usd_regulatory().with_im("simm", 10, 1000.0, 100.0)
    >>> result = csa.apply_im_terms(2000.0, 950.0)
    >>> result.required_collateral, result.transfer, result.segregated
    (1000.0, 0.0, True)
    """

    @property
    def gross_initial_margin(self) -> float:
        """Gross model IM before terms, in major units of currency.

        Returns
        -------
        float
            Gross model IM before terms, in major units of currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def required_collateral(self) -> float:
        """Target balance after the CSA threshold, in major units of currency.

        Returns
        -------
        float
            Target balance after the CSA threshold, in major units of currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def current_collateral(self) -> float:
        """Existing IM balance, in major units of currency.

        Returns
        -------
        float
            Existing IM balance, in major units of currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def transfer(self) -> float:
        """Signed transfer in major units of currency: positive posts, negative returns; zero below MTA.

        Returns
        -------
        float
            Signed transfer in major units of currency: positive posts, negative returns; zero below MTA.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """ISO currency shared by all account amounts.

        Returns
        -------
        str
            ISO currency shared by all account amounts.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def segregated(self) -> bool:
        """Whether required IM must be segregated without reuse for VM.

        Returns
        -------
        bool
            Whether required IM must be segregated without reuse for VM.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

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
