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

from finstack_quant.core.types import CreditRating
from finstack_quant.models.credit._structural import (
    AssetDynamics as AssetDynamics,
    CreditState as CreditState,
    DynamicRecoverySpec as DynamicRecoverySpec,
    EndogenousHazardSpec as EndogenousHazardSpec,
    MertonBarrierType as MertonBarrierType,
    MertonModel as MertonModel,
    RatingFactorTable as RatingFactorTable,
    SimulatedPaths as SimulatedPaths,
    ToggleExerciseModel as ToggleExerciseModel,
)
from finstack_quant.models.credit import lgd as lgd
from finstack_quant.models.credit import liability_management as liability_management
from finstack_quant.models.credit import migration as migration
from finstack_quant.models.credit import pd as pd
from finstack_quant.models.credit import recovery_waterfall as recovery_waterfall
from finstack_quant.models.credit import scoring as scoring

__all__ = [
    "AssetDynamics",
    "CreditState",
    "DynamicRecoverySpec",
    "EndogenousHazardSpec",
    "MertonBarrierType",
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
