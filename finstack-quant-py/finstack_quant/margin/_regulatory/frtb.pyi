"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/regulatory/frtb.rs``."""

from __future__ import annotations

from typing import Literal, NotRequired, TypedDict, Unpack
import datetime
from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable
from finstack_quant.margin._types.enums import NettingSetId

class _DrcPositionFields(TypedDict):
    """Input fields for an issuer's non-securitisation default exposure."""

    issuer: str
    maturity_years: float
    jtd_amount: float
    rating_bucket: int
    sector: Literal["sovereign", "corporate", "local_government"]
    seniority: Literal["covered_bond", "senior_unsecured", "subordinated", "equity"]
    pnl_adjustment: NotRequired[float]

class DrcPosition:
    """Default-risk position in FRTB reporting currency, before the LGD adjustment.

    Examples
    --------
    >>> from finstack_quant.margin import DrcPosition
    >>> trade = DrcPosition(
    ...     issuer="ACME",
    ...     maturity_years=2,
    ...     jtd_amount=100000,
    ...     rating_bucket=4,
    ...     sector="corporate",
    ...     seniority="senior_unsecured",
    ... )
    >>> trade.to_dict()["issuer"]
    'ACME'
    """

    def __init__(self, **fields: Unpack[_DrcPositionFields]) -> None:
        """Validate a non-securitisation position for the Default Risk Charge.

        Parameters
        ----------
        fields : _DrcPositionFields
            Nonempty issuer, nonnegative residual maturity in years, signed
            reporting-currency JTD basis before LGD, bucket 1 through 9,
            sector and seniority. P&L adjustment is signed in reporting
            currency and defaults to zero. Equity uses the corporate sector.

        Raises
        ------
        ValueError
            If fields are unknown, malformed or violate the native domains.
        """
        ...

    def to_dict(self) -> _DrcPositionFields:
        """Return the native position fields and the effective P&L adjustment.

        Returns
        -------
        _DrcPositionFields
            Independent copy using regulatory wire names.

        Raises
        ------
        ValueError
            If the native position cannot be exported.
        """
        ...

class FrtbSensitivities:
    """
    FRTB sensitivity portfolio for the Sensitivity-Based Approach.

    Build up delta / vega / curvature / DRC / RRAO inputs with the ``add_*``
    methods (or ``from_dataframe``), then pass to
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
    base_currency : str
        Reporting / base currency ISO code (required).

    Examples
    --------
    >>> sens = FrtbSensitivities("USD")
    >>> sens.add_girr_delta("USD", "USD-OIS", "5Y", 100_000.0)
    """

    def __init__(self, base_currency: str) -> None:
        """
        Create an empty FRTB sensitivity set in one reporting currency.

        Parameters
        ----------
        base_currency : str
            Required recognized ISO-4217 currency code used for all SBA sensitivities
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
        >>> original.add_girr_delta("USD", "USD-OIS", "5Y", 100_000.0)
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
    def from_dataframe(frame: pd.DataFrame | pl.DataFrame | Any, base_currency: str) -> FrtbSensitivities:
        """
        Bulk-load sensitivities from the long-format table ``to_arrow`` /
        ``to_pandas`` emits.

        Parameters
        ----------
        frame : pd.DataFrame | pl.DataFrame | Any
            Pandas ``DataFrame``, Polars ``DataFrame``, or Arrow table with
            columns ``risk_class``, ``kind``, ``issuer``, ``bucket``,
            ``tenor``, ``amount`` (plus ``curve_id`` required on ordinary GIRR
            delta, and DRC columns ``sector``, ``seniority``,
            ``maturity_years``, ``pnl_adjustment`` on ``drc``
            rows) encoded as ``to_arrow`` documents.
            ``curvature_up`` / ``curvature_down`` rows are recombined into
            pairs; ``rrao`` rows carry ``exotic_notional`` /
            ``other_notional``; each ``drc`` row is one default-risk
            position (a missing ``pnl_adjustment`` reads as ``0.0``).
        base_currency : str
            Reporting currency of every ``amount`` (the table does not
            carry it).

        Returns
        -------
        FrtbSensitivities
            Container with rows of the same key accumulated, decoded by the
            Rust ``FrtbSensitivities::from_rows`` (the inverse of
            ``to_arrow``).

        Raises
        ------
        ValueError
            If a risk class or kind is unknown, a required column or value
            is missing, or a bucket, currency or DRC label is invalid.
        TypeError
            If ``frame`` is not a pandas/Polars DataFrame or Arrow table.

        Examples
        --------
        >>> original = FrtbSensitivities("USD")
        >>> original.add_girr_delta("USD", "USD-OIS", "5Y", 100_000.0)
        >>> original.add_drc_position({
        ...     "issuer": "ACME",
        ...     "jtd_amount": 1_000_000.0,
        ...     "rating_bucket": 3,
        ...     "sector": "corporate",
        ...     "seniority": "senior_unsecured",
        ...     "maturity_years": 1.0,
        ...     "pnl_adjustment": 0.0,
        ... })
        >>> restored = FrtbSensitivities.from_dataframe(original.to_arrow(), "USD")
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
        >>> sens.add_girr_delta("USD", "USD-OIS", "5Y", 100_000.0)
        >>> sens.validate()
        """
        ...

    def add_girr_delta(self, currency: str, curve_id: str, tenor: str, amount: float) -> None:
        """Add an ordinary GIRR yield delta at one qualified curve vertex.

        Parameters
        ----------
        currency : str
            ISO-4217 risk currency used to select the GIRR currency bucket.
        curve_id : str
            Exact yield curve identifier, such as USD-OIS or USD-SOFR-3M.
        tenor : str
            Case-sensitive regulatory yield vertex such as 5Y.
        amount : float
            Signed reporting-currency P&L from the upward one-basis-point
            reprice, multiplied by 100. Matching vertices accumulate.

        Raises
        ------
        TypeError
            If an argument has an incompatible Python type.
        ValueError
            If currency, curve identifier, tenor, amount or its sum is invalid.
            Invalid contributions leave existing sensitivities unchanged.
        """
        ...

    def add_girr_inflation_delta(self, currency: str, amount: float) -> None:
        """
        Add a GIRR inflation delta sensitivity.

        Parameters
        ----------
        amount : float
            Base-currency P&L per 1 percentage point of inflation shift.
        currency : str
            Currency of the inflation curve; identifies the shocked risk factor.
        Raises
        ------
        ValueError
            If ``currency`` is not a recognized ISO currency code.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_girr_inflation_delta("USD", 1_000.0)
        >>> sens.validate()
        """
        ...

    def add_girr_xccy_basis_delta(self, currency: str, amount: float) -> None:
        """
        Add a GIRR cross-currency basis delta sensitivity.

        Parameters
        ----------
        amount : float
            Base-currency P&L per 1 percentage point of basis shift.
        currency : str
            Currency whose basis moves; identifies the shocked risk factor.
        Raises
        ------
        ValueError
            If ``currency`` is not a recognized ISO currency code.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_girr_xccy_basis_delta("EUR", 500.0)
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

    def add_girr_vega(self, currency: str, option_maturity: str, underlying_tenor: str, amount: float) -> None:
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
        currency : str
            Currency code; identifies the shocked risk factor.

        Raises
        ------
        ValueError
            If ``currency`` is not a recognized ISO currency code.
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

    def add_girr_curvature(self, currency: str, cvr_up: float, cvr_down: float) -> None:
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
        currency : str
            Currency code; identifies the shocked risk factor.

        Raises
        ------
        ValueError
            If ``currency`` is not a recognized ISO currency code.
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

    def add_drc_position(self, position: DrcPosition | dict[str, Any]) -> None:
        """Add the canonical Rust DrcPosition object.

        Parameters
        ----------
        position : dict[str, Any]
            issuer, signed jtd_amount in reporting currency before LGD,
            rating_bucket (1..9), sector (sovereign, corporate, local_government),
            seniority (covered_bond, senior_unsecured, subordinated, equity),
            non-negative maturity_years and optional pnl_adjustment (default 0).
            Equity requires corporate sector; securitisations are unsupported.

        Raises
        ------
        ValueError
            For malformed or unknown fields, or invalid Rust position terms.
        TypeError
            If the position contains values that cannot be converted to JSON.
        """
        ...

    def add_rrao_position(self, instrument_id: str, notional: float, is_exotic: bool) -> None:
        """
        Add a Residual Risk Add-On position.

        Parameters
        ----------
        instrument_id : str
            Instrument identifier.
        notional : float
            Gross notional in base currency.
        is_exotic : bool
            Required. ``True`` for an exotic underlying (1.0% weight); ``False`` for
            other residual risk such as gap, correlation or behavioural risk
            (0.1% weight).

        Notes
        -----
        This method does not raise; it updates stored state in place.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_rrao_position("EXOTIC-1", 5_000_000.0, True)
        >>> FrtbSbaEngine().calculate(sens).rrao
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
    def to_arrow(self) -> ArrowTable:
        """
        Export every populated sensitivity bucket as one long-format
        :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``risk_class``, ``bucket``, ``tenor``, ``issuer``, ``kind``,
        ``amount``, the ordinary GIRR ``curve_id``, then the DRC columns ``sector``, ``seniority``,
        ``maturity_years``, ``pnl_adjustment`` (null on non-DRC rows). One row per populated bucket; an empty
        container still carries all columns. Long format is used
        deliberately - a column per bucket would give a different schema for
        every portfolio.

        ``risk_class`` uses the same labels as the ``FrtbSbaResult``
        breakdown (``girr``, ``csr_non_sec``, ``csr_sec_ctp``,
        ``csr_sec_non_ctp``, ``equity``, ``commodity``, ``fx``), plus ``drc``
        and ``rrao`` for the two position lists.

        ``kind`` is ``delta``, ``vega``, ``repo_delta``, ``curvature_up``,
        ``curvature_down``, ``inflation_delta``, ``xccy_basis_delta``,
        ``jtd`` (DRC notional), or ``exotic_notional`` / ``other_notional``
        (RRAO). A curvature pair is split across two rows so ``amount`` stays
        scalar.

        ``issuer`` carries the name axis: a currency code for GIRR, a
        ``"CCY1/CCY2"`` pair for FX, an issuer, tranche, underlier, commodity
        name, or instrument id elsewhere. ``bucket`` is the FRTB bucket index
        (or DRC rating bucket) as a **string** (``pd.to_numeric`` if you need
        it numeric); ``tenor`` is the tenor or option maturity,
        ``"{tenor}/{basis}"`` for CSR and commodity deltas, and for GIRR vega the
        ``"{option_maturity}/{underlying_tenor}"`` pair. Both are ``None``
        where the risk class has no such axis.

        ``amount`` keeps each bucket's own convention: GIRR deltas are
        base-currency P&L per **1 percentage point** of curve shift (that is,
        ``100 x DV01``), DRC rows are signed JTD notionals before LGD, and
        RRAO rows are gross notionals. ``from_dataframe`` accepts this table
        back, ``drc`` rows included.

        Rows are sorted by ``(risk_class, kind, issuer, bucket, tenor)`` and
        then the DRC columns, so repeated exports of the same portfolio are
        identical.

        Returns
        -------
        ArrowTable
            One row per populated sensitivity bucket.

        Raises
        ------
        ValueError
            If the result cannot be serialized into an Arrow table.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export every populated sensitivity bucket as one long-format pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            One row per populated sensitivity bucket.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export every populated sensitivity bucket as one long-format Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            One row per populated sensitivity bucket.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export every populated sensitivity bucket as one long-format ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row per populated sensitivity bucket.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...

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
    >>> sensitivities.add_girr_delta("USD", "USD-OIS", "5Y", 100_000.0)
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
        >>> sens.add_girr_delta("USD", "USD-OIS", "5Y", 100_000.0)
        >>> FrtbSbaEngine().calculate(sens).binding_scenario
        'low'
        """
        ...

    def __repr__(self) -> str: ...

class FrtbSbaResult:
    """
    FRTB SBA capital-charge result (BCBS d457).

    Returned by :meth:`FrtbSbaEngine.calculate`.
    Amounts are floats in the sensitivity portfolio's base currency.

    Examples
    --------
    >>> from finstack_quant.margin import FrtbSbaEngine, FrtbSensitivities
    >>> sens = FrtbSensitivities("USD")
    >>> sens.add_girr_delta("USD", "USD-OIS", "5Y", 100_000.0)
    >>> result = FrtbSbaEngine().calculate(sens)
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
    def scenario_breakdown(self) -> dict[str, dict[str, dict[str, float]]]:
        """
        Per-risk-class charges under every evaluated correlation scenario.

        Returns
        -------
        dict[str, dict[str, dict[str, float]]]
            Keyed by scenario (``"low"``, ``"medium"``, ``"high"``); each value
            has ``delta_by_risk_class``, ``vega_by_risk_class`` and
            ``curvature_by_risk_class`` maps keyed by risk-class wire label.
            A scenario's maps sum to its ``scenario_charges`` entry, and the
            binding scenario's maps equal the top-level ``*_by_risk_class``
            properties.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> sens = FrtbSensitivities("USD")
        >>> sens.add_girr_delta("USD", "USD-OIS", "5Y", 1.0)
        >>> result = FrtbSbaEngine().calculate(sens)
        >>> sorted(result.scenario_breakdown) == sorted(result.scenario_charges)
        True
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
        >>> sens.add_girr_delta("USD", "USD-OIS", "5Y", 1.0)
        >>> result = FrtbSbaEngine().calculate(sens)
        >>> "numeric_mode" in result.meta
        True
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export the headline charge as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``total``, ``drc``, ``rrao`` (floats) and
            ``binding_scenario`` (string). One row.

        Notes
        -----
        This method does not raise; it derives the value from stored state.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export the headline charge as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Columns ``total``, ``drc``, ``rrao`` (floats) and
            ``binding_scenario`` (string). One row.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export the headline charge as a single-row Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Columns ``total``, ``drc``, ``rrao`` and ``binding_scenario``.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export the headline charge as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row describing the headline SBA charge.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def to_breakdown_table(self) -> ArrowTable:
        """
        Export the per-risk-class breakdown as a long-format :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``component`` (``"delta"``/``"vega"``/``"curvature"``),
            ``risk_class`` and ``charge``. Components do not sum to ``total``:
            SBA aggregates them with prescribed correlations, and ``drc`` /
            ``rrao`` sit outside this table.

        Notes
        -----
        This method does not raise; it derives the value from stored state.
        """
        ...

    def to_scenario_table(self) -> ArrowTable:
        """
        Export the per-scenario SBA charges as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
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
        >>> sens.add_girr_delta("USD", "USD-OIS", "5Y", 100_000.0)
        >>> FrtbSbaEngine().calculate(sens).to_scenario_table().to_dict()["scenario"]
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
