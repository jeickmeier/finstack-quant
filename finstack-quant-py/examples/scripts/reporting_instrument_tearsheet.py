"""Price a bond and render an instrument tear sheet.

Run: uv run python finstack-quant-py/examples/scripts/reporting_instrument_tearsheet.py
"""

from __future__ import annotations

import datetime as dt
from datetime import date
import json

from finstack_quant import reporting
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.valuations.instruments import instrument_cashflows
from finstack_quant.valuations.instruments import price_instrument


def main() -> None:
    bond = json.dumps({
        "schema": "finstack_quant.instrument/1",
        "instrument": {
            "type": "bond",
            "spec": {
                "id": "ACME 4.25% 2034",
                "notional": {"amount": "10000000", "currency": "USD"},
                "issue_date": "2024-03-15",
                "maturity": "2034-03-15",
                "cashflow_spec": {
                    "fixed": {
                        "coupon_type": "cash",
                        "rate": "0.0425",
                        "frequency": {"count": 6, "unit": "months"},
                        "day_count": "30_360",
                        "business_day_convention": "following",
                        "calendar_id": "weekends_only",
                        "stub": "none",
                        "end_of_month": False,
                        "payment_lag_days": 0,
                    }
                },
                "discount_curve_id": "USD-OIS",
                "call_put": None,
                "attributes": {"tags": [], "meta": {}},
            },
        },
    })

    # Standard-tenor curve so key-rate (bucketed) DV01 buckets are populated.
    mc = MarketContext().insert(
        DiscountCurve(
            "USD-OIS",
            date(2026, 6, 19),
            [
                (0.0, 1.0),
                (0.25, 0.989),
                (0.5, 0.978),
                (1.0, 0.956),
                (2.0, 0.912),
                (3.0, 0.868),
                (5.0, 0.79),
                (7.0, 0.715),
                (10.0, 0.64),
                (15.0, 0.52),
                (20.0, 0.43),
                (30.0, 0.30),
            ],
            day_count="act_365f",
        )
    )
    as_of = "2026-06-19"

    metrics = ["dirty_price", "clean_price", "accrued", "ytm", "duration_mod", "dv01", "bucketed_dv01"]
    result = price_instrument(
        bond, mc.to_json(), as_of, model="discounting", metrics=metrics
    )
    cashflows = instrument_cashflows(bond, mc.to_json(), as_of, "discounting").to_dataframe()

    ts = reporting.instrument_tearsheet(
        result,
        cashflows=cashflows,
        definition=json.loads(bond)["instrument"],
        title="ACME 4.25% 2034 Senior Notes",
        generated=dt.date.today(),
    )
    ts.save("instrument_tearsheet.html")
    print("Wrote instrument_tearsheet.html")


if __name__ == "__main__":
    main()
