# finstack-quant-valuations-fixed-income

The fixed-income family of the valuations stack, a crate of its own since the
valuations split. It implements the framework in
[`finstack-quant-valuations-core`](../core/README.md) and is re-exported by
the umbrella crate [`finstack-quant-valuations`](../README.md) as
`finstack_quant_valuations::instruments::fixed_income`, so every public path
there is unchanged. Depends on `finstack-quant-core`, `finstack-quant-cashflows`,
`finstack-quant-covenants`, `finstack-quant-models`, `finstack-quant-margin`
(the `Marginable` impl of `FiIndexTotalReturnSwap`) and valuations-core only;
no other instrument family. Three leaves described below are **not** in this
crate: `structured_credit/`, `cmo/` and `asset_backed_facility/` are the
second-tier crate [`finstack-quant-valuations-structured-credit`](../structured-credit/README.md),
which depends on this crate and on rates, registers with the
`STRUCTURED_CREDIT` family and is re-exported under
`instruments::fixed_income::…`; it reaches this crate's `#[doc(hidden)] pub`
engine entry points.

Twelve instrument families: cash bonds and loans, the agency mortgage complex
(pass-through, TBA, dollar roll, CMO), securitized credit, and two
fixed-income derivatives (bond futures, FI-index TRS). This is the largest
directory in the crate at roughly 92k lines, so this file is an index — it says
which leaf owns which product and which market convention that leaf follows,
and it collects the conventions shared across the family.

Four leaves carry their own README with the full public surface, layout and
worked examples. This file links to them and does not repeat them.

## Leaves

| Directory | Prices | Market convention / model | Own README |
|-----------|--------|---------------------------|------------|
| `bond/` | Fixed, floating, step-up, amortizing, callable/putable and PIK bonds | ISDA/ICMA accrual and clean-vs-dirty quoting; discounting, hazard-rate, tree/OAS and Merton-MC paths | [yes](bond/README.md) |
| `bond_future/` | Deliverable-basket bond futures | Marks the caller-supplied CTD only — refresh the selection when the basket can switch; `value()` will not search. Clean price per 100 face divided by the exchange conversion factor, marked against `terms.entry_price` with no further discounting because futures settle by daily variation margin. Carry uses `repo_curve_id` when present, else `discount_curve_id` | [yes](bond_future/README.md) |
| `convertible/` | Convertible bonds with soft call, put and make-whole | Credit-adjusted tree (Tsiveriotis–Fernandes 1998; Ayache–Forsyth–Vetzal 2003); anti-dilution policies for splits and dividends | no |
| `dollar_roll/` | MBS dollar rolls (sell front month, buy back month) | Drop converted to an implied ACT/360 financing rate for comparison against repo ("specialness"); carry inputs come from the MBS cashflow engine, not stylized amortization | no |
| `fi_trs/` | Fixed-income index total-return swaps | Deterministic carry analytic: the return leg earns a supplied continuously compounded index yield against a curve-driven financing leg. Not a full index mark-to-market model | no |
| `inflation_linked_bond/` | TIPS, index-linked gilts and other linkers | `IndexationMethod` = `Tips`, `Uk`, `Canadian`, `French`, `Japanese`; `DeflationProtection` = `None`, `MaturityOnly`, `AllPayments`. **Discounting is nominal** — the schedule already carries inflation-projected nominal cashflows, so `discount_curve_id` must name a nominal curve. Real yield solves against the unindexed real cashflows and the real dirty price | no |
| `mbs_passthrough/` | Agency MBS pass-throughs (FNMA, FHLMC, GNMA I/II) | PSA / constant-CPR / CPR-vector prepayment (`PrepaymentModelSpec`), scaled by an exponential rate multiplier for MC-OAS and effective duration; stated payment delay 55d for UMBS (FNMA and FHLMC), 45d GNMA I, 50d GNMA II, overridable per pool; net coupon = WAC − servicing fee − guarantee fee | no |
| `revolving_credit/` | Corporate revolvers with deterministic or stochastic utilization, tiered fees and optional hazard weighting | One cashflow engine drives both the deterministic and the Monte Carlo path | [yes](revolving_credit/README.md) |
| `tba/` | Agency TBA forwards | SIFMA good-delivery conventions: third-week settlement with a 48-hour notification deadline (`TbaTerm` = 15y/20y/30y). Without an explicit delivered pool it prices a generic on-the-run pool whose assumptions load from `data/assumptions/tba_assumptions.v1.json`; an explicit `assumed_pool` must carry the TBA coupon and a deliverable agency (FNMA/FHLMC UMBS are interchangeable). No specified-pool allocation or pay-up model | no |
| `term_loan/` | Institutional term loans and DDTLs — PIK and split coupons, OID, covenant events, borrower call schedules | Commitment limit plus draw calendar (contrast `bond`'s single funded notional); discounting or rates-credit tree | [yes](term_loan/README.md) |

The four mortgage leaves form one stack: `mbs_passthrough` owns the cashflow
engine and the prepayment/servicing/delay conventions; `tba` builds an assumed
pool on top of it; `dollar_roll` prices the difference between two TBA
settlement months using the same engine; `cmo` redistributes pass-through
collateral cashflows through a tranche waterfall. Change a prepayment or delay
convention in `mbs_passthrough` and all four move.

## Public surface

Import path: `finstack_quant_valuations_fixed_income::<leaf>` (or `finstack_quant_valuations::instruments::fixed_income::<leaf>` through the umbrella).
Every leaf directory is `pub mod`. The headline types are additionally
re-exported at the crate root and flat at `finstack_quant_valuations::instruments`:

`Bond`, `BondSettlementConvention`, `BondFuture`, `BondFutureBuilder`,
`BondFutureSpecs`, `DeliverableBond`, `AgencyCmo`, `CmoTranche`,
`CmoTrancheType`, `CmoWaterfall`, `ConvertibleBond`, `DollarRoll`,
`FiIndexTotalReturnSwap`, `InflationLinkedBond`, `AgencyMbsPassthrough`,
`AgencyProgram`, `PoolType`, `RevolvingCredit`, `StructuredCredit`,
`AgencyTba`, `TbaTerm`, `TermLoan`.

Types that exist only under the family path, not at `instruments::*`, include
`cmo::PacCollar`, `inflation_linked_bond::{IndexationMethod,
DeflationProtection}`, the `convertible::*`
conversion and greeks types, and everything the four README'd leaves export.

Inside a leaf, `metrics/`, `pricer.rs`/`pricing/` and `types.rs`/`types/` are
`pub(crate)` or private; their supported items surface through the leaf's
`pub use` list. The exceptions — genuinely public submodules — are
`cmo::tranches` (the PAC schedule only), `cmo::waterfall`, `dollar_roll::carry`,
`bond::cashflow_spec`, `bond::cashflows`, `bond::pricing`,
`revolving_credit::cashflow_engine`, `structured_credit::waterfall`,
`term_loan::spec` and `term_loan::overrides`.

## Family conventions

- **Numerics.** `Money` (Decimal-backed) for notionals, balances and PV; `f64`
  for rates, factors, spreads and greeks. No cross-currency arithmetic inside a
  leaf — FX collapse is the caller's, stamped with a policy.
- **Discounting.** Every leaf declares its curves through
  `market_dependencies()`. Inflation linkers discount on a *nominal* curve;
  mortgage leaves discount on the deal currency's OIS curve and take
  prepayment/credit behavior from the cashflow spec, not from the discount
  curve.
- **Prepayment and default assumptions** are `finstack_quant_cashflows`
  specs (`PrepaymentModelSpec`, `DefaultCurve`, `PrepaymentCurve`), not
  per-instrument ad-hoc curves. Structured-credit deal-type defaults live in
  `data/assumptions/structured_credit_assumptions.v1.json`; TBA pool assumptions
  in `data/assumptions/tba_assumptions.v1.json`. Both are embedded with
  `include_str!` and deserialized with `deny_unknown_fields`.
- **Balances.** `cashflow_export`'s `beginning_balance` / `ending_balance`
  columns exist on every row but are filled by concrete-type downcast, and today
  only `AgencyMbsPassthrough` fills them — an amortizing `Bond` or `TermLoan`
  leaves them `null`. CMO tranche pool state is deliberately `null` too, because
  the waterfall engine has no stable per-tranche balance hook yet. Do not read
  `null` here as missing data.
- **Determinism.** Monte Carlo paths (revolving credit, Merton bonds, stochastic
  structured credit, MBS OAS) take an explicit seed and must reproduce
  bit-identically across runs and thread counts.

## Registration

New leaves follow the six-step checklist in
[`../README.md`](../README.md#adding-an-instrument). Family-specific landing
sites, which are *not* all named after the directory:

| Step | Where, for this family |
|------|------------------------|
| Pricer | `register_pricers` in `src/family.rs` (`FIXED_INCOME`) for every leaf in this crate; `structured_credit`, `cmo` and `asset_backed_facility` register with `STRUCTURED_CREDIT` in the structured-credit crate's `structured_credit/family.rs` |
| Instrument key | `InstrumentType` variant in `../core/src/pricer/keys.rs` |
| JSON tag | One line in `with_instrument_json_registry!` in `../src/instruments/json_loader.rs` (umbrella), category `"fixed_income"`. Current tags: `bond`, `convertible_bond`, `inflation_linked_bond`, `term_loan`, `revolving_credit`, `asset_backed_facility`, `agency_mbs_passthrough`, `agency_tba`, `agency_cmo`, `dollar_roll`, `trs_fixed_income_index`, `bond_future`, `structured_credit`. `bond_future`, `structured_credit` and `asset_backed_facility` are registered `boxed:` because of their size |
| Metrics | `register_<name>_metrics(&mut MetricRegistry)` in the leaf's `metrics/`, called from `register_metrics` in `family.rs` |
| Margin | Optional `finstack_quant_margin::Marginable` impl in the instrument's `marginable.rs` (`fi_trs/marginable.rs`), reached only through `Instrument::as_marginable` |
| Schemas | `mise run rust-gen-schemas`, verified by `mise run rust-check-schemas` |

## Tests and benches

Integration tests that need only this crate, valuations-core and the
test-support crate live in `tests/<leaf>/`, compiled into the single
`fixed-income` target (`mise run rust-test-crate -- finstack-quant-valuations-fixed-income`):
most of `bond/`, `term_loan/` and `revolving_credit/`, `inflation_linked_bond/`
whole and `bond_future/serde.rs`. Tests that bootstrap hazard curves or build
`PricingOptions` through `finstack-quant-calibration` (`tests/support/credit.rs`),
price through the standard registries or round-trip `InstrumentJson` stay in
the umbrella's `../tests/instruments/<leaf>/`: `convertible/`, the credit-risk
metrics of `bond/` and `term_loan/`, `bond_future/integration.rs`,
`revolving_credit/{basic,analyst_coverage}.rs`, `term_loan/term_loan_tests.rs`,
the whole `trs/` suite and the loan-facility wire-key tests. `tests/support/
revolver_draws.rs` (the stochastic-revolver fixtures) is also `#[path]`-included
by the structured-credit crate's loan-pool tests. The agency mortgage leaves (`mbs_passthrough`,
`tba`, `dollar_roll`) have no test directory — they are covered by colocated
`#[cfg(test)]` modules plus the umbrella's registry, serde and
dependency-completeness contract tests.

Criterion benches stay in the umbrella's `../benches/`: `bond_pricing`,
`bond_future_bench`, `convertible_pricing`, `structured_credit_pricing`,
`merton_mc_pricing`, `fi_misc_pricing`, `inflation_pricing`.

## Related

- [`../README.md`](../README.md) — `Instrument` trait, JSON contract, add-an-instrument checklist
- [`../common_impl/README.md`](../common_impl/README.md) — the internal plumbing every leaf implements against
- [`../rates/README.md`](../rates/README.md) — swaps, options and money-market instruments
- [`../../metrics/README.md`](../../metrics/README.md) — `MetricId` and calculator registration
- [`../../calibration/README.md`](../../calibration/README.md) — building the `MarketContext` these leaves read
- [`../tests/instruments/README.md`](../tests/instruments/README.md) — test layout and generated fixtures
- Per-instrument rustdoc carries the formulas, references and worked examples;
  this file carries only what rustdoc cannot show.
