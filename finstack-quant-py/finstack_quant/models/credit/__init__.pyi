"""
Product-independent credit models, scoring, migration, PD, LGD, recovery, and
liability-management analytics.

Bindings for ``finstack_quant_models::credit``. Each submodule mirrors the Rust
module of the same name and is registered at runtime in ``sys.modules``
so that ``from finstack_quant.models.credit import scoring`` (or ``pd``, ``lgd``,
``migration``, ``recovery_waterfall``, ``liability_management``) works
transparently.

Note that the ``pd`` submodule (probability of default) shadows the common
``import pandas as pd`` alias; import it under another name, e.g.
``from finstack_quant.models.credit import pd as pdm``.

Examples
--------
>>> from finstack_quant.models.credit import pd as pdm
>>> pdm.central_tendency([0.01, 0.02, 0.03])
0.02

"""

from __future__ import annotations

from typing import Any

import pandas

from finstack_quant.core.types import CreditRating
from finstack_quant.models.credit._structural import (
    AssetDynamics as AssetDynamics,
    BarrierType as BarrierType,
    CreditState as CreditState,
    DynamicRecoverySpec as DynamicRecoverySpec,
    EndogenousHazardSpec as EndogenousHazardSpec,
    MertonModel as MertonModel,
    RatingFactorTable as RatingFactorTable,
    SimulatedPaths as SimulatedPaths,
    ToggleExerciseModel as ToggleExerciseModel,
)

__all__ = [
    "AssetDynamics",
    "BarrierType",
    "CreditState",
    "DynamicRecoverySpec",
    "EndogenousHazardSpec",
    "MertonModel",
    "RatingFactorTable",
    "SimulatedPaths",
    "ToggleExerciseModel",
    "lgd",
    "liability_management",
    "migration",
    "moodys_warf_factor",
    "pd",
    "recovery_waterfall",
    "scoring",
]

def moodys_warf_factor(rating: str | CreditRating) -> float:
    """
    Return the Moody's WARF factor for an exact canonical credit-rating notch.

    Parameters
    ----------
    rating : str | CreditRating
        Canonical rating from :mod:`finstack_quant.core.types`, or a rating
        string in S&P/Fitch (``"BBB-"``) or Moody's (``"Baa3"``) notation.

    Returns
    -------
    float
        Moody's ordinal weighted-average rating factor.

    Raises
    ------
    ValueError
        If the string is not a recognised rating, the embedded
        credit-assumptions registry is invalid, or the rating has no factor in
        the configured Moody's table.
    TypeError
        If ``rating`` is neither a string nor a ``CreditRating``.

    Examples
    --------
    >>> from finstack_quant.core.types import CreditRating
    >>> from finstack_quant.models.credit import moodys_warf_factor
    >>> moodys_warf_factor(CreditRating.B)
    2720.0
    >>> moodys_warf_factor("B")
    2720.0
    """
    ...

class liability_management:
    """
    Distressed-exchange hold-versus-tender economics and issuer LME analytics.

    Examples
    --------
    >>> from finstack_quant.models.credit import liability_management
    >>> analysis = liability_management.analyze_exchange_offer(60.0, 75.0, consent_fee=2.0)
    >>> (analysis.delta_npv, analysis.tender_recommended)
    (17.0, True)

    """

    TENDER_RECOMMENDATION_HURDLE: float
    """Multiple of ``old_npv`` that ``tender_total`` must exceed for a tender
    recommendation (``1.02``, i.e. a 2% pickup hurdle)."""

    class ExchangeOfferAnalysis:
        """
        Hold-versus-tender economics of a distressed exchange offer.

        Examples
        --------
        >>> from finstack_quant.models.credit import liability_management
        >>> analysis = liability_management.analyze_exchange_offer(60.0, 75.0, consent_fee=2.0)
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
        def from_json(json: str) -> liability_management.ExchangeOfferAnalysis:
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
            >>> value = liability_management.analyze_exchange_offer(60.0, 75.0, consent_fee=2.0)
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
        def from_json(json: str) -> liability_management.LeverageImpact:
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
            >>> value = liability_management.analyze_lme("tender_offer", 100.0, 0.8, ebitda=20.0).leverage_impact
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
        def leverage_impact(self) -> liability_management.LeverageImpact | None:
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
        def from_json(json: str) -> liability_management.LmeAnalysis:
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
            >>> value = liability_management.analyze_lme("tender_offer", 100.0, 0.8)
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

    @staticmethod
    def analyze_exchange_offer(
        old_pv: float,
        new_pv: float,
        consent_fee: float = 0.0,
        equity_sweetener_value: float = 0.0,
        exchange_type: str = "par_for_par",
    ) -> liability_management.ExchangeOfferAnalysis:
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
        consent_fee : float, optional
            Cash consent or early-tender fee paid to participating holders, in
            the same unit as ``old_pv``.
        equity_sweetener_value : float, optional
            Estimated value of equity or warrants attached to the new
            instrument, in the same unit as ``old_pv``.
        exchange_type : str, optional
            Offer structure: ``par_for_par`` (alias ``par``), ``discount``,
            ``uptier``, or ``downtier``. Case-insensitive; ``-`` is normalised
            to ``_``.

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
        >>> liability_management.analyze_exchange_offer(60.0, 75.0, consent_fee=2.0).tender_total
        77.0

        """
        ...

    @staticmethod
    def analyze_lme(
        lme_type: str,
        notional: float,
        repurchase_price_pct: float,
        opt_acceptance_pct: float = 1.0,
        ebitda: float | None = None,
    ) -> liability_management.LmeAnalysis:
        """
        Compute discount capture and leverage impact for an LME transaction.

        Parameters
        ----------
        lme_type : str
            Structure of the exercise: ``open_market`` (aliases
            ``open_market_repurchase``, ``omr``), ``tender_offer`` (alias
            ``tender``), ``amend_and_extend`` (aliases ``ae``, ``a&e``), or
            ``dropdown``. Case-insensitive; ``-`` and ``&`` normalise to ``_``.
        notional : float
            Outstanding face amount of the target instrument, in the caller's
            monetary unit. Must be finite and strictly positive.
        repurchase_price_pct : float
            Price as a fraction of par for repurchases and tenders (``(0, 1.5]``),
            the extension fee for amend-and-extend (``[0, 0.10]``), or the
            transferred-asset fraction for a dropdown (``[0, 1]``).
        opt_acceptance_pct : float, optional
            Fraction of holders participating, in ``[0, 1]``. Defaults to full
            participation.
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

class recovery_waterfall:
    """
    Absolute-priority recovery allocation with estate-inclusive collateral.

    Examples
    --------
    >>> from finstack_quant.models.credit import recovery_waterfall
    >>> claim = recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0)
    >>> result = recovery_waterfall.allocate_recovery(40.0, [claim])
    >>> (result.total_distributed, result.undistributed_estate, result.apr_satisfied)
    (40.0, 0.0, True)

    """

    class RecoveryClaim:
        """
        A claim participating in an absolute-priority recovery waterfall.

        Examples
        --------
        >>> from finstack_quant.models.credit import recovery_waterfall
        >>> claim = recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0, accrued=5.0)
        >>> (claim.id, claim.total_claim)
        ('SEN', 105.0)
        """

        def __init__(
            self,
            id: str,
            seniority: str,
            priority: int,
            principal: float,
            accrued: float = 0.0,
            penalties: float = 0.0,
            collateral_value: float | None = None,
            collateral_haircut: float = 0.0,
        ) -> None:
            """
            Create a claim for absolute-priority recovery allocation.

            Parameters
            ----------
            id : str
                Stable claim identifier retained on the resulting allocation.
            seniority : str
                Human-readable seniority label used in recovery reporting.
            priority : int
                Absolute-priority rank; lower values receive estate proceeds
                before higher values.
            principal : float
                Outstanding principal claim in the estate's monetary units.
            accrued : float, default 0.0
                Unpaid accrued interest added to the claim amount.
            penalties : float, default 0.0
                Contractual penalty or default-interest claim added to the total.
            collateral_value : float or None, default None
                Gross market value of collateral pledged to this claim, or
                ``None`` for an unsecured claim.
            collateral_haircut : float, default 0.0
                Decimal fraction of ``collateral_value`` deducted before
                estate allocation; must lie in ``[0, 1]``.

            Notes
            -----
            Construction does not raise; arguments are stored as supplied.
            """
            ...
        @property
        def id(self) -> str:
            """
            Stable identifier for this claim.

            Returns
            -------
            str
                Stable identifier retained on the resulting allocation.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def seniority(self) -> str:
            """
            Seniority class the claim sits in.

            Returns
            -------
            str
                Human-readable seniority label used in recovery reporting.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def priority(self) -> int:
            """
            Absolute-priority rank; lower ranks are paid first.

            Returns
            -------
            int
                Absolute-priority rank; lower values receive estate proceeds first.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def principal(self) -> float:
            """
            Principal outstanding, before accrued interest and penalties.

            Returns
            -------
            float
                Outstanding principal claim in the estate's monetary units.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def accrued(self) -> float:
            """
            Accrued but unpaid interest included in the claim.

            Returns
            -------
            float
                Unpaid accrued interest added to the claim amount.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def penalties(self) -> float:
            """
            Penalties and fees included in the claim.

            Returns
            -------
            float
                Contractual penalty or default-interest claim added to the total.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def collateral_value(self) -> float | None:
            """
            Gross value of collateral pledged to this claim.

            Returns
            -------
            float | None
                Pledged collateral market value, or ``None`` when the claim is unsecured.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def collateral_haircut(self) -> float:
            """
            Haircut applied to pledged collateral, as a fraction in ``[0, 1]``.

            Returns
            -------
            float
                Decimal haircut deducted from collateral value before estate allocation.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def total_claim(self) -> float:
            """
            Principal plus accrued interest and penalties.

            Returns
            -------
            float
                Sum of principal, accrued interest, and penalties in estate units.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @staticmethod
        def from_json(json: str) -> recovery_waterfall.RecoveryClaim:
            """
            Deserialize a ``RecoveryClaim`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            RecoveryClaim
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``RecoveryClaim`` JSON.

            Examples
            --------
            >>> value = recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0)
            >>> recovery_waterfall.RecoveryClaim.from_json(value.to_json()) == value
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

    class RecoveryAllocation:
        """
        Recovery allocated to one claim under absolute priority.

        Examples
        --------
        >>> from finstack_quant.models.credit import recovery_waterfall
        >>> claim = recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0)
        >>> allocation = recovery_waterfall.allocate_recovery(40.0, [claim]).allocations[0]
        >>> (allocation.id, allocation.total_recovery, allocation.recovery_rate)
        ('SEN', 40.0, 0.4)
        """

        @property
        def id(self) -> str:
            """
            Stable identifier for this claim.

            Returns
            -------
            str
                Claim identifier copied from the source ``RecoveryClaim``.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def seniority(self) -> str:
            """
            Seniority class the claim sits in.

            Returns
            -------
            str
                Human-readable seniority label copied from the source claim.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def priority(self) -> int:
            """
            Absolute-priority rank; lower ranks are paid first.

            Returns
            -------
            int
                Absolute-priority rank copied from the source claim.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def total_claim(self) -> float:
            """
            Principal plus accrued interest and penalties.

            Returns
            -------
            float
                Total admitted claim amount in estate units.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def collateral_recovery(self) -> float:
            """
            Amount recovered from pledged collateral.

            Returns
            -------
            float
                Recovery attributed to pledged collateral, in estate units.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def general_recovery(self) -> float:
            """
            Amount recovered from the general estate.

            Returns
            -------
            float
                Recovery attributed to the unsecured estate, in estate units.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def total_recovery(self) -> float:
            """
            Collateral plus general recovery.

            Returns
            -------
            float
                Sum of collateral and general recovery, in estate units.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def recovery_rate(self) -> float:
            """
            Total recovery divided by total claim, as a fraction in ``[0, 1]``.

            Returns
            -------
            float
                ``total_recovery / total_claim``, floored at zero when the claim is zero.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def deficiency(self) -> float:
            """
            Unrecovered claim after collateral and general recovery.

            Returns
            -------
            float
                ``total_claim - total_recovery``, floored at zero.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @staticmethod
        def from_json(json: str) -> recovery_waterfall.RecoveryAllocation:
            """
            Deserialize a ``RecoveryAllocation`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            RecoveryAllocation
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``RecoveryAllocation`` JSON.

            Examples
            --------
            >>> value = recovery_waterfall.allocate_recovery(
            ...     40.0, [recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0)]
            ... ).allocations[0]
            >>> recovery_waterfall.RecoveryAllocation.from_json(value.to_json()) == value
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
            Single-row frame with the allocation columns (``id``, ``seniority``, ``priority``, ``total_claim``, ``collateral_recovery``, ``general_recovery``, ``total_recovery``, ``recovery_rate``, ``deficiency``).

            Returns
            -------
            pandas.DataFrame
                Single-row frame with the allocation columns (``id``, ``seniority``, ``priority``, ``total_claim``, ``collateral_recovery``, ``general_recovery``, ``total_recovery``, ``recovery_rate``, ``deficiency``).

            Raises
            ------
            ValueError
                If the value cannot be serialized into a pandas object.
            """
            ...

    class RecoveryWaterfallResult:
        """
        Result of allocating a distributable estate across claims.

        Examples
        --------
        >>> from finstack_quant.models.credit import recovery_waterfall
        >>> claim = recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0)
        >>> result = recovery_waterfall.allocate_recovery(40.0, [claim])
        >>> (result.total_distributed, result.undistributed_estate, result.apr_satisfied)
        (40.0, 0.0, True)
        """

        @property
        def total_distributed(self) -> float:
            """
            Sum of every claim's total recovery.

            Returns
            -------
            float
                Aggregate recovery paid across all claims, in estate units.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def undistributed_estate(self) -> float:
            """
            Estate value left after all claims are satisfied.

            Returns
            -------
            float
                Residual estate after absolute-priority allocation, in estate units.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def apr_satisfied(self) -> bool:
            """
            Whether the run respected absolute priority end to end.

            Returns
            -------
            bool
                ``True`` when every senior claim was paid before any junior claim.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        @property
        def allocations(self) -> list[recovery_waterfall.RecoveryAllocation]:
            """
            Per-claim allocations, in absolute-priority order.

            Returns
            -------
            list[recovery_waterfall.RecoveryAllocation]
                One ``RecoveryAllocation`` per claim, ordered by increasing priority.

            Notes
            -----
            This accessor does not raise; it returns the stored value.
            """
            ...

        def to_dataframe(self) -> pandas.DataFrame:
            """
            Export the per-claim allocations as a pandas DataFrame.

            Columns: ``id``, ``seniority``, ``priority``, ``total_claim``,
            ``collateral_recovery``, ``general_recovery``, ``total_recovery``,
            ``recovery_rate``, ``deficiency``.

            One row per claim — the natural grain of a waterfall. Rows keep the
            Rust ordering (ascending ``priority``, then original claim order),
            so repeated exports of the same result are byte-identical. The
            estate-level fields (:attr:`total_distributed`,
            :attr:`undistributed_estate`, :attr:`apr_satisfied`) are
            deliberately not repeated on every row; read them from the result
            object.

            A waterfall with no claims yields a zero-row frame that still
            carries the columns above.

            Returns
            -------
            pandas.DataFrame
                One row per claim, in absolute-priority order.

            Raises
            ------
            ValueError
                If the result cannot be serialized into a pandas object.
            """
            ...

    @staticmethod
    def allocate_recovery(
        estate_value: float,
        claims: list[recovery_waterfall.RecoveryClaim],
    ) -> recovery_waterfall.RecoveryWaterfallResult:
        """
        Allocate an insolvent estate under absolute priority.

        Parameters
        ----------
        estate_value : float
            Cash estate available for distribution after any external costs,
            expressed in the same monetary units as each claim.
        claims : list[RecoveryClaim]
            Claims to rank by ``priority``. Collateral recovery is applied to
            each claim before general estate proceeds are distributed.

        Returns
        -------
        RecoveryWaterfallResult
            Per-claim recoveries, undistributed estate, and APR satisfaction.

        Raises
        ------
        ValueError
            If the estate or claim amounts are negative or non-finite, a claim
            identifier or seniority is blank, identifiers are duplicated,
            a haircut is outside ``[0, 1]``, a claim total overflows, or net
            collateral exceeds the estate.
        RuntimeError
            If the allocator cannot reserve its claim-index storage or a
            recovery-conservation invariant fails.


        Examples
        --------
        >>> from finstack_quant.models.credit import recovery_waterfall
        >>> claims = [recovery_waterfall.RecoveryClaim("SEN", "secured", 1, 100.0)]
        >>> recovery_waterfall.allocate_recovery(40.0, claims).allocations[0].recovery_rate
        0.4

        """
        ...

class scoring:
    """
    Academic credit scoring: Altman Z-Score family, Ohlson O-Score, Zmijewski.

    Every model returns a :class:`ScoringResult`; feed one with an
    ``implied_pd`` (Ohlson, Zmijewski) to ``pd.MasterScale.map_score``.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> round(scoring.altman_z_score(0.2, 0.3, 0.15, 1.5, 1.0).score, 3)
    3.055

    """

    class ScoringResult:
        """
        Outcome of one academic credit-scoring model.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> result = scoring.zmijewski_score(0.05, 0.5, 1.5)
        >>> (result.zone, result.implied_pd is not None, result.model)
        ('safe', True, 'Zmijewski Probit (1984)')
        """

        @property
        def score(self) -> float:
            """
            Raw score value (Z, Z', Z'', EM, O, or Zmijewski Y).

            Returns
            -------
            float
                Raw score value (Z, Z', Z'', EM, O, or Zmijewski Y).

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def zone(self) -> str:
            """
            Risk zone: ``"safe"``, ``"grey"`` or ``"distress"``.

            Returns
            -------
            str
                Risk zone: ``"safe"``, ``"grey"`` or ``"distress"``.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def implied_pd(self) -> float | None:
            """
            Native implied probability of default as a decimal, or ``None`` for the Altman family.

            Returns
            -------
            float | None
                Native implied probability of default as a decimal, or ``None`` for the Altman family.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def model(self) -> str:
            """
            Name of the model that produced this result.

            Returns
            -------
            str
                Name of the model that produced this result.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @staticmethod
        def from_json(json: str) -> scoring.ScoringResult:
            """
            Deserialize a ``ScoringResult`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            ScoringResult
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``ScoringResult`` JSON.

            Examples
            --------
            >>> value = scoring.zmijewski_score(0.05, 0.5, 1.5)
            >>> scoring.ScoringResult.from_json(value.to_json()) == value
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
            Single-row frame with ``model``, ``score``, ``zone``, ``implied_pd``.

            Returns
            -------
            pandas.DataFrame
                Single-row frame with ``model``, ``score``, ``zone``, ``implied_pd``.

            Raises
            ------
            ValueError
                If the value cannot be serialized into a pandas object.
            """
            ...
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    @staticmethod
    def altman_z_score(
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        market_equity_to_total_liabilities: float,
        sales_to_total_assets: float,
    ) -> scoring.ScoringResult:
        """
        Original Altman Z-Score (1968) for publicly traded manufacturers.

        ``Z = 1.2 * X1 + 1.4 * X2 + 3.3 * X3 + 0.6 * X4 + 1.0 * X5``

        Zone cutoffs: Z > 2.99 safe, 1.81 <= Z <= 2.99 grey, Z < 1.81 distress.

        Parameters
        ----------
        working_capital_to_total_assets : float
            Working capital / total assets (X1).
        retained_earnings_to_total_assets : float
            Retained earnings / total assets (X2).
        ebit_to_total_assets : float
            EBIT / total assets (X3).
        market_equity_to_total_liabilities : float
            Market value of equity / total liabilities (X4).
        sales_to_total_assets : float
            Sales / total assets (X5).

        Returns
        -------
        ScoringResult
            Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
            ``implied_pd``: ``None`` (calibrate score-to-PD separately).

        Raises
        ------
        ValueError
            If any ratio is non-finite.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> scoring.altman_z_score(0.2, 0.3, 0.15, 1.5, 1.0).zone
        'safe'
        """
        ...

    @staticmethod
    def altman_z_prime(
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        book_equity_to_total_liabilities: float,
        sales_to_total_assets: float,
    ) -> scoring.ScoringResult:
        """
        Altman Z'-Score for private firms.

        ``Z' = 0.717 * X1 + 0.847 * X2 + 3.107 * X3 + 0.420 * X4 + 0.998 * X5``

        Zone cutoffs: Z' > 2.90 safe, 1.23 <= Z' <= 2.90 grey, Z' < 1.23 distress.

        Parameters
        ----------
        working_capital_to_total_assets : float
            Working capital / total assets (X1).
        retained_earnings_to_total_assets : float
            Retained earnings / total assets (X2).
        ebit_to_total_assets : float
            EBIT / total assets (X3).
        book_equity_to_total_liabilities : float
            Book value of equity / total liabilities (X4).
        sales_to_total_assets : float
            Sales / total assets (X5).

        Returns
        -------
        ScoringResult
            Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
            ``implied_pd``: ``None``.

        Raises
        ------
        ValueError
            If any ratio is non-finite.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> scoring.altman_z_prime(0.2, 0.3, 0.15, 1.5, 1.0).zone
        'grey'
        """
        ...

    @staticmethod
    def altman_z_double_prime(
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        book_equity_to_total_liabilities: float,
    ) -> scoring.ScoringResult:
        """
        Altman Z''-Score for non-manufacturing firms (the emerging-market variant with the +3.25 constant is ``altman_em_score``).

        ``Z'' = 6.56 * X1 + 3.26 * X2 + 6.72 * X3 + 1.05 * X4``

        Zone cutoffs: Z'' > 2.60 safe, 1.10 <= Z'' <= 2.60 grey, Z'' < 1.10 distress.

        Parameters
        ----------
        working_capital_to_total_assets : float
            Working capital / total assets (X1).
        retained_earnings_to_total_assets : float
            Retained earnings / total assets (X2).
        ebit_to_total_assets : float
            EBIT / total assets (X3).
        book_equity_to_total_liabilities : float
            Book value of equity / total liabilities (X4).

        Returns
        -------
        ScoringResult
            Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
            ``implied_pd``: ``None``.

        Raises
        ------
        ValueError
            If any ratio is non-finite.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> scoring.altman_z_double_prime(0.2, 0.3, 0.15, 1.5).zone
        'safe'
        """
        ...

    @staticmethod
    def altman_em_score(
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        book_equity_to_total_liabilities: float,
    ) -> scoring.ScoringResult:
        """
        Altman EM-Score for emerging-market corporates (Altman, Hartzell & Peck 1995).

        ``EM = 3.25 + 6.56 * X1 + 3.26 * X2 + 6.72 * X3 + 1.05 * X4``

        Zone cutoffs: EM > 5.85 safe, 4.35 <= EM <= 5.85 grey, EM < 4.35 distress.

        Parameters
        ----------
        working_capital_to_total_assets : float
            Working capital / total assets (X1).
        retained_earnings_to_total_assets : float
            Retained earnings / total assets (X2).
        ebit_to_total_assets : float
            EBIT / total assets (X3).
        book_equity_to_total_liabilities : float
            Book value of equity / total liabilities (X4).

        Returns
        -------
        ScoringResult
            Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
            ``implied_pd``: ``None``.

        Raises
        ------
        ValueError
            If any ratio is non-finite.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> scoring.altman_em_score(0.2, 0.3, 0.15, 1.5).zone
        'safe'
        """
        ...

    @staticmethod
    def ohlson_o_score(
        log_total_assets_adjusted: float,
        total_liabilities_to_total_assets: float,
        working_capital_to_total_assets: float,
        current_liabilities_to_current_assets: float,
        liabilities_exceed_assets: float,
        net_income_to_total_assets: float,
        funds_from_operations_to_total_liabilities: float,
        negative_net_income_two_years: float,
        net_income_change: float,
    ) -> scoring.ScoringResult:
        """
        Ohlson O-Score (1980) nine-predictor logistic bankruptcy model.

        ``O = -1.32 - 0.407 * X1 + 6.03 * X2 - 1.43 * X3 + 0.0757 * X4 - 1.72 * X5 - 2.37 * X6 - 1.83 * X7 + 0.285 * X8 - 0.521 * X9; PD = 1 / (1 + exp(-O))``

        Zone cutoffs: PD < 0.019 safe, 0.019 <= PD <= 0.038 grey, PD > 0.038 distress.

        Parameters
        ----------
        log_total_assets_adjusted : float
            log(total assets / GNP price-level index) (X1).
        total_liabilities_to_total_assets : float
            Total liabilities / total assets (X2).
        working_capital_to_total_assets : float
            Working capital / total assets (X3).
        current_liabilities_to_current_assets : float
            Current liabilities / current assets (X4).
        liabilities_exceed_assets : float
            Indicator, exactly ``1.0`` if total liabilities exceed total assets else ``0.0`` (X5).
        net_income_to_total_assets : float
            Net income / total assets (X6).
        funds_from_operations_to_total_liabilities : float
            Funds from operations / total liabilities (X7).
        negative_net_income_two_years : float
            Indicator, exactly ``1.0`` if net income was negative in each of the last two years (X8).
        net_income_change : float
            ``(NI_t - NI_t-1) / (|NI_t| + |NI_t-1|)`` (X9).

        Returns
        -------
        ScoringResult
            Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
            ``implied_pd``: the logistic probability.

        Raises
        ------
        ValueError
            If any ratio is non-finite or an indicator is not exactly 0 or 1.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> scoring.ohlson_o_score(8.0, 0.4, 0.2, 0.5, 0.0, 0.1, 0.3, 0.0, 0.1).zone
        'grey'
        """
        ...

    @staticmethod
    def zmijewski_score(
        net_income_to_total_assets: float,
        total_liabilities_to_total_assets: float,
        current_assets_to_current_liabilities: float,
    ) -> scoring.ScoringResult:
        """
        Zmijewski (1984) probit bankruptcy score.

        ``Y = -4.336 - 4.513 * ROA + 5.679 * DebtRatio + 0.004 * CurrentRatio; PD = Phi(Y)``

        Zone cutoffs: PD < 0.10 safe, 0.10 <= PD <= 0.50 grey, PD > 0.50 distress.

        Parameters
        ----------
        net_income_to_total_assets : float
            Net income / total assets (ROA).
        total_liabilities_to_total_assets : float
            Total liabilities / total assets (debt ratio).
        current_assets_to_current_liabilities : float
            Current assets / current liabilities (current ratio).

        Returns
        -------
        ScoringResult
            Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
            ``implied_pd``: the probit probability.

        Raises
        ------
        ValueError
            If any ratio is non-finite.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> scoring.zmijewski_score(0.05, 0.5, 1.5).zone
        'safe'
        """
        ...

class pd:
    """
    Probability of default: PiT/TtC conversion, central-tendency calibration,
    the Basel IRB floor, and rating master scales.

    This submodule shadows the ``import pandas as pd`` alias; import it as
    ``from finstack_quant.models.credit import pd as pdm``.

    Examples
    --------
    >>> from finstack_quant.models.credit import pd as pdm
    >>> pdm.central_tendency([0.01, 0.02, 0.03])
    0.02

    """

    BASEL_IRB_PD_FLOOR: float
    """Basel IRB corporate PD floor, ``0.0003`` (3 bp) as a decimal."""

    class MasterScaleGrade:
        """
        One PD band in a rating master scale.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd as pdm
        >>> pdm.MasterScaleGrade("BBB", 0.005, 0.002).label
        'BBB'

        """

        def __init__(self, label: str, upper_pd: float, central_pd: float) -> None:
            """
            Construct one probability-of-default band on a master scale.

            Parameters
            ----------
            label : str
                Grade label (e.g. ``"BBB"``).
            upper_pd : float
                Inclusive upper PD bound of the band, a decimal in ``(0, 1]``.
            central_pd : float
                Representative PD assigned to the band, a decimal in ``(0, 1)``.

            Notes
            -----
            Construction does not raise; validation happens when the grade is
            placed in a ``MasterScale``.
            """
            ...

        @property
        def label(self) -> str:
            """
            Grade label (e.g. ``"BBB"``).

            Returns
            -------
            str
                Grade label (e.g. ``"BBB"``).

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def upper_pd(self) -> float:
            """
            Inclusive upper PD bound of the band (decimal).

            Returns
            -------
            float
                Inclusive upper PD bound of the band (decimal).

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def central_pd(self) -> float:
            """
            Representative PD assigned to anything falling in the band (decimal).

            Returns
            -------
            float
                Representative PD assigned to anything falling in the band (decimal).

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @staticmethod
        def from_json(json: str) -> pd.MasterScaleGrade:
            """
            Deserialize a ``MasterScaleGrade`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            MasterScaleGrade
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``MasterScaleGrade`` JSON.

            Examples
            --------
            >>> from finstack_quant.models.credit import pd
            >>> value = pd.MasterScaleGrade("BBB", 0.005, 0.002)
            >>> pd.MasterScaleGrade.from_json(value.to_json()) == value
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
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class MasterScaleResult:
        """
        Result of mapping a PD onto a master scale.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd as pdm
        >>> result = pdm.MasterScale.sp_assumptions().map_pd(0.003)
        >>> (result.grade, result.grade_index, result.central_pd)
        ('BBB', 3, 0.002)

        """

        @property
        def grade(self) -> str:
            """
            Label of the grade the PD mapped into.

            Returns
            -------
            str
                Label of the grade the PD mapped into.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def central_pd(self) -> float:
            """
            Central PD of the assigned grade (the notched value).

            Returns
            -------
            float
                Central PD of the assigned grade (the notched value).

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def input_pd(self) -> float:
            """
            The PD that was mapped, before notching.

            Returns
            -------
            float
                The PD that was mapped, before notching.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def grade_index(self) -> int:
            """
            Zero-based index of the assigned grade in the scale.

            Returns
            -------
            int
                Zero-based index of the assigned grade in the scale.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @staticmethod
        def from_json(json: str) -> pd.MasterScaleResult:
            """
            Deserialize a ``MasterScaleResult`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            MasterScaleResult
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``MasterScaleResult`` JSON.

            Examples
            --------
            >>> from finstack_quant.models.credit import pd
            >>> value = pd.MasterScale.sp_assumptions().map_pd(0.003)
            >>> pd.MasterScaleResult.from_json(value.to_json()) == value
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
            Single-row frame with ``grade``, ``grade_index``, ``input_pd``, ``central_pd``.

            Returns
            -------
            pandas.DataFrame
                Single-row frame with ``grade``, ``grade_index``, ``input_pd``, ``central_pd``.

            Raises
            ------
            ValueError
                If the value cannot be serialized into a pandas object.
            """
            ...
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class MasterScale:
        """
        Ordered PD bands mapping a continuous PD onto discrete rating grades.

        Bands must be strictly increasing in ``upper_pd`` and each grade's
        ``central_pd`` must fall inside its own band; PDs are decimals in
        ``[0, 1]``.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd as pdm
        >>> scale = pdm.MasterScale.sp_assumptions()
        >>> (scale.n_grades, scale.map_pd(0.003).grade)
        (8, 'BBB')

        """

        def __init__(self, grades: list[pd.MasterScaleGrade]) -> None:
            """
            Build a master scale from ordered grades.

            Parameters
            ----------
            grades : list[MasterScaleGrade]
                Bands in ascending ``upper_pd`` order, strongest grade first.

            Raises
            ------
            ValueError
                If the list is empty, a PD lies outside its valid range, or
                the bands are not strictly ascending.
            """
            ...

        @staticmethod
        def sp_assumptions() -> pd.MasterScale:
            """
            Library PD-band assumptions using S&P-style labels.

            The labels resemble S&P notation as a reporting convention only;
            the boundaries and central PDs are library assumptions, not
            agency-published statistics.

            Returns
            -------
            MasterScale
                Eight-grade scale from ``AAA`` to ``CC/C``.

            Raises
            ------
            ValueError
                If the embedded credit registry is invalid.

            Examples
            --------
            >>> from finstack_quant.models.credit import pd as pdm
            >>> pdm.MasterScale.sp_assumptions().n_grades
            8
            """
            ...

        @staticmethod
        def moodys_assumptions() -> pd.MasterScale:
            """
            Library PD-band assumptions using Moody's-style labels.

            As with :meth:`sp_assumptions`, the labels are a reporting
            convention rather than an agency calibration.

            Returns
            -------
            MasterScale
                Moody's-labelled library scale.

            Raises
            ------
            ValueError
                If the embedded credit registry is invalid.

            Examples
            --------
            >>> from finstack_quant.models.credit import pd as pdm
            >>> pdm.MasterScale.moodys_assumptions().n_grades > 0
            True
            """
            ...

        @staticmethod
        def from_registry_id(scale_id: str) -> pd.MasterScale:
            """
            Load a master scale by id from the embedded credit registry.

            Parameters
            ----------
            scale_id : str
                Registry identifier of the scale.

            Returns
            -------
            MasterScale
                The registry scale.

            Raises
            ------
            KeyError
                If ``scale_id`` is unknown.
            ValueError
                If the registry is invalid.

            Examples
            --------
            >>> from finstack_quant.models.credit import pd as pdm
            >>> isinstance(pdm.MasterScale.from_registry_id("sp_assumptions"), pdm.MasterScale)
            True
            """
            ...

        def map_pd(self, pd: float) -> pd_mod.MasterScaleResult:
            """
            Map a PD onto its rating grade.

            The first band whose inclusive ``upper_pd`` covers ``pd`` wins.

            Parameters
            ----------
            pd : float
                Probability of default as a decimal in ``[0, 1]``.

            Returns
            -------
            MasterScaleResult
                Assigned grade, its index, the input and central PD.

            Raises
            ------
            ValueError
                If ``pd`` is non-finite or outside ``[0, 1]`` (a percent /
                decimal mix-up such as ``5.0`` is rejected, not clamped).
            """
            ...

        def map_pds(self, pds: list[float]) -> pandas.DataFrame:
            """
            Map several PDs and return one grading table.

            Parameters
            ----------
            pds : list[float]
                Probabilities of default as decimals in ``[0, 1]``.

            Returns
            -------
            pandas.DataFrame
                Columns ``grade``, ``grade_index``, ``input_pd``,
                ``central_pd``; one row per input in input order.

            Raises
            ------
            ValueError
                If any PD is non-finite or outside ``[0, 1]``.
            """
            ...

        def map_score(self, result: scoring.ScoringResult) -> pd.MasterScaleResult:
            """
            Map a scoring result's implied PD onto its rating grade.

            Parameters
            ----------
            result : ScoringResult
                Output of a ``scoring`` model carrying an ``implied_pd``
                (Ohlson, Zmijewski).

            Returns
            -------
            MasterScaleResult
                Grade assigned to ``result.implied_pd``.

            Raises
            ------
            ValueError
                If the result has no implied PD (Altman family) or the PD is
                non-finite.
            """
            ...

        @property
        def n_grades(self) -> int:
            """
            Number of grades in the scale.

            Returns
            -------
            int
                Number of grades in the scale.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def grades(self) -> list[pd.MasterScaleGrade]:
            """
            The scale's grades, in ascending PD order.

            Returns
            -------
            list[pd.MasterScaleGrade]
                The scale's grades, in ascending PD order.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        def to_dataframe(self) -> pandas.DataFrame:
            """
            Frame with ``label``, ``upper_pd``, ``central_pd``; one row per grade in ascending PD order.

            Returns
            -------
            pandas.DataFrame
                Frame with ``label``, ``upper_pd``, ``central_pd``; one row per grade in ascending PD order.

            Raises
            ------
            ValueError
                If the value cannot be serialized into a pandas object.
            """
            ...
        @staticmethod
        def from_json(json: str) -> pd.MasterScale:
            """
            Deserialize a ``MasterScale`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            MasterScale
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``MasterScale`` JSON.

            Examples
            --------
            >>> from finstack_quant.models.credit import pd
            >>> value = pd.MasterScale.sp_assumptions()
            >>> pd.MasterScale.from_json(value.to_json()).to_json() == value.to_json()
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
        def __len__(self) -> int:
            """
            Number of grades in the scale.

            Returns
            -------
            int
                Same as :attr:`n_grades`.

            Notes
            -----
            This method does not raise.
            """
            ...
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    @staticmethod
    def pit_to_ttc(pd_pit: float, asset_correlation: float, cycle_index: float) -> float:
        """
        Convert a Point-in-Time PD to a Through-the-Cycle PD.

        Merton-Vasicek single-factor model (Basel II IRB):
        ``PD_TtC = Phi(Phi^-1(PD_PiT) * sqrt(1 - rho) + sqrt(rho) * z)``.

        Parameters
        ----------
        pd_pit : float
            Point-in-Time PD as a decimal in ``(0, 1)``.
        asset_correlation : float
            Asset correlation ``rho`` in ``(0, 1)``; Basel uses 0.12-0.24 for
            corporates.
        cycle_index : float
            Systematic factor ``z``: ``0`` average, ``< 0`` downturn, ``> 0``
            benign.

        Returns
        -------
        float
            Through-the-Cycle PD as a decimal.

        Raises
        ------
        ValueError
            If ``pd_pit`` or ``asset_correlation`` is outside ``(0, 1)`` or any
            input is non-finite.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd as pdm
        >>> round(pdm.pit_to_ttc(0.03, 0.12, -1.0), 4)
        0.0174
        """
        ...

    @staticmethod
    def ttc_to_pit(pd_ttc: float, asset_correlation: float, cycle_index: float) -> float:
        """
        Convert a Through-the-Cycle PD to a Point-in-Time PD.

        Merton-Vasicek single-factor model (Basel II IRB):
        ``PD_PiT = Phi((Phi^-1(PD_TtC) - sqrt(rho) * z) / sqrt(1 - rho))``.

        Parameters
        ----------
        pd_ttc : float
            Through-the-Cycle PD as a decimal in ``(0, 1)``.
        asset_correlation : float
            Asset correlation ``rho`` in ``(0, 1)``.
        cycle_index : float
            Systematic factor ``z``: ``0`` average, ``< 0`` downturn, ``> 0``
            benign.

        Returns
        -------
        float
            Point-in-Time PD as a decimal.

        Raises
        ------
        ValueError
            If ``pd_ttc`` or ``asset_correlation`` is outside ``(0, 1)`` or any
            input is non-finite.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd as pdm
        >>> pdm.ttc_to_pit(0.02, 0.12, -1.0) > 0.02
        True
        """
        ...

    @staticmethod
    def central_tendency(annual_default_rates: list[float]) -> float:
        """
        Long-run average PD from annual default rates (arithmetic mean).

        This is the standard regulatory TtC approach (Basel IRB, EBA
        GL/2017/16); zero-default years are valid observations.

        Parameters
        ----------
        annual_default_rates : list[float]
            Observed annual default rates as decimals in ``[0, 1]``; at least
            one.

        Returns
        -------
        float
            Arithmetic mean in ``[0, 1]``.

        Raises
        ------
        ValueError
            If the list is empty or any rate is non-finite or outside
            ``[0, 1]``.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd as pdm
        >>> pdm.central_tendency([0.01, 0.02, 0.03])
        0.02
        """
        ...

    @staticmethod
    def apply_basel_irb_pd_floor(pd: float) -> float:
        """
        Apply the Basel IRB corporate PD floor: ``max(pd, BASEL_IRB_PD_FLOOR)``.

        Parameters
        ----------
        pd : float
            Probability of default as a decimal.

        Returns
        -------
        float
            The floored PD (``0.0003`` when ``pd`` is below 3 bp).

        Notes
        -----
        This function does not raise.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd as pdm
        >>> pdm.apply_basel_irb_pd_floor(0.0001)
        0.0003
        """
        ...

class lgd:
    """
    Loss-given-default: seniority Beta recovery, workout LGD, downturn
    adjustments, EAD.

    Collateral types accepted as strings: ``cash``, ``securities``, ``receivables``, ``inventory``, ``equipment``, ``real_estate``, ``intellectual_property``, ``other``.

    Examples
    --------
    >>> from finstack_quant.models.credit import lgd
    >>> lgd.ead_revolver(60.0, 40.0, 0.5)
    80.0

    """

    class BetaRecovery:
        """
        Beta-distributed recovery rate parameterised by mean and standard deviation.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> recovery = lgd.BetaRecovery(0.4, 0.2)
        >>> (recovery.mean, round(recovery.mean_lgd, 2))
        (0.4, 0.6)

        """

        def __init__(self, mean: float, std_dev: float) -> None:
            """
            Build a Beta recovery distribution from its first two moments.

            Parameters
            ----------
            mean : float
                Mean recovery rate as a decimal in ``(0, 1)``.
            std_dev : float
                Standard deviation; must satisfy ``std_dev**2 < mean * (1 - mean)``.

            Raises
            ------
            ValueError
                If the moments cannot parameterise a Beta distribution.
            """
            ...

        @property
        def mean(self) -> float:
            """
            Mean recovery rate (decimal).

            Returns
            -------
            float
                Mean recovery rate (decimal).

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def std_dev(self) -> float:
            """
            Standard deviation of the recovery rate.

            Returns
            -------
            float
                Standard deviation of the recovery rate.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def alpha(self) -> float:
            """
            Beta shape parameter alpha.

            Returns
            -------
            float
                Beta shape parameter alpha.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def beta_param(self) -> float:
            """
            Beta shape parameter beta.

            Returns
            -------
            float
                Beta shape parameter beta.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def variance(self) -> float:
            """
            Variance of the recovery rate (``std_dev**2``).

            Returns
            -------
            float
                Variance of the recovery rate (``std_dev**2``).

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def mode(self) -> float | None:
            """
            Mode of the distribution, or ``None`` when a shape parameter is <= 1.

            Returns
            -------
            float | None
                Mode of the distribution, or ``None`` when a shape parameter is <= 1.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def mean_lgd(self) -> float:
            """
            Expected loss given default, ``1 - mean``.

            Returns
            -------
            float
                Expected loss given default, ``1 - mean``.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        def quantile(self, p: float) -> float:
            """
            Recovery rate at probability ``p``.

            Parameters
            ----------
            p : float
                Probability in ``(0, 1)``.

            Returns
            -------
            float
                Recovery rate as a decimal.

            Raises
            ------
            ValueError
                If ``p`` is non-finite or outside ``(0, 1)``.
            """
            ...

        def sample_seeded(self, n_samples: int, seed: int) -> list[float]:
            """
            Draw recovery rates with a deterministic PCG64 RNG.

            Parameters
            ----------
            n_samples : int
                Number of draws.
            seed : int
                RNG seed; the same seed yields the same sequence.

            Returns
            -------
            list[float]
                ``n_samples`` recovery rates as decimals.

            Raises
            ------
            ValueError
                If sampling fails.
            """
            ...
        @staticmethod
        def from_json(json: str) -> lgd.BetaRecovery:
            """
            Deserialize a ``BetaRecovery`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            BetaRecovery
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``BetaRecovery`` JSON.

            Examples
            --------
            >>> value = lgd.BetaRecovery(0.4, 0.2)
            >>> lgd.BetaRecovery.from_json(value.to_json()).to_json() == value.to_json()
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
            Single-row frame with ``mean``, ``std_dev``, ``alpha``, ``beta_param``.

            Returns
            -------
            pandas.DataFrame
                Single-row frame with ``mean``, ``std_dev``, ``alpha``, ``beta_param``.

            Raises
            ------
            ValueError
                If the value cannot be serialized into a pandas object.
            """
            ...
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class CollateralPiece:
        """
        One collateral piece in a workout waterfall.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.CollateralPiece("real_estate", 80.0, 0.3).liquidation_value
        56.0

        """

        def __init__(self, collateral_type: str, book_value: float, haircut: float) -> None:
            """
            Build a collateral piece.

            Parameters
            ----------
            collateral_type : str
                One of ``cash``, ``securities``, ``receivables``, ``inventory``, ``equipment``, ``real_estate``, ``intellectual_property``, ``other``.
            book_value : float
                Pre-haircut book value (non-negative), in the exposure's currency.
            haircut : float
                Liquidation haircut as a decimal in ``[0, 1]``.

            Raises
            ------
            ValueError
                For an unknown type, a negative value, or a haircut outside
                ``[0, 1]``.
            """
            ...

        @property
        def collateral_type(self) -> str:
            """
            Canonical collateral-type label.

            Returns
            -------
            str
                Canonical collateral-type label.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def book_value(self) -> float:
            """
            Pre-haircut book value.

            Returns
            -------
            float
                Pre-haircut book value.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def haircut(self) -> float:
            """
            Liquidation haircut as a decimal in ``[0, 1]``.

            Returns
            -------
            float
                Liquidation haircut as a decimal in ``[0, 1]``.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def liquidation_value(self) -> float:
            """
            ``book_value * (1 - haircut)``.

            Returns
            -------
            float
                ``book_value * (1 - haircut)``.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @staticmethod
        def from_json(json: str) -> lgd.CollateralPiece:
            """
            Deserialize a ``CollateralPiece`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            CollateralPiece
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``CollateralPiece`` JSON.

            Examples
            --------
            >>> value = lgd.CollateralPiece("cash", 10.0, 0.0)
            >>> lgd.CollateralPiece.from_json(value.to_json()).to_json() == value.to_json()
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
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class WorkoutCosts:
        """
        Direct and indirect workout cost rates as decimal fractions of EAD.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.WorkoutCosts(0.05, 0.03).total_rate
        0.08

        """

        def __init__(self, direct_cost_rate: float, indirect_cost_rate: float) -> None:
            """
            Build a cost specification.

            Parameters
            ----------
            direct_cost_rate : float
                Direct (legal, administrative) costs as a decimal fraction of
                EAD (>= 0).
            indirect_cost_rate : float
                Indirect (opportunity) costs as a decimal fraction of EAD (>= 0).

            Raises
            ------
            ValueError
                For negative or non-finite rates.
            """
            ...

        @staticmethod
        def zero() -> lgd.WorkoutCosts:
            """
            Zero workout costs.

            Returns
            -------
            WorkoutCosts
                Both rates ``0.0``.

            Notes
            -----
            This constructor does not raise.

            Examples
            --------
            >>> from finstack_quant.models.credit import lgd
            >>> lgd.WorkoutCosts.zero().total_rate
            0.0
            """
            ...

        @staticmethod
        def standard() -> lgd.WorkoutCosts:
            """
            Registry-default workout costs.

            Returns
            -------
            WorkoutCosts
                Default direct / indirect rates from the embedded registry.

            Raises
            ------
            ValueError
                If the embedded credit registry is invalid.

            Examples
            --------
            >>> from finstack_quant.models.credit import lgd
            >>> lgd.WorkoutCosts.standard().total_rate > 0.0
            True
            """
            ...

        @property
        def direct_cost_rate(self) -> float:
            """
            Direct cost rate (decimal fraction of EAD).

            Returns
            -------
            float
                Direct cost rate (decimal fraction of EAD).

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def indirect_cost_rate(self) -> float:
            """
            Indirect cost rate (decimal fraction of EAD).

            Returns
            -------
            float
                Indirect cost rate (decimal fraction of EAD).

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def total_rate(self) -> float:
            """
            ``direct_cost_rate + indirect_cost_rate``.

            Returns
            -------
            float
                ``direct_cost_rate + indirect_cost_rate``.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @staticmethod
        def from_json(json: str) -> lgd.WorkoutCosts:
            """
            Deserialize a ``WorkoutCosts`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            WorkoutCosts
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``WorkoutCosts`` JSON.

            Examples
            --------
            >>> value = lgd.WorkoutCosts(0.05, 0.03)
            >>> lgd.WorkoutCosts.from_json(value.to_json()).to_json() == value.to_json()
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
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class WorkoutLgdResult:
        """
        Net recovery, LGD, and recovery rate from a workout evaluation.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> result = lgd.workout_lgd(100.0, [("real_estate", 80.0, 0.3)], 0.05, 0.03, 2.0, 0.05)
        >>> round(result.lgd + result.recovery_rate, 12)
        1.0

        """

        @property
        def net_recovery(self) -> float:
            """
            Post-cost, post-discount recovery amount (floored at zero), in EAD units.

            Returns
            -------
            float
                Post-cost, post-discount recovery amount (floored at zero), in EAD units.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def lgd(self) -> float:
            """
            Loss given default as a decimal in ``[0, 1]``.

            Returns
            -------
            float
                Loss given default as a decimal in ``[0, 1]``.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def recovery_rate(self) -> float:
            """
            Recovery rate ``1 - lgd`` as a decimal in ``[0, 1]``.

            Returns
            -------
            float
                Recovery rate ``1 - lgd`` as a decimal in ``[0, 1]``.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @staticmethod
        def from_json(json: str) -> lgd_mod.WorkoutLgdResult:
            """
            Deserialize a ``WorkoutLgdResult`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            WorkoutLgdResult
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``WorkoutLgdResult`` JSON.

            Examples
            --------
            >>> value = lgd.workout_lgd(100.0, [("cash", 50.0, 0.0)], 0.0, 0.0, 1.0, 0.0)
            >>> lgd.WorkoutLgdResult.from_json(value.to_json()) == value
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
            Single-row frame with ``net_recovery``, ``lgd``, ``recovery_rate``.

            Returns
            -------
            pandas.DataFrame
                Single-row frame with ``net_recovery``, ``lgd``, ``recovery_rate``.

            Raises
            ------
            ValueError
                If the value cannot be serialized into a pandas object.
            """
            ...
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class WorkoutLgd:
        """
        Workout (collateral-waterfall) LGD model built via ``WorkoutLgd.builder()``.

        ``net_recovery = (min(sum liquidation values, EAD) - costs * EAD) * DF``
        and ``lgd = 1 - clamp(net_recovery / EAD, 0, 1)`` where ``DF``
        discounts over the workout horizon (Basel workout-LGD methodology).

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> model = (
        ...     lgd.WorkoutLgd
        ...     .builder()
        ...     .collateral(lgd.CollateralPiece("cash", 50.0, 0.0))
        ...     .workout_years(0.0)
        ...     .discount_rate(0.0)
        ...     .costs(lgd.WorkoutCosts.zero())
        ...     .build()
        ... )
        >>> model.lgd(100.0)
        0.5

        """

        @staticmethod
        def builder() -> lgd_mod.WorkoutLgdBuilder:
            """
            Start a fluent builder (the only construction entry point).

            Returns
            -------
            WorkoutLgdBuilder
                Empty builder; unset fields fall back to registry defaults.

            Notes
            -----
            This method does not raise.

            Examples
            --------
            >>> from finstack_quant.models.credit import lgd
            >>> isinstance(lgd.WorkoutLgd.builder().build(), lgd.WorkoutLgd)
            True
            """
            ...

        def evaluate(self, ead: float) -> lgd_mod.WorkoutLgdResult:
            """
            Evaluate net recovery, LGD, and recovery rate at ``ead``.

            Parameters
            ----------
            ead : float
                Exposure at default (> 0), in the collateral's currency.

            Returns
            -------
            WorkoutLgdResult
                Consistent ``net_recovery`` / ``lgd`` / ``recovery_rate``.

            Raises
            ------
            ValueError
                If ``ead`` is non-finite or non-positive.
            """
            ...

        def lgd(self, ead: float) -> float:
            """
            Loss given default at ``ead``.

            Parameters
            ----------
            ead : float
                Exposure at default (> 0).

            Returns
            -------
            float
                LGD as a decimal in ``[0, 1]``.

            Raises
            ------
            ValueError
                If ``ead`` is non-finite or non-positive.
            """
            ...

        def net_recovery(self, ead: float) -> float:
            """
            Net recovery amount at ``ead``.

            Parameters
            ----------
            ead : float
                Exposure at default (> 0).

            Returns
            -------
            float
                Post-cost, discounted recovery amount (floored at zero).

            Raises
            ------
            ValueError
                If ``ead`` is non-finite or non-positive.
            """
            ...

        def recovery_rate(self, ead: float) -> float:
            """
            Recovery rate ``1 - lgd`` at ``ead``.

            Parameters
            ----------
            ead : float
                Exposure at default (> 0).

            Returns
            -------
            float
                Recovery rate as a decimal in ``[0, 1]``.

            Raises
            ------
            ValueError
                If ``ead`` is non-finite or non-positive.
            """
            ...

        @property
        def collateral(self) -> list[lgd_mod.CollateralPiece]:
            """
            Ordered collateral waterfall, highest priority first.

            Returns
            -------
            list[lgd.CollateralPiece]
                Ordered collateral waterfall, highest priority first.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def workout_years(self) -> float:
            """
            Expected workout duration in years.

            Returns
            -------
            float
                Expected workout duration in years.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def discount_rate(self) -> float:
            """
            Annual decimal discount rate over the workout horizon.

            Returns
            -------
            float
                Annual decimal discount rate over the workout horizon.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def costs(self) -> lgd_mod.WorkoutCosts:
            """
            Direct and indirect cost rates.

            Returns
            -------
            lgd.WorkoutCosts
                Direct and indirect cost rates.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        def to_dataframe(self) -> pandas.DataFrame:
            """
            Frame of the collateral waterfall with ``collateral_type``, ``book_value``, ``haircut``; one row per piece.

            Returns
            -------
            pandas.DataFrame
                Frame of the collateral waterfall with ``collateral_type``, ``book_value``, ``haircut``; one row per piece.

            Raises
            ------
            ValueError
                If the value cannot be serialized into a pandas object.
            """
            ...
        @staticmethod
        def from_json(json: str) -> lgd_mod.WorkoutLgd:
            """
            Deserialize a ``WorkoutLgd`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            WorkoutLgd
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``WorkoutLgd`` JSON.

            Examples
            --------
            >>> value = lgd.WorkoutLgd.builder().build()
            >>> lgd.WorkoutLgd.from_json(value.to_json()).to_json() == value.to_json()
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
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class WorkoutLgdBuilder:
        """
        Fluent builder for :class:`WorkoutLgd`; obtain via ``WorkoutLgd.builder()``.

        Unset ``workout_years`` / ``discount_rate`` / ``costs`` fall back to
        the embedded registry defaults at :meth:`build`.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> builder = lgd.WorkoutLgd.builder().workout_years(2.0).discount_rate(0.05)
        >>> builder.build().workout_years
        2.0

        """

        def collateral(self, piece: lgd.CollateralPiece) -> lgd.WorkoutLgdBuilder:
            """
            Append one collateral piece to the waterfall (highest priority first).

            Parameters
            ----------
            piece : CollateralPiece
                Collateral to append.

            Returns
            -------
            WorkoutLgdBuilder
                ``self`` for chaining.

            Raises
            ------
            ValueError
                If the builder was already consumed by :meth:`build`.
            """
            ...

        def collateral_pieces(self, pieces: list[lgd.CollateralPiece]) -> lgd.WorkoutLgdBuilder:
            """
            Append several collateral pieces in order.

            Parameters
            ----------
            pieces : list[CollateralPiece]
                Collateral to append, highest priority first.

            Returns
            -------
            WorkoutLgdBuilder
                ``self`` for chaining.

            Raises
            ------
            ValueError
                If the builder was already consumed by :meth:`build`.
            """
            ...

        def workout_years(self, years: float) -> lgd.WorkoutLgdBuilder:
            """
            Set the expected workout duration.

            Parameters
            ----------
            years : float
                Workout duration in years (>= 0).

            Returns
            -------
            WorkoutLgdBuilder
                ``self`` for chaining.

            Raises
            ------
            ValueError
                If the builder was already consumed by :meth:`build`.
            """
            ...

        def discount_rate(self, rate: float) -> lgd.WorkoutLgdBuilder:
            """
            Set the discount rate over the workout horizon.

            Parameters
            ----------
            rate : float
                Annual decimal discount rate (>= 0).

            Returns
            -------
            WorkoutLgdBuilder
                ``self`` for chaining.

            Raises
            ------
            ValueError
                If the builder was already consumed by :meth:`build`.
            """
            ...

        def costs(self, costs: lgd.WorkoutCosts) -> lgd.WorkoutLgdBuilder:
            """
            Set the workout cost rates.

            Parameters
            ----------
            costs : WorkoutCosts
                Direct and indirect cost rates.

            Returns
            -------
            WorkoutLgdBuilder
                ``self`` for chaining.

            Raises
            ------
            ValueError
                If the builder was already consumed by :meth:`build`.
            """
            ...

        def build(self) -> lgd.WorkoutLgd:
            """
            Validate and build the model; the builder is consumed.

            Returns
            -------
            WorkoutLgd
                Validated workout model.

            Raises
            ------
            ValueError
                If ``workout_years`` or ``discount_rate`` is negative or
                non-finite, or the builder was already consumed.
            """
            ...

    class DownturnLgd:
        """
        Downturn LGD adjuster (stressed approximation or regulatory floor).

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.DownturnLgd.regulatory_floor(0.05, 0.25).adjust(0.10)
        0.25

        """

        @staticmethod
        def stressed(asset_correlation: float, lgd_sensitivity: float, stress_quantile: float) -> lgd.DownturnLgd:
            """
            Stressed approximation:
            ``LGD_base + lgd_sensitivity * sqrt(rho) * Phi^-1(q) * sqrt(LGD_base * (1 - LGD_base))``.

            Parameters
            ----------
            asset_correlation : float
                Asset correlation ``rho`` in ``(0, 1)``; Basel 0.12-0.24.
            lgd_sensitivity : float
                LGD sensitivity to the systematic factor (>= 0); typical 0.3-0.5.
            stress_quantile : float
                Downturn quantile in ``(0, 1)``, e.g. ``0.999``.

            Returns
            -------
            DownturnLgd
                Adjuster with ``method == "stressed_approximation"``.

            Raises
            ------
            ValueError
                On out-of-range parameters.

            Examples
            --------
            >>> from finstack_quant.models.credit import lgd
            >>> lgd.DownturnLgd.stressed(0.15, 0.4, 0.999).adjust(0.4) > 0.4
            True
            """
            ...

        @staticmethod
        def regulatory_floor(add_on: float, floor: float) -> lgd.DownturnLgd:
            """
            Regulatory floor: ``max(LGD_base + add_on, floor)``.

            Parameters
            ----------
            add_on : float
                Flat add-on (>= 0); typical 0.05-0.10.
            floor : float
                Absolute floor in ``[0, 1]``; typical 0.10 secured / 0.25
                unsecured.

            Returns
            -------
            DownturnLgd
                Adjuster with ``method == "regulatory_floor"``.

            Raises
            ------
            ValueError
                On out-of-range parameters.

            Examples
            --------
            >>> from finstack_quant.models.credit import lgd
            >>> lgd.DownturnLgd.regulatory_floor(0.05, 0.25).adjust(0.30)
            0.35
            """
            ...

        @staticmethod
        def from_registry_id(id: str) -> lgd.DownturnLgd:
            """
            Load a regulatory-floor preset by id from the embedded registry.

            Parameters
            ----------
            id : str
                Registry preset identifier (e.g. ``"basel_unsecured"``).

            Returns
            -------
            DownturnLgd
                The preset adjuster.

            Raises
            ------
            KeyError
                If ``id`` is unknown.
            ValueError
                If the preset uses an unsupported method.

            Examples
            --------
            >>> from finstack_quant.models.credit import lgd
            >>> lgd.DownturnLgd.from_registry_id("basel_unsecured").method
            'regulatory_floor'
            """
            ...

        @staticmethod
        def basel_secured() -> lgd.DownturnLgd:
            """
            Registry default secured-exposure floor (Basel).

            Returns
            -------
            DownturnLgd
                Secured regulatory-floor preset.

            Raises
            ------
            ValueError
                If the embedded credit registry is invalid.

            Examples
            --------
            >>> from finstack_quant.models.credit import lgd
            >>> lgd.DownturnLgd.basel_secured().method
            'regulatory_floor'
            """
            ...

        @staticmethod
        def basel_unsecured() -> lgd.DownturnLgd:
            """
            Registry ``basel_unsecured`` floor preset.

            Returns
            -------
            DownturnLgd
                Unsecured regulatory-floor preset.

            Raises
            ------
            ValueError
                If the embedded credit registry is invalid.

            Examples
            --------
            >>> from finstack_quant.models.credit import lgd
            >>> lgd.DownturnLgd.basel_unsecured().adjust(0.0) > 0.0
            True
            """
            ...

        def adjust(self, base_lgd: float) -> float:
            """
            Downturn LGD for ``base_lgd``, clamped to ``[0, 1]``.

            Parameters
            ----------
            base_lgd : float
                Through-the-cycle LGD as a decimal in ``[0, 1]``.

            Returns
            -------
            float
                Downturn LGD as a decimal.

            Raises
            ------
            ValueError
                If ``base_lgd`` is non-finite or outside ``[0, 1]``.
            """
            ...

        @property
        def method(self) -> str:
            """
            Canonical method name: ``"stressed_approximation"`` or ``"regulatory_floor"``.

            Returns
            -------
            str
                Canonical method name: ``"stressed_approximation"`` or ``"regulatory_floor"``.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def params(self) -> Any:
            """
            Method parameters as a mapping in canonical JSON form.

            Returns
            -------
            Any
                Method parameters as a mapping in canonical JSON form.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @staticmethod
        def from_json(json: str) -> lgd.DownturnLgd:
            """
            Deserialize a ``DownturnLgd`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            DownturnLgd
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``DownturnLgd`` JSON.

            Examples
            --------
            >>> value = lgd.DownturnLgd.regulatory_floor(0.05, 0.25)
            >>> lgd.DownturnLgd.from_json(value.to_json()).to_json() == value.to_json()
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
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class EadCalculator:
        """
        Exposure-at-default calculator: ``EAD = drawn + undrawn * CCF``.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> calc = lgd.EadCalculator.revolver(60.0, 40.0)
        >>> (calc.ead, calc.utilization)
        (90.0, 0.6)

        """

        def __init__(self, drawn: float, undrawn: float, ccf: float) -> None:
            """
            Build a calculator with an explicit credit conversion factor.

            Parameters
            ----------
            drawn : float
                Currently drawn amount (>= 0).
            undrawn : float
                Undrawn commitment (>= 0).
            ccf : float
                Credit conversion factor as a decimal in ``[0, 1]``.

            Raises
            ------
            ValueError
                For negative or non-finite amounts or a CCF outside ``[0, 1]``.
            """
            ...

        @staticmethod
        def term_loan(drawn: float) -> lgd.EadCalculator:
            """
            Fully drawn term loan (no undrawn component, CCF ``1.0``).

            Parameters
            ----------
            drawn : float
                Drawn principal (>= 0).

            Returns
            -------
            EadCalculator
                Calculator whose ``ead`` equals ``drawn``.

            Raises
            ------
            ValueError
                If ``drawn`` is negative or non-finite.

            Examples
            --------
            >>> from finstack_quant.models.credit import lgd
            >>> lgd.EadCalculator.term_loan(100.0).ead
            100.0
            """
            ...

        @staticmethod
        def revolver(drawn: float, undrawn: float) -> lgd.EadCalculator:
            """
            Revolver with the Basel IRB CCF of ``0.75``.

            Parameters
            ----------
            drawn : float
                Currently drawn amount (>= 0).
            undrawn : float
                Undrawn commitment (>= 0).

            Returns
            -------
            EadCalculator
                Calculator with ``ead = drawn + 0.75 * undrawn``.

            Raises
            ------
            ValueError
                If an amount is negative or non-finite.

            Examples
            --------
            >>> from finstack_quant.models.credit import lgd
            >>> lgd.EadCalculator.revolver(60.0, 40.0).ead
            90.0
            """
            ...

        def leq_from_observed_ead(self, observed_ead: float) -> float | None:
            """
            Loan-equivalent exposure implied by an observed EAD.

            Parameters
            ----------
            observed_ead : float
                Realised exposure at default in the facility's currency.

            Returns
            -------
            float | None
                ``(observed_ead - drawn) / undrawn``, or ``None`` when there is
                no undrawn amount.

            Notes
            -----
            This method does not raise.
            """
            ...

        @property
        def ead(self) -> float:
            """
            ``drawn + undrawn * ccf``.

            Returns
            -------
            float
                ``drawn + undrawn * ccf``.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def utilization(self) -> float:
            """
            ``drawn / (drawn + undrawn)``, or ``0.0`` when there is no commitment.

            Returns
            -------
            float
                ``drawn / (drawn + undrawn)``, or ``0.0`` when there is no commitment.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def total_commitment(self) -> float:
            """
            ``drawn + undrawn``.

            Returns
            -------
            float
                ``drawn + undrawn``.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @staticmethod
        def from_json(json: str) -> lgd.EadCalculator:
            """
            Deserialize a ``EadCalculator`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            EadCalculator
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``EadCalculator`` JSON.

            Examples
            --------
            >>> value = lgd.EadCalculator.revolver(60.0, 40.0)
            >>> lgd.EadCalculator.from_json(value.to_json()).to_json() == value.to_json()
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
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    @staticmethod
    def seniority_recovery_stats(
        seniority: str,
        rating_agency: str | None = None,
    ) -> lgd.BetaRecovery:
        """
        Historical Beta recovery distribution for a seniority class.

        Parameters
        ----------
        seniority : str
            One of ``1st_lien_secured``, ``2nd_lien_secured``,
            ``senior_secured``, ``senior_unsecured``, ``subordinated``,
            ``junior_subordinated``.
        rating_agency : str | None
            ``"moodys"`` (canonical) or ``"sp"``. ``None`` selects the
            registry default calibration (Moody's historical).

        Returns
        -------
        BetaRecovery
            Moment-matched Beta distribution for the class.

        Raises
        ------
        ValueError
            If ``seniority`` or ``rating_agency`` is unknown, or the selected
            calibration has no entry for the class.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.seniority_recovery_stats("senior_secured").mean
        0.52

        """
        ...

    @staticmethod
    def beta_recovery_sample(
        mean: float,
        std: float,
        n_samples: int,
        seed: int,
    ) -> list[float]:
        """
        Draw recovery rates from ``BetaRecovery(mean, std)`` with a seeded PCG64 RNG.

        Thin twin of ``BetaRecovery(mean, std).sample_seeded(n_samples, seed)``.

        Parameters
        ----------
        mean : float
            Mean recovery rate in ``(0, 1)``.
        std : float
            Standard deviation; must satisfy ``std**2 < mean * (1 - mean)``.
        n_samples : int
            Number of draws to produce.
        seed : int
            RNG seed; the same seed yields the same sequence.

        Returns
        -------
        list[float]
            ``n_samples`` recovery rates as decimals.

        Raises
        ------
        ValueError
            If the moments are invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.beta_recovery_sample(0.4, 0.2, 3, 42) == lgd.beta_recovery_sample(0.4, 0.2, 3, 42)
        True

        """
        ...

    @staticmethod
    def beta_recovery_quantile(mean: float, std: float, q: float) -> float:
        """
        Recovery rate at quantile ``q`` of ``BetaRecovery(mean, std)``.

        Thin twin of ``BetaRecovery(mean, std).quantile(q)``.

        Parameters
        ----------
        mean : float
            Mean recovery rate in ``(0, 1)``.
        std : float
            Standard deviation; must satisfy ``std**2 < mean * (1 - mean)``.
        q : float
            Probability in ``(0, 1)``.

        Returns
        -------
        float
            Recovery rate as a decimal.

        Raises
        ------
        ValueError
            If the moments or ``q`` are invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> 0.0 < lgd.beta_recovery_quantile(0.4, 0.2, 0.5) < 1.0
        True

        """
        ...

    @staticmethod
    def workout_lgd(
        ead: float,
        collateral: list[tuple[str, float, float]],
        direct_cost_pct: float,
        indirect_cost_pct: float,
        time_to_resolution_years: float,
        discount_rate: float,
    ) -> lgd.WorkoutLgdResult:
        """
        Workout net recovery, LGD, and recovery rate in one call.

        One-shot twin of ``WorkoutLgd.builder()...build().evaluate(ead)``.

        Parameters
        ----------
        ead : float
            Exposure at default (> 0).
        collateral : list[tuple[str, float, float]]
            ``(collateral_type, book_value, haircut)`` triples; the type is one
            of ``cash``, ``securities``, ``receivables``, ``inventory``, ``equipment``, ``real_estate``, ``intellectual_property``, ``other`` and ``haircut`` is a decimal in ``[0, 1]``.
        direct_cost_pct : float
            Direct resolution costs as a decimal fraction of EAD (>= 0).
        indirect_cost_pct : float
            Indirect resolution costs as a decimal fraction of EAD (>= 0).
        time_to_resolution_years : float
            Expected workout duration in years (>= 0).
        discount_rate : float
            Annual decimal discount rate for the workout period (>= 0).

        Returns
        -------
        WorkoutLgdResult
            ``net_recovery``, ``lgd`` and ``recovery_rate``.

        Raises
        ------
        ValueError
            For an unknown collateral type or any invalid input.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> result = lgd.workout_lgd(100.0, [("cash", 50.0, 0.0)], 0.0, 0.0, 0.0, 0.0)
        >>> (result.net_recovery, result.lgd)
        (50.0, 0.5)

        """
        ...

    @staticmethod
    def downturn_lgd_stressed(
        base_lgd: float,
        asset_correlation: float,
        lgd_sensitivity: float,
        stress_quantile: float,
    ) -> float:
        """
        Stressed downturn adjustment of a base LGD.

        Thin twin of ``DownturnLgd.stressed(...).adjust(base_lgd)``:
        ``LGD_base + lgd_sensitivity * sqrt(rho) * Phi^-1(q) * sqrt(LGD_base * (1 - LGD_base))``
        clamped to ``[0, 1]`` (a mean-plus-multiple-of-Bernoulli-stdev
        approximation, not the Frye-Jacobs 2012 model).

        Parameters
        ----------
        base_lgd : float
            Through-the-cycle LGD in ``[0, 1]``.
        asset_correlation : float
            Asset correlation ``rho`` in ``(0, 1)``; Basel 0.12-0.24.
        lgd_sensitivity : float
            LGD sensitivity to the systematic factor (>= 0); typical 0.3-0.5.
        stress_quantile : float
            Downturn quantile in ``(0, 1)``, e.g. ``0.999``.

        Returns
        -------
        float
            Downturn LGD as a decimal.

        Raises
        ------
        ValueError
            On out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.downturn_lgd_stressed(0.4, 0.15, 0.4, 0.999) > 0.4
        True

        """
        ...

    @staticmethod
    def downturn_lgd_regulatory_floor(
        base_lgd: float,
        add_on: float,
        floor: float,
    ) -> float:
        """
        Regulatory-floor downturn adjustment: ``max(LGD_base + add_on, floor)``.

        Thin twin of ``DownturnLgd.regulatory_floor(add_on, floor).adjust(base_lgd)``;
        the result is clamped to ``[0, 1]``.

        Parameters
        ----------
        base_lgd : float
            Through-the-cycle LGD in ``[0, 1]``.
        add_on : float
            Flat add-on (>= 0); typical 0.05-0.10.
        floor : float
            Absolute floor in ``[0, 1]``; typical 0.10 secured / 0.25 unsecured.

        Returns
        -------
        float
            Downturn LGD as a decimal.

        Raises
        ------
        ValueError
            On out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.downturn_lgd_regulatory_floor(0.10, 0.05, 0.25)
        0.25

        """
        ...

    @staticmethod
    def ead_term_loan(principal: float) -> float:
        """
        Exposure at default for a fully drawn term loan (``principal`` itself).

        Parameters
        ----------
        principal : float
            Drawn principal (>= 0).

        Returns
        -------
        float
            ``principal``.

        Raises
        ------
        ValueError
            If ``principal`` is negative or non-finite.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.ead_term_loan(100.0)
        100.0

        """
        ...

    @staticmethod
    def ead_revolver(drawn: float, undrawn: float, ccf: float) -> float:
        """
        Exposure at default for a revolving facility: ``drawn + undrawn * ccf``.

        Parameters
        ----------
        drawn : float
            Currently drawn amount (>= 0).
        undrawn : float
            Undrawn commitment (>= 0).
        ccf : float
            Credit conversion factor in ``[0, 1]``; Basel IRB ``0.75``.

        Returns
        -------
        float
            Exposure at default.

        Raises
        ------
        ValueError
            On negative amounts or a CCF outside ``[0, 1]``.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.ead_revolver(60.0, 40.0, 0.5)
        80.0

        """
        ...

class migration:
    """
    Credit migration: rating scales, transition matrices, generators, and CTMC simulation.

    Examples
    --------
    >>> from finstack_quant.models.credit import migration
    >>> migration.RatingScale.custom_with_default(["A", "D"], "D").labels()
    ['A', 'D']

    """

    class RatingScale:
        """
        Ordinal rating scale (highest grade first) with an optional absorbing
        default state.

        Examples
        --------
        >>> from finstack_quant.models.credit import migration
        >>> scale = migration.RatingScale.custom_with_default(["A", "D"], "D")
        >>> (scale.n_states, scale.index_of("A"), scale.default_state(), scale.labels())
        (2, 0, 1, ['A', 'D'])
        >>> (scale.warf("A"), scale.rating_from_warf(120.0))
        (120.0, 'A')

        """

        @staticmethod
        def standard() -> migration.RatingScale:
            """
            The standard whole-letter scale (AAA .. CCC, D), highest grade first.

            Returns
            -------
            RatingScale
                Eight-state scale with ``D`` absorbing.

            Notes
            -----
            This constructor does not raise.

            Examples
            --------
            >>> from finstack_quant.models.credit import migration
            >>> migration.RatingScale.standard().labels()[0]
            'AAA'
            """
            ...

        @staticmethod
        def standard_with_nr() -> migration.RatingScale:
            """
            The standard scale with an explicit not-rated state appended.

            Returns
            -------
            RatingScale
                Standard scale plus ``NR``.

            Notes
            -----
            This constructor does not raise.

            Examples
            --------
            >>> from finstack_quant.models.credit import migration
            >>> migration.RatingScale.standard_with_nr().labels()[-2]
            'NR'
            """
            ...

        @staticmethod
        def notched() -> migration.RatingScale:
            """
            A notched scale (AA+/AA/AA-, ...) rather than whole grades.

            Returns
            -------
            RatingScale
                Notched scale with ``D`` absorbing.

            Notes
            -----
            This constructor does not raise.

            Examples
            --------
            >>> from finstack_quant.models.credit import migration
            >>> "AA+" in migration.RatingScale.notched().labels()
            True
            """
            ...

        @staticmethod
        def custom(labels: list[str]) -> migration.RatingScale:
            """
            A scale from explicit labels; the last label is the absorbing default.

            Parameters
            ----------
            labels : list[str]
                At least two distinct labels, highest grade first.

            Returns
            -------
            RatingScale
                Custom scale.

            Raises
            ------
            ValueError
                For fewer than two labels or duplicates.

            Examples
            --------
            >>> from finstack_quant.models.credit import migration
            >>> migration.RatingScale.custom(["A", "B", "D"]).default_state()
            2
            """
            ...

        @staticmethod
        def custom_with_default(labels: list[str], default_label: str) -> migration.RatingScale:
            """
            A custom scale with an explicit default (absorbing) state label.

            Parameters
            ----------
            labels : list[str]
                At least two distinct labels, highest grade first.
            default_label : str
                Label of the absorbing default state; must be in ``labels``.

            Returns
            -------
            RatingScale
                Custom scale.

            Raises
            ------
            ValueError
                For fewer than two labels or duplicates.
            KeyError
                If ``default_label`` is not in ``labels``.

            Examples
            --------
            >>> from finstack_quant.models.credit import migration
            >>> migration.RatingScale.custom_with_default(["A", "D"], "D").default_state()
            1
            """
            ...

        @property
        def n_states(self) -> int:
            """
            Number of rating states in the scale.

            Returns
            -------
            int
                Number of rating states in the scale.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...

        def index_of(self, label: str) -> int | None:
            """
            Index of a label in the scale.

            Parameters
            ----------
            label : str
                Rating label to look up.

            Returns
            -------
            int | None
                Zero-based state index, or ``None`` if absent.

            Notes
            -----
            This method does not raise.
            """
            ...

        def index_of_required(self, label: str) -> int:
            """
            Index of a label in the scale, raising if absent.

            Parameters
            ----------
            label : str
                Rating label to look up.

            Returns
            -------
            int
                Zero-based state index.

            Raises
            ------
            KeyError
                If ``label`` is not in the scale.
            """
            ...

        def label_of(self, index: int) -> str | None:
            """
            Label at a state index.

            Parameters
            ----------
            index : int
                Zero-based state index.

            Returns
            -------
            str | None
                Label, or ``None`` when ``index`` is out of range.

            Notes
            -----
            This method does not raise.
            """
            ...

        def default_state(self) -> int | None:
            """
            Index of the default state.

            Returns
            -------
            int | None
                Zero-based index, or ``None`` if the scale has no default state.

            Notes
            -----
            This method does not raise.
            """
            ...

        def labels(self) -> list[str]:
            """
            Rating labels, highest grade first.

            Returns
            -------
            list[str]
                Labels in scale order.

            Notes
            -----
            This method does not raise.
            """
            ...

        def warf(self, label: str) -> float:
            """
            Weighted-average rating factor for a label.

            Parameters
            ----------
            label : str
                Rating label.

            Returns
            -------
            float
                Moody's-style WARF factor.

            Raises
            ------
            KeyError
                If the label is unknown or has no WARF factor.
            """
            ...

        def rating_from_warf(self, warf: float) -> str:
            """
            Nearest rating label for a weighted-average rating factor.

            Parameters
            ----------
            warf : float
                Non-negative, finite WARF value.

            Returns
            -------
            str
                Label whose factor is closest to ``warf``.

            Raises
            ------
            ValueError
                If ``warf`` is non-finite or negative.
            """
            ...

        @staticmethod
        def from_json(json: str) -> migration.RatingScale:
            """
            Deserialize a ``RatingScale`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            RatingScale
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``RatingScale`` JSON.

            Examples
            --------
            >>> value = migration.RatingScale.standard()
            >>> migration.RatingScale.from_json(value.to_json()) == value
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
        def __len__(self) -> int:
            """
            Number of rating states.

            Returns
            -------
            int
                Same as :attr:`n_states`.

            Notes
            -----
            This method does not raise.
            """
            ...
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class TransitionMatrix:
        """
        Row-stochastic rating transition matrix over a horizon in years.

        Rows are origin states and columns destination states in ``scale``
        order.

        Examples
        --------
        >>> from finstack_quant.models.credit import migration
        >>> scale = migration.RatingScale.custom(["A", "D"])
        >>> matrix = migration.TransitionMatrix(scale, [[0.9, 0.1], [0.0, 1.0]], 1.0)
        >>> (matrix.probability("A", "D"), matrix.horizon, matrix.n_states)
        (0.1, 1.0, 2)

        """

        def __init__(
            self, scale: migration.RatingScale, data: list[float] | list[list[float]] | Any, horizon: float
        ) -> None:
            """
            Build a transition matrix.

            Parameters
            ----------
            scale : RatingScale
                Rating scale defining row/column order.
            data : list[float] | list[list[float]] | numpy.ndarray
                Probabilities, row-major flat (``n * n`` values), nested rows,
                or a 2-D array.
            horizon : float
                Horizon the probabilities cover, in years (> 0).

            Raises
            ------
            ValueError
                If the dimension does not match the scale, a row does not sum
                to one, an entry is outside ``[0, 1]``, the default state is
                not absorbing, or the horizon is invalid.
            """
            ...

        @staticmethod
        def from_dataframe(
            df: pandas.DataFrame,
            horizon: float,
            scale: migration.RatingScale | None = None,
        ) -> migration.TransitionMatrix:
            """
            Build a transition matrix from a labelled square ``pandas.DataFrame``.

            Parameters
            ----------
            df : pandas.DataFrame
                Square frame whose index (origins) and columns (destinations)
                carry the same labels in scale order.
            horizon : float
                Horizon in years (> 0).
            scale : RatingScale | None
                Scale to validate against; defaults to
                ``RatingScale.custom(list(df.index))`` (last label absorbing).

            Returns
            -------
            TransitionMatrix
                Validated matrix.

            Raises
            ------
            ValueError
                If index and columns differ, or the matrix is invalid for the
                scale.

            Examples
            --------
            >>> import pandas
            >>> from finstack_quant.models.credit import migration
            >>> df = pandas.DataFrame([[0.9, 0.1], [0.0, 1.0]], index=["A", "D"], columns=["A", "D"])
            >>> migration.TransitionMatrix.from_dataframe(df, 1.0).probability("A", "D")
            0.1
            """
            ...

        def probability(self, from_: str, to: str) -> float:
            """
            Transition probability between labelled states.

            Parameters
            ----------
            from_ : str
                Origin state label.
            to : str
                Destination state label.

            Returns
            -------
            float
                Probability over the matrix horizon.

            Raises
            ------
            KeyError
                For an unknown label.
            """
            ...

        def probability_by_index(self, from_: int, to: int) -> float:
            """
            Transition probability between state indices.

            Parameters
            ----------
            from_ : int
                Origin state index.
            to : int
                Destination state index.

            Returns
            -------
            float
                Probability over the matrix horizon.

            Raises
            ------
            IndexError
                If an index is out of range.
            """
            ...

        def row(self, from_: str) -> list[float]:
            """
            One row of transition probabilities, indexed by destination state.

            Parameters
            ----------
            from_ : str
                Origin state label.

            Returns
            -------
            list[float]
                Probabilities in scale order.

            Raises
            ------
            KeyError
                For an unknown label.
            """
            ...

        def compose(self, other: migration.TransitionMatrix) -> migration.TransitionMatrix:
            """
            Compose with another matrix on the same scale: ``P(s + t) = P(s) @ P(t)``.

            Parameters
            ----------
            other : TransitionMatrix
                Matrix over the same rating scale.

            Returns
            -------
            TransitionMatrix
                Composed matrix with horizon ``self.horizon + other.horizon``.

            Raises
            ------
            ValueError
                If the scales differ.
            """
            ...

        def to_matrix(self) -> list[list[float]]:
            """
            Row-major copy of the underlying matrix.

            Returns
            -------
            list[list[float]]
                Nested rows in scale order.

            Notes
            -----
            This method does not raise.
            """
            ...

        def default_probabilities(self) -> list[float] | None:
            """
            Probability of reaching the default state per origin state.

            Returns
            -------
            list[float] | None
                One value per origin state, or ``None`` when the scale has no
                default state.

            Notes
            -----
            This method does not raise.
            """
            ...

        @property
        def horizon(self) -> float:
            """
            Horizon this matrix covers, in years.

            Returns
            -------
            float
                Horizon this matrix covers, in years.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def n_states(self) -> int:
            """
            Number of rating states in the scale.

            Returns
            -------
            int
                Number of rating states in the scale.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def scale(self) -> migration.RatingScale:
            """
            The rating scale defining row/column order.

            Returns
            -------
            migration.RatingScale
                The rating scale defining row/column order.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        def to_dataframe(self) -> pandas.DataFrame:
            """
            Labelled square frame (index = origin, columns = destination).

            Returns
            -------
            pandas.DataFrame
                Labelled square frame (index = origin, columns = destination).

            Raises
            ------
            ValueError
                If the value cannot be serialized into a pandas object.
            """
            ...
        @staticmethod
        def from_json(json: str) -> migration.TransitionMatrix:
            """
            Deserialize a ``TransitionMatrix`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            TransitionMatrix
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``TransitionMatrix`` JSON.

            Examples
            --------
            >>> value = migration.TransitionMatrix(migration.RatingScale.custom(["A", "D"]), [0.9, 0.1, 0.0, 1.0], 1.0)
            >>> migration.TransitionMatrix.from_json(value.to_json()).to_json() == value.to_json()
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
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class GeneratorMatrix:
        """
        Annualized continuous-time Markov generator ``Q`` (rows sum to zero,
        non-negative off-diagonals) over a rating scale.

        Examples
        --------
        >>> from finstack_quant.models.credit import migration
        >>> scale = migration.RatingScale.custom(["A", "D"])
        >>> gen = migration.GeneratorMatrix(scale, [[-0.1, 0.1], [0.0, 0.0]])
        >>> (gen.intensity("A", "D"), gen.exit_rate("A"), gen.n_states)
        (0.1, 0.1, 2)

        """

        def __init__(self, scale: migration.RatingScale, data: list[float] | list[list[float]] | Any) -> None:
            """
            Build a generator matrix.

            Parameters
            ----------
            scale : RatingScale
                Rating scale defining row/column order.
            data : list[float] | list[list[float]] | numpy.ndarray
                Intensities per year, row-major flat, nested rows, or a 2-D
                array.

            Raises
            ------
            ValueError
                If the dimension does not match the scale, a row does not sum
                to zero, an off-diagonal is negative, or the default state is
                not absorbing.
            """
            ...

        @staticmethod
        def from_transition_matrix(p: migration.TransitionMatrix) -> migration.GeneratorMatrix:
            """
            Embed a transition matrix as a generator via the matrix logarithm
            (Israel-Rosenthal-Wei with Kreinin-Sidenius regularization).

            Parameters
            ----------
            p : TransitionMatrix
                Source matrix.

            Returns
            -------
            GeneratorMatrix
                Annualized generator with extraction diagnostics stamped.

            Raises
            ------
            RuntimeError
                If no valid generator exists (complex or non-positive
                eigenvalues) or the round-trip error exceeds the default
                tolerance.

            Examples
            --------
            >>> from finstack_quant.models.credit import migration
            >>> scale = migration.RatingScale.custom(["A", "D"])
            >>> p = migration.TransitionMatrix(scale, [0.9, 0.1, 0.0, 1.0], 1.0)
            >>> migration.GeneratorMatrix.from_transition_matrix(p).round_trip_error >= 0.0
            True
            """
            ...

        @staticmethod
        def from_transition_matrix_with_tol(
            p: migration.TransitionMatrix,
            round_trip_tol: float,
        ) -> migration.GeneratorMatrix:
            """
            Like :meth:`from_transition_matrix` with an explicit round-trip tolerance.

            Parameters
            ----------
            p : TransitionMatrix
                Source matrix.
            round_trip_tol : float
                Non-negative infinity-norm tolerance on ``exp(Q * h) - P(h)``.

            Returns
            -------
            GeneratorMatrix
                Annualized generator.

            Raises
            ------
            RuntimeError
                If no valid generator exists or the round-trip error exceeds
                ``round_trip_tol``.

            Examples
            --------
            >>> from finstack_quant.models.credit import migration
            >>> scale = migration.RatingScale.custom(["A", "D"])
            >>> p = migration.TransitionMatrix(scale, [0.9, 0.1, 0.0, 1.0], 1.0)
            >>> migration.GeneratorMatrix.from_transition_matrix_with_tol(p, 1e-6).n_states
            2
            """
            ...

        def intensity(self, from_: str, to: str) -> float:
            """
            Off-diagonal generator intensity (per year) between labelled states.

            Parameters
            ----------
            from_ : str
                Origin state label.
            to : str
                Destination state label.

            Returns
            -------
            float
                Annualized intensity.

            Raises
            ------
            KeyError
                For an unknown label.
            """
            ...

        def exit_rate(self, state: str) -> float:
            """
            Total intensity of leaving a state (the negated diagonal entry).

            Parameters
            ----------
            state : str
                State label.

            Returns
            -------
            float
                Annualized exit intensity.

            Raises
            ------
            KeyError
                For an unknown label.
            """
            ...

        def to_matrix(self) -> list[list[float]]:
            """
            Row-major copy of the underlying matrix.

            Returns
            -------
            list[list[float]]
                Nested rows in scale order.

            Notes
            -----
            This method does not raise.
            """
            ...

        @property
        def n_states(self) -> int:
            """
            Number of rating states in the scale.

            Returns
            -------
            int
                Number of rating states in the scale.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def scale(self) -> migration.RatingScale:
            """
            The rating scale defining row/column order.

            Returns
            -------
            migration.RatingScale
                The rating scale defining row/column order.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def regularization_l1(self) -> float:
            """
            L1 mass clamped by Kreinin-Sidenius regularization during extraction (zero for directly constructed generators).

            Returns
            -------
            float
                L1 mass clamped by Kreinin-Sidenius regularization during extraction (zero for directly constructed generators).

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def round_trip_error(self) -> float:
            """
            Infinity-norm error from reconstructing the source transition matrix (zero for directly constructed generators).

            Returns
            -------
            float
                Infinity-norm error from reconstructing the source transition matrix (zero for directly constructed generators).

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        def to_dataframe(self) -> pandas.DataFrame:
            """
            Labelled square frame (index = origin, columns = destination).

            Returns
            -------
            pandas.DataFrame
                Labelled square frame (index = origin, columns = destination).

            Raises
            ------
            ValueError
                If the value cannot be serialized into a pandas object.
            """
            ...
        @staticmethod
        def from_json(json: str) -> migration.GeneratorMatrix:
            """
            Deserialize a ``GeneratorMatrix`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            GeneratorMatrix
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``GeneratorMatrix`` JSON.

            Examples
            --------
            >>> value = migration.GeneratorMatrix(migration.RatingScale.custom(["A", "D"]), [-0.1, 0.1, 0.0, 0.0])
            >>> migration.GeneratorMatrix.from_json(value.to_json()).to_json() == value.to_json()
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
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class RatingPath:
        """
        One simulated rating trajectory recorded as ``(time, new_state)`` transitions.

        Examples
        --------
        >>> from finstack_quant.models.credit import migration
        >>> scale = migration.RatingScale.custom(["A", "D"])
        >>> gen = migration.GeneratorMatrix(scale, [-0.25, 0.25, 0.0, 0.0])
        >>> path = migration.MigrationSimulator(gen, 3.0).simulate(0, 1, 42)[0]
        >>> (path.label_at(0.0), path.horizon)
        ('A', 3.0)

        """

        def state_at(self, t: float) -> int:
            """
            Rating state index occupied at time ``t`` (right-continuous at jumps).

            Parameters
            ----------
            t : float
                Time in years within ``[0, horizon]``.

            Returns
            -------
            int
                Zero-based state index.

            Notes
            -----
            This method does not raise.
            """
            ...

        def label_at(self, t: float) -> str:
            """
            Rating label occupied at time ``t``.

            Parameters
            ----------
            t : float
                Time in years within ``[0, horizon]``.

            Returns
            -------
            str
                Rating label.

            Notes
            -----
            This method does not raise.
            """
            ...

        def defaulted(self) -> bool:
            """
            Whether the path reached the default state.

            Returns
            -------
            bool
                ``True`` when the absorbing default state was entered.

            Notes
            -----
            This method does not raise.
            """
            ...

        def default_time(self) -> float | None:
            """
            Time of default in years.

            Returns
            -------
            float | None
                Default time, or ``None`` if the path never defaulted.

            Notes
            -----
            This method does not raise.
            """
            ...

        def n_transitions(self) -> int:
            """
            Number of recorded transitions, including the initial ``(0.0, s0)`` entry.

            Returns
            -------
            int
                Transition count.

            Notes
            -----
            This method does not raise.
            """
            ...

        def transitions(self) -> list[tuple[float, int]]:
            """
            Every ``(time, new_state)`` event on the path.

            Returns
            -------
            list[tuple[float, int]]
                Events in time order; the first is always ``(0.0, initial_state)``.

            Notes
            -----
            This method does not raise.
            """
            ...

        @property
        def horizon(self) -> float:
            """
            Simulation horizon in years.

            Returns
            -------
            float
                Simulation horizon in years.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def scale(self) -> migration.RatingScale:
            """
            The rating scale the state indices refer to.

            Returns
            -------
            migration.RatingScale
                The rating scale the state indices refer to.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @staticmethod
        def from_json(json: str) -> migration.RatingPath:
            """
            Deserialize a ``RatingPath`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            RatingPath
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``RatingPath`` JSON.

            Examples
            --------
            >>> value = migration.MigrationSimulator(
            ...     migration.GeneratorMatrix(migration.RatingScale.custom(["A", "D"]), [-0.25, 0.25, 0.0, 0.0]), 3.0
            ... ).simulate(0, 1, 42)[0]
            >>> migration.RatingPath.from_json(value.to_json()).to_json() == value.to_json()
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
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class RatingPaths:
        """
        Collection of simulated rating paths from ``MigrationSimulator.simulate``.

        Indexable like a list of :class:`RatingPath`; ``to_dataframe()`` gives
        one long frame over all transitions.

        Examples
        --------
        >>> from finstack_quant.models.credit import migration
        >>> scale = migration.RatingScale.custom(["A", "D"])
        >>> gen = migration.GeneratorMatrix(scale, [-0.25, 0.25, 0.0, 0.0])
        >>> paths = migration.MigrationSimulator(gen, 3.0).simulate(0, 8, 42)
        >>> (len(paths), 0.0 <= paths.default_rate <= 1.0)
        (8, True)

        """

        @property
        def paths(self) -> list[migration.RatingPath]:
            """
            The paths as a list of ``RatingPath``.

            Returns
            -------
            list[migration.RatingPath]
                The paths as a list of ``RatingPath``.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def default_rate(self) -> float:
            """
            Fraction of paths that reached the default state.

            Returns
            -------
            float
                Fraction of paths that reached the default state.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        def to_dataframe(self) -> pandas.DataFrame:
            """
            Long frame with ``path`` (int), ``time`` (float, years), ``state`` (int), ``label`` (str); one row per recorded transition including the initial state, ordered by path then time.

            Returns
            -------
            pandas.DataFrame
                Long frame with ``path`` (int), ``time`` (float, years), ``state`` (int), ``label`` (str); one row per recorded transition including the initial state, ordered by path then time.

            Raises
            ------
            ValueError
                If the value cannot be serialized into a pandas object.
            """
            ...
        @staticmethod
        def from_json(json: str) -> migration.RatingPaths:
            """
            Deserialize a ``RatingPaths`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            RatingPaths
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``RatingPaths`` JSON.

            Examples
            --------
            >>> value = migration.MigrationSimulator(
            ...     migration.GeneratorMatrix(migration.RatingScale.custom(["A", "D"]), [-0.25, 0.25, 0.0, 0.0]), 3.0
            ... ).simulate(0, 2, 42)
            >>> migration.RatingPaths.from_json(value.to_json()).to_json() == value.to_json()
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
        def __len__(self) -> int:
            """
            Number of paths.

            Returns
            -------
            int
                Path count.

            Notes
            -----
            This method does not raise.
            """
            ...

        def __getitem__(self, index: int) -> migration.RatingPath:
            """
            Path at ``index`` (negative indices count from the end).

            Parameters
            ----------
            index : int
                Zero-based path index.

            Returns
            -------
            RatingPath
                The selected path.

            Raises
            ------
            IndexError
                If ``index`` is out of range.
            """
            ...
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    class MigrationSimulator:
        """
        Gillespie CTMC simulator over a generator matrix and horizon.

        Examples
        --------
        >>> from finstack_quant.models.credit import migration
        >>> scale = migration.RatingScale.custom(["A", "D"])
        >>> gen = migration.GeneratorMatrix(scale, [-0.25, 0.25, 0.0, 0.0])
        >>> sim = migration.MigrationSimulator(gen, 3.0)
        >>> (sim.horizon, len(sim.simulate(0, 4, 7)))
        (3.0, 4)

        """

        def __init__(self, generator: migration.GeneratorMatrix, horizon: float) -> None:
            """
            Build a simulator.

            Parameters
            ----------
            generator : GeneratorMatrix
                Annualized generator to simulate under.
            horizon : float
                Simulation horizon in years (> 0).

            Raises
            ------
            ValueError
                If ``horizon`` is non-positive or non-finite.
            """
            ...

        def simulate(self, initial_state: int, n_paths: int, seed: int) -> migration.RatingPaths:
            """
            Simulate rating paths from ``initial_state``.

            Paths are generated with the canonical ``Pcg64`` RNG seeded from
            ``seed``; identical seeds reproduce identical paths. The GIL is
            released during simulation.

            Parameters
            ----------
            initial_state : int
                Starting state index in the generator's scale.
            n_paths : int
                Number of paths (> 0).
            seed : int
                Seed for the canonical ``Pcg64`` generator; equal seeds give
                identical paths.

            Returns
            -------
            RatingPaths
                Simulated paths.

            Raises
            ------
            ValueError
                If the state index is out of range or ``n_paths`` is zero.
            """
            ...

        def empirical_matrix(self, n_paths_per_state: int, seed: int) -> migration.TransitionMatrix:
            """
            Build an empirical transition matrix by simulating from every state.

            Parameters
            ----------
            n_paths_per_state : int
                Paths simulated from each origin state (> 0).
            seed : int
                RNG seed for the canonical ``Pcg64`` generator.

            Returns
            -------
            TransitionMatrix
                Empirical matrix over the simulator horizon.

            Raises
            ------
            ValueError
                If ``n_paths_per_state`` is zero.
            """
            ...

        @property
        def horizon(self) -> float:
            """
            Simulation horizon in years.

            Returns
            -------
            float
                Simulation horizon in years.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @property
        def generator(self) -> migration.GeneratorMatrix:
            """
            The generator matrix simulated under.

            Returns
            -------
            migration.GeneratorMatrix
                The generator matrix simulated under.

            Notes
            -----
            This accessor does not raise; it returns the stored or derived value.
            """
            ...
        @staticmethod
        def from_json(json: str) -> migration.MigrationSimulator:
            """
            Deserialize a ``MigrationSimulator`` from its canonical JSON form.

            Parameters
            ----------
            json : str
                JSON text produced by :meth:`to_json` (strict serde; unknown or
                invalid fields are rejected).

            Returns
            -------
            MigrationSimulator
                Reconstructed value.

            Raises
            ------
            ValueError
                If the payload is not valid ``MigrationSimulator`` JSON.

            Examples
            --------
            >>> value = migration.MigrationSimulator(
            ...     migration.GeneratorMatrix(migration.RatingScale.custom(["A", "D"]), [-0.25, 0.25, 0.0, 0.0]), 3.0
            ... )
            >>> migration.MigrationSimulator.from_json(value.to_json()).to_json() == value.to_json()
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
        def __repr__(self) -> str:
            """
            Python-style representation rendered from the canonical fields.

            Returns
            -------
            str
                ``Name(field=value, ...)`` with Python literals.

            Notes
            -----
            This method does not raise.
            """
            ...

    @staticmethod
    def project(generator: migration.GeneratorMatrix, t: float) -> migration.TransitionMatrix:
        """
        Project a generator to a transition matrix: ``P(t) = exp(Q * t)``.

        Parameters
        ----------
        generator : GeneratorMatrix
            Continuous-time generator with non-negative off-diagonals and rows
            summing to zero.
        t : float
            Horizon in years; must be non-negative.

        Returns
        -------
        TransitionMatrix
            Row-stochastic migration probabilities over ``t`` years.

        Raises
        ------
        ValueError
            If ``t`` is negative or the projection does not produce a valid
            row-stochastic matrix.

        Sources
        -------
        Israel, Rosenthal & Wei (2001), *Mathematical Finance* 11(2), 245-265.

        Examples
        --------
        >>> from finstack_quant.models.credit import migration
        >>> scale = migration.RatingScale.custom(["AAA", "D"])
        >>> gen = migration.GeneratorMatrix(scale, [-0.01, 0.01, 0.0, 0.0])
        >>> round(migration.project(gen, 5.0).probability("AAA", "D"), 6)
        0.048771

        """
        ...

pd_mod = pd
lgd_mod = lgd
