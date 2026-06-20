# finstack-quant-py/examples/scripts/reporting_instrument_tearsheet.py
"""Price a bond and render an instrument tear sheet.

Run: uv run python finstack-quant-py/examples/scripts/reporting_instrument_tearsheet.py
"""

from __future__ import annotations

import datetime as dt
import json
from datetime import date

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant import reporting


def main() -> None:
    bond = json.dumps({"type": "bond", "spec": {
        "id": "ACME 4.25% 2034", "notional": {"amount": "10000000", "currency": "USD"},
        "issue_date": "2024-03-15", "maturity": "2034-03-15",
        "cashflow_spec": {"Fixed": {"coupon_type": "Cash", "rate": 0.0425,
            "freq": {"count": 6, "unit": "months"}, "dc": "Thirty360", "bdc": "following",
            "calendar_id": "weekends_only", "stub": "None", "end_of_month": False, "payment_lag_days": 0}},
        "discount_curve_id": "USD-OIS", "call_put": None, "attributes": {"tags": [], "meta": {}}, "pricing_overrides": {}}})

    # Standard-tenor curve so key-rate (bucketed) DV01 buckets are populated.
    mc = MarketContext().insert(DiscountCurve("USD-OIS", date(2026, 6, 19),
        [(0.0, 1.0), (0.25, 0.989), (0.5, 0.978), (1.0, 0.956), (2.0, 0.912),
         (3.0, 0.868), (5.0, 0.79), (7.0, 0.715), (10.0, 0.64),
         (15.0, 0.52), (20.0, 0.43), (30.0, 0.30)],
        day_count="act_365f"))
    as_of = "2026-06-19"

    # Build the market once, then render in a single call (prices internally):
    ts = reporting.instrument_tearsheet(
        json.loads(bond), market=mc, as_of=as_of, market_price=99.5,
        title="ACME 4.25% 2034 Senior Notes", generated=dt.date.today())
    ts.save("instrument_tearsheet.html")
    print("Wrote instrument_tearsheet.html")


if __name__ == "__main__":
    main()
