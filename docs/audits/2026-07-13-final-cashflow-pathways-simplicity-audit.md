# Audit Report: `finstack-quant::{cashflows,valuations}::cashflow-pathways`

**Scope:** all cashflow construction, classification, composition, provider, export, and validation paths under `finstack-quant/cashflows/src` and `finstack-quant/valuations/src`, plus related Python/WASM bindings, facades, stubs, parity contracts, and focused contract tests.  
**Bindings in scope:**

- `finstack-quant-py/src/bindings/{cashflows,valuations}` — exists
- `finstack-quant-wasm/src/api/{cashflows,valuations}` — exists
- Python stubs/package exports, JS facades, `index.d.ts`, and `finstack-quant-py/parity_contract.toml` — exists

**Date:** 2026-07-13  
**Auditor:** finstack-simplify / Phase 1 (read-only code audit; this report is the only created artifact)

## Executive summary

The earlier consolidation work succeeded at the center: `CashFlow` now owns its accrual state, rich builders and simple adapters converge on `CashFlowSchedule::from_parts`, JSON inputs translate into the canonical coupon/payment program, balance replay and WAL share the same state model, and Python/WASM cashflow functions are thin delegates. Those are genuine single pathways, not merely renamed duplicates.

The full system is not yet 100% single-path green. This audit found **six high-impact**, **six medium-impact**, and **two low-impact** remaining findings. The highest-leverage issue is that `CashflowProvider` finalization is a caller convention rather than an enforced boundary: product code can filter dates, omit rows, classify flows, sort, merge, or override flattened output before shared normalization ever sees the data. The next two structural blockers are externally mutable/deserializable schedules and a same-currency-only merge helper that forces FX/XCCY products back into manual `flows.extend` composition.

Focused validation passed:

- `finstack-quant-cashflows` library tests: **144 passed**
- valuations cashflow integration suite: **33 passed**
- binding parity topology selection: **389 passed**
- focused Python cashflow/valuation binding tests: **32 passed**

The worktree already contained unrelated QuantLib golden/generator edits when this audit began, and additional bond/tree source edits appeared concurrently before closeout. None of those in-flight files were modified or reverted by this audit.

## Surface area inventory

**Capability — rich cashflow construction**

- Canonical entry point: [`CashFlowSchedule::builder()`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:516)
- Canonical internal sink: [`CashFlowSchedule::from_parts`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:466)
- Justified alternate inputs:
  - [`schedule_from_dated_flows`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/traits.rs:197) — already-computed, unclassified `(Date, Money)` input
  - [`schedule_from_classified_flows`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/traits.rs:272) — already-computed `CashFlow` input that must retain kind/accrual metadata
- Residual bypasses:
  - public mutable schedule fields at [`schedule.rs:230`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:230)
  - deserialization constructs `Self` directly at [`schedule.rs:372`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:372)

**Capability — provider-facing public schedules**

- Contract: [`CashflowProvider::cashflow_schedule`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/traits.rs:125) must return future-filtered, PIK-free, canonically ordered flows
- Shared finalizer: [`CashFlowSchedule::normalize_public`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:561)
- Alternate pathways found:
  - providers that pre-filter inside raw generation
  - providers that manually filter or return without `normalize_public`
  - product-local sorting and representation stamping
- Inventory: 43 explicit production implementations — 33 non-empty/delegating, eight intentionally empty, and two intentional errors for unsupported physical-delivery schedules — plus 27 placeholder products using the shared empty-provider macro.

**Capability — flattened dated cashflows**

- Canonical derivation: default [`CashflowProvider::dated_cashflows`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/traits.rs:143), which derives only cash-settlement rows from the canonical schedule
- Alternate pathway found: [`StructuredCredit::dated_cashflows`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/structured_credit/types/mod.rs:849) aggregates every schedule row by date, including non-cash state rows

**Capability — multi-leg schedule composition**

- Intended shared entry point: [`merge_cashflow_schedules`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:822)
- Alternate pathways found:
  - [`BasisSwap`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/basis_swap/types.rs:642) extends the primary leg directly
  - [`FxSwap`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fx/fx_swap/types.rs:368) builds four one-row schedules and concatenates them
  - [`XccySwap`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/xccy_swap/types.rs:906) concatenates four schedule components
  - IRS recreates flows and loses per-flow accrual metadata at [`irs/cashflow.rs:733`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/irs/cashflow.rs:733)

**Capability — contractual periods and payment dates**

- Canonical entry point: [`build_periods(BuildPeriodsParams)`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/periods.rs:255)
- Justified specialized entry point: `build_single_period`, used for caplet/floorlet semantics but sharing the same enrichment engine
- Alternate pathways found: direct core `ScheduleBuilder` use in TRS, term-loan, revolving-credit, commodity-swap, and structured-credit cashflow generation
- Half-migrated helper: [`BuildPeriodsParams::from_schedule`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/periods.rs:74) has no call sites while valuations contains roughly 31 direct parameter literals

**Capability — mortgage/structured-credit rate conversion**

- Canonical checked functions: [`credit_rates::cpr_to_smm`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/credit_rates.rs:47) and [`smm_to_cpr`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/credit_rates.rs:99)
- Alternate pathways found:
  - MBS checked copies at [`mbs_passthrough/prepayment.rs:204`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/mbs_passthrough/prepayment.rs:204)
  - structured-credit clamping copies at [`structured_credit/utils/rates.rs:56`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/structured_credit/utils/rates.rs:56)

**Capability — valuation cashflow export**

- Canonical provider call: [`instrument_cashflows` obtains `cashflow_schedule`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/cashflow_export.rs:221)
- Alternate computation found:
  - MBS is projected a second time for row diagnostics at [`cashflow_export.rs:225`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/cashflow_export.rs:225)
  - credit-adjusted row PV is copied into [`compute_pv`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/cashflow_export.rs:427) instead of using the cashflows aggregation kernel

**Capability — host-language cashflow APIs**

- Canonical Rust JSON operations are direct delegates from PyO3 at [`bindings/cashflows/mod.rs:24`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/src/bindings/cashflows/mod.rs:24) and WASM at [`api/cashflows/mod.rs:12`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/src/api/cashflows/mod.rs:12)
- `bond_from_cashflows_json` delegates directly to the same Rust constructor in both hosts
- Python's DataFrame helper is presentation-only and calls the canonical JSON exporter
- Structural alternate: WASM places the JSON-market and reusable-market variants in different namespaces
- No Python or WASM code independently constructs, sorts, classifies, accrues, or values cashflows

**Capability — compatibility inputs**

- Legacy fixed/floating JSON arrays are translated one-way into `coupon_program` at [`json.rs:181`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/json.rs:181)
- Legacy schedule accrual sidecars are translated one-way into per-flow `CashFlowAccrual` at [`schedule.rs:281`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:281)
- These are compatibility adapters, not second computation engines. They do not block a single runtime pathway, though they remain schema debt to sunset in a breaking release.

## Findings

### F1 — [Category: bypassable-finalizer / parallel-lifecycle-policy]

**Files:**

- [`cashflows/src/traits.rs:110`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/traits.rs:110)
- [`cashflows/src/builder/schedule.rs:549`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:549)
- [`valuations/.../fi_trs/types.rs:282`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/fi_trs/types.rs:282)
- [`valuations/.../irs/cashflow.rs:393`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/irs/cashflow.rs:393)
- [`valuations/.../mbs_passthrough/pricer.rs:116`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/mbs_passthrough/pricer.rs:116)

**What:** The provider contract defines one public lifecycle policy — retain `date >= as_of`, omit pure PIK, sort canonically, and stamp representation — but `normalize_public` is an optional call made by each implementation. `FIIndexTotalReturnSwap` ignores `as_of` entirely; DCF and real-estate providers hand-filter; IRS, CMS, YoY inflation, CDS index, MBS, structured credit, and XCCY MtM generation remove rows before the finalizer, including several `date == as_of` rows that the public contract says to retain.

**Why it's slop:** The lifecycle is a distributed convention rather than a single pathway. A shared helper cannot recover data that a product-specific generator already discarded, so identical public APIs have product-dependent settlement boundaries.

**Proposed fix:** Introduce one non-overridable provider boundary, for example a sealed/raw schedule implementation plus a blanket public finalizer. Raw generators should produce complete classified schedules; only the shared boundary may apply `as_of`, PIK, ordering, and representation policy. Migrate all 33 non-empty/delegating providers and delete local lifecycle filtering and repeated finalization code.

**Invariants touched:** ISDA, precedence, parity  
**Impact:** H  
**Risk:** M  
**Tier:** 4

---

### F2 — [Category: parallel-api / derived-path-override]

**Files:**

- [`cashflows/src/traits.rs:131`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/traits.rs:131)
- [`valuations/.../structured_credit/types/mod.rs:843`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/structured_credit/types/mod.rs:843)
- [`valuations/.../simulation_engine.rs:2818`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/structured_credit/pricing/simulation_engine.rs:2818)

**What:** `dated_cashflows` is documented as a convenience derived from `cashflow_schedule`, but the trait explicitly permits overrides. `StructuredCredit` uses that escape hatch to aggregate every row by date, including `DefaultedNotional`, while the canonical default filters non-cash state rows and preserves row identity.

**Why it's slop:** One capability has two meanings under the same trait method. Callers cannot know whether they receive a cash-only projection or a date-aggregated state ledger without inspecting the concrete instrument.

**Proposed fix:** Make dated extraction a non-overridable schedule method/free function and remove all provider overrides. If structured-credit analytics needs same-day aggregation, add an explicitly named operation such as `aggregate_cash_settlements_by_date`, implemented from canonical dated flows.

**Invariants touched:** Decimal, parity, precedence  
**Impact:** H  
**Risk:** M  
**Tier:** 4

---

### F3 — [Category: classification-loss / parallel-mapping]

**Files:**

- [`commodity_swap/types.rs:453`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/commodity/commodity_swap/types.rs:453)
- [`commodity_swap/types.rs:570`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/commodity/commodity_swap/types.rs:570)
- [`cms_swap/types.rs:456`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/cms_swap/types.rs:456)
- [`inflation_swap/types.rs:537`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/inflation_swap/types.rs:537)
- [`cashflows/src/builder/schedule.rs:604`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:604)

**What:** Commodity-swap fixed/floating payments, CMS/funding coupons, and inflation fixed/inflation legs are converted to raw dated money and then stamped uniformly as `CFKind::Notional`. The original leg semantics and available period/rate metadata are discarded before the shared schedule is built.

**Why it's slop:** Product code is doing a lossy, product-specific mapping around the classified-flow pathway. The shared analytics layer treats `Notional` as principal, so this is also observable in outstanding balance and WAL rather than being a harmless display label.

**Proposed fix:** Construct classified flows once with `Fixed`, `FloatReset`, and `InflationCoupon`, carrying `CashFlowAccrual` and rate metadata from shared periods. Then pass those rows through `schedule_from_classified_flows` and the one provider finalizer.

**Invariants touched:** Decimal, ISDA, precedence  
**Impact:** H  
**Risk:** M  
**Tier:** 4

---

### F4 — [Category: duplicate-computation / numerical-policy-drift]

**Files:**

- [`cashflows/src/builder/credit_rates.rs:47`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/credit_rates.rs:47)
- [`mbs_passthrough/prepayment.rs:204`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/mbs_passthrough/prepayment.rs:204)
- [`structured_credit/utils/rates.rs:56`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/structured_credit/utils/rates.rs:56)

**What:** CPR/SMM conversion exists in three places. Cashflows and MBS expose checked versions with separate error text; structured credit has clamping `f64 -> f64` copies and adds its own CDR/MDR analogues.

**Why it's slop:** The same financial kernel has multiple owners and two hidden invalid-input policies: reject versus clamp. Small boundary changes can alter MBS and structured-credit projections independently.

**Proposed fix:** Keep the checked annual/monthly conversion kernel in cashflows and delegate MBS directly. Where structured-credit compatibility requires clamping, make that policy explicit in a narrowly named boundary adapter that clamps first and then calls the shared kernel; add CDR/MDR to the same canonical kernel family.

**Invariants touched:** Decimal, precedence  
**Impact:** H  
**Risk:** M  
**Tier:** 4

---

### F5 — [Category: bypassable-constructor / split-validation]

**Files:**

- [`cashflows/src/builder/schedule.rs:230`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:230)
- [`cashflows/src/builder/schedule.rs:270`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:270)
- [`cashflows/src/builder/schedule.rs:466`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:466)
- [`cashflows/src/json.rs:516`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/json.rs:516)

**What:** `CashFlowSchedule` exposes all fields publicly, while its sorting constructor is crate-private. Custom deserialization translates legacy accrual data and then returns a direct `Self { ... }`, bypassing `from_parts`; direct serde users therefore can receive unsorted schedules. `CashFlowSchedule::validate` checks primitive/order invariants, while the JSON validation function first sorts and then applies stronger economic validation.

**Why it's slop:** Construction, mutation, canonicalization, and validation do not share one enforceable owner. `normalize_public` consequently performs defensive sorting on every public schedule call because upstream state cannot be trusted.

**Proposed fix:** Route builder, adapters, serde, and merge through one canonical constructor and one immutable validation contract. Make flow mutation controlled through append/merge APIs and private accessors; phase public field removal as an explicit breaking change. Keep the legacy accrual wire translation at the deserialize boundary, then immediately canonicalize through the same constructor.

**Invariants touched:** serde, parity, precedence  
**Impact:** H  
**Risk:** H  
**Tier:** 4

---

### F6 — [Category: parallel-composition / representation-mismatch]

**Files:**

- [`cashflows/src/builder/schedule.rs:822`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:822)
- [`basis_swap/types.rs:647`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/basis_swap/types.rs:647)
- [`fx_swap/types.rs:388`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fx/fx_swap/types.rs:388)
- [`xccy_swap/types.rs:941`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/xccy_swap/types.rs:941)
- [`irs/cashflow.rs:733`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/irs/cashflow.rs:733)

**What:** The shared merge helper rejects every flow whose currency differs from one selected notional. The schedule type nevertheless already carries mixed-currency FX/XCCY flows, so Basis, FX Swap, and XCCY manually extend the first schedule's vector and retain only its metadata. IRS has a separate same-currency assembly path that recreates rows with `CashFlow::new`, dropping builder-produced accrual metadata.

**Why it's slop:** The canonical combiner cannot represent the valid composite products in scope, forcing each instrument to own ordering and metadata-loss behavior. This is the clearest remaining product-local schedule pathway.

**Proposed fix:** Make the schedule model's mixed-currency semantics explicit and let one shared combiner preserve all rows and metadata; currency-specific analytics should validate homogeneity at the point where they require it. Migrate IRS and Basis to the shared merge, construct FX Swap in one classified/datetime schedule call, migrate XCCY to the shared composite route, and remove every production `flows.extend`.

**Invariants touched:** FX, serde, parity, ISDA  
**Impact:** H  
**Risk:** H  
**Tier:** 4

---

### F7 — [Category: incomplete-contract-test / false-assurance]

**Files:**

- [`valuations/tests/cashflows/provider_contract.rs:1`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/tests/cashflows/provider_contract.rs:1)
- [`valuations/tests/cashflows/provider_contract.rs:55`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/tests/cashflows/provider_contract.rs:55)
- [`valuations/tests/cashflows/provider_contract.rs:203`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/tests/cashflows/provider_contract.rs:203)

**What:** The provider suite says it covers all implementations, but the full verifier is applied only to Bond, IRS, Repo, and TermLoan; eight empty providers receive weaker checks and most non-empty providers are absent. Its pure-flatten assertion includes all schedule rows, contradicting the trait default's intentional non-cash filtering.

**Why it's slop:** A supposedly universal contract is maintained as a small hand-picked sample and encodes a second interpretation of dated flows. It cannot prevent the exact drift found in F1–F3 and F6.

**Proposed fix:** Make the provider matrix explicit and table-driven, covering every non-empty provider plus empty/error categories. Test post-start `as_of`, same-day boundaries, PIK/non-cash filtering, canonical kind/accrual metadata, ordering, and metadata union; make its flatten assertion use the shared cash-settlement predicate.

**Invariants touched:** parity, ISDA, precedence  
**Impact:** M  
**Risk:** L  
**Tier:** 1

---

### F8 — [Category: dead-public-api / test-only-production-surface]

**Files:**

- [`cashflows/src/builder/mod.rs:80`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/mod.rs:80)
- [`cashflows/src/builder/emission/credit.rs:71`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/emission/credit.rs:71)
- [`cashflows/src/builder/emission/credit.rs:221`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/emission/credit.rs:221)

**What:** `emit_default_on` and `emit_prepayment_on` are doc-hidden public exports with no production callers. They exist only for cashflows integration tests and valuations test support; real MBS, CMO, and structured-credit pathways use product-specific projection rows and `schedule_from_classified_flows`.

**Why it's slop:** A test-only alternative emission universe appears in the production API but is not consumed by the canonical builder.

**Proposed fix:** Move the tests into module-level unit tests or an explicit test-support feature, delete the public re-exports and orphan emitter module, and decide the separate serde compatibility fate of `DefaultEvent` without keeping unused executable APIs.

**Invariants touched:** serde, parity  
**Impact:** M  
**Risk:** M  
**Tier:** 3

---

### F9 — [Category: duplicate-computation / export-side-reprojection]

**Files:**

- [`cashflow_export.rs:221`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/cashflow_export.rs:221)
- [`cashflow_export.rs:225`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/cashflow_export.rs:225)
- [`cashflow_export.rs:427`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/cashflow_export.rs:427)
- [`mbs_passthrough/types.rs:175`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/mbs_passthrough/types.rs:175)
- [`cashflows/src/aggregation.rs:513`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/aggregation.rs:513)

**What:** The generic exporter first obtains the canonical provider schedule, then downcasts MBS and runs the projection engine again to recover SMM and balances. It also contains a hand-written per-row credit PV function documented as mirroring cashflows' `credit_adjusted_period_pv`.

**Why it's slop:** A read/export path owns a second full projection and a second financial kernel. The MBS calls can use different horizon arguments, and credit treatment can drift between aggregate pricing and exported rows.

**Proposed fix:** Make the MBS projection produce one artifact containing the canonical schedule plus optional row diagnostics and have both the provider/export path consume it once. Promote a focused, checked per-row credit-PV primitive from cashflows and delegate export calculations to it.

**Invariants touched:** Decimal, parity, precedence  
**Impact:** M  
**Risk:** M  
**Tier:** 4

---

### F10 — [Category: half-migration / parallel-date-generation]

**Files:**

- [`cashflows/src/builder/periods.rs:74`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/periods.rs:74)
- [`common_impl/parameters/trs_common.rs:120`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/parameters/trs_common.rs:120)
- [`term_loan/cashflows.rs:131`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/term_loan/cashflows.rs:131)
- [`revolving_credit/utils.rs:52`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/revolving_credit/utils.rs:52)
- [`commodity_swap/types.rs:412`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/commodity/commodity_swap/types.rs:412)
- [`structured_credit/simulation_engine.rs:850`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/structured_credit/pricing/simulation_engine.rs:850)

**What:** Legacy `build_dates` is gone and the cashflows compiler shares the same period engine, but several contractual cashflow paths still call core `ScheduleBuilder` directly. Term loan separately builds coupon, principal, and commitment-fee routes; revolving credit has a permissive calendar fallback that differs from the strict shared resolver. Meanwhile the intended `BuildPeriodsParams::from_schedule` bridge is unused.

**Why it's slop:** Date generation is mechanically shared at the lowest level but policy enrichment — accrual boundaries, payment adjustment, calendar resolution, and duplicate-date checks — remains product-owned.

**Proposed fix:** Migrate contractual grids that already own `ScheduleParams` through `BuildPeriodsParams::from_schedule` and `build_periods`; keep observation/quote grids on core `ScheduleBuilder` only where they are not cashflow periods. Consolidate term-loan coupon and commitment-fee periods in a dedicated regression slice, then either retain a used `from_schedule` bridge or delete it.

**Invariants touched:** ISDA, serde, precedence  
**Impact:** M  
**Risk:** M  
**Tier:** 4

---

### F11 — [Category: binding-drift / namespace-split]

**Files:**

- [`finstack-quant-wasm/exports/valuations.js:35`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/exports/valuations.js:35)
- [`finstack-quant-wasm/exports/valuations/instruments.js:10`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/exports/valuations/instruments.js:10)
- [`finstack-quant-py/parity_contract.toml:1146`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/parity_contract.toml:1146)
- [`finstack-quant-wasm/index.d.ts:1429`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/index.d.ts:1429)

**What:** WASM exposes the JSON-market route as `valuations.instrumentCashflowsJson` but the pre-parsed-market route as `valuations.instruments.instrumentCashflowsWithMarket`. The parity contract says instrument helpers live under `valuations.instruments` while preserving this split through an explicit Python-to-JS mapping.

**Why it's slop:** Two variants of one capability are separated by transport representation rather than domain. Both already call the same Rust function, so this is avoidable host topology drift rather than required computation diversity.

**Proposed fix:** Place both variants under `valuations.instruments`, update `index.d.ts`, the JS facade, and the parity contract in one breaking slice, and remove the root method instead of retaining a permanent alias.

**Invariants touched:** parity  
**Impact:** M  
**Risk:** M  
**Tier:** 4

---

### F12 — [Category: parallel-api / boundary-semantics]

**Files:**

- [`cashflows/src/builder/coupon_api.rs:460`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/coupon_api.rs:460)
- [`cashflows/src/builder/coupon_api.rs:481`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/coupon_api.rs:481)
- [`cashflows/src/json.rs:62`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/json.rs:62)
- [`cashflows/src/json.rs:101`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/json.rs:101)

**What:** Fixed step-ups retain two public builder/JSON models: `step_up_cf(StepUpCouponSpec)` and `fixed_stepup_decimal(FixedRateProgram)`. The second has no production valuation consumer and uses a different dated-boundary representation; arbitrary explicit windows are already covered by the canonical payment program.

**Why it's slop:** Two plausible public names encode overlapping fixed-rate programs with subtly different boundary semantics, so JSON callers must choose an implementation detail rather than one canonical concept.

**Proposed fix:** Keep `StepUpCouponSpec` as the canonical step-up representation. Translate any required compatibility input at the JSON boundary, use explicit fixed windows for truly arbitrary programs, and remove `fixed_stepup_decimal` plus `FixedRateProgram` in a breaking release.

**Invariants touched:** Decimal, serde, parity  
**Impact:** M  
**Risk:** H  
**Tier:** 4

---

### F13 — [Category: redundant-helper / defensive-boilerplate]

**Files:**

- [`cashflows/src/traits.rs:197`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/traits.rs:197)
- [`cashflows/src/builder/schedule.rs:73`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:73)
- [`cashflows/src/builder/schedule.rs:867`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:867)
- [`fx_forward/types.rs:742`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fx/fx_forward/types.rs:742)
- [`cds/types.rs:1130`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/credit_derivatives/cds/types.rs:1130)

**What:** `schedule_from_dated_flows` repeats empty/notional/constructor work rather than mapping once into the classified adapter; merge sorts and then `from_parts` sorts again; `sort_flows` is public only for a benchmark and one integration test. Valuations still repeats representation expressions, overwrites day count already passed to constructors, and pre-sorts immediately before `normalize_public` or canonical constructors.

**Why it's slop:** These are small remnants of the earlier migration that make readers question which step is authoritative.

**Proposed fix:** After F1, F5, and F6 land, run one mechanical cleanup: delegate the dated adapter, sort only in the canonical constructor, demote `sort_flows`, remove pre-sorts/day-count overwrites, and make representation a finalizer concern only.

**Invariants touched:** none  
**Impact:** L  
**Risk:** L  
**Tier:** 2 (Tier 3 only for demoting the public sorter)

---

### F14 — [Category: binding-policy-leak / wrapper-only]

**Files:**

- [`finstack-quant-py/src/bindings/valuations/mod.rs:117`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/src/bindings/valuations/mod.rs:117)
- [`finstack-quant-py/src/bindings/valuations/pricing.rs:169`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/src/bindings/valuations/pricing.rs:169)
- [`finstack-quant-py/finstack_quant/valuations/instruments/__init__.pyi:147`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/finstack_quant/valuations/instruments/__init__.pyi:147)
- [`finstack-quant-wasm/src/api/valuations/pricing.rs:190`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/src/api/valuations/pricing.rs:190)

**What:** Python reparses and pretty-serializes the canonical Rust result from `validate_instrument_json`, while WASM returns it directly. Python also silently defaults the cashflow-export model to `"discounting"`; canonical Rust and both WASM variants require an explicit model.

**Why it's slop:** Binding code owns byte-format and default-selection policy despite the rule that Rust is canonical. Neither behavior is a separate cashflow algorithm, but both create host-dependent results around the same cashflow route.

**Proposed fix:** Return the canonical Rust JSON string directly. Either require `model` explicitly in Python or define the default once in Rust and expose it identically in both hosts; update the stub and parity tests in the same slice.

**Invariants touched:** serde, parity  
**Impact:** L  
**Risk:** M  
**Tier:** 3/4

## Slop clusters

### Cluster A — enforce one provider boundary

**Includes findings:** F1, F2, F7, F13.

**Why it's a cluster:** Public lifecycle normalization, dated-flow derivation, and the contract suite describe one provider abstraction. Fixing them independently would either preserve override escape hatches or leave tests certifying the old semantics.

**Recommended consolidation:** Introduce the raw-to-public finalizer, make dated extraction derived and non-overridable, migrate the full provider inventory, then replace the sample contract suite with the inventory-driven matrix. Remove local filtering/sorting/representation boilerplate only after this boundary owns it.

### Cluster B — canonical schedule ownership and composition

**Includes findings:** F5, F6, F13.

**Why it's a cluster:** Public mutation and an underpowered merge helper are why product code still owns ordering and metadata reconciliation.

**Recommended consolidation:** First define schedule currency/notional semantics, then route serde and all constructors through one canonical constructor, introduce one metadata-preserving combiner, migrate IRS/Basis/FX/XCCY, and finally close direct flow mutation.

### Cluster C — preserve financial semantics once

**Includes findings:** F3, F4, F9.

**Why it's a cluster:** These findings lose or independently reconstruct economic meaning after shared schedules exist: kind/accrual classification, mortality conversion, MBS diagnostics, and credit PV.

**Recommended consolidation:** Preserve classification at emission time, centralize annual/monthly rate policy in cashflows, return MBS schedule plus diagnostics from one projection, and expose the focused row-PV primitive needed by the exporter.

### Cluster D — finish period and builder migration

**Includes findings:** F8, F10, F12.

**Why it's a cluster:** The residual public emitters and alternate step-up/date programs are the remaining API fossils around an otherwise consolidated builder/period engine.

**Recommended consolidation:** Migrate contractual grids through `build_periods`, choose one step-up representation, then delete test-only emitters and unused bridges once their call sites are gone.

### Cluster E — binding topology and policy

**Includes findings:** F11, F14.

**Why it's a cluster:** Rust computation is already shared; only host namespace, formatting, and default policy remain inconsistent.

**Recommended consolidation:** Treat this as one parity-aware breaking slice: move both WASM cashflow exporters under `valuations.instruments`, remove Python-owned defaults/serialization, update stubs/types/facades/contracts together, and run the structural and behavioral parity suites.

## Binding drift

**Structural drift:**

- F11: WASM splits one valuation cashflow capability between `valuations` and `valuations.instruments`.
- The four cashflow JSON functions themselves are structurally aligned across Rust, Python, WASM, facades, and the parity contract.

**Logic drift (logic that leaked into bindings):**

- No binding constructs, normalizes, sorts, classifies, accrues, discounts, or aggregates cashflows independently.
- F14 is the only residual host-owned policy: Python pretty-reserializes canonical JSON and chooses a model default.
- Python's DataFrame conversion and WASM's reusable `Market` handle are justified presentation/transport layers, not alternate financial pathways.

**Parity contract impact:**

- F11 requires moving `instrumentCashflowsJson` under the nested `instruments` facade and updating `python_path_js_map`, `nested_exports`, `index.d.ts`, and facade tests.
- F14 requires a synchronized Python signature/stub change if `model` becomes explicit.
- Removing fixed-rate-program compatibility (F12), public emitters (F8), schedule fields (F5), or legacy input adapters later requires explicit semver/schema review.

## Hazards (non-simplicity problems discovered incidentally)

- **H1 —** [`fi_trs/types.rs:287`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/fi_trs/types.rs:287) ignores `as_of`, so a post-start query can return historical payment dates in direct violation of the provider contract. Severity: **high**.
- **H2 —** [`commodity_swap/types.rs:581`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/commodity/commodity_swap/types.rs:581), [`cms_swap/types.rs:655`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/cms_swap/types.rs:655), and [`inflation_swap/types.rs:544`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/inflation_swap/types.rs:544) label coupon payments as principal. Shared outstanding-balance and WAL analytics therefore consume them as notional movements. Severity: **high**.
- **H3 —** [`cashflow_export.rs:268`](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/cashflow_export.rs:268) rejects mixed-currency schedules even though the generic cashflow row model and product inventory include FX Swap/XCCY. The related Python/WASM exporter cannot represent those valid providers. Severity: **medium**.
- **H4 —** Raw generators in IRS, CMS, YoY inflation, CDS index, MBS, structured credit, and XCCY MtM use a strict post-`as_of` boundary before shared finalization. Same-day contractual cashflows can disappear despite the trait's `date >= as_of` rule. Severity: **medium**.

## Scorecard

- API simplicity: **3/5** — the center is coherent, but provider finalization, composition, and step-up inputs still expose competing routes.
- Redundancy level: **3/5** — major prior duplication is gone; mortality, export PV/projection, date policy, and small finalization copies remain.
- Consistency: **3/5** — most providers use canonical constructors, but lifecycle, classification, and multi-leg assembly remain product-dependent.
- Binding hygiene: **4/5** — host bindings are thin; only namespace/default/serialization drift remains.
- Maintainability: **3/5** — shared primitives are strong, but public schedule mutation and incomplete provider contract coverage make regressions too easy.

**Overall:** **3.2/5**

## Top 5 highest-leverage changes

1. **F1 + F2** — enforce one raw-to-public provider boundary and make dated flows purely derived. Removes distributed lifecycle policy across 33 non-empty providers.
2. **F5 + F6** — canonicalize construction/serde and provide one metadata-preserving mixed-currency combiner. Eliminates every product-local schedule assembly path.
3. **F3** — preserve classified flow/accrual semantics for commodity, CMS, and inflation swaps. Removes lossy remapping and fixes shared balance/WAL behavior.
4. **F4** — make cashflows the sole owner of annual/monthly mortality conversions. Deletes two independent CPR/SMM implementations and makes clamping explicit.
5. **F9** — produce MBS schedule/diagnostics once and share the row-PV kernel. Removes a second projection and a copied financial formula from the binding export path.

## Next steps

Proceed to Phase 2 (Plan) by turning Clusters A–E into dependency-ordered PR-sized slices. Cluster A should go first because it defines the public contract that every later construction, classification, and composition change must satisfy; Cluster B should follow before the numerical/export cleanup.

**Awaiting user input:** confirm the finding priorities or request implementation planning for the remaining clusters.
