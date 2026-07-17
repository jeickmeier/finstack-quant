# Audit Report: `finstack-quant::{core,cashflows,valuations}`

**Scope:** all Rust source under `finstack-quant/core/src`, `finstack-quant/cashflows/src`, and `finstack-quant/valuations/src`, plus related Python/WASM bindings, stubs, facades, schemas, parity contracts, tests, and examples.
**Bindings in scope:**

- `finstack-quant-py/src/bindings/{core,cashflows,valuations}` — exists
- `finstack-quant-wasm/src/api/{core,cashflows,valuations}` — exists
- Python stubs/package exports, JS facades, `index.d.ts`, generated WASM types, and `parity_contract.toml` — exists

**Date:** 2026-07-12
**Auditor:** finstack-simplify / Phase 1 (read-only)

## Executive summary

The audit covered 182 core files/93,369 LOC, 33 cashflows files/16,974 LOC, 928 valuations files/330,498 LOC, plus 19,098 LOC of target binding Rust. It found 33 current opportunities: 23 high-impact, nine medium-impact, and one low-impact cleanup group.

The largest problems are competing representations rather than isolated cosmetic wrappers: three market-dependency APIs, non-atomic cashflow rows, duplicated schedule conventions, a half-migrated pricing-override bag, and independent copies of financial kernels. Highest-leverage move: make cashflow rows atomic and finish the valuations dependency migration before further API growth.

This is a static Phase 1 audit; no code changed. Baseline checks passed:

- Core: 1,372 passed, 1 ignored
- Cashflows: 402 passed, 5 ignored
- Python structural parity: 392 passed
- Worktree remained clean

Previously fixed F1–F25 items were rechecked and excluded. These include Tenor invalid states, legacy schedule-policy runtime booleans, unused local-vol/FX-delta arguments, vanna-volga remnants, structured-credit scenario-tree prototypes, and registry overwrite ambiguity.

## Surface area inventory

**Capability — market-data mutation**

- Canonical entry point proposed: mutable `MarketContext::insert_*_mut` family at [context/mod.rs:L1711](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/context/mod.rs:1711)
- Alternate pathways found:
  - Consuming `insert_*` family at [context/mod.rs:L1395](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/context/mod.rs:1395)
  - Python `mem::take` wrappers despite mutable kernels at [context.rs:L64](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/src/bindings/core/market_data/context.rs:64)

**Capability — interpolation construction**

- Canonical entry point proposed: enum/static `InterpStyle::build_enum` at [interp/types.rs:L373](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/interp/types.rs:373)
- Alternate pathways found:
  - Boxed `InterpStyle::build` at [interp/types.rs:L297](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/interp/types.rs:297)
  - Public dynamic `InterpFn` abstraction at [interp/traits.rs:L49](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/interp/traits.rs:49)

**Capability — cashflow construction**

- Canonical entry point identified: `CashFlowSchedule::builder`
- Alternate pathways found:
  - `CashflowScheduleBuildSpec` JSON representation at [json.rs:L23](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/json.rs:23)
  - Low-level public date, period, rate, sorting, and emission modules at [builder/mod.rs:L48](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/mod.rs:48)
  - Instrument-specific schedule generation and sorting inside valuations

**Capability — schedule generation**

- Canonical entry point proposed: one enriched `build_schedule(BuildPeriodsParams)`
- Alternate pathways found:
  - `build_dates` at [date_generation.rs:L261](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/date_generation.rs:261)
  - `build_periods` and `build_single_period` at [periods.rs:L150](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/periods.rs:150)

**Capability — annual/monthly credit-rate conversion**

- Canonical entry point proposed: checked cashflows kernel at [credit_rates.rs:L47](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/credit_rates.rs:47)
- Alternate pathways found:
  - MBS copy at [prepayment.rs:L197](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/mbs_passthrough/prepayment.rs:197)
  - Structured-credit clamping copy at [rates.rs:L40](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/structured_credit/utils/rates.rs:40)

**Capability — instrument market dependencies**

- Canonical entry point identified: `Instrument::market_dependencies` at [instrument.rs:L908](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/traits/instrument.rs:908)
- Alternate pathways found:
  - `CurveDependencies` at [curve_dependencies.rs:L32](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/traits/curve_dependencies.rs:32)
  - `EquityDependencies` at [equity_dependencies.rs:L31](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/traits/equity_dependencies.rs:31)

**Capability — pricing configuration**

- Canonical entry points proposed: focused `InstrumentPricingOverrides`, `MetricPricingOverrides`, and `ScenarioPricingOverrides`
- Alternate pathway found:
  - Flattened catch-all `PricingOverrides` and mirrored builders at [pricing_overrides.rs:L789](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/pricing_overrides.rs:789)

**Capability — option pricing kernels**

- Canonical entry point proposed: checked core Black/Bachelier primitives
- Alternate pathways found:
  - Valuations Black-Scholes/Black-76 at [vanilla.rs:L191](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/models/closed_form/vanilla.rs:191)
  - Valuations Bachelier at [normal.rs:L33](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/models/volatility/normal.rs:33)

**Capability — host-language pricing**

- Canonical entry point identified: generic JSON pricing functions
- Alternate pathways found:
  - WASM FX JSON-holding object facade at [fx.rs:L89](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/src/api/valuations/fx.rs:89)
  - WASM string-market and parsed-`Market` overloads at [pricing.rs:L131](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/src/api/valuations/pricing.rs:131)

## Findings

### F1 — [Category: dead-public-type / parallel-api]

**Files:**

- [common parameters:L164-L500](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/parameters/market.rs:164)
- [equity option parameters:L8-L74](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/equity/equity_option/parameters.rs:8)
- [cap/floor parameters:L14-L110](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/cap_floor/parameters.rs:14)

**What:** Public common `EquityOptionParams`, `FxOptionParams`, and `CapFloorParams` compete with the parameter types actually accepted by the corresponding instruments. The common equity/cap types have no production consumers; the cap definitions also disagree materially on typed frequency, Decimal use, calendar, stub, and BDC.

**Why it's slop:** This is the parallel-public-type pattern: plausible DTOs were exported without becoming canonical construction inputs.

**Proposed fix:** Delete the disconnected types and root re-exports. If host request DTOs are required, name them `*Request` and add checked conversions into the real construction types.

**Invariants touched:** Decimal, serde, parity
**Impact:** H
**Risk:** M
**Tier:** 3

---

### F2 — [Category: parallel-api / duplicate-computation]

**Files:**

- [SABR calibration:L196-L681](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/models/volatility/sabr/calibration.rs:196)
- [SABR derivative route:L21-L145](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/models/volatility/sabr_derivatives.rs:21)
- [Python SABR validation:L383-L455](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/src/bindings/valuations/sabr.rs:383)

**What:** SABR exposes seven public calibration variants and a separate roughly 540-line derivative calibration family. No production caller uses the derivative route; tests require it to agree with the standard route. Bindings additionally repeat vector-length validation already owned by Rust.

**Why it's slop:** Two algorithms implement the same public capability and are maintained mainly by equivalence tests.

**Proposed fix:** Retain one calibrator with focused shift/ATM options. Privatize or delete derivative-only DTOs and delegate binding validation to Rust.

**Invariants touched:** negative-rate shift, objective weights, ATM pinning
**Impact:** H
**Risk:** M
**Tier:** 3

---

### F3 — [Category: temporal-builder / swallowed-error]

**Files:**

- [coupon builder:L19-L82](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/coupon_api.rs:19)
- [principal builder:L11-L55](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/principal.rs:11)
- [builder terminal:L334-L399](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/orchestrator.rs:334)

**What:** Coupon calls before `principal()` record an error and drop the leg; a later `principal()` clears that error, allowing a successful schedule with missing economics. `amortization()` before principal silently does nothing.

**Why it's slop:** Builder correctness depends on undocumented temporal state, sticky errors, and a reset operation that preserves only part of accumulated state.

**Proposed fix:** Require principal/horizon at builder construction, or make fallible mutators return `Result`. Remove silent no-ops and error clearing. Rename `build_with_curves` only as part of this redesign.

**Invariants touched:** precedence, contractual completeness
**Impact:** H
**Risk:** M
**Tier:** 3

---

### F4 — [Category: duplicate-policy / parallel-sort]

**Files:**

- [canonical cashflow order:L27-L88](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:27)
- [revolving-credit order:L30-L53](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/revolving_credit/cashflow_engine.rs:30)

**What:** Revolving credit has a local cashflow rank that disagrees with the canonical schedule rank: PIK precedes amortization locally, while the canonical engine applies amortization first. Its tie-breaking is also incomplete.

**Why it's slop:** Ordering policy is independently encoded in an instrument implementation despite a canonical schedule sorter already existing.

**Proposed fix:** Build through a metadata-safe canonical constructor and delete instrument-local rank/sort functions.

**Invariants touched:** precedence, balance replay, deterministic serde
**Impact:** H
**Risk:** M
**Tier:** 3

---

### F5 — [Category: duplicate-financial-kernel]

**Files:**

- [cashflows CPR/SMM:L47-L118](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/credit_rates.rs:47)
- [MBS copy:L197-L227](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/mbs_passthrough/prepayment.rs:197)
- [structured-credit rates:L40-L180](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/structured_credit/utils/rates.rs:40)

**What:** CPR/SMM and CDR/MDR conversions exist in three copies. Cashflows and MBS reject invalid inputs; structured credit silently clamps them. PSA/SDA seasoning behavior is likewise divided between hard-coded cashflow logic and a structured-credit assumptions registry.

**Why it's slop:** The same financial transformation has multiple owners and incompatible boundary policy.

**Proposed fix:** Create one numerically stable checked annual/monthly mortality kernel. Any clamping must be an explicitly named caller-boundary operation; move standard seasoning parameters to one dependency-neutral owner.

**Invariants touched:** rate units, numerical stability, financial conventions
**Impact:** H
**Risk:** M
**Tier:** 4

---

### F6 — [Category: parallel-api]

**Files:**

- [MarketContext consuming API:L1395-L1708](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/context/mod.rs:1395)
- [MarketContext mutable API:L1711-L1835](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/context/mod.rs:1711)
- [Python context wrappers:L64-L138](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/src/bindings/core/market_data/context.rs:64)

**What:** `MarketContext` has complete consuming and mutable copies of its insertion, FX, collateral, and index APIs. The mutable family was introduced for bindings, but Python still uses the `mem::take` dance the mutable methods were meant to eliminate.

**Why it's slop:** This is a full mirrored API family whose wrappers differ only in receiver ownership.

**Proposed fix:** Make `&mut self` canonical. Retain only deliberate `with_*` consuming conveniences where chaining is materially useful, and have bindings call mutable kernels directly.

**Invariants touched:** FX, credit-index rebinding, cache invalidation
**Impact:** H
**Risk:** M
**Tier:** 3

---

### F7 — [Category: binding-parallel-api]

**Files:**

- [WASM math JsValue API:L88-L130](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/src/api/core/math.rs:88)
- [WASM math typed-array API:L132-L167](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/src/api/core/math.rs:132)
- [WASM math exports:L23-L50](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/exports/core.js:23)

**What:** WASM exports `mean` and `meanArray`, `variance` and `varianceArray`, and equivalent pairs for covariance, correlation, quantile, compensated sums, and consecutive runs. Cholesky likewise has nested and flat public variants.

**Why it's slop:** Host transport shape created a second naming universe for identical calculations.

**Proposed fix:** Keep one typed-array Rust primitive per calculation. Let the JS facade accept the existing `NumericArray = number[] | Float64Array` union and normalize nested matrix inputs there.

**Invariants touched:** parity, floating-point identity
**Impact:** H
**Risk:** M
**Tier:** 3

---

### F8 — [Category: parallel-api / unused-abstraction]

**Files:**

- [boxed interpolation factory:L297-L370](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/interp/types.rs:297)
- [static interpolation factory:L373-L431](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/interp/types.rs:373)
- [InterpFn trait:L49](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/interp/traits.rs:49)

**What:** All five interpolation variants are constructed twice: once into `Box<dyn InterpFn>` and once into the enum used by every production curve. The boxed path has no production workspace consumer.

**Why it's slop:** A speculative dynamic-dispatch abstraction duplicates the real static-dispatch implementation.

**Proposed fix:** Keep generic strategies and one enum-backed construction kernel. Remove the boxed factory/trait, or implement a compatibility adapter from the canonical enum during deprecation.

**Invariants touched:** extrapolation, monotonicity, numerical outputs
**Impact:** H
**Risk:** M
**Tier:** 2/3

---

### F9 — [Category: parallel-domain-type]

**Files:**

- [instrument BarrierType:L11-L51](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/exotics/barrier_option/types.rs:11)
- [closed-form BarrierType:L247-L258](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/models/closed_form/barrier.rs:247)
- [autocall payoff duplicates:L44-L64](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/equity/autocallable/types.rs:44)
- [Position duplicates:L92-L140](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/parameters/market.rs:92)

**What:** Several domain classifications have multiple public representations:

- Four up/down × in/out barrier enums across instruments, MC, trees, and closed forms
- Duplicate autocallable `FinalPayoffType`
- Duplicate cliquet `CliquetPayoffType`
- Two incompatible long/short `Position` enums
- `AgencyProgram::Gnma` as a runtime duplicate of `GnmaII`

**Why it's slop:** Equivalent finite state spaces require exhaustive conversion glue and drift in serde/parser behavior.

**Proposed fix:** Establish one dependency-neutral type for each domain concept. Preserve old paths with aliases or checked conversions during migration; do not merge genuinely different credit-barrier concepts.

**Invariants touched:** serde, payoff dispatch, barrier inequalities, position sign
**Impact:** H
**Risk:** M/H
**Tier:** 3/4

---

### F10 — [Category: zero-state-wrapper]

**Files:**

- [CalendarRegistry:L17-L77](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/dates/calendar/registry.rs:17)
- [generated calendar APIs:L95-L115](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/dates/calendar/mod.rs:95)
- [DayCount registry lifetime:L187-L227](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/dates/daycount.rs:187)

**What:** `CalendarRegistry` contains only `PhantomData`; `global()` initializes no state and `resolve_str` delegates to an existing generated free function. It nevertheless propagates singleton and lifetime plumbing across all three crates and bindings.

**Why it's slop:** A registry abstraction implies configurable state that does not exist.

**Proposed fix:** Standardize on `calendar_by_id`, `available_calendars`, and a typed-ID helper. Remove the singleton, lifetime parameter, and the auxiliary weekends-only ownership wrapper.

**Invariants touched:** ISDA, calendars, serde
**Impact:** H
**Risk:** M/H
**Tier:** 4

---

### F11 — [Category: half-migration / parallel-trait]

**Files:**

- [CurveDependencies:L32-L35](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/traits/curve_dependencies.rs:32)
- [MarketDependencies:L54-L83](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/dependencies.rs:54)
- [DV01 legacy consumer:L270-L285](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/metrics/sensitivities/dv01.rs:270)
- [EquityOption triple implementation:L187-L213](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/equity/equity_option/types.rs:187)

**What:** Valuations maintains `CurveDependencies`, `EquityDependencies`, and the declared-canonical `Instrument::market_dependencies`. There are roughly 70 curve, nine equity, and 44 market-dependency definitions, while metrics consume different generations.

**Why it's slop:** This is a broad half-migration with adapters and repeated instrument declarations.

**Proposed fix:** First enrich `MarketDependencies` with typed volatility requests and reference strikes. Then migrate all generic metric bounds and delete both legacy traits. Do not rely on an empty default for production instruments.

**Invariants touched:** curve roles, factor discovery, vega localization
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F12 — [Category: parallel-state]

**Files:**

- [CashFlowMeta/CashFlowSchedule:L200-L248](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:200)
- [metadata-safe sorting:L682-L894](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:682)
- [term-loan append/sort:L437-L450](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/term_loan/cashflows.rs:437)

**What:** `flows`, `accrual_periods`, and `accrual_day_counts` are independently mutable vectors. More than 200 lines of filtering/sorting/merge code preserve indices manually. A current term-loan path can append/sort flows without metadata and later cause all aligned accrual metadata to be replaced by `None`.

**Why it's slop:** One logical row is represented as three collections, making invalid states easy and routine transformations fragile.

**Proposed fix:** Introduce one atomic scheduled-flow record containing the flow, accrual period, day count, and eventually resolved rate decomposition. Make schedule storage private and version the serde migration.

**Invariants touched:** serde, day count, precedence, balance replay
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F13 — [Category: duplicate-schema / lossy-conversion]

**Files:**

- [ScheduleParams:L12-L54](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/specs/schedule.rs:12)
- [coupon schedule copies:L81-L159](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/specs/coupon.rs:81)
- [step-up schedule copies:L647-L719](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/specs/coupon.rs:647)

**What:** `FixedCouponSpec`, `FloatingCouponSpec`, and `StepUpCouponSpec` duplicate subsets of `ScheduleParams`. Conversion helpers hard-code `adjust_accrual_dates = false`, so full-horizon and JSON APIs cannot express behavior available in lower-level window APIs.

**Why it's slop:** Repeated configuration types encode the same convention with asymmetric fields and lossy adapters.

**Proposed fix:** Embed one `ScheduleParams`, using temporary serde flattening if necessary. Separate floating-index terms from payment-schedule terms and replace the three step-up pathways with one explicit coupon program.

**Invariants touched:** ISDA, serde, calendars, stubs
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F14 — [Category: half-migration / parallel-api]

**Files:**

- [build_dates:L228-L315](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/date_generation.rs:228)
- [build_periods:L20-L235](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/periods.rs:20)
- [builder exports:L70-L74](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/mod.rs:70)

**What:** `build_dates` returns a partially enriched schedule, while `build_periods` layers on adjusted accrual boundaries, reset dates, and year fractions. Production valuations callers use both.

**Why it's slop:** A migration introduced a richer schedule type without retiring the lower-level public construction pathway.

**Proposed fix:** Create one `build_schedule(BuildPeriodsParams) -> PeriodSchedule`; make the single-period case delegate to it, migrate callers, and privatize old entry points.

**Invariants touched:** calendars, stubs, reset/payment lag, day count
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F15 — [Category: binding-drift / parallel-schema]

**Files:**

- [cashflow JSON spec:L23-L45](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/json.rs:23)
- [rich Rust coupon APIs:L378-L724](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/coupon_api.rs:378)
- [cashflow parity contract:L200-L249](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/parity_contract.toml:200)

**What:** Rust supports payment programs, step-up coupons, floating-margin steps, and fixed-to-floating programs. The JSON/Python/WASM product supports only fixed/floating coupons, fees, and principal events.

**Why it's slop:** Rust and bindings are evolving against different configuration models, creating a second product rather than a thin conversion layer.

**Proposed fix:** Define one serializable coupon/payment-program schema and make both the Rust builder and bindings compile it.

**Invariants touched:** serde, parity, contract economics
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F16 — [Category: schema-ownership]

**Files:**

- [valuations cashflow schema loader:L204-L241](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/schema.rs:204)
- [schema generator:L106-L122](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/bin/gen_schemas.rs:106)
- [stale coupon schema:L88](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/schemas/cashflow/1/coupon_specs.schema.json:88)

**What:** Cashflows owns the canonical types and JSON bridge, but valuations owns and publishes their schemas. The schema still references the old valuations namespace and omits newer coupon surfaces.

**Why it's slop:** Ownership is inverted, so schema maintenance depends on a downstream crate.

**Proposed fix:** Generate and store cashflow schemas beside the cashflows crate. Valuations should consume the published resource rather than own it.

**Invariants touched:** serde, schema IDs, parity
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F17 — [Category: half-migration / configuration-sprawl]

**Files:**

- [focused overrides:L548-L781](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/pricing_overrides.rs:548)
- [catch-all overrides:L789-L850](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/pricing_overrides.rs:789)
- [mirrored builders:L884-L1148](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/pricing_overrides.rs:884)
- [transitional trait hooks:L353-L410](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/traits/instrument.rs:353)

**What:** Focused override types now exist, but instruments still store a flattened catch-all, duplicate its builders, and implement transitional full-bag hooks. At least 34 instruments retain direct `PricingOverrides` fields.

**Why it's slop:** The migration added focused types without removing the old runtime representation.

**Proposed fix:** Retain flattening only in a wire adapter. Store focused runtime state and pass metric/scenario controls through their owning pipelines; then remove full-bag hooks and forwarding builders.

**Invariants touched:** serde, bump units, deterministic seeds, scenario precedence
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F18 — [Category: duplicate-financial-kernel]

**Files:**

- [core Black kernel:L110-L391](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/volatility/pricing/black.rs:110)
- [core Bachelier:L63-L151](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/volatility/pricing/bachelier.rs:63)
- [valuations vanilla formulas:L191-L530](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/models/closed_form/vanilla.rs:191)
- [valuations Bachelier:L33-L90](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/models/volatility/normal.rs:33)

**What:** Core and valuations independently implement Black-Scholes, Black-76, Bachelier, d1/d2, Greeks, and geometric-Asian primitives. Degenerate ATM Black-76 d1 behavior already differs: core returns the digital limit, valuations returns zero.

**Why it's slop:** Financial mathematics with sensitive boundary conventions has multiple canonical-looking owners.

**Proposed fix:** Core owns unit-annuity states, prices, and Greeks. Valuations dispatches option type and applies DF/annuity/notional. Select and document degenerate-limit behavior before deleting copies.

**Invariants touched:** zero-vol/expiry limits, Greek units, put-call parity
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F19 — [Category: parallel-pipeline]

**Files:**

- [validation contract:L497-L518](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/traits/instrument.rs:497)
- [price_with_metrics:L852-L888](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/traits/instrument.rs:852)
- [registry dispatch:L282-L330](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/pricer/registry.rs:282)
- [generic pricer:L65-L99](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/common_impl/pricing/generic.rs:65)

**What:** `value` validates, while the canonical direct-Rust `price_with_metrics` route reaches `base_value` through the registry without calling `validate_for_pricing`.

**Why it's slop:** Validation, scenario application, effective-date resolution, PV rounding, and metric orchestration are split across overlapping pricing entry points.

**Proposed fix:** Create one internal lifecycle: validate → resolve as-of → base/raw PV → scenario → details → metrics → metadata. Every Rust and binding entry point delegates to it.

**Invariants touched:** validation, scenario exactly once, raw PV, result stamping
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F20 — [Category: parallel-construction / binding-logic]

**Files:**

- [delta-surface builder:L119-L238](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/surfaces/delta_vol_surface.rs:119)
- [actual FX delta surface:L155-L226](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/surfaces/fx_delta_vol_surface.rs:155)
- [Python optional-wing branch:L193-L252](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/src/bindings/core/market_data/curves/surfaces.rs:193)
- [WASM optional-wing branch:L701-L770](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/src/api/core/market_data.rs:701)

**What:** `FxDeltaVolSurfaceBuilder` builds a generic `VolSurface`, while the named `FxDeltaVolSurface` has a separate constructor family. Python and WASM independently implement optional 10-delta selection and guard a Rust pillar accessor that can panic.

**Why it's slop:** Construction policy and safety checks live in three layers around two related Rust types.

**Proposed fix:** One builder should build `FxDeltaVolSurface`; keep generic conversion private and expose one fallible pillar accessor used by both bindings.

**Invariants touched:** FX, wing ordering, interpolation, parity
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F21 — [Category: no-op-api / compatibility-fossil]

**Files:**

- [EvalOpts cache field:L36-L80](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/expr/eval.rs:36)
- [no-op cache methods:L218-L235](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/expr/eval.rs:218)
- [serialized cache strategy:L317-L353](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/expr/dag.rs:317)

**What:** Expression cache configuration remains public and serialized even though caching was removed. `with_cache` returns `self`, `has_cache` always returns false, and execution plans compute unused cache recommendations.

**Why it's slop:** Removed behavior survives as a visible subsystem rather than a private deserialization compatibility shim.

**Proposed fix:** Stop emitting cache fields and remove runtime APIs. If old payload support is required, consume old fields through private wire types.

**Invariants touched:** serde
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F22 — [Category: try-shadow]

**Files:**

- [Money constructors:L210-L248](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/money/types.rs:210)
- [Rate constructors:L173-L229](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/types/rates.rs:173)
- [Percentage constructors:L641-L670](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/types/rates.rs:641)
- [panicking conversions:L318-L324](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/types/rates.rs:318)

**What:** Core value types retain panicking and fallible constructor shadows plus backward-compatible panicking `From<f64>` implementations. Bindings already use the fallible paths.

**Why it's slop:** Every call site must choose between nearly identical APIs based on hidden trust assumptions.

**Proposed fix:** Make fallible construction canonical. Restrict trusted construction to private helpers or explicitly named `_unchecked` operations and stage removal of panicking conversions.

**Invariants touched:** Decimal, finite values, serde
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F23 — [Category: parallel-state / legacy-field]

**Files:**

- [arbitrage config:L69-L85](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/arbitrage/mod.rs:69)
- [forward precedence:L170-L190](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/arbitrage/mod.rs:170)
- [Python arguments:L190-L227](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/src/bindings/core/market_data/arbitrage.rs:190)

**What:** Arbitrage configuration stores both scalar `forward` and vector `forward_prices`, with precedence and broadcast logic repeated by the grid adapter and exposed in Python.

**Why it's slop:** One conceptual input has two runtime fields and multiple normalization locations.

**Proposed fix:** Use one normalized `ForwardPrices` representation. Accept the scalar legacy form only at serde/binding boundaries.

**Invariants touched:** per-expiry forwards, local-vol density, serde
**Impact:** H
**Risk:** H
**Tier:** 4

---

### F24 — [Category: duplicate-state-machine / recomputation]

**Files:**

- [cashflow balance APIs:L438-L679](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/schedule.rs:438)
- [DataFrame replay:L521-L596](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/dataframe.rs:521)
- [DataFrame floating reconstruction:L95-L153](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/dataframe.rs:95)
- [canonical projection:L414-L459](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/rate_helpers.rs:414)

**What:** Cashflow analytics repeatedly reconstructs information already implied during schedule construction:

- An incomplete `outstanding_path_per_flow` beside canonical `outstanding_by_date`
- An independent DataFrame balance state machine
- DataFrame base-rate reconstruction using reset-to-payment rather than canonical fixed-tenor projection
- Multiple WAL implementations with different classification/error policies

**Why it's slop:** Reporting and metrics reverse-engineer contract state instead of consuming canonical annotated rows.

**Proposed fix:** One balance replay primitive should produce pre/post states. Carry resolved index-rate decomposition in the scheduled row and derive DataFrame/WAL views from canonical classified flows.

**Invariants touched:** Decimal, day count, PIK ordering, reporting
**Impact:** M
**Risk:** H
**Tier:** 4

---

### F25 — [Category: half-migration / duplicate-state]

**Files:**

- [swaption legacy fields:L45-L60](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/swaption/types/swaption.rs:45)
- [swaption full-leg fields:L98-L113](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/swaption/types/swaption.rs:98)
- [fallback leg synthesis:L384-L505](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/swaption/types/swaption.rs:384)

**What:** Swaption stores legacy frequency/day-count scalars alongside optional complete fixed/float leg specifications, validates their combination, and synthesizes legs through fallback accessors.

**Why it's slop:** Migration compatibility is represented in runtime state rather than a deserialization adapter.

**Proposed fix:** Make full leg specs canonical and convert legacy wire fields into them during deserialization.

**Invariants touched:** serde, curve roles, compounded overnight legs, calendars
**Impact:** M
**Risk:** H
**Tier:** 4

---

### F26 — [Category: binding-logic / wrapper-only]

**Files:**

- [Python analytic dispatch:L247-L433](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/src/bindings/valuations/analytic.rs:247)
- [WASM analytic dispatch:L191-L330](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/src/api/valuations/analytic.rs:191)
- [WASM FX object facade:L89-L367](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/src/api/valuations/fx.rs:89)

**What:** Python and WASM independently route barrier, Asian, lookback, quanto, and option-style strings to Rust functions. WASM additionally exposes ten classes that retain validated instrument JSON strings and duplicate generic price/metric entry points.

**Why it's slop:** Bindings own financial dispatch policy and a WASM-only object model instead of wrapping canonical Rust types or one generic instrument handle.

**Proposed fix:** Add checked typed request facades in Rust. Bindings perform only host conversion. Replace the FX macro universe with the generic JSON API or one canonical `Instrument` handle.

**Invariants touched:** parity, defaults, accepted labels, error mapping
**Impact:** M
**Risk:** M
**Tier:** 3

---

### F27 — [Category: parallel-wire-format / namespace-drift]

**Files:**

- [cashflow raw/envelope APIs:L76-L100](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/json.rs:76)
- [build/validate pairs:L240-L334](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/json.rs:240)
- [bond helper canonical owner:L395-L434](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/lib.rs:395)
- [Python foreign export:L169-L204](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/src/bindings/cashflows/mod.rs:169)

**What:** Cashflows exposes raw and versioned-envelope build/validate pairs, but downstream functions accept only raw schedules. The envelope always stamps default rounding context. Separately, a valuations bond constructor is exported only under `cashflows`.

**Why it's slop:** The boundary has two non-composable wire formats plus a cross-crate namespace exception.

**Proposed fix:** Choose one binding wire format or remove the envelope. If retained, pass real provenance. Move bond construction to `valuations.instruments` or create a first-class custom-cashflow instrument there.

**Invariants touched:** serde, schema versioning, parity
**Impact:** M
**Risk:** M
**Tier:** 3/4

---

### F28 — [Category: construction-sprawl / binding-parallel-api]

**Files:**

- [VolSurface construction:L971-L1115](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/surfaces/vol_surface.rs:971)
- [Python ForwardCurve constructors:L105-L169](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-py/src/bindings/core/market_data/curves/forward.rs:105)
- [WASM ForwardCurve constructors:L274-L308](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/src/api/core/market_data.rs:274)
- [WASM market pricing overloads:L215-L268](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/src/api/valuations/pricing.rs:215)

**What:** Several host-facing types expose alternate construction/transport forms with identical semantics: Python `ForwardCurve(...)` versus `from_knots`, WASM positional constructor versus `fromOptions`, string-market pricing versus parsed-`Market` pricing, and several VolSurface grid/row/builder forms.

**Why it's slop:** Convenience variants have become equal-status APIs, expanding stubs, facades, contracts, and tests.

**Proposed fix:** Select one canonical typed/options-object construction path per host. Put array-shape normalization and string-to-handle adaptation in facades rather than exporting new calculation names.

**Invariants touched:** parity, serde, interpolation
**Impact:** M
**Risk:** M
**Tier:** 3

---

### F29 — [Category: temporal-builder]

**Files:**

- [HierarchyBuilder state:L17-L20](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/hierarchy/builder.rs:17)
- [silent tag/curve operations:L68-L91](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/hierarchy/builder.rs:68)

**What:** `HierarchyBuilder` stores a current path, defers errors, and silently discards `tag` or `curve_ids` calls made before `add_node`.

**Why it's slop:** Correctness depends on call order rather than the type or method signature.

**Proposed fix:** Use an atomic `add_node(path, curve_ids, tags)` operation or an explicit node sub-builder.

**Invariants touched:** hierarchy targeting, serde
**Impact:** M
**Risk:** M
**Tier:** 3

---

### F30 — [Category: dead-code / migration-scaffold]

**Files:**

- [MarketContextSplit:L242-L273](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/calibration/api/market_datum.rs:242)
- [v2 rejection:L141-L162](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/calibration/api/validate.rs:141)

**What:** Public `MarketContextSplit` converts a legacy snapshot into v3 envelope inputs, but all external workspace use is test support and v2 envelopes are explicitly rejected.

**Why it's slop:** A completed migration scaffold remains in the primary public API.

**Proposed fix:** Move it to test/migration support or a clearly time-boxed compatibility module.

**Invariants touched:** serde
**Impact:** M
**Risk:** L/M
**Tier:** 3

---

### F31 — [Category: configuration-leakage]

**Files:**

- [range-accrual model contract:L62-L80](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/range_accrual/pricer.rs:62)
- [seed-driven model selection:L355-L377](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/range_accrual/pricer.rs:355)

**What:** Presence of `mc_seed_scenario`, nominally a deterministic-metrics control, still switches range-accrual valuation from static replication to Monte Carlo.

**Why it's slop:** One configuration field owns two unrelated concerns and bypasses the canonical `ModelKey` dispatch.

**Proposed fix:** Select models only through `ModelKey`; use the seed field only after an MC model is selected.

**Invariants touched:** deterministic MC, pricing model
**Impact:** M
**Risk:** M
**Tier:** 3

---

### F32 — [Category: overexposed-internal / compatibility-fossil]

**Files:**

- [BumpedFxProvider:L222-L308](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/money/fx/providers.rs:222)
- [public arbitrage strategies:L54-L59](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/arbitrage/mod.rs:54)
- [cashflow emission exports:L48-L92](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/mod.rs:48)
- [InstrumentType::Loan:L27-L32](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/pricer/keys.rs:27)

**What:** Several implementation or migration details remain public:

- `BumpedFxProvider`, despite `FxMatrix::with_bumped_rate` being canonical
- Concrete arbitrage strategies/trait with no production external consumer
- Test-only default/prepayment emitters
- Unpriceable `InstrumentType::Loan` beside real `TermLoan`
- Legacy `MasterScale::*_empirical` aliases
- Mostly unused schedule presets and typed constructor shadows

**Why it's slop:** Internal helpers and compatibility fossils enlarge the semver surface without providing independent capability.

**Proposed fix:** Demote implementation details, map legacy loan input to `TermLoan`, and stage aliases through deprecation or wire-only adapters.

**Invariants touched:** serde, parity, FX
**Impact:** M
**Risk:** M
**Tier:** 2/3

---

### F33 — [Category: dead-code / wrapper-only]

**Files:**

- [Money formatting wrappers:L125-L207](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/money/types.rs:125)
- [unused TBA helper:L123-L141](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/tba/pricer.rs:123)
- [unused CMS wrapper:L496-L498](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/rates/cms_option/replication_pricer.rs:496)

**What:** A low-risk tail remains: three wrappers around canonical `Money::format_with`, unused pricing helpers, test-only rough-Heston fallback code, a one-caller tuple wrapper, an ignored floating-rate compatibility parameter, unused schedule presets, and repeated seven-symbol binding manifests.

**Why it's slop:** These are deletion-only wrappers or micro-optimizations without current production value.

**Proposed fix:** Delete unused functions, retain `Display` plus `format_with`, remove ignored parameters during the next breaking window, and derive binding manifests from one source.

**Invariants touched:** parity, formatting
**Impact:** L
**Risk:** L
**Tier:** 1/2

## Slop clusters

### Cluster A — Atomic cashflow state

**Includes findings:** F3, F4, F12–F16, F24, F27.

**Why it's a cluster:** Builder order, schedule conventions, date generation, JSON parity, sorting, accrual metadata, reporting, and schema ownership all meet at `CashFlowSchedule`.

**Recommended consolidation:** Introduce an atomic scheduled-flow row and one serializable schedule/coupon-program specification first. Then migrate generation, filtering, sorting, aggregation, reporting, bindings, and schemas together. Piecemeal fixes would leave adapters and parallel representations behind.

### Cluster B — Valuations runtime contract

**Includes findings:** F1, F9, F11, F17, F19, F25, F30, F31.

**Why it's a cluster:** These are incomplete migrations around what an instrument is, what data it needs, how it is configured, and which lifecycle prices it.

**Recommended consolidation:** Enrich `MarketDependencies`, define focused runtime override ownership, and establish one pricing lifecycle. Only then delete compatibility traits, DTOs, and fields.

### Cluster C — Canonical numerical ownership

**Includes findings:** F2, F5, F18, F20.

**Why it's a cluster:** Each involves financially sensitive algorithms duplicated across crates or bindings with already-visible boundary-policy differences.

**Recommended consolidation:** Select one dependency-neutral owner per kernel and pin current behavior with golden/boundary tests before redirecting callers.

### Cluster D — Core public-surface reduction

**Includes findings:** F6–F8, F10, F21–F23, F28, F29, F32, F33.

**Why it's a cluster:** These findings are alternate receiver styles, boxed/static paths, zero-state wrappers, compatibility fields, and exposed internals.

**Recommended consolidation:** Apply deletion and delegation in small semver-aware slices, keeping compatibility parsing separate from runtime APIs.

### Cluster E — Binding boundary cleanup

**Includes findings:** F7, F15, F20, F26–F28, F33.

**Why it's a cluster:** The bindings are structurally synchronized but expose duplicate transport shapes and own pieces of financial dispatch/validation.

**Recommended consolidation:** Add checked Rust request facades, reduce each host to one construction/transport path, then regenerate stubs, facades, declarations, and parity pins in the same slice.

## Binding drift

**Structural drift:**

- The 392 structural parity tests pass.
- Cashflows’ Rust builder is materially richer than its JSON/Python/WASM surface (F15).
- WASM has a separate FX object facade absent from Python’s JSON-first model (F26).
- `bond_from_cashflows_json` is owned by valuations but exported under cashflows (F27).
- Core Python contract entries for `cashflow`, `error`, `explain`, `expr`, `prelude`, and `validation` remain marked “missing.” User-facing omissions should be bound; Rust plumbing should be classified as intentionally excluded rather than permanently missing.
- The strict WASM core subset is documented and is not itself drift.

**Logic drift (logic that leaked into bindings):**

- Python/WASM analytic option dispatch and validation (F26).
- Optional FX 10-delta wing selection and pillar bounds checks (F20).
- SABR input-length validation (F2).
- WASM typed/untyped math adaptation (F7).
- Generated TypeScript declares lowercase accrual-method values while Rust serde expects `Linear`/`Compounded`.

**Parity contract impact:**

- F7: remove `*Array`/duplicate matrix symbols from WASM pins.
- F15/F16: update cashflow module and schema ownership entries.
- F20: update FX-delta constructor/member pins.
- F26: remove or replace WASM FX wrapper class/member pins.
- F27: move `bond_from_cashflows_json` mapping to valuations.
- F28: remove duplicate constructor and market-overload declarations.
- F33: generate the cashflow symbol manifest instead of hand-repeating it.

## Hazards (non-simplicity problems discovered incidentally)

- **H1 —** [registry.rs:L282](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/pricer/registry.rs:282) — direct Rust `price_with_metrics` can bypass `validate_for_pricing`. Severity: high.
- **H2 —** [term-loan cashflows:L437](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/term_loan/cashflows.rs:437) — appending/sorting flows independently can cause accrual metadata to be discarded. Severity: high.
- **H3 —** [accrual.rs:L390](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/accrual.rs:390) — same-payment-date coupons collapse into one bucket, losing distinct accrual period/rate/factor. Its day-count conflict sentinel can revert from conflict to a concrete convention on a third flow. Severity: high.
- **H4 —** [principal.rs:L32](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/principal.rs:32) — `principal()` can clear a sticky builder error after the associated contract leg has already been dropped. Severity: high.
- **H5 —** [fx_delta_vol_surface.rs:L226](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/surfaces/fx_delta_vol_surface.rs:226) — public Rust pillar lookup can panic; bindings independently guard it. Severity: high.
- **H6 —** [dataframe.rs:L95](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/dataframe.rs:95) — reporting recomputes floating base rates using reset-to-payment rather than the canonical fixed-tenor projection. Severity: medium-high.
- **H7 —** [structured-credit rates:L40](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/src/instruments/fixed_income/structured_credit/utils/rates.rs:40) — invalid CPR/CDR inputs are clamped where other crates reject them. Severity: medium-high.
- **H8 —** [generated CashflowSchedule.ts:L44](/Users/jeickmeier/Projects/finstack-quant/finstack-quant-wasm/types/generated/CashflowSchedule.ts:44) — TypeScript advertises lowercase accrual methods incompatible with Rust serde. Severity: medium.
- **H9 —** [calendar registry:L65](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/dates/calendar/registry.rs:65) — `resolve_many_vec` silently drops unknown calendar IDs. Severity: medium.
- **H10 —** [cashflow envelope:L87](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/json.rs:87) — envelope provenance always describes default configuration, not necessarily the active configuration. Severity: medium.
- **H11 —** [hierarchy builder:L68](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/hierarchy/builder.rs:68) — reordered builder calls silently lose tags and curve IDs. Severity: medium.
- **H12 —** Core and valuations disagree on degenerate ATM Black-76 d1 behavior; consolidation must choose the intended delta convention before redirecting callers. Severity: medium.

## Scorecard

- API simplicity: 2/5 — canonical APIs exist, but compatibility and migration layers frequently remain public beside them.
- Redundancy level: 2/5 — significant duplication persists in financial kernels, schedule state, configuration, and bindings.
- Consistency: 2/5 — duplicate representations already disagree on invalid-input, ordering, and boundary policy.
- Binding hygiene: 3/5 — structural parity is strong, but bindings still own dispatch and duplicate transport APIs.
- Maintainability: 2/5 — several central abstractions require synchronized edits across many files and parallel state representations.

**Overall:** 2/5

## Top 5 highest-leverage changes

1. **F12** — Make cashflow rows and accrual metadata atomic. Removes roughly 200–300 LOC of index choreography and eliminates several hazards.
2. **F11** — Finish the `MarketDependencies` migration. Removes roughly 800–1,200 LOC of trait implementations, builders, and adapters.
3. **F17** — Move `PricingOverrides` compatibility to the wire boundary. Removes roughly 500–900 LOC of forwarding builders and trait hooks.
4. **F1** — Delete disconnected option parameter DTOs. Removes roughly 300 LOC and a misleading root API.
5. **F2** — Retire the unused derivative SABR calibration family. Removes roughly 540+ LOC plus duplicate binding validation.

## Next steps

Proceed to Phase 2 (Plan) to break the five clusters into invariant-aware PR-sized slices, beginning with the atomic cashflow state model or the lower-risk dead-public-type/SABR deletions.

**Awaiting user input:** confirm priorities or request a Phase 2 plan for all findings.
