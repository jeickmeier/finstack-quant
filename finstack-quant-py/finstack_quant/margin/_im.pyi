"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/im.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.config import FinstackConfig
from finstack_quant.margin._calculators import ImResult
from finstack_quant.margin._im_curvature import SimmCurvatureSensitivity
from finstack_quant.margin._types import CollateralAssetClass, EligibleCollateralSchedule

class SimmSensitivities:
    """
    ISDA SIMM sensitivity portfolio.

    Stores signed sensitivity amounts by SIMM risk class and bucket. Amounts
    are currency amounts in ``base_currency``, not percentages or spot
    levels: rate and credit deltas are DV01/CS01-style amounts per 1bp move,
    vegas are sigma times dPV/dsigma before VRW, HVR and concentration,
    and curvature preserves factor identity and option expiry before scaling.

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

    def __init__(self, base_currency: str) -> None:
        """
        Create an empty SIMM sensitivity set.

        Parameters
        ----------
        base_currency : str
            Required ISO currency code for the currency in which the sensitivity
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
    def from_dataframe(frame: pd.DataFrame, base_currency: str) -> SimmSensitivities:
        """
        Bulk-load sensitivities from the long-format frame ``to_dataframe``
        emits (CRIF-style).

        Parameters
        ----------
        frame : pd.DataFrame
            Columns ``risk_class``, ``kind``, ``issuer``, ``bucket``,
            ``tenor``, ``amount``, ``expiry_tenor`` with the ``to_dataframe`` encoding:
            ``issuer`` is the currency for ``interest_rate``/``fx`` delta,
            the ``"CCY1/CCY2"`` pair for FX vega, the issuer for credit and
            the underlier for equity; ``bucket`` is the credit sector or the
            commodity bucket; ``tenor`` is the SIMM tenor where the risk
            class has one; ``kind`` is ``delta``, ``vega`` or ``curvature``.
            Curvature requires ``expiry_tenor``, uses ``issuer`` as factor,
            ``bucket`` as the curvature bucket and ``tenor`` as risk tenor.
            Other kinds may omit the expiry column or leave it null.
        base_currency : str
            Currency in which every ``amount`` is expressed (the frame does
            not carry it).

        Returns
        -------
        SimmSensitivities
            Container with rows of the same key accumulated, decoded by the
            Rust ``SimmSensitivities::from_rows`` (the inverse of
            ``to_dataframe``).

        Raises
        ------
        ValueError
            If a risk class, kind, sector or currency is unknown, a required
            column or value is missing, or a curvature row is invalid.
        AttributeError
            If ``frame`` has no ``to_dict`` method (not a ``DataFrame``).

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
            Signed sigma times dPV/dsigma in base_currency, before HVR, VRW
            and concentration. Non-IR sigma follows SIMM paragraph 10(b).

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
            Signed sigma times dPV/dsigma in base_currency, before HVR, VRW
            and concentration. Non-IR sigma follows SIMM paragraph 10(b).

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
            Signed sigma times dPV/dsigma in base_currency, before HVR, VRW
            and concentration. Non-IR sigma follows SIMM paragraph 10(b).

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
            Signed P&L in base_currency per 1% relative price increase,
            equal to price times dPV/dprice times 0.01 before risk weights.

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
            Signed sigma times dPV/dsigma in base_currency, before HVR, VRW
            and concentration. Non-IR sigma follows SIMM paragraph 10(b).

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
            Signed P&L in base_currency per 1% relative price increase,
            equal to price times dPV/dprice times 0.01 before risk weights.

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
            Signed sigma times dPV/dsigma in base_currency, before HVR, VRW
            and concentration. Non-IR sigma follows SIMM paragraph 10(b).

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
            Signed P&L in base_currency per 1% relative price increase,
            equal to price times dPV/dprice times 0.01 before risk weights.

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
            Signed sigma times dPV/dsigma in base_currency, before HVR, VRW
            and concentration. Non-IR sigma follows SIMM paragraph 10(b).

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

    def add_curvature(self, sensitivity: SimmCurvatureSensitivity) -> None:
        """Append an expiry-resolved curvature input before tenor scaling and netting.

        Parameters
        ----------
        sensitivity : SimmCurvatureSensitivity
            Validated factor input in this container's currency, before SF, HVR,
            VRW and concentration. Expiry is preserved even for offsetting vegas.

        Raises
        ------
        TypeError
            If sensitivity is not a SimmCurvatureSensitivity instance.
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
        ``amount``, ``expiry_tenor``. One row per populated bucket; an empty container still
        carries all seven columns. Long format is used deliberately - a column
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
        alone). Curvature uses ``issuer`` for factor identity. ``bucket`` holds the SIMM credit sector for
        bucketed credit deltas (e.g. ``"sovereign"``) and the commodity bucket
        label; it is ``None`` elsewhere. ``tenor`` is the SIMM risk tenor where the risk class has one;
        ``expiry_tenor`` is required for curvature and null for other kinds.

        ``amount`` is a signed currency sensitivity in the container's base
        currency, per 1bp rate/credit or 1% relative equity/FX/commodity move for delta.
        Vega and curvature use sigma times dPV/dsigma before HVR, VRW or SF. ``from_dataframe`` accepts this frame back.

        Rows are sorted by ``(risk_class, kind, issuer, bucket, tenor, expiry_tenor)`` so
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

    @staticmethod
    def from_finstack_config(version: str, config: FinstackConfig) -> SimmCalculator:
        """
        Create a SIMM calculator from a config's margin-registry overlay.

        Parameters
        ----------
        version : str
            Canonical SIMM version label such as ``"v2_6"``.
        config : FinstackConfig
            Config whose ``"margin.registry.v1"`` extension (if any) overlays
            the embedded margin registry; without one the result equals
            ``SimmCalculator(version)``.

        Returns
        -------
        SimmCalculator
            Calculator using the merged registry's SIMM parameters and its
            registry margin period of risk.

        Raises
        ------
        ValueError
            If the version is unknown, or the overlay is malformed or yields
            incomplete SIMM parameters.

        Examples
        --------
        >>> from finstack_quant.core.config import FinstackConfig
        >>> SimmCalculator.from_finstack_config("v2_6", FinstackConfig()).mpor_days
        10
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

    @staticmethod
    def from_finstack_config(config: FinstackConfig) -> ScheduleImCalculator:
        """
        Create the BCBS-IOSCO schedule calculator from a config's
        margin-registry overlay.

        Parameters
        ----------
        config : FinstackConfig
            Config whose ``"margin.registry.v1"`` extension (if any) overlays
            the embedded registry's ``schedule_im`` section; without one the
            result equals ``bcbs_standard()``.

        Returns
        -------
        ScheduleImCalculator
            Calculator built from the merged ``bcbs_iosco`` schedule entry.

        Raises
        ------
        ValueError
            If the overlay is malformed or the merged registry has no
            ``bcbs_iosco`` schedule.

        Examples
        --------
        >>> from finstack_quant.core.config import FinstackConfig
        >>> from finstack_quant.margin import ScheduleImCalculator
        >>> ScheduleImCalculator.from_finstack_config(FinstackConfig()).rate("interest_rate", 5.0)
        0.04
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
