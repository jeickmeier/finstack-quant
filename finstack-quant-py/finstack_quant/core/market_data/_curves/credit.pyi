"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/credit.rs``."""

from __future__ import annotations

from typing import Any, Optional, Sequence
import pandas as pd
from finstack_quant.core.market_data._curves.hazard import HazardCurve

class BaseCorrelationCurve:
    """
    Base-correlation curve for synthetic credit index tranche pricing.

    Stores ``(detachment_pct, correlation)`` knots where detachment points are
    in **percent** of index notional (``3.0`` for a 0-3% tranche) and
    correlations are decimals in ``[0, 1]``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import BaseCorrelationCurve
    >>> curve = BaseCorrelationCurve("CDX-IG", [(3.0, 0.25), (7.0, 0.40), (10.0, 0.55)])
    >>> curve.correlation(3.0)
    0.25
    >>> curve.detachment_points
    [3.0, 7.0, 10.0]

    """

    def __init__(self, id: str, knots: Sequence[tuple[float, float]]) -> None:
        """
        Construct a base-correlation curve from ``(detachment_pct, correlation)`` knots.

        Parameters
        ----------
        id : str
            Unique curve identifier (typically index name plus maturity).
        knots : Sequence[tuple[float, float]]
            ``(detachment_pct, correlation)`` pairs; detachment in percent of
            notional within ``[0, 100]``, correlation as a decimal in ``[0, 1]``.
            Decreasing correlation shapes are accepted and do not certify arbitrage-free pricing.

        Raises
        ------
        ValueError
            If fewer than two knots are supplied, values are non-finite, correlations
            are outside ``[0, 1]``, or detachments are duplicated or outside ``[0, 100]``.

        Examples
        --------
        >>> from finstack_quant.core.market_data import BaseCorrelationCurve
        >>> BaseCorrelationCurve("CDX-IG", [(3.0, 0.25), (10.0, 0.55)]).correlations
        [0.25, 0.55]

        """
        ...

    @staticmethod
    def from_json(json: str) -> BaseCorrelationCurve:
        """
        Deserialize a curve from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        BaseCorrelationCurve

        Raises
        ------
        ValueError
            If the JSON is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import BaseCorrelationCurve
        >>> curve = BaseCorrelationCurve("CDX-IG", [(3.0, 0.25), (10.0, 0.55)])
        >>> BaseCorrelationCurve.from_json(curve.to_json()) == curve
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the curve cannot be serialized.
        """
        ...

    def correlation(self, detachment_pct: float) -> float:
        """
        Interpolated base correlation (decimal) at a detachment point.

        Parameters
        ----------
        detachment_pct : float
            Detachment point in percent of index notional.

        Returns
        -------
        float
            Base correlation as a decimal in ``[0, 1]``.
        Notes
        -----
        This method does not raise.

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export knots as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``detachment_pct`` and ``correlation``; one row per knot.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this curve is registered and looked up under.

        Returns
        -------
        str
            The curve id supplied at construction (for example ``"USD-OIS"``); market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def detachment_points(self) -> list[float]:
        """
        Tranche detachment grid the correlations are quoted on.

        Returns
        -------
        list[float]
            Ascending detachment points in percent of index notional (``7.0`` is the 7% point), not decimals.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def correlations(self) -> list[float]:
        """
        Base correlations quoted at each detachment point.

        Returns
        -------
        list[float]
            One decimal correlation in ``[0, 1]`` per detachment point, in the same order as :attr:`detachment_points`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interp_style(self) -> str:
        """
        Interpolation scheme used between detachment points.

        Returns
        -------
        str
            Style label such as ``"linear"``, fixing how correlations between quoted detachment points are produced.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extrapolation(self) -> str:
        """
        Policy applied outside the quoted detachment range.

        Returns
        -------
        str
            Policy label such as ``"flat"``, governing queries below the first or above the last detachment point.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class CreditIndexData:
    """
    Credit index data bundle for synthetic tranche pricing.

    Groups the index hazard curve, the base-correlation curve, the number of
    constituents and the index recovery assumption. The bundle holds shared
    curve handles and has no JSON form of its own; serialize the
    :class:`MarketContext` it is inserted into instead.

    Examples
    --------
    >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
    >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
    >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
    >>> data = CreditIndexData(125, 0.4, hazard, base_corr)
    >>> (data.num_constituents, data.index_credit_curve.id)
    (125, 'CDX-IG')

    """

    def __init__(
        self,
        num_constituents: int,
        recovery_rate: float,
        index_credit_curve: HazardCurve,
        base_correlation_curve: BaseCorrelationCurve,
        *,
        issuer_curves: Optional[dict[str, HazardCurve]] = None,
        issuer_recovery_rates: Optional[dict[str, float]] = None,
        issuer_weights: Optional[dict[str, float]] = None,
    ) -> None:
        """
        Construct credit index data, optionally with issuer-level detail.

        Without the keyword arguments the bundle is homogeneous: every
        constituent uses the index curve, the index recovery and weight
        ``1 / num_constituents``. Supplying ``issuer_curves`` enables
        heterogeneous (bespoke) tranche pricing.

        Parameters
        ----------
        num_constituents : int
            Number of names in the index (e.g. ``125`` for CDX IG).
        recovery_rate : float
            Index recovery assumption as a decimal fraction in ``[0, 1]``.
        index_credit_curve : HazardCurve
            Hazard curve for the index as a whole.
        base_correlation_curve : BaseCorrelationCurve
            Base correlations by detachment point.
        issuer_curves : dict[str, HazardCurve], optional
            Hazard curve per issuer identifier (ticker, CUSIP, ...). When
            given it must cover exactly ``num_constituents`` distinct issuers.
        issuer_recovery_rates : dict[str, float], optional
            Recovery per issuer as a decimal in ``[0, 1]``; requires
            ``issuer_curves`` and only its identifiers. Issuers left out use
            ``recovery_rate``.
        issuer_weights : dict[str, float], optional
            Non-negative notional weight per issuer (decimal fractions summing
            to ``1.0`` within ``1e-9``); requires ``issuer_curves`` and must
            cover exactly its identifiers. Without it each issuer weighs
            ``1 / num_constituents``.

        Raises
        ------
        ValueError
            If ``num_constituents`` is zero, a recovery is outside ``[0, 1]``,
            the issuer curves do not cover ``num_constituents`` issuers, the
            issuer recoveries or weights name an unknown issuer (or are given
            without ``issuer_curves``), or the weights are negative, do not
            cover every issuer or do not sum to ``1.0``.
        TypeError
            If ``issuer_curves`` values are not :class:`HazardCurve` objects.

        Examples
        --------
        >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
        >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
        >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
        >>> CreditIndexData(125, 0.4, hazard, base_corr).recovery_rate
        0.4

        """
        ...

    @property
    def num_constituents(self) -> int:
        """
        Size of the index the bundle describes.

        Returns
        -------
        int
            Count of reference names in the index (``125`` for a standard CDX or iTraxx series); each carries equal weight unless ``issuer_weights`` was supplied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_rate(self) -> float:
        """
        Recovery assumption applied uniformly across the index.

        Returns
        -------
        float
            Recovery as a decimal fraction of notional (``0.4`` is 40%) used for every constituent without an issuer-level recovery.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def index_credit_curve(self) -> HazardCurve:
        """
        Credit curve describing the index level of risk.

        Returns
        -------
        HazardCurve
            The :class:`HazardCurve` bootstrapped from index quotes, used to price the index and to anchor tranche pricing.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_correlation_curve(self) -> BaseCorrelationCurve:
        """
        Correlation skew used to price index tranches.

        Returns
        -------
        BaseCorrelationCurve
            The :class:`BaseCorrelationCurve` supplying a base correlation per detachment point.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def get_issuer_curve(self, issuer_id: str) -> HazardCurve:
        """
        Hazard curve of one issuer, falling back to the index curve.

        Parameters
        ----------
        issuer_id : str
            Issuer identifier as supplied in ``issuer_curves``.

        Returns
        -------
        HazardCurve
            The issuer's curve, or ``index_credit_curve`` when the bundle has
            no curve for ``issuer_id`` (homogeneous assumption).

        Notes
        -----
        This method does not raise for an unknown issuer.

        Examples
        --------
        >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
        >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
        >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
        >>> data = CreditIndexData(
        ...     2,
        ...     0.4,
        ...     hazard,
        ...     base_corr,
        ...     issuer_curves={"A": HazardCurve.flat("A", "2025-01-01", 0.02, 0.3), "B": hazard},
        ...     issuer_recovery_rates={"A": 0.3},
        ...     issuer_weights={"A": 0.25, "B": 0.75},
        ... )
        >>> data.get_issuer_curve("A").id, data.get_issuer_curve("Z").id
        ('A', 'CDX-IG')
        """
        ...

    def has_issuer_curves(self) -> bool:
        """
        Whether issuer-level curves are present (heterogeneous pricing mode).

        Returns
        -------
        bool
            ``True`` when the bundle carries per-issuer hazard curves.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
        >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
        >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
        >>> data = CreditIndexData(
        ...     2,
        ...     0.4,
        ...     hazard,
        ...     base_corr,
        ...     issuer_curves={"A": HazardCurve.flat("A", "2025-01-01", 0.02, 0.3), "B": hazard},
        ...     issuer_recovery_rates={"A": 0.3},
        ...     issuer_weights={"A": 0.25, "B": 0.75},
        ... )
        >>> data.has_issuer_curves()
        True
        """
        ...

    def issuer_ids(self) -> list[str]:
        """
        Issuer identifiers with their own curve, sorted.

        Returns
        -------
        list[str]
            Sorted identifiers; empty for a homogeneous bundle.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
        >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
        >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
        >>> data = CreditIndexData(
        ...     2,
        ...     0.4,
        ...     hazard,
        ...     base_corr,
        ...     issuer_curves={"A": HazardCurve.flat("A", "2025-01-01", 0.02, 0.3), "B": hazard},
        ...     issuer_recovery_rates={"A": 0.3},
        ...     issuer_weights={"A": 0.25, "B": 0.75},
        ... )
        >>> data.issuer_ids()
        ['A', 'B']
        """
        ...

    def get_issuer_recovery(self, issuer_id: str) -> float:
        """
        Recovery rate of one issuer, falling back to the index recovery.

        Parameters
        ----------
        issuer_id : str
            Issuer identifier.

        Returns
        -------
        float
            The issuer's recovery as a decimal, or ``recovery_rate`` when none
            was supplied for ``issuer_id``.

        Notes
        -----
        This method does not raise for an unknown issuer.

        Examples
        --------
        >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
        >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
        >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
        >>> data = CreditIndexData(
        ...     2,
        ...     0.4,
        ...     hazard,
        ...     base_corr,
        ...     issuer_curves={"A": HazardCurve.flat("A", "2025-01-01", 0.02, 0.3), "B": hazard},
        ...     issuer_recovery_rates={"A": 0.3},
        ...     issuer_weights={"A": 0.25, "B": 0.75},
        ... )
        >>> data.get_issuer_recovery("A"), data.get_issuer_recovery("B")
        (0.3, 0.4)
        """
        ...

    def get_issuer_weight(self, issuer_id: str) -> float:
        """
        Notional weight of one issuer, falling back to equal weighting.

        Parameters
        ----------
        issuer_id : str
            Issuer identifier.

        Returns
        -------
        float
            The issuer's weight as a decimal fraction, or
            ``1 / num_constituents`` when none was supplied for ``issuer_id``.

        Notes
        -----
        This method does not raise for an unknown issuer.

        Examples
        --------
        >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
        >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
        >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
        >>> data = CreditIndexData(
        ...     2,
        ...     0.4,
        ...     hazard,
        ...     base_corr,
        ...     issuer_curves={"A": HazardCurve.flat("A", "2025-01-01", 0.02, 0.3), "B": hazard},
        ...     issuer_recovery_rates={"A": 0.3},
        ...     issuer_weights={"A": 0.25, "B": 0.75},
        ... )
        >>> data.get_issuer_weight("A"), data.get_issuer_weight("B")
        (0.25, 0.75)
        """
        ...

    def __repr__(self) -> str: ...
