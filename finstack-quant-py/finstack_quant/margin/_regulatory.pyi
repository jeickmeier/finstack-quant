"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/regulatory.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.margin._types import NettingSetId

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
    base_currency : str
        Reporting / base currency ISO code (required).

    Examples
    --------
    >>> sens = FrtbSensitivities("USD")
    >>> sens.add_girr_delta("USD", "5Y", 100_000.0)
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
        >>> original.add_girr_delta("USD", "5Y", 100_000.0)
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
    def from_dataframe(frame: pd.DataFrame, base_currency: str) -> FrtbSensitivities:
        """
        Bulk-load sensitivities from the long-format frame ``to_dataframe``
        emits.

        Parameters
        ----------
        frame : pd.DataFrame
            Columns ``risk_class``, ``kind``, ``issuer``, ``bucket``,
            ``tenor``, ``amount`` (plus ``sector``, ``seniority``,
            ``maturity_years``, ``pnl_adjustment`` on ``drc``
            rows) encoded as ``to_dataframe`` documents.
            ``curvature_up`` / ``curvature_down`` rows are recombined into
            pairs; ``rrao`` rows carry ``exotic_notional`` /
            ``other_notional``; each ``drc`` row is one default-risk
            position (a missing ``pnl_adjustment`` reads as ``0.0``).
        base_currency : str
            Reporting currency of every ``amount`` (the frame does not
            carry it).

        Returns
        -------
        FrtbSensitivities
            Container with rows of the same key accumulated, decoded by the
            Rust ``FrtbSensitivities::from_rows`` (the inverse of
            ``to_dataframe``).

        Raises
        ------
        ValueError
            If a risk class or kind is unknown, a required column or value
            is missing, or a bucket, currency or DRC label is invalid.
        AttributeError
            If ``frame`` has no ``to_dict`` method (not a ``DataFrame``).

        Examples
        --------
        >>> original = FrtbSensitivities("USD")
        >>> original.add_girr_delta("USD", "5Y", 100_000.0)
        >>> original.add_drc_position({
        ...     "issuer": "ACME",
        ...     "jtd_amount": 1_000_000.0,
        ...     "rating_bucket": 3,
        ...     "sector": "corporate",
        ...     "seniority": "senior_unsecured",
        ...     "maturity_years": 1.0,
        ...     "pnl_adjustment": 0.0,
        ... })
        >>> restored = FrtbSensitivities.from_dataframe(original.to_dataframe(), "USD")
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
        >>> sens.add_girr_delta("USD", "5Y", 100_000.0)
        >>> sens.validate()
        """
        ...

    def add_girr_delta(self, currency: str, tenor: str, amount: float) -> None:
        """
        Add a GIRR delta sensitivity (currency P&L per 1 percentage-point move).

        Parameters
        ----------
        tenor : str
            GIRR tenor bucket, such as ``"5Y"``.
        amount : float
            Signed sensitivity amount per 1 percentage-point move (``100 * DV01``).
        currency : str
            Currency code; identifies the shocked risk factor.

        Raises
        ------
        ValueError
            If ``currency`` is not a recognized ISO currency code.
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

    def add_drc_position(self, position: dict[str, Any]) -> None:
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
        ``amount``, then the DRC columns ``sector``, ``seniority``,
        ``maturity_years``, ``pnl_adjustment`` (``None`` /
        ``NaN`` on non-DRC rows). One row per populated bucket; an empty
        container still carries all ten columns. Long format is used
        deliberately - a column per bucket would give a different schema for
        every portfolio.

        ``risk_class`` uses the same labels as the ``frtb_sba_charge``
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
        RRAO rows are gross notionals. ``from_dataframe`` accepts this frame
        back, ``drc`` rows included.

        Rows are sorted by ``(risk_class, kind, issuer, bucket, tenor)`` and
        then the DRC columns, so repeated exports of the same portfolio are
        identical.

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
    >>> sensitivities.add_girr_delta("USD", "5Y", 100_000.0)
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
        >>> sens.add_girr_delta("USD", "5Y", 100_000.0)
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
    >>> sens.add_girr_delta("USD", "5Y", 100_000.0)
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
        >>> sens.add_girr_delta("USD", "5Y", 1.0)
        >>> result = frtb_sba_charge(sens)
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
        >>> sens.add_girr_delta("USD", "5Y", 1.0)
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
        >>> sens.add_girr_delta("USD", "5Y", 100_000.0)
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
    is_option : bool
        Whether the trade is an option (required, as in the wire form).
    option_type : str
        ``"call_long"``, ``"call_short"``, ``"put_long"`` or
        ``"put_short"``; required when ``is_option`` is ``True``.

    supervisory_category : str
        Required for credit, equity and commodity; absent for IR/FX. Labels:
        ``credit_aa``, ``credit_a``, ``credit_bbb``, ``credit_bb``, ``credit_b``,
        ``credit_ccc``, ``credit_index_ig``, ``credit_index_hy``,
        ``equity_single_name``, ``equity_index``, ``commodity_electricity``,
        ``commodity_other``. Selects the supervisory factor and correlation.
    option_maturity_date : datetime.date | str
        Required option expiry, at or before the underlying end date. Absent
        for linear trades. Separates option maturity from underlying duration.

    Examples
    --------
    >>> from finstack_quant.margin import SaCcrTrade
    >>> trade = SaCcrTrade(
    ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
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
        is_option: bool,
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
        is_option : bool
            Whether the trade is an option.
        option_type : str | None, optional
            Option type label, required when ``is_option`` is ``True``.

        supervisory_category : str | None, optional
            Required for credit, equity and commodity; absent for IR/FX. Labels:
            ``credit_aa``, ``credit_a``, ``credit_bbb``, ``credit_bb``, ``credit_b``,
            ``credit_ccc``, ``credit_index_ig``, ``credit_index_hy``,
            ``equity_single_name``, ``equity_index``, ``commodity_electricity``,
            ``commodity_other``. Selects the supervisory factor and correlation.
        option_maturity_date : datetime.date | str
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
            ``direction``, ``supervisory_delta``, ``mtm``, ``is_option``;
            ``option_type``, ``supervisory_category`` and
            ``option_maturity_date`` may be null. Dates may be ISO strings or
            date-like values.

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
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
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
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
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
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
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
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
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
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
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
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
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
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
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
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
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
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
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
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
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
        ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
        ... )
        >>> round(SaCcrEngine().calculate_ead(config, [trade]).ead, 2)
        30982.83
        """
        ...

    def __repr__(self) -> str: ...

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
    >>> sens.add_girr_delta("USD", "5Y", 100_000.0)
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
    ...     "t1", "interest_rate", 1_000_000, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False
    ... )
    >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-01")
    >>> result = saccr_ead([trade], config)
    >>> (round(result.rc, 2), round(result.pfe, 2), round(result.ead, 2))
    (0.0, 22130.59, 30982.83)
    """
    ...
