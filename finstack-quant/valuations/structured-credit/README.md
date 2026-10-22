# finstack-quant-valuations-structured-credit

The structured-credit tier of the valuations stack: securitised deals, agency
CMOs and asset-backed warehouse facilities. It implements the framework in
[`finstack-quant-valuations-core`](../core/README.md) on top of two instrument
families — [`finstack-quant-valuations-fixed-income`](../fixed-income/README.md)
(bond cashflow specs, loan terms, the revolving-credit Monte Carlo engine, the
agency pass-through pool model) and
[`finstack-quant-valuations-rates`](../rates/README.md) (the deal-level IRS
hedge) — and is re-exported by the umbrella crate
[`finstack-quant-valuations`](../README.md) as
`finstack_quant_valuations::instruments::fixed_income::{structured_credit, cmo,
asset_backed_facility}`, so every public path there is unchanged. Depends on
`finstack-quant-core`, `finstack-quant-cashflows`, `finstack-quant-models`,
valuations-core and those two family crates; no other family.

## Modules

| Directory | Prices | Engine | README |
|-----------|--------|--------|--------|
| `src/structured_credit/` | ABS, RMBS, CMBS and CLO deals — collateral pool, tranche stack, payment waterfall with OC/IC coverage tests, deal-type assumptions | Period-by-period pool and waterfall simulation; stochastic mode runs the same engine over simulated prepayment/default paths and the instrument-level pool over the revolver Monte Carlo process | [yes](src/structured_credit/README.md) |
| `src/cmo/` | Agency CMO tranches (`Sequential`, `Pac`, `Support`, `InterestOnly`, `PrincipalOnly`, `Accrual`) | Pass-through collateral from `mbs_passthrough` redistributed through the CMO waterfall; Z-tranche accrual is capitalised and redirected as accretion-directed principal (Fabozzi, *MBS Handbook* 7e, Ch. 21) | no |
| `src/asset_backed_facility/` | Warehouse and forward-flow lines against a collateral pool — advance rates, eligibility, concentration limits, borrowing-base test, unused fee, revolving period, early-amortisation events and term-out | Synthesises a two-class `structured_credit` deal (facility note + residual) and runs the pool/waterfall engine | no |
| `src/structured_credit/family.rs` | Not an instrument | `STRUCTURED_CREDIT`, the pricer and metric registrations of all three modules | — |

Deal-type defaults (fees, constructor defaults, SDA parameters, stochastic
calibrations) are the embedded registry
[`data/assumptions/structured_credit_assumptions.v1.json`](data/assumptions/structured_credit_assumptions.v1.json).

```rust
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_structured_credit::asset_backed_facility::AssetBackedFacility;
use finstack_quant_valuations_structured_credit::structured_credit::StructuredCredit;

let deal = StructuredCredit::example().unwrap();
let facility = AssetBackedFacility::example().unwrap();
assert_eq!(facility.synthesized_deal().unwrap().tranches.tranches.len(), 2);
assert!(!deal.id().is_empty());
```

## Public surface

Import path: `finstack_quant_valuations_structured_credit::<module>` (or
`finstack_quant_valuations::instruments::fixed_income::<module>` through the
umbrella); the crate root exports only the `STRUCTURED_CREDIT` family static.
`StructuredCredit`, `AgencyCmo`, `CmoTranche`, `CmoTrancheType`, `CmoWaterfall`
and `AssetBackedFacility` are also flat at `finstack_quant_valuations::instruments`. The stochastic result DTOs
(`StochasticPricingResult`, `TranchePricingResult`,
`StructuredCreditPricingMode`, `TrancheSeniority`) are defined in
valuations-core `results::structured_credit` and re-exported from
`structured_credit`.

## Tests and benches

Integration tests that need only this crate, its two family dependencies,
valuations-core and the test-support crate live in `tests/`, compiled into the
single `structured-credit` target
(`mise run rust-test-crate -- finstack-quant-valuations-structured-credit`).
`tests/support/` includes two builders by path from the crates that own
them: the IRS builder (`rates/tests/support/rates.rs`) for deal hedge swaps
and the stochastic revolver fixtures
(`fixed-income/tests/support/revolver_draws.rs`) for instrument-level loan
pools. The JSON round-trip and `price_instrument` tests of the deal and the
facility stay in the umbrella's `../tests/instruments/{structured_credit,
asset_backed_facility}/`. Benches (`structured_credit_pricing`,
`structured_credit_stochastic`, the CMO groups of `fi_misc_pricing`) stay in
the umbrella.
