"""Build a publication-quality performance tear sheet from a price DataFrame.

Run: uv run python finstack-quant-py/examples/scripts/reporting_performance_tearsheet.py
Writes ``performance_tearsheet.html`` to the current directory.
"""

from __future__ import annotations

import datetime as dt

import pandas as pd

from finstack_quant import reporting
from finstack_quant.analytics import Performance


def main() -> None:
    # 1) Your data: a DataFrame of returns (or raw lists via Performance.from_panel(PerformancePanel(...))).
    idx = pd.bdate_range("2021-01-04", "2024-12-31")
    import math

    rets = [0.0005 + 0.003 * math.sin(i / 7.0) for i in range(len(idx))]
    returns = pd.DataFrame({"Global Macro Composite": rets}, index=idx)

    # 2) Compute analytics (you own the config).
    perf = Performance.from_returns(returns, initial_date=dt.date(2020, 12, 31), frequency="daily", benchmark_ticker="Global Macro Composite")

    # 3) Render the tear sheet (Direction A house style by default).
    ts = reporting.performance_tearsheet(
        perf,
        risk_free_rate=0.0,
        mar=0.0,
        omega_threshold=0.0,
        confidence=0.95,
        window=252,
        title="Global Macro Composite",
        generated=dt.date.today(),
    )
    ts.save("performance_tearsheet.html")
    print("Wrote performance_tearsheet.html")


if __name__ == "__main__":
    main()
