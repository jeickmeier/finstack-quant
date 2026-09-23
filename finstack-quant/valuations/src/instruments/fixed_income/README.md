# Fixed Income

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
| `asset_backed_facility/` | Warehouse lines and asset-backed facilities against a collateral pool — advance rates, eligibility, concentration limits, borrowing-base test, unused fee, revolving period, early-amortization events and term-out | Synthesizes a two-class `structured_credit` deal (facility note + residual) and runs the pool/waterfall engine; `BorrowingBaseRules` also drive the `CoverageTestType::BorrowingBase` test in any deal | no |
| `bond/` | Fixed, floating, step-up, amortizing, callable/putable and PIK bonds | ISDA/ICMA accrual and clean-vs-dirty quoting; discounting, hazard-rate, tree/OAS and Merton-MC paths | [yes](bond/README.md) |
| `bond_future/` | Deliverable-basket bond futures | Marks the caller-supplied CTD only — refresh it with `determine_ctd_by_implied_repo` when the basket can switch; `value()` will not search. Clean price per 100 face divided by the exchange conversion factor, marked against `quoted_price` with no further discounting because futures settle by daily variation margin. Carry uses `repo_curve_id` when present, else `discount_curve_id` | [yes](bond_future/README.md) |
| `cmo/` | Agency CMO tranches | `CmoTrancheType` = `Sequential`, `Pac`, `Support`, `InterestOnly`, `PrincipalOnly`, `Accrual`. Z-tranche accrual is capitalized and redirected as accretion-directed principal (Fabozzi, *MBS Handbook* 7e, Ch. 21). Every fixed-coupon principal tranche must sit at or below the collateral net pass-through coupon | no |
| `convertible/` | Convertible bonds with soft call, put and make-whole | Credit-adjusted tree (Tsiveriotis–Fernandes 1998; Ayache–Forsyth–Vetzal 2003); anti-dilution policies for splits and dividends | no |
| `dollar_roll/` | MBS dollar rolls (sell front month, buy back month) | Drop converted to an implied ACT/360 financing rate for comparison against repo ("specialness"); carry inputs come from the MBS cashflow engine, not stylized amortization | no |
| `fi_trs/` | Fixed-income index total-return swaps | Deterministic carry analytic: the return leg earns a supplied continuously compounded index yield against a curve-driven financing leg. Not a full index mark-to-market model | no |
| `inflation_linked_bond/` | TIPS, index-linked gilts and other linkers | `IndexationMethod` = `Tips`, `Uk`, `Canadian`, `French`, `Japanese`; `DeflationProtection` = `None`, `MaturityOnly`, `AllPayments`. **Discounting is nominal** — the schedule already carries inflation-projected nominal cashflows, so `discount_curve_id` must name a nominal curve. Real yield solves against the unindexed real cashflows and the real dirty price | no |
| `mbs_passthrough/` | Agency MBS pass-throughs (FNMA, FHLMC, GNMA I/II) | PSA / constant-CPR / Richard–Roll prepayment; stated payment delay 55d for UMBS (FNMA and FHLMC), 45d GNMA I, 50d GNMA II, overridable per pool; net coupon = WAC − servicing fee − guarantee fee | no |
| `revolving_credit/` | Corporate revolvers with deterministic or stochastic utilization, tiered fees and optional hazard weighting | One cashflow engine drives both the deterministic and the Monte Carlo path | [yes](revolving_credit/README.md) |
| `structured_credit/` | ABS, RMBS, CMBS and CLO deals — pool, tranche stack, waterfall, coverage tests | Period-by-period pool and waterfall simulation; stochastic mode runs the same engine over simulated prepayment/default paths | [yes](structured_credit/README.md) |
| `tba/` | Agency TBA forwards | SIFMA good-delivery conventions: third-week settlement with a 48-hour notification deadline (`TbaTerm` = 15y/20y/30y). Pool allocation is a simplified on-the-run assumed pool, not full good-delivery rules; assumptions load from `data/assumptions/tba_assumptions.v1.json` | no |
| `term_loan/` | Institutional term loans and DDTLs — PIK and split coupons, OID, covenant events, borrower call schedules | Commitment limit plus draw calendar (contrast `bond`'s single funded notional); discounting or rates-credit tree | [yes](term_loan/README.md) |

The four mortgage leaves form one stack: `mbs_passthrough` owns the cashflow
engine and the prepayment/servicing/delay conventions; `tba` builds an assumed
pool on top of it; `dollar_roll` prices the difference between two TBA
settlement months using the same engine; `cmo` redistributes pass-through
collateral cashflows through a tranche waterfall. Change a prepayment or delay
convention in `mbs_passthrough` and all four move.

## Public surface

Import path: `finstack_quant_valuations::instruments::fixed_income::<leaf>`.
Every leaf directory is `pub mod`. The headline types are additionally
re-exported flat at `finstack_quant_valuations::instruments`:

`Bond`, `BondSettlementConvention`, `BondFuture`, `BondFutureBuilder`,
`BondFutureSpecs`, `DeliverableBond`, `AgencyCmo`, `CmoTranche`,
`CmoTrancheType`, `CmoWaterfall`, `ConvertibleBond`, `DollarRoll`,
`FIIndexTotalReturnSwap`, `InflationLinkedBond`, `AgencyMbsPassthrough`,
`AgencyProgram`, `PoolType`, `RevolvingCredit`, `StructuredCredit`,
`AgencyTba`, `TbaTerm`, `TermLoan`.

Types that exist only under the family path, not at `instruments::*`, include
`cmo::PacCollar`, `tba::TbaSettlement`, `inflation_linked_bond::{IndexationMethod,
DeflationProtection, InflationLinkedBondParams}`, the `convertible::*`
conversion and greeks types, and everything the four README'd leaves export.

Inside a leaf, `metrics/`, `pricer.rs`/`pricing/` and `types.rs`/`types/` are
`pub(crate)` or private; their supported items surface through the leaf's
`pub use` list. The exceptions — genuinely public submodules — are
`cmo::tranches`, `cmo::waterfall`, `dollar_roll::carry`,
`mbs_passthrough::prepayment`, `mbs_passthrough::servicing`, `tba::allocation`,
`tba::settlement`, `bond::cashflow_spec`, `bond::cashflows`, `bond::pricing`,
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
| Pricer | `src/pricer/fixed_income.rs` for most leaves. `Bond` and `BondFuture` register in `src/pricer/rates.rs`; `StructuredCredit` in `src/pricer/credit.rs` |
| Instrument key | `InstrumentType` variant in `src/pricer/keys.rs` |
| JSON tag | One line in `with_instrument_json_registry!` in `../json_loader.rs`, category `"fixed_income"`. Current tags: `bond`, `convertible_bond`, `inflation_linked_bond`, `term_loan`, `revolving_credit`, `asset_backed_facility`, `agency_mbs_passthrough`, `agency_tba`, `agency_cmo`, `dollar_roll`, `trs_fixed_income_index`, `bond_future`, `structured_credit`. `bond_future`, `structured_credit` and `asset_backed_facility` are registered `boxed:` because of their size |
| Metrics | `register_<name>_metrics(&mut MetricRegistry)` in the leaf's `metrics/`, called from `register_fixed_income_instrument_metrics` in `src/metrics/core/standard_registry.rs` |
| Margin | Optional `finstack_quant_margin::Marginable` impl in `../marginable.rs`, reached only through `Instrument::as_marginable` |
| Schemas | `mise run rust-gen-schemas`, verified by `mise run rust-check-schemas` |

## Tests and benches

Integration tests live in `../../../tests/instruments/<leaf>/`, all compiled
into the single `instruments` target. Leaves with a dedicated directory: `asset_backed_facility`, `bond`,
`bond_future`, `convertible`, `inflation_linked_bond`, `revolving_credit`,
`structured_credit`, `term_loan`. `fi_trs` shares `tests/instruments/trs/` with
the equity TRS (`test_fi_index_trs.rs`). The four mortgage leaves
(`mbs_passthrough`, `tba`, `dollar_roll`, `cmo`) have no test directory — they
are covered by colocated `#[cfg(test)]` modules plus the cross-cutting registry,
serde and dependency-completeness contract tests.

```bash
# whole target
cargo nextest run -p finstack-quant-valuations --test instruments

# one leaf (filter is a substring match on the test name)
cargo nextest run -p finstack-quant-valuations --test instruments bond::
cargo nextest run -p finstack-quant-valuations --test instruments structured_credit::

# colocated unit tests for a leaf without a test directory
cargo nextest run -p finstack-quant-valuations --lib fixed_income::mbs_passthrough

# whole workspace, what CI runs
mise run rust-test
```

Use `cargo nextest`, not `cargo test` — the latter also runs doc tests, which
this project keeps out of the normal loop. Lint with `mise run rust-lint`.

Criterion benches in `../../../benches/`: `bond_pricing`, `bond_future_bench`,
`convertible_pricing`, `structured_credit_pricing`, `merton_mc_pricing`,
`cashflow_generation`, `inflation_pricing`, and `fi_misc_pricing` for the rest.

```bash
cargo bench -p finstack-quant-valuations --bench bond_pricing
mise run rust-bench          # all workspace benches, short sampling
```

## Related

- [`../README.md`](../README.md) — `Instrument` trait, JSON contract, add-an-instrument checklist
- [`../common_impl/README.md`](../common_impl/README.md) — the internal plumbing every leaf implements against
- [`../rates/README.md`](../rates/README.md) — swaps, options and money-market instruments
- [`../../metrics/README.md`](../../metrics/README.md) — `MetricId` and calculator registration
- [`../../calibration/README.md`](../../calibration/README.md) — building the `MarketContext` these leaves read
- [`../../../tests/instruments/README.md`](../../../tests/instruments/README.md) — test layout and generated fixtures
- Per-instrument rustdoc carries the formulas, references and worked examples;
  this file carries only what rustdoc cannot show.
