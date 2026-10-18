# Shared listed-derivative support

Reusable exchange terms, option-on-future pricing mechanics, and lifecycle
logic. Concrete persisted instruments live under their asset-class modules;
`common_impl` is never an instrument registry or portfolio/scenario JSON
destination. Product-family coverage and routing metadata live under
`market/listed`.

## Asset-class consumers

| Type | Main use | Pricing and risk |
|------|----------|------------------|
| `CommodityFuture` | Commodity, digital-asset, freight, rubber, and other price-index futures | Single fixing or arithmetic-average settlement; realized observations are strict and only future observations project. Delta is the signed exposure to the projected price curve. |
| `FxFuture` | Deliverable and cash-settled currency futures | Covered-interest-parity fair price, daily-margin P&L, point delta, parallel and bucketed curve DV01. |
| `EquityFuture` | Equity-index, single-stock, dividend-carry, and fixed-currency quanto futures | Domestic cost of carry with continuous or discrete dividends; optional fixed-currency quanto drift; spot delta and curve DV01. |
| `EquityTotalReturnFuture` | Eurex-style index total-return futures | Spot plus accrued distributions less accrued funding plus quoted basis; exposes spot, distribution, funding, and spread sensitivities. |
| `InterestRateFuture` | Short-rate futures (SOFR, CORRA, Fed Funds, EURIBOR) | `100 − settlement rate − convexity` model price; multiplier is `tick_value / tick_size` ($2,500 for CME SR3). |
| `BondFuture` | Deliverable-basket government-bond futures | Carry-adjusted CTD clean price over its conversion factor; multiplier is the per-contract face / 100. |
| `VolatilityIndexFuture` | VIX, VSTOXX and similar volatility-index futures | Volatility-index curve level at settlement; the SOQ `settlement_price` is required on the settlement date itself. |
| `InterestRateFutureOption` | Listed or bilateral options on short-rate, government-bond, and other rate futures prices | One caller-configured rates instrument using shared Black-76/Bachelier, exercise, margining, settlement, and Greek mechanics. |
| `EquityFutureOption` | Options on equity and equity-index futures | Asset-owned wrapper around the shared option-on-future mechanics. |
| `VolatilityIndexFutureOption` | Options on volatility-index futures such as VSTOXX | Volatility-owned wrapper around the shared option-on-future mechanics. |
| `FxFutureOption` | Options on currency futures | Asset-owned wrapper around the shared option-on-future mechanics. |
| `CommodityFutureOption` | Options on commodity and digital-asset futures | Asset-owned wrapper around the shared option-on-future mechanics. |

`ListedFutureTerms` centralizes contracts, multiplier, settlement currency,
entry and official marks, long/short direction, last-trading and settlement
dates, and cash versus physical settlement. After trading ends, a final mark is
mandatory. Physical contracts expose a signed `ListedDeliveryObligation`; bond
conversion factors and accrued invoice interest remain in `BondFuture`.

## Lifecycle rules

- A live future uses its exchange mark when supplied, otherwise its modeled
  settlement price.
- From last trading through settlement, the official final settlement price is
  required; values are zero after settlement.
- A future option requires a recorded exercise/expiry observation from expiry
  onward. Cash settlement becomes a fixed receivable; futures delivery becomes
  a signed underlying future entered at the strike and remains exposed until
  the delivered future settles.
- Futures-style options report variation-margin P&L against an explicit trade
  or prior-settlement option reference price; they do not report the full
  undiscounted theoretical quote as position value.
- Missing historical average-price or overnight-rate fixings fail explicitly;
  they are never replaced by current forwards.
