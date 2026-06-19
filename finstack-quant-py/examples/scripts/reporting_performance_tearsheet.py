# finstack-quant-py/examples/scripts/reporting_performance_tearsheet.py
"""Build a publication-quality performance tear sheet from a price DataFrame.

Run: uv run python finstack-quant-py/examples/scripts/reporting_performance_tearsheet.py
Writes ``performance_tearsheet.html`` to the current directory.
"""

from __future__ import annotations

import datetime as dt

import pandas as pd

from finstack_quant.analytics import Performance
from finstack_quant import reporting


def main() -> None:
    # 1) Your data: a DataFrame of returns (or build from prices via from_arrays).
    idx = pd.bdate_range("2021-01-04", "2024-12-31")
    import math
    rets = [0.0005 + 0.003 * math.sin(i / 7.0) for i in range(len(idx))]
    returns = pd.DataFrame({"Global Macro Composite": rets}, index=idx)

    # 2) Compute analytics (you own the config).
    perf = Performance.from_returns(returns, freq="daily")

    # 3) Render the tear sheet (Direction A house style by default).
    ts = reporting.performance_tearsheet(
        perf,
        title="Global Macro Composite",
        generated=dt.date.today(),
    )
    ts.save("performance_tearsheet.html")
    print("Wrote performance_tearsheet.html")


if __name__ == "__main__":
    main()
