"""
Distressed-exchange hold-versus-tender economics and issuer LME analytics.

Examples
--------
>>> from finstack_quant.models.credit import liability_management
>>> analysis = liability_management.analyze_exchange_offer(60.0, 75.0, 2.0, 0.0, "par_for_par")
>>> (analysis.delta_npv, analysis.tender_recommended)
(17.0, True)

"""

from __future__ import annotations

from typing import Any

import pandas

__all__ = [
    "ExchangeOfferAnalysis",
    "LeverageImpact",
    "LmeAnalysis",
    "TENDER_RECOMMENDATION_HURDLE",
    "analyze_exchange_offer",
    "analyze_lme",
]

TENDER_RECOMMENDATION_HURDLE: float
"""Multiple of ``old_npv`` that ``tender_total`` must exceed for a tender
recommendation (``1.02``, i.e. a 2% pickup hurdle)."""

class ExchangeOfferAnalysis:
    """
    Hold-versus-tender economics of a distressed exchange offer.

    Examples
    --------
    >>> from finstack_quant.models.credit import liability_management
    >>> analysis = liability_management.analyze_exchange_offer(60.0, 75.0, 2.0, 0.0, "par_for_par")
    >>> (analysis.delta_npv, analysis.tender_recommended)
    (17.0, True)

    """

    @property
    def exchange_type(self) -> str:
        """
        Return the canonical exchange structure for this analysis.

        Returns
        -------
        str
            One of ``par_for_par``, ``discount``, ``uptier``, ``downtier``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def old_npv(self) -> float:
        """
        Return the hold-out present value used in the comparison.

        Returns
        -------
        float
            Present value of the existing claim if it is not tendered.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def new_npv(self) -> float:
        """
        Return the present value of the new instrument offered.

        Returns
        -------
        float
            Present value received on tendering, excluding fees.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def consent_fee(self) -> float:
        """
        Return the cash consent or early-tender fee.

        Returns
        -------
        float
            Fee paid to participating holders, in the input unit.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def equity_sweetener_value(self) -> float:
        """
        Return the value of equity or warrants attached to the offer.

        Returns
        -------
        float
            Estimated sweetener value, in the input unit.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tender_total(self) -> float:
        """
        Return the total tender consideration.

        Returns
        -------
        float
            ``new_npv + consent_fee + equity_sweetener_value``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def delta_npv(self) -> float:
        """
        Return the NPV pickup from tendering.

        Returns
        -------
        float
            ``tender_total - old_npv``; negative when holding out wins.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def breakeven_recovery(self) -> float:
        """
        Return the hold-out recovery that matches the tender.

        Returns
        -------
        float
            Fraction of the hold-out present value, capped at ``1.0``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tender_recommended(self) -> bool:
        """
        Return whether the offer clears the 2% tender hurdle.

        Returns
        -------
        bool
            True when ``tender_total > old_npv * 1.02``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pandas.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Columns: ``exchange_type``, ``old_npv``, ``new_npv``,
        ``consent_fee``, ``equity_sweetener_value``, ``tender_total``,
        ``delta_npv``, ``breakeven_recovery``, ``tender_recommended``.

        One offer is one flat record, so a one-row frame is the right
        shape: ``pd.concat`` over several candidate offers gives a
        hold-versus-tender comparison table directly.

        Returns
        -------
        pandas.DataFrame
            Single-row frame of the offer's hold-versus-tender economics.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ExchangeOfferAnalysis:
        """
        Deserialize a ``ExchangeOfferAnalysis`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        ExchangeOfferAnalysis
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``ExchangeOfferAnalysis`` JSON.

        Examples
        --------
        >>> value = liability_management.analyze_exchange_offer(60.0, 75.0, 2.0, 0.0, "par_for_par")
        >>> liability_management.ExchangeOfferAnalysis.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

class LeverageImpact:
    """
    Gross-leverage impact of a liability management exercise.

    Examples
    --------
    >>> from finstack_quant.models.credit import liability_management
    >>> impact = liability_management.analyze_lme("open_market_repurchase", 100.0, 0.70, 0.50, 20.0).leverage_impact
    >>> (impact.pre_leverage, impact.post_leverage)
    (5.0, 2.5)

    """

    @property
    def pre_total_debt(self) -> float:
        """
        Return gross debt of the target instrument before the exercise.

        Returns
        -------
        float
            Outstanding face amount, in the input unit.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def post_total_debt(self) -> float:
        """
        Return gross debt of the target instrument after the exercise.

        Returns
        -------
        float
            Face amount remaining once retired par is removed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pre_leverage(self) -> float:
        """
        Return gross debt over EBITDA before the exercise.

        Returns
        -------
        float
            Leverage as a multiple, so ``8.0`` reads as 8.0x.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def post_leverage(self) -> float:
        """
        Return gross debt over EBITDA after the exercise.

        Returns
        -------
        float
            Leverage as a multiple, so ``4.8`` reads as 4.8x.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def leverage_reduction(self) -> float:
        """
        Return the turns of leverage removed by the exercise.

        Returns
        -------
        float
            ``pre_leverage - post_leverage``, in turns.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LeverageImpact:
        """
        Deserialize a ``LeverageImpact`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        LeverageImpact
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``LeverageImpact`` JSON.

        Examples
        --------
        >>> value = liability_management.analyze_lme("tender_offer", 100.0, 0.8, 1.0, ebitda=20.0).leverage_impact
        >>> liability_management.LeverageImpact.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def to_dataframe(self) -> pandas.DataFrame:
        """
        Single-row frame with ``pre_total_debt``, ``post_total_debt``, ``pre_leverage``, ``post_leverage``, ``leverage_reduction``.

        Returns
        -------
        pandas.DataFrame
            Single-row frame with ``pre_total_debt``, ``post_total_debt``, ``pre_leverage``, ``post_leverage``, ``leverage_reduction``.

        Raises
        ------
        ValueError
            If the value cannot be serialized into a pandas object.
        """
        ...

class LmeAnalysis:
    """
    Issuer-side economics of a liability management exercise.

    Examples
    --------
    >>> from finstack_quant.models.credit import liability_management
    >>> analysis = liability_management.analyze_lme("open_market_repurchase", 100.0, 0.70, 0.50)
    >>> (analysis.notional_reduction, analysis.discount_capture)
    (50.0, 15.0)

    """

    @property
    def lme_type(self) -> str:
        """
        Return the canonical LME structure for this analysis.

        Returns
        -------
        str
            One of ``open_market_repurchase``, ``tender_offer``,
            ``amend_and_extend``, ``dropdown``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cost(self) -> float:
        """
        Return the cash paid by the issuer.

        Returns
        -------
        float
            Repurchase consideration or consent fees, in the input unit.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def notional_reduction(self) -> float:
        """
        Return the face amount retired by the exercise.

        Returns
        -------
        float
            Par extinguished; zero for amend-and-extend and dropdowns.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def discount_capture(self) -> float:
        """
        Return the discount captured by the issuer.

        Returns
        -------
        float
            ``notional_reduction - cost``, in the input unit.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def discount_capture_pct(self) -> float:
        """
        Return the discount captured as a fraction of par retired.

        Returns
        -------
        float
            Fraction in ``[0, 1]``; zero when no par is retired.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def remaining_holder_impact_pct(self) -> float:
        """
        Return the value fraction diverted from non-participating holders.

        Returns
        -------
        float
            Nonzero only for a dropdown transaction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def leverage_impact(self) -> LeverageImpact | None:
        """
        Return the gross-leverage block, when EBITDA was supplied.

        Returns
        -------
        LeverageImpact or None
            None when no positive EBITDA was provided.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pandas.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Columns: ``lme_type``, ``cost``, ``notional_reduction``,
        ``discount_capture``, ``discount_capture_pct``,
        ``remaining_holder_impact_pct``, ``pre_total_debt``,
        ``post_total_debt``, ``pre_leverage``, ``post_leverage``,
        ``leverage_reduction``.

        One exercise is one flat record, so a one-row frame is the right
        shape: ``pd.concat`` over several structures gives a
        discount-capture comparison table directly.

        The five leverage columns come from :attr:`leverage_impact` and are
        flattened onto the same row rather than nested. They are ``None``
        (and therefore ``object`` dtype) when no positive EBITDA was
        supplied; coerce with ``pd.to_numeric`` before aggregating a mixed
        set.

        Returns
        -------
        pandas.DataFrame
            Single-row frame of the exercise's issuer-side economics.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LmeAnalysis:
        """
        Deserialize a ``LmeAnalysis`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        LmeAnalysis
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``LmeAnalysis`` JSON.

        Examples
        --------
        >>> value = liability_management.analyze_lme("tender_offer", 100.0, 0.8, 1.0)
        >>> liability_management.LmeAnalysis.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

def analyze_exchange_offer(
    old_pv: float,
    new_pv: float,
    consent_fee: float,
    equity_sweetener_value: float,
    exchange_type: str,
) -> ExchangeOfferAnalysis:
    """
    Compare hold-versus-tender economics for a distressed exchange offer.

    Parameters
    ----------
    old_pv : float
        Present value of the existing claim if it is not tendered, in the
        caller's monetary unit. Must be finite and non-negative.
    new_pv : float
        Present value of the new instrument received on tendering,
        expressed in the same unit as ``old_pv``.
    consent_fee : float
        Cash consent or early-tender fee paid to participating holders, in
        the same unit as ``old_pv``; pass ``0.0`` when there is none.
    equity_sweetener_value : float
        Estimated value of equity or warrants attached to the new
        instrument, in the same unit as ``old_pv``; pass ``0.0`` when there
        is none.
    exchange_type : str
        Offer structure, as the canonical snake_case label:
        ``par_for_par``, ``discount``, ``uptier``, or ``downtier``. Any
        other spelling raises. Every argument is required, as in Rust and
        WASM.

    Returns
    -------
    ExchangeOfferAnalysis
        Tender total, NPV pickup, breakeven recovery, and the tender
        recommendation against the 2% hurdle.

    Raises
    ------
    ValueError
        If an amount is negative or non-finite, or ``exchange_type`` is not
        a recognised structure.

    Examples
    --------
    >>> from finstack_quant.models.credit import liability_management
    >>> liability_management.analyze_exchange_offer(60.0, 75.0, 2.0, 0.0, "par_for_par").tender_total
    77.0

    """
    ...

def analyze_lme(
    lme_type: str,
    notional: float,
    repurchase_price_pct: float,
    opt_acceptance_pct: float,
    ebitda: float | None = None,
) -> LmeAnalysis:
    """
    Compute discount capture and leverage impact for an LME transaction.

    Parameters
    ----------
    lme_type : str
        Structure of the exercise, as the canonical snake_case label:
        ``open_market_repurchase``, ``tender_offer``, ``amend_and_extend``,
        or ``dropdown``. Any other spelling raises.
    notional : float
        Outstanding face amount of the target instrument, in the caller's
        monetary unit. Must be finite and strictly positive.
    repurchase_price_pct : float
        Price as a fraction of par for repurchases and tenders (``(0, 1.5]``),
        the extension fee for amend-and-extend (``[0, 0.10]``), or the
        transferred-asset fraction for a dropdown (``[0, 1]``).
    opt_acceptance_pct : float
        Fraction of holders participating, in ``[0, 1]``; required, as in
        Rust and WASM.
    ebitda : float or None, optional
        EBITDA in the same unit as ``notional``. A positive value adds the
        ``leverage_impact`` block; None or a non-positive value omits it.

    Returns
    -------
    LmeAnalysis
        Cash cost, par retired, discount captured, impact on remaining
        holders, and the optional gross-leverage block.

    Raises
    ------
    ValueError
        If ``notional`` is not positive, ``opt_acceptance_pct`` is outside
        ``[0, 1]``, ``repurchase_price_pct`` is outside the range admitted
        by ``lme_type``, or ``lme_type`` is not recognised.

    Examples
    --------
    >>> from finstack_quant.models.credit import liability_management
    >>> liability_management.analyze_lme("open_market_repurchase", 100.0, 0.70, 0.50).discount_capture
    15.0

    """
    ...
