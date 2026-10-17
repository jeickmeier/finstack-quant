"""
Type stubs for ``finstack_quant.models.factor``.

``FactorCovarianceMatrix`` and ``FactorModelConfig`` live here because Rust
exports them from the ``factor`` crate root; the credit hierarchy is the
``credit`` subpackage.

Examples
--------
>>> from finstack_quant.models.factor import credit
>>> try:
...     credit.CreditFactorModel.from_json("{}")
... except ValueError as exc:
...     "missing field" in str(exc)
True
"""

from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt
import pandas as pd

from finstack_quant.models.factor import credit as credit
from finstack_quant.models.factor import risk as risk
from finstack_quant.models.factor import schema as schema

class FactorCovarianceMatrix:
    """
    Validated factor covariance matrix with deterministic row-major storage.

    Entries are annualized covariances in the factors' native units. The
    constructor validates squareness, unique identifiers, symmetry and positive
    semidefiniteness. Instances compare equal when identifiers and data match.

    Examples
    --------
    >>> from finstack_quant.models.factor import FactorCovarianceMatrix
    >>> matrix = FactorCovarianceMatrix(["a", "b"], [[0.04, 0.0], [0.0, 0.01]])
    >>> (matrix.variance("a"), float(matrix.to_dataframe().loc["b", "b"]))
    (0.04, 0.01)
    """

    def __init__(
        self,
        factor_ids: list[str],
        data: list[float] | list[list[float]] | npt.NDArray[np.float64],
    ) -> None:
        """
        Build and validate a covariance matrix.

        Parameters
        ----------
        factor_ids : list[str]
            Ordered, unique factor identifiers defining both axes.
        data : list[float] | list[list[float]] | numpy.ndarray
            Annualized covariances — a nested list or 2-D array of shape
            ``(n, n)``, or a flat row-major list of ``n * n`` values, in
            ``factor_ids`` order.

        Raises
        ------
        ValueError
            If ``data`` is not ``n x n``, an identifier repeats, the matrix is
            asymmetric, or it is not positive semidefinite.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FactorCovarianceMatrix:
        """Deserialize and validate a covariance matrix from canonical JSON.

        Parameters
        ----------
        json : str
            Object with ordered ``factor_ids``, dimension ``n``, and row-major ``data``.

        Returns
        -------
        FactorCovarianceMatrix
            Validated symmetric positive-semidefinite covariance matrix.

        Raises
        ------
        ValueError
            If JSON is malformed, dimensions disagree, IDs repeat, or the matrix is invalid.

        Examples
        --------
        >>> FactorCovarianceMatrix.from_json('{"factor_ids":[],"n":0,"data":[]}').n_factors
        0
        """
        ...

    def to_json(self) -> str:
        """Serialize this matrix to canonical JSON.

        Returns
        -------
        str
            Compact JSON with ordered axes and row-major covariance data.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def n_factors(self) -> int:
        """Return the number of covariance axes.

        Returns
        -------
        int
            Number of factor IDs, equal to the matrix row and column count.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    @property
    def factor_ids(self) -> list[str]:
        """Return ordered factor identifiers for rows and columns.

        Returns
        -------
        list[str]
            Independent copy of the canonical covariance-axis order.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    @property
    def data(self) -> list[float]:
        """Return row-major annualized covariance values.

        Returns
        -------
        list[float]
            ``n_factors * n_factors`` entries in canonical factor order.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    def variance(self, factor_id: str) -> float:
        """Return one factor's annualized variance.

        Parameters
        ----------
        factor_id : str
            Canonical factor identifier to query.

        Returns
        -------
        float
            Diagonal covariance entry, or ``0.0`` when the factor is unknown.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def covariance(self, lhs: str, rhs: str) -> float:
        """Return annualized covariance between two factors.

        Parameters
        ----------
        lhs : str
            Row factor identifier.
        rhs : str
            Column factor identifier.

        Returns
        -------
        float
            Covariance entry, or ``0.0`` when either factor is unknown.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def correlation(self, lhs: str, rhs: str) -> float:
        """Return correlation between two factors.

        Parameters
        ----------
        lhs : str
            First factor identifier.
        rhs : str
            Second factor identifier.

        Returns
        -------
        float
            Covariance normalized by both standard deviations; ``0.0`` for unknown or zero-variance factors.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def to_numpy(self) -> npt.NDArray[np.float64]:
        """
        The matrix as an ``(n, n)`` float64 NumPy array.

        Returns
        -------
        numpy.ndarray
            Row-major covariance in ``factor_ids`` order.

        Notes
        -----
        This method does not raise; it copies the stored data.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        The matrix as a square pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Indexed and columned by ``factor_ids``.

        Raises
        ------
        ValueError
            If pandas cannot build the frame.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...

class FactorModelConfig:
    """
    Portfolio factor-model configuration assembled at a forecast horizon.

    Examples
    --------
    >>> from finstack_quant.models.factor import FactorModelConfig
    >>> config = FactorModelConfig.from_json(
    ...     '{"factors":[],"covariance":{"factor_ids":[],"n":0,"data":[]},"matching":{"mapping_table":[]},"risk_measure":"variance"}'
    ... )
    >>> config.n_factors
    0
    """

    @staticmethod
    def from_json(json: str) -> FactorModelConfig:
        """Deserialize and validate a factor-model configuration.

        Parameters
        ----------
        json : str
            Canonical Rust ``FactorModelConfig`` JSON.

        Returns
        -------
        FactorModelConfig
            Typed configuration with matching and covariance invariants checked.

        Raises
        ------
        ValueError
            If JSON is malformed or matching rules reference undeclared factors.

        Examples
        --------
        >>> FactorModelConfig.from_json(
        ...     '{"factors":[],"covariance":{"factor_ids":[],"n":0,"data":[]},"matching":{"mapping_table":[]},"risk_measure":"variance"}'
        ... ).factor_ids
        []
        """
        ...

    def to_json(self) -> str:
        """Serialize this configuration to canonical JSON.

        Returns
        -------
        str
            Compact JSON accepted by factor-risk workflows.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def validate(self) -> None:
        """Validate that matching rules emit only declared factor IDs.

        Raises
        ------
        ValueError
            If a matcher references an undeclared factor or duplicates issuer rows.
        """
        ...

    @property
    def n_factors(self) -> int:
        """Return the number of configured factors.

        Returns
        -------
        int
            Length of the factor definition list.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    @property
    def factor_ids(self) -> list[str]:
        """Return factor-definition IDs in canonical order.

        Returns
        -------
        list[str]
            Ordered IDs aligned to covariance axes.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    @property
    def factors(self) -> list[dict[str, Any]]:
        """Return structured factor definitions.

        Returns
        -------
        list[dict[str, Any]]
            Independent Python representation of canonical factor-definition fields.

        Raises
        ------
        ValueError
            If conversion to Python values fails.
        """
        ...

    @property
    def covariance(self) -> FactorCovarianceMatrix:
        """Return the covariance matrix aligned to ``factor_ids``.

        Returns
        -------
        FactorCovarianceMatrix
            Independent typed covariance wrapper.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    @property
    def matching(self) -> dict[str, Any]:
        """Return the declarative factor-matching configuration.

        Returns
        -------
        dict[str, Any]
            Structured Python representation of the canonical matching variant.

        Raises
        ------
        ValueError
            If conversion to Python values fails.
        """
        ...

    @property
    def risk_measure(self) -> str | dict[str, Any]:
        """Return the canonical risk-measure value.

        Returns
        -------
        str or dict[str, Any]
            Scalar label for variance/volatility or structured VaR/ES parameters.

        Raises
        ------
        ValueError
            If conversion to Python values fails.
        """
        ...

    @property
    def bump_config(self) -> dict[str, Any] | None:
        """Return optional finite-difference bump overrides.

        Returns
        -------
        dict[str, Any] or None
            Structured bump configuration, or ``None`` when defaults apply.

        Raises
        ------
        ValueError
            If conversion to Python values fails.
        """
        ...

    @property
    def unmatched_policy(self) -> str | None:
        """Return the policy for unmatched dependencies.

        Returns
        -------
        str or None
            ``"strict"``, ``"warn"``, or ``None`` for the strict default.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

__all__ = [
    "FactorCovarianceMatrix",
    "FactorModelConfig",
    "credit",
    "risk",
    "schema",
]
