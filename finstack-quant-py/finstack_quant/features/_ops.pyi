"""Stub declarations mirroring ``finstack-quant-py/src/bindings/features/ops.rs``."""

from __future__ import annotations

import datetime
from collections.abc import Sequence
from types import ModuleType
from typing import Any
import numpy as np
import numpy.typing as npt
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable

class TimeSeriesOp:
    """
    Time-series (per-entity, backward-looking) operation selector.

    Accepts the snake_case name (``TimeSeriesOp("returns")``) or an
    ``UPPER_SNAKE`` member (``TimeSeriesOp["RETURNS"]``). ``values()``
    lists every accepted name; ``param_keys`` lists the JSON ``params`` keys the
    operation reads (any other key is rejected).

    Examples
    --------
    >>> from finstack_quant.features import TimeSeriesOp
    >>> TimeSeriesOp("returns").param_keys
    ['periods']
    """

    __members__: dict[str, TimeSeriesOp]

    def __init__(self, name: str) -> None:
        """
        Parse an operation from its snake_case name.

        Parameters
        ----------
        name : str
            Canonical operation name; see :meth:`values`.

        Raises
        ------
        ValueError
            If ``name`` is not accepted; the message lists every accepted name.

        Examples
        --------
        >>> from finstack_quant.features import TimeSeriesOp
        >>> TimeSeriesOp("returns").name
        'returns'
        """
        ...

    @property
    def name(self) -> str:
        """
        Canonical snake_case operation name.

        Returns
        -------
        str
            Wire name such as ``"returns"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def param_keys(self) -> list[str]:
        """
        JSON ``params`` keys this operation reads.

        Returns
        -------
        list[str]
            Accepted parameter keys; any other key raises ``ValueError``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @staticmethod
    def values() -> list[str]:
        """
        Every accepted operation name, in declaration order.

        Returns
        -------
        list[str]
            Canonical names.

        Examples
        --------
        >>> from finstack_quant.features import TimeSeriesOp
        >>> "returns" in TimeSeriesOp.values()
        True

        Notes
        -----
        This method does not raise.
        """
        ...

    def __class_getitem__(cls, key: str) -> TimeSeriesOp:
        """
        Look up an operation by ``UPPER_SNAKE`` member name.

        Parameters
        ----------
        key : str
            Member name such as ``"RETURNS"``.

        Returns
        -------
        TimeSeriesOp
            The matching operation.

        Raises
        ------
        KeyError
            If ``key`` is not a member.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

class CrossSectionalOp:
    """
    Cross-sectional (per-timestamp) operation selector.

    Accepts the snake_case name (``CrossSectionalOp("winsorize")``) or an
    ``UPPER_SNAKE`` member (``CrossSectionalOp["WINSORIZE"]``). ``values()``
    lists every accepted name; ``param_keys`` lists the JSON ``params`` keys the
    operation reads (any other key is rejected).

    Examples
    --------
    >>> from finstack_quant.features import CrossSectionalOp
    >>> CrossSectionalOp("winsorize").param_keys
    ['lower', 'upper']
    """

    __members__: dict[str, CrossSectionalOp]

    def __init__(self, name: str) -> None:
        """
        Parse an operation from its snake_case name.

        Parameters
        ----------
        name : str
            Canonical operation name; see :meth:`values`.

        Raises
        ------
        ValueError
            If ``name`` is not accepted; the message lists every accepted name.

        Examples
        --------
        >>> from finstack_quant.features import CrossSectionalOp
        >>> CrossSectionalOp("winsorize").name
        'winsorize'
        """
        ...

    @property
    def name(self) -> str:
        """
        Canonical snake_case operation name.

        Returns
        -------
        str
            Wire name such as ``"winsorize"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def param_keys(self) -> list[str]:
        """
        JSON ``params`` keys this operation reads.

        Returns
        -------
        list[str]
            Accepted parameter keys; any other key raises ``ValueError``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @staticmethod
    def values() -> list[str]:
        """
        Every accepted operation name, in declaration order.

        Returns
        -------
        list[str]
            Canonical names.

        Examples
        --------
        >>> from finstack_quant.features import CrossSectionalOp
        >>> "winsorize" in CrossSectionalOp.values()
        True

        Notes
        -----
        This method does not raise.
        """
        ...

    def __class_getitem__(cls, key: str) -> CrossSectionalOp:
        """
        Look up an operation by ``UPPER_SNAKE`` member name.

        Parameters
        ----------
        key : str
            Member name such as ``"WINSORIZE"``.

        Returns
        -------
        CrossSectionalOp
            The matching operation.

        Raises
        ------
        KeyError
            If ``key`` is not a member.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

class PairwiseOp:
    """
    Pairwise rolling operation selector (cov / corr / beta).

    Accepts the snake_case name (``PairwiseOp("rolling_beta")``) or an
    ``UPPER_SNAKE`` member (``PairwiseOp["ROLLING_BETA"]``). ``values()``
    lists every accepted name; ``param_keys`` lists the JSON ``params`` keys the
    operation reads (any other key is rejected).

    Examples
    --------
    >>> from finstack_quant.features import PairwiseOp
    >>> PairwiseOp("rolling_beta").param_keys
    ['window', 'min_periods']
    """

    __members__: dict[str, PairwiseOp]

    def __init__(self, name: str) -> None:
        """
        Parse an operation from its snake_case name.

        Parameters
        ----------
        name : str
            Canonical operation name; see :meth:`values`.

        Raises
        ------
        ValueError
            If ``name`` is not accepted; the message lists every accepted name.

        Examples
        --------
        >>> from finstack_quant.features import PairwiseOp
        >>> PairwiseOp("rolling_beta").name
        'rolling_beta'
        """
        ...

    @property
    def name(self) -> str:
        """
        Canonical snake_case operation name.

        Returns
        -------
        str
            Wire name such as ``"rolling_beta"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def param_keys(self) -> list[str]:
        """
        JSON ``params`` keys this operation reads.

        Returns
        -------
        list[str]
            Accepted parameter keys; any other key raises ``ValueError``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @staticmethod
    def values() -> list[str]:
        """
        Every accepted operation name, in declaration order.

        Returns
        -------
        list[str]
            Canonical names.

        Examples
        --------
        >>> from finstack_quant.features import PairwiseOp
        >>> "rolling_beta" in PairwiseOp.values()
        True

        Notes
        -----
        This method does not raise.
        """
        ...

    def __class_getitem__(cls, key: str) -> PairwiseOp:
        """
        Look up an operation by ``UPPER_SNAKE`` member name.

        Parameters
        ----------
        key : str
            Member name such as ``"ROLLING_BETA"``.

        Returns
        -------
        PairwiseOp
            The matching operation.

        Raises
        ------
        KeyError
            If ``key`` is not a member.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
