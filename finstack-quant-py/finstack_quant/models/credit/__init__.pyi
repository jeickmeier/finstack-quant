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
import datetime
import numpy as np
from numpy.typing import NDArray
import pandas
import polars as pl
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import HazardCurve
from finstack_quant.core.table import ArrowTable, LabeledSeries
from typing import Any
from finstack_quant.models.credit import lgd as lgd
from finstack_quant.models.credit import liability_management as liability_management
from finstack_quant.models.credit import migration as migration
from finstack_quant.models.credit import pd as pd
from finstack_quant.models.credit import recovery_waterfall as recovery_waterfall
from finstack_quant.models.credit import scoring as scoring

from finstack_quant.models.credit._rating import (
    RatingFactorTable as RatingFactorTable,
    moodys_warf_factor as moodys_warf_factor,
)
from finstack_quant.models.credit._merton import (
    MertonBarrierType as MertonBarrierType,
    AssetDynamics as AssetDynamics,
    MertonReport as MertonReport,
    merton_report as merton_report,
    MertonModel as MertonModel,
)
from finstack_quant.models.credit._paths import (
    SimulatedPaths as SimulatedPaths,
)
from finstack_quant.models.credit._recovery_hazard import (
    DynamicRecoverySpec as DynamicRecoverySpec,
    EndogenousHazardSpec as EndogenousHazardSpec,
)
from finstack_quant.models.credit._toggle import (
    CreditState as CreditState,
    ToggleExerciseModel as ToggleExerciseModel,
)

__all__ = [
    "AssetDynamics",
    "CreditState",
    "DynamicRecoverySpec",
    "EndogenousHazardSpec",
    "MertonBarrierType",
    "MertonModel",
    "MertonReport",
    "RatingFactorTable",
    "SimulatedPaths",
    "ToggleExerciseModel",
    "lgd",
    "liability_management",
    "migration",
    "merton_report",
    "moodys_warf_factor",
    "pd",
    "recovery_waterfall",
    "scoring",
]
