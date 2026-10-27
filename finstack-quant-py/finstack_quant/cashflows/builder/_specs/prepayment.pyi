"""Stub declarations mirroring ``finstack-quant-py/src/bindings/cashflows/builder/specs/prepayment.rs``."""

from __future__ import annotations

from typing import Any

class PrepaymentModelSpec:
    """
    Prepayment rate model used to project prepayments across a pool.

    Constructed via named factories (:meth:`constant_cpr`, :meth:`psa`,
    :meth:`cmbs_with_lockout`) mirroring the Rust curve
    presets.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import PrepaymentModelSpec
    >>> PrepaymentModelSpec.constant_cpr(0.06).to_dict()["cpr"]
    0.06
    """

    @staticmethod
    def constant_cpr(cpr: float) -> PrepaymentModelSpec:
        """
        Constant CPR (Constant Prepayment Rate) with no seasoning curve.

        Parameters
        ----------
        cpr : float
            Annual constant prepayment rate (e.g. ``0.06`` for 6%).

        Returns
        -------
        PrepaymentModelSpec
            A prepayment model using a flat annual CPR.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import PrepaymentModelSpec
        >>> PrepaymentModelSpec.constant_cpr(0.06).to_dict()["cpr"]
        0.06
        """
        ...

    @staticmethod
    def psa(speed_multiplier: float) -> PrepaymentModelSpec:
        """
        PSA (Public Securities Association) seasoning curve.

        Parameters
        ----------
        speed_multiplier : float
            Curve speed relative to 100% PSA (``1.0`` = 100% PSA).

        Returns
        -------
        PrepaymentModelSpec
            A prepayment model driven by the PSA seasoning ramp.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import PrepaymentModelSpec
        >>> PrepaymentModelSpec.psa(1.0) is not None
        True
        """
        ...

    @staticmethod
    def cmbs_with_lockout(lockout_months: int, post_lockout_cpr: float) -> PrepaymentModelSpec:
        """
        CMBS-style lockout: zero prepayment, then a constant CPR.

        Parameters
        ----------
        lockout_months : int
            Number of months with zero prepayment from origination.
        post_lockout_cpr : float
            Annual constant CPR applied after the lockout period ends.

        Returns
        -------
        PrepaymentModelSpec
            A prepayment model with a lockout followed by constant CPR.

        Raises
        ------
        OverflowError
            If *lockout_months* is outside the unsigned 32-bit integer range.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import PrepaymentModelSpec
        >>> PrepaymentModelSpec.cmbs_with_lockout(24, 0.10) is not None
        True
        """
        ...

    @staticmethod
    def abs(speed: float) -> PrepaymentModelSpec:
        """
        ABS convention: ``speed`` of the ORIGINAL balance prepays each month,
        so ``SMM_t = speed / (1 - speed * (t - 1))``.

        Parameters
        ----------
        speed : float
            Monthly prepayment as a decimal fraction of the original balance
            (``0.015`` = 1.5% ABS), in ``[0, 1)``.

        Returns
        -------
        PrepaymentModelSpec
            A prepayment model on the ABS curve.

        Raises
        ------
        OverflowError
            Never; range validation happens in :meth:`validate`.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import PrepaymentModelSpec
        >>> PrepaymentModelSpec.abs(0.015).to_dict()["speed"]
        0.015
        """
        ...

    @staticmethod
    def vector(monthly_cpr: list[float]) -> PrepaymentModelSpec:
        """
        Explicit CPR vector by seasoning month; the last entry extends to
        maturity.

        Parameters
        ----------
        monthly_cpr : list[float]
            Annual CPR as a decimal per seasoning month (non-empty).

        Returns
        -------
        PrepaymentModelSpec
            A prepayment model on the vector curve.

        Raises
        ------
        TypeError
            If ``monthly_cpr`` is not a list of floats.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import PrepaymentModelSpec
        >>> PrepaymentModelSpec.vector([0.05, 0.10]).to_dict()["monthly_cpr"]
        [0.05, 0.1]
        """
        ...

    def validate(self) -> None:
        """
        Validate the curve shape.

        Raises
        ------
        ValueError
            If a rate is non-finite or outside ``[0, 1]``, the ABS speed is out
            of range, or a vector is empty.
        """
        ...

    def smm(self, seasoning_months: int) -> float:
        """
        Single-month mortality (SMM) for the supplied seasoning.

        Parameters
        ----------
        seasoning_months : int
            Number of months since origination.

        Returns
        -------
        float
            The single-month prepayment rate implied by this model at the
            given seasoning.

        Raises
        ------
        ValueError
            If the underlying curve parameters are invalid, such as a
            negative PSA speed multiplier.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Active prepayment model and its parameters as a dict in the canonical wire form.

        Returns
        -------
        dict
            The serde wire form, for example ``{"curve": "constant", "cpr": 0.06}``.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import PrepaymentModelSpec
        >>> PrepaymentModelSpec.constant_cpr(0.06).to_dict()["cpr"]
        0.06
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Strict-serde JSON document; round-trips through :meth:`from_json`.

        Raises
        ------
        ValueError
            If a field cannot be represented in JSON (non-finite float).

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import PrepaymentModelSpec
        >>> isinstance(PrepaymentModelSpec.psa(1.0).to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> PrepaymentModelSpec:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        PrepaymentModelSpec
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import PrepaymentModelSpec
        >>> value = PrepaymentModelSpec.psa(1.0)
        >>> PrepaymentModelSpec.from_json(value.to_json()).to_json() == value.to_json()
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Pickle support via the JSON wire form (``from_json``, ``(to_json(),)``).

        Returns
        -------
        tuple
            ``(from_json, (json,))`` reconstructor pair.

        Raises
        ------
        ValueError
            If the value holds a non-finite float that JSON cannot carry.
        """
        ...
