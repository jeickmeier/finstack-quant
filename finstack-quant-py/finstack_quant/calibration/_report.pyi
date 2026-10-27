"""Stub declarations mirroring ``finstack-quant-py/src/bindings/calibration/report.rs``."""

from typing import Any, TypedDict
import pandas as pd

class QuoteQuality:
    """Fit quality for one calibration quote.

    Examples:
    --------
    >>> from finstack_quant.calibration import QuoteQuality
    >>> q = QuoteQuality.from_json('{"quote_label":"s5y","quote_value":0.042,"residual":0.0,"sensitivity":1.0}')
    >>> q.quote_label
    's5y'

    """

    @property
    def quote_label(self) -> str:
        """
        Identifier of the quote this row describes.

        This property does not raise.

        Returns
        -------
        str
            Identifier of the quote this row describes.
        """

    @property
    def quote_value(self) -> float | None:
        """
        Market quote the calibration instrument was built from.

        Native quote units: decimal rate for deposits, FRAs, swaps and
        inflation swaps; price for rate futures; basis points for CDS par
        spreads and cross-currency basis; decimal fractions for CDS and tranche upfronts.

        This property does not raise.

        Returns
        -------
        float or None
            The quote value, or ``None`` when diagnostics were not computed or
            the target's quotes carry no single scalar value.
        """

    @property
    def residual(self) -> float:
        """
        Signed calibration residual; the calibration target is zero.

        Uses the report's ``residual_units``: typically instrument PV per unit
        notional or a decimal volatility difference.

        This property does not raise.

        Returns
        -------
        float
            Signed error in the report's declared residual units.
        """

    @property
    def sensitivity(self) -> float:
        """
        Absolute residual derivative for a bootstrap knot or maximum across global-fit parameters.

        This property does not raise.

        Returns
        -------
        float
            Residual units per parameter unit; ``NaN`` when not recorded.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this row.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> QuoteQuality:
        """
        Rebuild a row from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        QuoteQuality
            The decoded row.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import QuoteQuality
        >>> QuoteQuality.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...
    def __repr__(self) -> str: ...

class CalibrationDiagnostics:
    """Per-quote fit quality and conditioning for one calibration step.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationDiagnostics
    >>> d = CalibrationDiagnostics.from_json(
    ...     '{"per_quote":[],"condition_number":null,"jacobian":null,"max_residual":0.0,"rms_residual":0.0}'
    ... )
    >>> d.max_residual
    0.0

    """

    @property
    def per_quote(self) -> list[QuoteQuality]:
        """
        Fit quality rows, one per calibration quote.

        This property does not raise.

        Returns
        -------
        list[QuoteQuality]
            Fit quality rows, one per calibration quote.
        """

    @property
    def condition_number(self) -> float | None:
        """
        Condition number of the Jacobian, or None when not computed.

        This property does not raise.

        Returns
        -------
        float | None
            Condition number of the Jacobian, or None when not computed.
        """

    @property
    def jacobian(self) -> list[list[float]] | None:
        """
        Finite-difference Jacobian of the residuals at the solution.

        ``jacobian[i][j]`` is the derivative of the residual of quote ``i``
        (in ``per_quote`` order) with respect to solved parameter ``j`` (the
        knot values in increasing knot-time order), in the report's residual
        units per unit of the parameter. The entries are unweighted;
        ``condition_number`` is computed from this matrix with the residual
        weights applied, and ``per_quote[i].sensitivity`` equals
        ``max(abs(v) for v in jacobian[i])``.

        This property does not raise.

        Returns
        -------
        list[list[float]] | None
            One row per quote and one column per solved parameter. ``None``
            for sequential bootstraps, which build no full Jacobian, and when
            a parameter bump could not be priced.
        """

    @property
    def max_residual(self) -> float:
        """
        Largest absolute residual across quotes, in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Largest absolute residual across quotes, in the target's own units.
        """

    @property
    def rms_residual(self) -> float:
        """
        Root-mean-square residual across quotes, in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Root-mean-square residual across quotes, in the target's own units.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """Return the per-quote rows as a DataFrame.

        Returns
        -------
        pandas.DataFrame
            Columns ``quote_id``, ``quote_value``, ``residual`` and
            ``sensitivity``, one row per quote.

        Raises
        ------
        RuntimeError
            If pandas is not installed.

        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of these diagnostics.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationDiagnostics:
        """
        Rebuild diagnostics from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CalibrationDiagnostics
            The decoded diagnostics.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationDiagnostics
        >>> CalibrationDiagnostics.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CalibrationReport:
    """Convergence, residuals and diagnostics for one calibration step or plan.

    Raw residual statistics (``max_residual``, ``rmse``) are in the target's own
    units. The tolerance-scaled aggregates (``max_residual_ratio``,
    ``rmse_ratio``) divide each residual by its step tolerance, so they are
    comparable across steps and are below 1.0 for an accepted fit.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, calibrate
    >>> calibrate(CalibrationEnvelope(CalibrationPlan([], id="smoke"))).report.success
    True

    """

    @property
    def success(self) -> bool:
        """
        Whether every quote repriced inside its tolerance.

        This property does not raise.

        Returns
        -------
        bool
            Whether every quote repriced inside its tolerance.
        """

    @property
    def residuals(self) -> dict[str, float]:
        """
        Raw residual per quote id, in the target's own units.

        This property does not raise.

        Returns
        -------
        dict[str, float]
            Raw residual per quote id, in the target's own units.
        """

    @property
    def iterations(self) -> int:
        """
        Number of solver iterations consumed.

        This property does not raise.

        Returns
        -------
        int
            Number of solver iterations consumed.
        """

    @property
    def objective_value(self) -> float:
        """
        Final objective value reached by the solver.

        This property does not raise.

        Returns
        -------
        float
            Final objective value reached by the solver.
        """

    @property
    def max_residual(self) -> float:
        """
        Largest absolute raw residual, in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Largest absolute raw residual, in the target's own units.
        """

    @property
    def rmse(self) -> float:
        """
        Root-mean-square raw residual, in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Root-mean-square raw residual, in the target's own units.
        """

    @property
    def max_residual_ratio(self) -> float | None:
        """
        Largest ``|residual| / tolerance``, or None when no tolerance applies.

        This property does not raise.

        Returns
        -------
        float | None
            Largest ``|residual| / tolerance``, or None when no tolerance applies.
        """

    @property
    def rmse_ratio(self) -> float | None:
        """
        Root-mean-square ``|residual| / tolerance``, or None when unavailable.

        This property does not raise.

        Returns
        -------
        float | None
            Root-mean-square ``|residual| / tolerance``, or None when unavailable.
        """

    @property
    def validation_passed(self) -> bool:
        """
        Whether post-solve validation accepted the calibrated object.

        This property does not raise.

        Returns
        -------
        bool
            Whether post-solve validation accepted the calibrated object.
        """

    @property
    def validation_error(self) -> str | None:
        """
        Post-solve validation message, or None when validation passed.

        This property does not raise.

        Returns
        -------
        str | None
            Post-solve validation message, or None when validation passed.
        """

    @property
    def convergence_reason(self) -> str:
        """
        Why the solver stopped, for example ``"tolerance_reached"``.

        This property does not raise.

        Returns
        -------
        str
            Why the solver stopped, for example ``"tolerance_reached"``.
        """

    @property
    def metadata(self) -> dict[str, Any]:
        """
        Free-form solver metadata recorded by the step.

        This property does not raise.

        Returns
        -------
        dict[str, Any]
            Free-form solver metadata recorded by the step.
        """

    @property
    def solver_tolerance(self) -> float:
        """
        Tolerance applied to this step, in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Tolerance applied to this step, in the target's own units.
        """

    @property
    def solver_max_iterations(self) -> int:
        """
        Iteration budget applied to this step.

        This property does not raise.

        Returns
        -------
        int
            Iteration budget applied to this step.
        """

    @property
    def model_version(self) -> str | None:
        """
        Version tag of the calibrated model, or None.

        This property does not raise.

        Returns
        -------
        str | None
            Version tag of the calibrated model, or None.
        """

    @property
    def worst_quote_id(self) -> str | None:
        """
        Identifier of the worst-fitting quote, or None when there are no quotes.

        This property does not raise.

        Returns
        -------
        str | None
            Identifier of the worst-fitting quote, or None when there are no quotes.
        """

    @property
    def worst_quote_residual(self) -> float | None:
        """
        Raw residual of the worst-fitting quote, or None.

        This property does not raise.

        Returns
        -------
        float | None
            Raw residual of the worst-fitting quote, or None.
        """

    @property
    def success_tolerance(self) -> float | None:
        """
        Tolerance used for the success decision, or None.

        This property does not raise.

        Returns
        -------
        float | None
            Tolerance used for the success decision, or None.
        """

    @property
    def solver_method(self) -> str | None:
        """
        Numerical procedure that produced this report, as its wire name.

        One of ``"sequential_bootstrap"`` (knots solved one at a time by a
        bracketed root find), ``"global_fit_lm_weighted_lsq"`` (all parameters
        solved together by weighted Levenberg-Marquardt),
        ``"per_slice_least_squares"`` (one SABR or SVI smile fit per expiry or
        expiry/tenor bucket), ``"scalar_root_find"``, ``"scalar_minimization"``
        or ``"plan_execution"`` (the plan-level aggregate; no solver ran).

        This property does not raise.

        Returns
        -------
        str | None
            Wire name of the solver method, or ``None`` for a manually constructed
            report with no solver method supplied.
        """

    @property
    def residual_units(self) -> str | None:
        """
        Units of ``residuals``, ``max_residual`` and ``rmse``, as a wire name.

        ``"pv_per_unit_notional"`` is the calibration instrument's present
        value divided by the residual notional (``1e-4`` is one basis point of
        notional); ``"quoted_volatility"`` is model minus quoted volatility in
        the quote's own convention, as a decimal;
        ``"discounted_upfront_fraction"`` is the discounted tranche value as a
        fraction of tranche notional; and
        ``"absolute_residual_over_step_tolerance"`` is the dimensionless
        ``|residual| / step_tolerance`` used by the plan-level report.
        This typed field is authoritative; metadata supplies no fallback.

        This property does not raise.

        Returns
        -------
        str | None
            Wire name of the residual units, or ``None`` when a manually constructed
            report supplies no units. Missing units are unknown.
        """

    @property
    def fitted_slices(self) -> list[dict[str, Any]]:
        """
        Smile parameters fitted per quoted expiry, in increasing expiry order.

        The equity SABR (``vol_surface``) and SVI (``svi_surface``) steps fit
        one smile per quoted expiry and then sample the smiles onto the target
        grid; the gridded surface keeps only the sampled volatilities, so the
        fitted parameters are reported here. Each dict has:

        - ``expiry``: quoted expiry as an Act/365F year fraction from the
          calibration base date (not necessarily a target-grid expiry).
        - ``parameters``: ``{"sabr": {"alpha", "beta", "nu", "rho", "shift"}}``
          or ``{"svi": {"a", "b", "rho", "m", "sigma"}}`` (raw SVI total
          variance in log-moneyness ``ln(K / F)``).
        - ``iterations``: solver iterations spent on the slice; the slices sum
          to ``iterations`` of the report.
        - ``max_relative_quote_error``: largest ``|model - market| / market``
          volatility error of the fitted smile over the slice's quotes, as a
          decimal. Present for SABR slices only.

        Returns
        -------
        list[dict[str, Any]]
            One dict per quoted expiry. Empty for every other step kind,
            including the swaption SABR cube, whose node parameters are stored
            on the cube itself.

        Raises
        ------
        ValueError
            If the slices cannot be converted to Python objects.
        """

    @property
    def diagnostics(self) -> CalibrationDiagnostics | None:
        """
        Per-quote diagnostics, or None when diagnostics were not computed.

        This property does not raise.

        Returns
        -------
        CalibrationDiagnostics | None
            Per-quote diagnostics, or None when diagnostics were not computed.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """Return the per-quote residuals as a DataFrame.

        Returns
        -------
        pandas.DataFrame
            Columns ``quote_id``, ``quote_value``, ``residual`` and
            ``sensitivity``. Quote value and sensitivity are NaN when
            diagnostics were not computed.

        Raises
        ------
        RuntimeError
            If pandas is not installed.

        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this report.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationReport:
        """
        Rebuild a report from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CalibrationReport
            The decoded report.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationReport
        >>> CalibrationReport.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class _DependencyNode(TypedDict):
    """Declared step and the market-object identifiers it consumes and produces."""

    step_index: int
    step_id: str
    kind: str
    reads: list[str]
    writes: list[str]

class _DependencyGraph(TypedDict):
    """Initially available market identifiers and dependency nodes in plan order."""

    initial_ids: list[str]
    nodes: list[_DependencyNode]

class CalibrationValidationReport:
    """Result of static, pre-solve validation of a calibration envelope.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, dry_run
    >>> dry_run(CalibrationEnvelope(CalibrationPlan([], id="smoke"))).is_valid
    True

    """

    @property
    def is_valid(self) -> bool:
        """
        Whether the envelope passed every static check.

        This property does not raise.

        Returns
        -------
        bool
            Whether the envelope passed every static check.
        """

    @property
    def errors(self) -> list[dict[str, Any]]:
        """
        Static findings, each with its code, message and offending identifier.

        This property does not raise.

        Returns
        -------
        list[dict[str, Any]]
            Static findings, each with its code, message and offending identifier.
        """

    @property
    def dependency_graph(self) -> _DependencyGraph:
        """Return initially available market identifiers and per-step dependencies.

        This property does not raise.

        Returns
        -------
        _DependencyGraph
            ``initial_ids`` lists the sorted market-object identifiers supplied
            before execution. ``nodes`` follows declared plan order; each node
            contains its zero-based ``step_index``, ``step_id``, step ``kind``,
            consumed market identifiers in ``reads``, and produced market
            identifiers in ``writes``. An empty plan has no nodes.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """Return the static findings as a DataFrame.

        Returns
        -------
        pandas.DataFrame
            One row per finding, with the finding's fields as columns. Empty
            when the envelope is valid.

        Raises
        ------
        RuntimeError
            If pandas is not installed.

        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this report.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationValidationReport:
        """
        Rebuild a report from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CalibrationValidationReport
            The decoded report.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationValidationReport
        >>> CalibrationValidationReport.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
