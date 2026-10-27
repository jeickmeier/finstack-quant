"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/types.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.core.config import FinstackConfig

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
