# Asset range-accrual digital-spread replication, GBM and quanto carry

- Record: `VALUATIONS-FX-AND-EXOTICS-ASSET-RANGE-ACCRUAL`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

Asset range accruals pay a coupon proportional to observation count inside a band, with no principal repayment in this instrument. Static digital replication is the default; GBM paths are an explicit alternative. Rate-linked inputs are rejected by both asset pricers.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Validate sorted observations, positive notional/accrual interval, coupon≥0, lower<upper, paired historical counts and payment no earlier than last observation. Observations on/before valuation require exact count totals; future dates are strictly later. Payment date defaults to last observation. Value is zero on/after payment. Fully observed but unpaid value is `DF×N×coupon×year_fraction(start,last_obs)×in_range/total`.

For each future observation, compute forward `S exp(-q t)/DF_asset`; without quanto asset/payoff curve coincide. With quanto use `S exp[-(q+ρ σ_asset σ_fx)t]/DF_asset`, FX vol sampled at the currency CIP forward and payoff DF used only for the final coupon discount. Static replication queries volatility separately at each call-spread strike. Approximate `P(S>K)` by `[C(K_lo)-C(K_hi)]/(K_hi-K_lo)` using undiscounted Black calls, fixed absolute width 0.0025 (half-width 0.00125) and lower strike floor `1e-6`. Clamp each digital probability and their lower-minus-upper difference to [0,1]. Fully deterministic surfaces/overrides use inclusive forward-in-band membership instead. Sum future probabilities plus realized in-range count, divide by total counts, multiply coupon accrual/notional and payment DF.

GBM samples one volatility at payment horizon/current spot, creates drift nodes at observation/payment dates (including quanto carry), merges observation times into the grid, applies fixing multipliers, and computes the same historical-plus-future observation fraction. Generic spot/vol finite-difference Greeks reprice this method. Rho registers discount-only DV01, while DV01 registers combined curve risk; these are distinct registry settings.

## Selection and controls

Default `StaticReplication` uses dated smile call spreads; `MonteCarloGBM` uses a constant volatility plus datewise carry schedule. Complete rate-index fields may be representable in shared terms, but require the separate callable rate-linked instrument/model. Bounds are absolute or multiplied by the spot passed at valuation.

Shared GBM path defaults and instrument overrides apply. Static digital-spread width and strike floor are internal constants without a market-unit scaling control. `RelativeToInitialSpot` computes barriers from the current supplied valuation spot; there is no separately stored inception reference spot in these terms.

## Limitations, failure and fallback

The fixed 0.0025 spread is absolute price units despite its code comment saying 25bp. Probability clamping can conceal smile arbitrage. Relative bounds rebase with current spot and spot risk, rather than a locked inception reference. MC constant volatility and static smile replication are different distributions, especially across maturities/quanto terms.

## Alternatives and tradeoffs

An analytic digital at frozen volatility is cheaper but omits smile-slope effects. Density-consistent smile differentiation, adaptive spread widths or local-volatility paths could align price/risk more closely; none is implemented here. Rate range accruals require rate dynamics, not renaming the asset spot.

## Evidence

- [finstack-quant/valuations/exotics/src/exotics/range_accrual/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/range_accrual/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/range_accrual/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/types.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

Canonical reference index: [Black 1976](../../../REFERENCES.md#black-1976); [Haug](../../../REFERENCES.md#haug-2007-option-pricing-formulas). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
