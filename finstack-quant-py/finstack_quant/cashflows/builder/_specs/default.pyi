"""Stub declarations mirroring ``finstack-quant-py/src/bindings/cashflows/builder/specs/default.rs``."""

from __future__ import annotations

from typing import Any

class DefaultModelSpec:
    """
    Default rate model used to project defaults across a pool or portfolio.

    Constructed via named factories (:meth:`constant_cdr`, :meth:`sda`,
    :meth:`vector`, ...) mirroring the Rust curve presets.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import DefaultModelSpec
    >>> DefaultModelSpec.constant_cdr(0.02).to_dict()["cdr"]
    0.02
    """

    @staticmethod
    def constant_cdr(cdr: float) -> DefaultModelSpec:
        """
        Constant CDR (Constant Default Rate) with no seasoning curve.

        Parameters
        ----------
        cdr : float
            Annual constant default rate (e.g. ``0.02`` for 2%).

        Returns
        -------
        DefaultModelSpec
            A default model using a flat annual CDR.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import DefaultModelSpec
        >>> DefaultModelSpec.constant_cdr(0.02).to_dict()["cdr"]
        0.02
        """
        ...

    @staticmethod
    def sda(speed_multiplier: float) -> DefaultModelSpec:
        """
        SDA (Standard Default Assumption) seasoning curve.

        Parameters
        ----------
        speed_multiplier : float
            Curve speed relative to 100% SDA (``1.0`` = 100% SDA).

        Returns
        -------
        DefaultModelSpec
            A default model driven by the SDA seasoning ramp.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import DefaultModelSpec
        >>> DefaultModelSpec.sda(1.0) is not None
        True
        """
        ...

    @staticmethod
    def vector(monthly_cdr: list[float]) -> DefaultModelSpec:
        """
        Explicit CDR vector by seasoning month; the last entry extends to
        maturity.

        Parameters
        ----------
        monthly_cdr : list[float]
            Annual CDR as a decimal per seasoning month (non-empty).

        Returns
        -------
        DefaultModelSpec
            A default model on the vector curve.

        Raises
        ------
        TypeError
            If ``monthly_cdr`` is not a list of floats.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import DefaultModelSpec
        >>> DefaultModelSpec.vector([0.01, 0.02]).to_dict()["monthly_cdr"]
        [0.01, 0.02]
        """
        ...

    @staticmethod
    def cumulative_loss(cumulative_net_loss_pct: list[float], severity: float) -> DefaultModelSpec:
        """
        Cumulative net-loss curve; defaults are the loss increments grossed up
        by severity.

        Parameters
        ----------
        cumulative_net_loss_pct : list[float]
            Cumulative loss in percent of the original balance per seasoning
            month (non-decreasing).
        severity : float
            Loss given default as a decimal in ``(0, 1]``.

        Returns
        -------
        DefaultModelSpec
            A default model on the cumulative-loss curve.

        Raises
        ------
        TypeError
            If ``cumulative_net_loss_pct`` is not a list of floats.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import DefaultModelSpec
        >>> DefaultModelSpec.cumulative_loss([1.0, 2.0], 0.5).to_dict()["severity"]
        0.5
        """
        ...

    @staticmethod
    def timing(cumulative_default_rate: float, annual_pct: list[float]) -> DefaultModelSpec:
        """
        Default-timing curve: a lifetime default rate spread over years of
        seasoning.

        Parameters
        ----------
        cumulative_default_rate : float
            Lifetime default rate as a decimal of the original balance.
        annual_pct : list[float]
            Percent of the lifetime defaults realized in each year of seasoning
            (sums to 100).

        Returns
        -------
        DefaultModelSpec
            A default model on the timing curve.

        Raises
        ------
        TypeError
            If ``annual_pct`` is not a list of floats.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import DefaultModelSpec
        >>> DefaultModelSpec.timing(0.10, [20.0, 30.0, 50.0]).to_dict()["cumulative_default_rate"]
        0.1
        """
        ...

    def validate(self) -> None:
        """
        Validate the curve shape.

        Raises
        ------
        ValueError
            If a rate is non-finite or out of range, a cumulative-loss curve
            decreases, or a timing curve does not sum to 100.
        """
        ...

    def mdr(self, seasoning_months: int) -> float:
        """
        Monthly default rate (MDR) for the supplied seasoning.

        Parameters
        ----------
        seasoning_months : int
            Number of months since origination.

        Returns
        -------
        float
            The single-month default rate implied by this model at the
            given seasoning.

        Raises
        ------
        ValueError
            If the underlying curve parameters are invalid, such as a
            negative SDA speed multiplier.
        """
        ...

    def mdr_with_survival(self, seasoning_months: int, surviving_balance_fraction: float) -> float:
        """
        Monthly default rate on the balance actually surviving to the month.

        Mirrors Rust ``DefaultModelSpec::mdr_with_survival``. Identical to
        :meth:`mdr` for the constant, SDA and vector curves. Cumulative-loss
        and timing curves state defaults as a share of the original balance,
        so their month's default share is divided by
        ``surviving_balance_fraction``; applying the result to the current
        balance reproduces the curve's defaults regardless of amortization
        and prepayments.

        Parameters
        ----------
        seasoning_months : int
            Number of months since origination or pool start.
        surviving_balance_fraction : float
            Current pool balance divided by the balance the curve is expressed
            against, as a finite non-negative decimal (may exceed 1.0 after
            par build).

        Returns
        -------
        float
            Monthly default rate as a decimal in ``[0, 1]``; zero once the
            surviving balance is exhausted.

        Raises
        ------
        ValueError
            If ``surviving_balance_fraction`` is negative or non-finite, or the
            curve parameters are invalid.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import DefaultModelSpec
        >>> spec = DefaultModelSpec.timing(0.10, [50.0, 50.0])
        >>> round(spec.mdr_with_survival(6, 1.0), 6), round(spec.mdr_with_survival(6, 0.5), 6)
        (0.004167, 0.008333)
        """
        ...

    def cumulative_default_fraction(self, seasoning_months: int) -> float | None:
        """
        Cumulative defaults through a seasoning month as a fraction of the
        original balance.

        Mirrors Rust ``DefaultModelSpec::cumulative_default_fraction``.

        Parameters
        ----------
        seasoning_months : int
            Number of months since origination; month 0 is before any default.

        Returns
        -------
        float or None
            The fraction for cumulative-loss and timing curves (may exceed 1.0
            for a cumulative-loss curve with replenishment or par build);
            ``None`` for rate-based curves.

        Raises
        ------
        ValueError
            If the curve parameters are invalid.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import DefaultModelSpec
        >>> round(DefaultModelSpec.timing(0.10, [50.0, 50.0]).cumulative_default_fraction(12), 12)
        0.05
        >>> DefaultModelSpec.constant_cdr(0.02).cumulative_default_fraction(12) is None
        True
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Active default model and its parameters as a dict in the canonical wire form.

        Returns
        -------
        dict
            The serde wire form, for example ``{"curve": "constant", "cdr": 0.02}``.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import DefaultModelSpec
        >>> DefaultModelSpec.constant_cdr(0.02).to_dict()["cdr"]
        0.02
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
        >>> from finstack_quant.cashflows.builder import DefaultModelSpec
        >>> isinstance(DefaultModelSpec.constant_cdr(0.02).to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> DefaultModelSpec:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        DefaultModelSpec
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import DefaultModelSpec
        >>> value = DefaultModelSpec.constant_cdr(0.02)
        >>> DefaultModelSpec.from_json(value.to_json()).to_json() == value.to_json()
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
