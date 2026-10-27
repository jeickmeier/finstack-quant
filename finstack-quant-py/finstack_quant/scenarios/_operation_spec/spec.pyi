"""Stub declarations mirroring ``finstack-quant-py/src/bindings/scenarios/operation_spec/spec.rs``."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from finstack_quant.scenarios._operation_spec.hierarchy import HierarchyTarget
from finstack_quant.scenarios._operation_spec.kinds import CurveKind, TenorMatchMode, TimeRollMode
from finstack_quant.scenarios._operation_spec.rate_binding import RateBindingSpec

class OperationSpec:
    """
    One shock, time roll, or binding inside a ``ScenarioSpec``.

    Each classmethod corresponds to one Rust ``OperationSpec`` variant;
    ``to_json()`` produces the canonical wire form. Units follow the
    constructor name:

    - ``*_pct`` fields are percentage points (``5.0`` = +5%).
    - ``*_bp`` fields are additive basis points (1 bp = 1e-4) — except on
      ``CurveKind.commodity()`` curves, where ``bp`` is **percent of the
      forward** (a commodity price curve has no rate to shift).
    - Vol-index ``*_pts`` are index points (``1.0`` on 18.5 → 19.5).
    - Correlation and base-correlation ``*_pts`` are **decimal correlation**
      (``0.02`` = +0.02, not percentage points).

    Every enum-valued argument accepts the typed wrapper or its snake-case
    label (``CurveKind.discount()`` or ``"discount"``).

    Examples
    --------
    >>> from finstack_quant.scenarios import OperationSpec, CurveKind
    >>> op = OperationSpec.curve_parallel_bp(CurveKind.discount(), "USD-OIS", 10.0)
    >>> op.kind
    'curve_parallel_bp'
    >>> op == OperationSpec.curve_parallel_bp("discount", "USD-OIS", 10.0)
    True
    """

    @classmethod
    def market_fx_pct(cls, base: str, quote: str, pct: float) -> OperationSpec:
        """
        FX rate percent shift (``pct = 5.0`` strengthens ``base`` by 5%).

        Parameters
        ----------
        base : str
            Base currency code.
        quote : str
            Quote currency code.
        pct : float
            Percent shift applied to the FX rate.

        Returns
        -------
        OperationSpec
            The ``market_fx_pct`` operation.

        Raises
        ------
        ValueError
            If ``base`` or ``quote`` is not a recognized ISO currency code.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.market_fx_pct("USD", "EUR", -5.0).kind
        'market_fx_pct'
        """
        ...

    @classmethod
    def equity_price_pct(cls, ids: list[str], pct: float) -> OperationSpec:
        """
        Equity price percent shock applied to all supplied identifiers.

        Parameters
        ----------
        ids : list[str]
            Equity identifier strings.
        pct : float
            Percent shock applied to each price.

        Returns
        -------
        OperationSpec
            The ``equity_price_pct`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.equity_price_pct(["SPY"], -10.0).kind
        'equity_price_pct'
        """
        ...

    @classmethod
    def instrument_price_pct_by_attr(
        cls, attrs: Mapping[str, str] | Sequence[tuple[str, str]], pct: float
    ) -> OperationSpec:
        """
        Instrument price percent shock by exact attribute match.

        Parameters
        ----------
        attrs : Mapping[str, str] | Sequence[tuple[str, str]]
            Attribute key-value pairs that must all match; insertion order is
            preserved. Must be non-empty (validated on apply).
        pct : float
            Percent shock applied to matched instruments (``-5.0`` = -5%).

        Returns
        -------
        OperationSpec
            The ``instrument_price_pct_by_attr`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.instrument_price_pct_by_attr({"sector": "tech"}, -5.0).kind
        'instrument_price_pct_by_attr'
        """
        ...

    @classmethod
    def curve_parallel_bp(
        cls,
        curve_kind: CurveKind | str,
        curve_id: str,
        bp: float,
        discount_curve_id: str | None = None,
    ) -> OperationSpec:
        """
        Parallel basis-point shift on a single named curve.

        Parameters
        ----------
        curve_kind : CurveKind | str
            Curve family: ``"discount"``, ``"forward"``, ``"par_cds"``,
            ``"inflation"`` or ``"commodity"``.
        curve_id : str
            Identifier of the one curve to shock, as registered in the
            market context (for example ``"USD-OIS"``).
        bp : float
            Additive shift in basis points applied to every node; for
            ``CurveKind.commodity()`` the value is **percent of the
            forward** rather than basis points.
        discount_curve_id : str, optional
            Discount curve used when re-bootstrapping shocked ParCDS
            quotes. ``None`` (the default) leaves the curve's own
            discounting unchanged.

        Returns
        -------
        OperationSpec
            A single scenario operation describing the parallel shift.

        Raises
        ------
        ValueError
            If ``curve_kind`` is not an accepted label.
        TypeError
            If ``curve_id`` is not a single string identifier.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind, OperationSpec
        >>> OperationSpec.curve_parallel_bp(CurveKind.discount(), "USD-OIS", 10.0).kind
        'curve_parallel_bp'
        """
        ...
    @classmethod
    def curve_node_bp(
        cls,
        curve_kind: CurveKind | str,
        curve_id: str,
        nodes: list[tuple[str, float]],
        match_mode: TenorMatchMode | str | None = None,
        discount_curve_id: str | None = None,
    ) -> OperationSpec:
        """
        Node-level basis-point shifts on a curve.

        Parameters
        ----------
        curve_kind : CurveKind | str
            Curve family, typed or as its wire label.
        curve_id : str
            Curve identifier in ``MarketContext``.
        nodes : list[tuple[str, float]]
            List of ``(tenor, bp)`` pairs (percent of forward for commodity
            curves).
        match_mode : TenorMatchMode | str, optional
            Tenor alignment strategy (``"exact"`` or ``"interpolate"``).
            Defaults to ``"interpolate"`` (key-rate bump at the requested
            tenor); ``"exact"`` requires a matching pillar.
        discount_curve_id : str, optional
            Discount curve used when re-bootstrapping shocked ParCDS quotes.

        Returns
        -------
        OperationSpec
            The ``curve_node_bp`` operation.

        Raises
        ------
        ValueError
            If ``curve_kind`` or ``match_mode`` is not an accepted label.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind, OperationSpec
        >>> OperationSpec.curve_node_bp(CurveKind.discount(), "USD-OIS", [("5Y", 10.0)]).kind
        'curve_node_bp'
        """
        ...

    @classmethod
    def vol_index_parallel_pts(cls, curve_id: str, points: float) -> OperationSpec:
        """
        Parallel shock to a volatility-index curve in absolute index points.

        Parameters
        ----------
        curve_id : str
            Volatility-index curve identifier.
        points : float
            Absolute index-point shift.

        Returns
        -------
        OperationSpec
            The ``vol_index_parallel_pts`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.vol_index_parallel_pts("VIX", 2.0).kind
        'vol_index_parallel_pts'
        """
        ...

    @classmethod
    def vol_index_node_pts(
        cls,
        curve_id: str,
        nodes: list[tuple[str, float]],
        match_mode: TenorMatchMode | str | None = None,
    ) -> OperationSpec:
        """
        Node-level shocks to a volatility-index curve in absolute index points.

        Parameters
        ----------
        curve_id : str
            Volatility-index curve identifier.
        nodes : list[tuple[str, float]]
            List of ``(tenor, points)`` pairs.
        match_mode : TenorMatchMode | str, optional
            Tenor alignment strategy (``"exact"`` or ``"interpolate"``).
            Defaults to ``"interpolate"`` (key-rate bump at the requested
            tenor); ``"exact"`` requires a matching pillar.

        Returns
        -------
        OperationSpec
            The ``vol_index_node_pts`` operation.

        Raises
        ------
        ValueError
            If ``match_mode`` is not an accepted label.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.vol_index_node_pts("VIX", [("1M", 2.0)]).kind
        'vol_index_node_pts'
        """
        ...

    @classmethod
    def base_corr_parallel_pts(cls, surface_id: str, points: float) -> OperationSpec:
        """
        Parallel base-correlation shift in decimal correlation.

        Parameters
        ----------
        surface_id : str
            Base-correlation surface identifier.
        points : float
            Additive decimal correlation shift (``0.02`` = +0.02, not
            percentage points).

        Returns
        -------
        OperationSpec
            The ``base_corr_parallel_pts`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.base_corr_parallel_pts("CDX", 0.01).kind
        'base_corr_parallel_pts'
        """
        ...

    @classmethod
    def base_corr_bucket_pts(
        cls,
        surface_id: str,
        points: float,
        detachment_bp: list[int] | None = None,
    ) -> OperationSpec:
        """
        Bucketed base-correlation shock by detachment.

        Parameters
        ----------
        surface_id : str
            Base-correlation surface identifier.
        points : float
            Additive decimal correlation shift (``0.02`` = +0.02).
        detachment_bp : list[int], optional
            Detachment points (in bp) to target. ``None`` targets all.

        Returns
        -------
        OperationSpec
            The ``base_corr_bucket_pts`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.base_corr_bucket_pts("CDX", 0.01).kind
        'base_corr_bucket_pts'
        """
        ...

    @classmethod
    def vol_surface_parallel_pct(cls, vol_surface_id: str, pct: float) -> OperationSpec:
        """
        Parallel percent shift to a volatility surface.

        Parameters
        ----------
        vol_surface_id : str
            Volatility-surface identifier.
        pct : float
            Percent shift applied to every vol quote.

        Returns
        -------
        OperationSpec
            The ``vol_surface_parallel_pct`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.vol_surface_parallel_pct("SPX", 10.0).kind
        'vol_surface_parallel_pct'
        """
        ...

    @classmethod
    def vol_surface_bucket_pct(
        cls,
        vol_surface_id: str,
        pct: float,
        tenors: list[str] | None = None,
        strikes: list[float] | None = None,
    ) -> OperationSpec:
        """
        Bucketed volatility surface percent shock.

        Parameters
        ----------
        vol_surface_id : str
            Volatility-surface identifier.
        pct : float
            Percent shift applied to matched vol quotes.
        tenors : list[str], optional
            Tenor labels to target. ``None`` targets all.
        strikes : list[float], optional
            Strike levels to target. ``None`` targets all.

        Returns
        -------
        OperationSpec
            The ``vol_surface_bucket_pct`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.vol_surface_bucket_pct("SPX", 10.0).kind
        'vol_surface_bucket_pct'
        """
        ...

    @classmethod
    def stmt_forecast_percent(cls, node_id: str, pct: float) -> OperationSpec:
        """
        Statement forecast percent change.

        Parameters
        ----------
        node_id : str
            Statement forecast node identifier.
        pct : float
            Percent change applied to the forecast value.

        Returns
        -------
        OperationSpec
            The ``stmt_forecast_percent`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.stmt_forecast_percent("revenue", 5.0).kind
        'stmt_forecast_percent'
        """
        ...

    @classmethod
    def stmt_forecast_assign(cls, node_id: str, value: float) -> OperationSpec:
        """
        Statement forecast value assignment.

        Parameters
        ----------
        node_id : str
            Statement forecast node identifier.
        value : float
            Finite absolute value in the node's units. Monetary forecasts retain
            their currency and interpret this as major currency units; actuals
            are preserved.

        Returns
        -------
        OperationSpec
            The ``stmt_forecast_assign`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.stmt_forecast_assign("revenue", 100.0).kind
        'stmt_forecast_assign'
        """
        ...

    @classmethod
    def rate_binding(cls, binding: RateBindingSpec) -> OperationSpec:
        """
        Bind a statement rate node to a curve for the lifetime of the scenario.

        Parameters
        ----------
        binding : RateBindingSpec
            Rate binding configuration.

        Returns
        -------
        OperationSpec
            The ``rate_binding`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec, RateBindingSpec
        >>> binding = RateBindingSpec("revenue", "USD-OIS", "5Y")
        >>> OperationSpec.rate_binding(binding).kind
        'rate_binding'
        """
        ...

    @classmethod
    def instrument_spread_bp_by_attr(
        cls, attrs: Mapping[str, str] | Sequence[tuple[str, str]], bp: float
    ) -> OperationSpec:
        """
        Instrument spread shock (additive basis points) by exact attribute match.

        Parameters
        ----------
        attrs : Mapping[str, str] | Sequence[tuple[str, str]]
            Attribute key-value pairs that must all match; insertion order is
            preserved.
        bp : float
            Additive basis-point shift applied to matched instruments.

        Returns
        -------
        OperationSpec
            The ``instrument_spread_bp_by_attr`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.instrument_spread_bp_by_attr([("sector", "tech")], 20.0).kind
        'instrument_spread_bp_by_attr'
        """
        ...

    @classmethod
    def instrument_price_pct_by_type(cls, instrument_types: list[str], pct: float) -> OperationSpec:
        """
        Instrument price shock by ``InstrumentType`` (snake_case strings).

        Parameters
        ----------
        instrument_types : list[str]
            Instrument type identifiers in snake_case.
        pct : float
            Percent shock applied to matched instruments.

        Returns
        -------
        OperationSpec
            The ``instrument_price_pct_by_type`` operation.

        Raises
        ------
        ValueError
            If any entry in ``instrument_types`` is not a recognized instrument type.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.instrument_price_pct_by_type(["bond"], -5.0).kind
        'instrument_price_pct_by_type'
        """
        ...

    @classmethod
    def instrument_spread_bp_by_type(cls, instrument_types: list[str], bp: float) -> OperationSpec:
        """
        Instrument spread shock by ``InstrumentType`` (snake_case strings).

        Parameters
        ----------
        instrument_types : list[str]
            Instrument type identifiers in snake_case.
        bp : float
            Basis-point shift applied to matched instruments.

        Returns
        -------
        OperationSpec
            The ``instrument_spread_bp_by_type`` operation.

        Raises
        ------
        ValueError
            If any entry in ``instrument_types`` is not a recognized instrument type.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.instrument_spread_bp_by_type(["bond"], 20.0).kind
        'instrument_spread_bp_by_type'
        """
        ...

    @classmethod
    def asset_correlation_pts(cls, delta_pts: float) -> OperationSpec:
        """
        Structured-credit asset-correlation shock in decimal correlation.

        Parameters
        ----------
        delta_pts : float
            Additive decimal correlation shift (``0.05`` adds 0.05 to the
            correlation). Requires instruments in the execution context.

        Returns
        -------
        OperationSpec
            The ``asset_correlation_pts`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.asset_correlation_pts(0.05).kind
        'asset_correlation_pts'
        """
        ...

    @classmethod
    def prepay_default_correlation_pts(cls, delta_pts: float) -> OperationSpec:
        """
        Structured-credit prepay/default correlation shock in decimal
        correlation.

        Parameters
        ----------
        delta_pts : float
            Additive decimal correlation shift (``0.05`` adds 0.05). Requires
            instruments in the execution context.

        Returns
        -------
        OperationSpec
            The ``prepay_default_correlation_pts`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.prepay_default_correlation_pts(0.05).kind
        'prepay_default_correlation_pts'
        """
        ...

    @classmethod
    def hierarchy_curve_parallel_bp(
        cls,
        curve_kind: CurveKind | str,
        target: HierarchyTarget | str,
        bp: float,
        discount_curve_id: str | None = None,
    ) -> OperationSpec:
        """
        Hierarchy-targeted parallel curve shift (basis points; percent of
        forward for commodity curves).

        Parameters
        ----------
        curve_kind : CurveKind | str
            Curve family, typed or as its wire label.
        target : HierarchyTarget | str
            Typed target or its JSON string
            (``{"path": [...], "tag_filter": {...}}``).
        bp : float
            Additive basis-point shift applied to every node of every curve
            in the targeted subtree.
        discount_curve_id : str, optional
            Discount curve used when re-bootstrapping shocked ParCDS quotes.

        Returns
        -------
        OperationSpec
            The ``hierarchy_curve_parallel_bp`` operation.

        Raises
        ------
        ValueError
            If ``curve_kind`` is not an accepted label or ``target`` is not
            valid ``HierarchyTarget`` JSON.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind, HierarchyTarget, OperationSpec
        >>> OperationSpec.hierarchy_curve_parallel_bp(CurveKind.discount(), HierarchyTarget(["Credit"]), 10.0).kind
        'hierarchy_curve_parallel_bp'
        """
        ...

    @classmethod
    def hierarchy_vol_surface_parallel_pct(cls, target: HierarchyTarget | str, pct: float) -> OperationSpec:
        """
        Hierarchy-targeted vol-surface percent shift.

        Parameters
        ----------
        target : HierarchyTarget | str
            Typed target or its JSON string.
        pct : float
            Percent shift applied to matched vol quotes.

        Returns
        -------
        OperationSpec
            The ``hierarchy_vol_surface_parallel_pct`` operation.

        Raises
        ------
        ValueError
            If ``target`` is not valid JSON for a ``HierarchyTarget``.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.hierarchy_vol_surface_parallel_pct('{"path":["Equity"]}', 10.0).kind
        'hierarchy_vol_surface_parallel_pct'
        """
        ...

    @classmethod
    def hierarchy_equity_price_pct(cls, target: HierarchyTarget | str, pct: float) -> OperationSpec:
        """
        Hierarchy-targeted equity price percent shift.

        Parameters
        ----------
        target : HierarchyTarget | str
            Typed target or its JSON string.
        pct : float
            Percent shift applied to matched equity prices.

        Returns
        -------
        OperationSpec
            The ``hierarchy_equity_price_pct`` operation.

        Raises
        ------
        ValueError
            If ``target`` is not valid JSON for a ``HierarchyTarget``.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.hierarchy_equity_price_pct('{"path":["Equity"]}', -5.0).kind
        'hierarchy_equity_price_pct'
        """
        ...

    @classmethod
    def hierarchy_base_corr_parallel_pts(cls, target: HierarchyTarget | str, points: float) -> OperationSpec:
        """
        Hierarchy-targeted base-correlation parallel shift.

        Parameters
        ----------
        target : HierarchyTarget | str
            Typed target or its JSON string.
        points : float
            Additive decimal correlation shift (``0.02`` = +0.02).

        Returns
        -------
        OperationSpec
            The ``hierarchy_base_corr_parallel_pts`` operation.

        Raises
        ------
        ValueError
            If ``target`` is not valid JSON for a ``HierarchyTarget``.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.hierarchy_base_corr_parallel_pts('{"path":["Credit"]}', 0.01).kind
        'hierarchy_base_corr_parallel_pts'
        """
        ...

    @classmethod
    def time_roll_forward(
        cls,
        period: str,
        apply_shocks: bool = True,
        roll_mode: TimeRollMode | str | None = None,
    ) -> OperationSpec:
        """
        Roll the valuation horizon forward (e.g. ``"1M"``).

        ``apply_shocks`` defaults to ``True`` to mirror the Rust
        ``#[serde(default = "default_true")]`` attribute.

        Parameters
        ----------
        period : str
            Tenor string for the roll period (e.g. ``"1M"``, ``"3M"``,
            ``"1Y"``); rejected by :meth:`validate` / ``ScenarioSpec`` when
            it does not parse as a tenor.
        apply_shocks : bool, default True
            Whether to apply scenario shocks after the time roll.
        roll_mode : TimeRollMode | str, optional
            Calendar-vs-business-day roll mode (``"business_days"``,
            ``"calendar_days"``, ``"approximate"``). Defaults to business days.

        Returns
        -------
        OperationSpec
            The ``time_roll_forward`` operation.

        Raises
        ------
        ValueError
            If ``roll_mode`` is not an accepted label.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.time_roll_forward("1M").kind
        'time_roll_forward'
        """
        ...

    def validate(self) -> None:
        """
        Validate this operation with the canonical Rust rules.

        Raises
        ------
        ValueError
            If an identifier is empty, a numeric field is non-finite, a
            variant-specific floor is violated (FX ``pct <= -100``, price
            ``pct < -100``), or a tenor / time-roll period does not parse.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.time_roll_forward("3M").validate() is None
        True
        """
        ...

    def requires_instruments(self) -> bool:
        """
        Whether this operation needs instruments in the execution context.

        Returns
        -------
        bool
            ``True`` for instrument-scoped shocks and ``time_roll_forward``.

        Raises
        ------
        None
            This method does not raise.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.market_fx_pct("USD", "EUR", 1.0).requires_instruments()
        False
        """
        ...

    def mutates_instruments(self) -> bool:
        """
        Whether this operation can replace or mutate instruments.

        Returns
        -------
        bool
            ``True`` for instrument price, spread, and structured-credit
            correlation shocks; ``False`` otherwise (a time roll only reads).

        Raises
        ------
        None
            This method does not raise.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.asset_correlation_pts(0.05).mutates_instruments()
        True
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on the variant and every field.

        Parameters
        ----------
        other : object
            Value to compare; non-``OperationSpec`` values compare unequal.

        Returns
        -------
        bool
            ``True`` when both operations serialize identically.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON-serialized ``OperationSpec``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> OperationSpec:
        """
        Deserialize an ``OperationSpec`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by ``to_json``.

        Returns
        -------
        OperationSpec
            Parsed operation specification.

        Raises
        ------
        ValueError
            If the JSON is malformed or the operation kind is unknown.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.from_json('{"kind":"market_fx_pct","base":"USD","quote":"EUR","pct":-5.0}').kind
        'market_fx_pct'
        """
        ...

    @property
    def kind(self) -> str:
        """
        Variant discriminator (the serde ``kind`` tag value).

        Returns
        -------
        str
            Snake-case operation kind string.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
