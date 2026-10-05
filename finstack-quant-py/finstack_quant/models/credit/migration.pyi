"""
Credit migration: rating scales, transition matrices, generators, and CTMC simulation.

Examples
--------
>>> from finstack_quant.models.credit import migration
>>> migration.RatingScale.custom_with_default(["A", "D"], "D").labels()
['A', 'D']

"""

from __future__ import annotations

from typing import Any

import pandas

__all__ = [
    "GeneratorMatrix",
    "MigrationSimulator",
    "RatingPath",
    "RatingPaths",
    "RatingScale",
    "TransitionMatrix",
    "project",
]

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
    def standard() -> RatingScale:
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
    def standard_with_nr() -> RatingScale:
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
    def notched() -> RatingScale:
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
    def custom(labels: list[str]) -> RatingScale:
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
    def custom_with_default(labels: list[str], default_label: str) -> RatingScale:
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
    def from_json(json: str) -> RatingScale:
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

    def __init__(self, scale: RatingScale, data: list[float] | list[list[float]] | Any, horizon: float) -> None:
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
        scale: RatingScale | None = None,
    ) -> TransitionMatrix:
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
        ValueError
            If an index is outside the rating scale.
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

    def compose(self, other: TransitionMatrix) -> TransitionMatrix:
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
    def scale(self) -> RatingScale:
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
    def from_json(json: str) -> TransitionMatrix:
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

    def __init__(self, scale: RatingScale, data: list[float] | list[list[float]] | Any) -> None:
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
    def from_transition_matrix(p: TransitionMatrix) -> GeneratorMatrix:
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
        p: TransitionMatrix,
        round_trip_tol: float,
    ) -> GeneratorMatrix:
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
    def scale(self) -> RatingScale:
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
    def from_json(json: str) -> GeneratorMatrix:
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
    def scale(self) -> RatingScale:
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
    def from_json(json: str) -> RatingPath:
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
    def paths(self) -> list[RatingPath]:
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
    def from_json(json: str) -> RatingPaths:
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

    def __getitem__(self, index: int) -> RatingPath:
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

    def __init__(self, generator: GeneratorMatrix, horizon: float) -> None:
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

    def simulate(self, initial_state: int, n_paths: int, seed: int) -> RatingPaths:
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

    def empirical_matrix(self, n_paths_per_state: int, seed: int) -> TransitionMatrix:
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
    def generator(self) -> GeneratorMatrix:
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
    def from_json(json: str) -> MigrationSimulator:
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

def project(generator: GeneratorMatrix, t: float) -> TransitionMatrix:
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
