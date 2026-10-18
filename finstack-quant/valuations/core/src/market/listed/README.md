# Listed market catalog

`listed_product_catalog` maps the liquid product families reviewed on CME,
Eurex, Montréal Exchange, and SGX to canonical asset-class instrument types.
All product definitions and route settings live in the versioned
`data/listed/listed_product_catalog.v1.json` sidecar; Rust only parses,
validates, filters, and serializes that data.
It is maintained market/reference metadata, not a live security master or
liquidity feed. Callers must still load active expiries, current multipliers,
ticks, calendars, marks, fixings, curves, and volatility quotes from an
exchange or market-data vendor.

The one material partial route is deliverable government-bond futures:
cheapest-to-deliver, conversion-factor, and invoice economics are supported by
`BondFuture`, while delivery-day timing and wildcard optionality are not
model-valued.
