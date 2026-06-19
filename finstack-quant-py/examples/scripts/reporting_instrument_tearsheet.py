# finstack-quant-py/examples/scripts/reporting_instrument_tearsheet.py
"""Price a bond and render an instrument tear sheet.

Run: uv run python finstack-quant-py/examples/scripts/reporting_instrument_tearsheet.py
"""

from __future__ import annotations

import datetime as dt
import json
from datetime import date

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.valuations import ValuationResult, instrument_cashflows, price_instrument_with_metrics
from finstack_quant import reporting


def main() -> None:
    bond = json.dumps({"type": "bond", "spec": {
        "id": "ACME 4.25% 2034", "notional": {"amount": "10000000", "currency": "USD"},
        "issue_date": "2024-03-15", "maturity": "2034-03-15",
        "cashflow_spec": {"Fixed": {"coupon_type": "Cash", "rate": 0.0425,
            "freq": {"count": 6, "unit": "months"}, "dc": "Thirty360", "bdc": "following",
            "calendar_id": "weekends_only", "stub": "None", "end_of_month": False, "payment_lag_days": 0}},
        "discount_curve_id": "USD-OIS", "call_put": None, "attributes": {"tags": [], "meta": {}}, "pricing_overrides": {}}})

    mc = MarketContext()
    mc.insert(DiscountCurve("USD-OIS", date(2026, 6, 19),
        [(0.0, 1.0), (0.5, 0.98), (1.0, 0.96), (2.0, 0.92), (3.0, 0.88), (5.0, 0.80), (10.0, 0.65)],
        day_count="act_365f"))
    as_of = "2026-06-19"

    # Price with the recommended metric set for a full bond sheet.
    # Some metrics (ytw, oas, asw_par) require a market-quoted clean price to be present in
    # pricing_overrides; we drop those here so the script runs without additional market data.
    _NEEDS_QUOTE = {"ytw", "oas", "asw_par"}
    metrics = [m for m in reporting.recommended_metrics("bond") if m not in _NEEDS_QUOTE]
    result = ValuationResult.from_json(price_instrument_with_metrics(
        bond, mc.to_json(), as_of, model="discounting", metrics=metrics))
    _, cashflows = instrument_cashflows(bond, mc.to_json(), as_of, model="discounting")

    ts = reporting.instrument_tearsheet(result, definition=json.loads(bond), cashflows=cashflows,
                                        title="ACME 4.25% 2034 Senior Notes", generated=dt.date.today())
    ts.save("instrument_tearsheet.html")
    print("Wrote instrument_tearsheet.html")


if __name__ == "__main__":
    main()
