# Consolidation Plan: `cashflows + valuations::cashflow-pathways`

**Based on:** Audit Report dated 2026-07-13  
**User priorities:** resolve every F1–F14 finding and finish with one canonical cashflow construction, provider, composition, export, and binding path  
**Plan date:** 2026-07-13

## Slicing principles applied

- One theme per slice and one commit per slice.
- Delete-only and local collapses precede invariant-sensitive public changes.
- Rust remains canonical; binding triplets move together.
- Existing legacy JSON/accrual inputs may survive only as one-way compatibility adapters into canonical Rust state.
- Focused checks run while iterating; each slice must pass its affected Rust/binding gates before the next starts.
- Unrelated bond/tree and QuantLib worktree edits are preserved and excluded from every commit.

## Slice 1 — remove orphan credit emitters

**Tier:** 3  
**Estimated net LOC:** −250  
**Files touched:** cashflows credit-emission module/re-exports and tests/test support that call it.  
**Addresses:** F8.  
**Invariants:** public Rust surface, serde review for `DefaultEvent`.  
**Verify:** cashflows tests, valuations cashflow tests, Rust lint, parity selection.  
**Rollback:** atomic deletion commit.

## Slice 2 — collapse residual schedule helper boilerplate

**Tier:** 2/3  
**Estimated net LOC:** −40  
**Files touched:** `cashflows/src/traits.rs`, `cashflows/src/builder/schedule.rs`, affected benchmark/test callers, redundant valuation pre-sorts.  
**Addresses:** F13.  
**Invariants:** deterministic ordering.  
**Verify:** cashflows tests, valuations cashflow tests, Rust lint.  
**Depends on:** Slice 1.

## Slice 3 — return canonical Rust JSON and canonical model policy

**Tier:** 4  
**Estimated net LOC:** −15  
**Files touched:** Python valuations binding/stub and focused tests; Rust/WASM only if a canonical default is selected.  
**Addresses:** F14.  
**Invariants:** serde output, Python/WASM parity.  
**Verify:** Python build/lint/tests and parity tests.

## Slice 4 — unify WASM valuation-cashflow namespace

**Tier:** 4  
**Estimated net LOC:** neutral  
**Files touched:** WASM valuation facades, `index.d.ts`, parity contract, facade tests.  
**Addresses:** F11.  
**Invariants:** binding topology/parity.  
**Verify:** WASM build/lint/tests, Python parity tests.

## Slice 5 — centralize mortality-rate conversions

**Tier:** 4  
**Estimated net LOC:** −80  
**Files touched:** cashflows credit-rate kernel, MBS prepayment module, structured-credit rate utility/tests.  
**Addresses:** F4.  
**Invariants:** numerical boundary behavior; explicit reject-versus-clamp policy.  
**Verify:** focused MBS/structured-credit tests, Rust lint/test, relevant goldens.

## Slice 6 — make provider contract coverage truthful

**Tier:** 1/2  
**Estimated net LOC:** +test coverage  
**Files touched:** valuations cashflow provider contract helpers/tests.  
**Addresses:** F7 and prepares F1–F3/F6.  
**Invariants:** lifecycle boundary, cash-settlement classification.  
**Verify:** valuations cashflow integration suite.

## Slice 7 — enforce the raw-to-public provider finalizer

**Tier:** 4  
**Estimated net LOC:** −100 after migration  
**Files touched:** cashflows provider traits and all non-empty valuation providers, grouped mechanically by asset family.  
**Addresses:** F1.  
**Invariants:** `date >= as_of`, PIK omission, ordering, representation, public API.  
**Verify:** provider contract suite after each asset-family migration, Rust lint/test, parity.

## Slice 8 — make dated cashflows purely derived

**Tier:** 4  
**Estimated net LOC:** −25  
**Files touched:** cashflows provider API, structured-credit provider, provider contract tests.  
**Addresses:** F2.  
**Invariants:** cash-only filtering, row identity, parity.  
**Verify:** provider contract and structured-credit suites, Rust lint/test.

## Slice 9 — preserve swap cashflow classification

**Tier:** 4  
**Estimated net LOC:** neutral  
**Files touched:** commodity swap, CMS swap, inflation swaps, focused schedule/WAL tests.  
**Addresses:** F3.  
**Invariants:** `CFKind`, accrual/rate metadata, outstanding balance, WAL, same-day ordering.  
**Verify:** focused product tests, valuations cashflow tests, Rust lint/test, relevant goldens.

## Slice 10 — finish contractual period migration

**Tier:** 4  
**Estimated net LOC:** −80  
**Files touched:** TRS schedule parameters, term-loan cashflows, revolving-credit schedule utilities, commodity swap, structured-credit simulation, period helper/tests.  
**Addresses:** F10.  
**Invariants:** ISDA day counts, calendars, BDC, stub/EOM policy.  
**Verify:** focused product tests, calendar/day-count vectors, Rust lint/test.

## Slice 11 — canonicalize schedule construction and validation

**Tier:** 4  
**Estimated net LOC:** neutral with broad call-site migration  
**Files touched:** cashflow schedule type/serde/JSON validation plus schedule literal/read/mutation call sites across workspace crates.  
**Addresses:** F5.  
**Invariants:** serde compatibility, deterministic ordering, public API.  
**Verify:** full Rust lint/test, cashflow JSON/schema tests, Python/WASM rebuild and parity.

## Slice 12 — provide one metadata-preserving composite route

**Tier:** 4  
**Estimated net LOC:** −100  
**Files touched:** cashflows merge logic and IRS, Basis, FX Swap, XCCY providers/tests.  
**Addresses:** F6.  
**Invariants:** FX/mixed-currency semantics, accrual metadata, ordering, serde.  
**Verify:** focused IRS/Basis/FX/XCCY tests, Rust lint/test, cashflow export tests.

## Slice 13 — project and value exported cashflows once

**Tier:** 4  
**Estimated net LOC:** −60  
**Files touched:** cashflows aggregation API, MBS projection artifact, generic valuation cashflow exporter/tests.  
**Addresses:** F9.  
**Invariants:** credit PV, recovery treatment, MBS balance path, binding JSON.  
**Verify:** MBS and credit goldens, exporter tests, full Rust/binding/parity stack.

## Slice 14 — collapse fixed step-up representations

**Tier:** 4  
**Estimated net LOC:** −100  
**Files touched:** cashflows coupon API/spec/JSON/schema/tests and parity fixtures if present.  
**Addresses:** F12.  
**Invariants:** Decimal rates, boundary dates, serde compatibility.  
**Verify:** cashflows JSON/schema tests, Rust lint/test, Python/WASM/parity tests.

## Slice dependency graph

```text
S1 -> S2
S3 -> S4
S5
S6 -> S7 -> S8 -> S9 -> S10 -> S11 -> S12 -> S13 -> S14
```

The low-risk branches S1–S5 land first. S6–S14 then form the invariant-sensitive spine: establish tests, enforce the provider boundary, preserve row semantics, centralize periods and schedule ownership, consolidate composition/export, and finally remove the last overlapping program representation.

## Not in this plan

- The existing one-way legacy JSON arrays and accrual-sidecar readers are retained as compatibility adapters unless their removal is required to complete F12; they do not own computation.
- Unrelated bond pricing/tree and QuantLib golden edits already present in the worktree.
- Performance work beyond eliminating the audited duplicate projections.

## What we expect at the end

- One provider lifecycle finalizer and one derived dated-flow view.
- One canonical schedule constructor/validator with controlled composition.
- No product-local `flows.extend`, sorting, lifecycle filtering, or coupon-to-notional flattening.
- One mortality conversion family and one exported row-PV kernel.
- One fixed step-up representation.
- Thin, topologically consistent Python/WASM bindings.
- Full Rust, Python, WASM, parity, and focused financial gates green.
