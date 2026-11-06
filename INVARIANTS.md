# finstack Workspace Invariants

This catalogue retains detailed cross-crate financial, numerical, and API
contracts. [Accepted decisions](docs/decisions/README.md) own the agreed
policy, scope, and rationale; this file owns the finer operational rules
referenced by those decisions. API-specific details remain with their
designated documentation and serialized-contract sources.

**Policy status and evidence status are separate.** An accepted decision
governs only its stated scope. Related ADR links do not independently approve
every existing method, default, or detailed clause. Existing requirements
remain in force unless explicitly superseded; an unmapped requirement is not
permission to change or discard it.

**Required** identifies a normative rule. **Evidence** identifies supporting
types, source, schemas, or tests and the scope they cover. Source inspection
and the presence of a test do not establish complete runtime verification.
Partial or unverified coverage is stated separately from the requirement.

The [invariant-to-decision map](docs/decisions/INVARIANT_MAP.md) identifies a
corresponding accepted decision for every normative clause, including rules
in prose and tables. The invariant-detail review is complete; acceptance
establishes the stated contracts, not complete implementation or validation.

Outstanding implementation, reconciliation, and verification work belongs in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md). Resolve conflicts through the
specific accepted decision and authorized scope, as described in
[section 9](#9-authority-and-enforcement); do not change expectations merely
to match the current code.

---

## 1. Money and model numerics

**Authority:** [ADR 0020](docs/decisions/financial/conventions/0020-numerical-representation-and-rounding.md).
**Scope:** monetary representation, arithmetic, conversion, and rounding;
model `f64` numerics remain distinct from contractual monetary amounts.

### 1.1 Representation

**Decision:** [0020](docs/decisions/financial/conventions/0020-numerical-representation-and-rounding.md)
is accepted for representation, Money construction without implicit currency
quantization, explicit rounding, and non-finite monetary-input rejection.
[Clause map M01–M05](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

`finstack_quant_core::money::Money` stores
`rust_decimal::Decimal` plus a `Currency`. `Money::new` accepts `f64` and returns `Result`; `Money::amount()` returns `f64`.
`Money::from_decimal` and `Money::amount_decimal()` provide the lossless
Decimal path.

**Required:**

- Curves, rates, volatilities, correlations, returns, greeks, optimizers, and
  Monte Carlo paths MAY use `f64`.
- **New** accounting, ledger, settlement, regulatory-reporting, and
  margin-dispute paths MUST use `Decimal` or `Money` without an intermediate
  `f64` round-trip. Existing aggregation gaps are tracked in
  [monetary follow-up](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up);
  they do not permit new `f64` settlement-grade paths.
- Wrapping an `f64` result in `Money` gives it currency semantics and Decimal
  storage; it does **not** make the preceding calculation Decimal-exact.
- `Money::new` preserves sub-minor-unit precision. It does
  not imply ISO-4217 quantization. Currency-scale rounding MUST be requested
  through configuration or applied explicitly when an amount is finalized.
- Non-finite monetary inputs MUST be rejected.

**Evidence:** [Money storage, constructors, and regression tests](finstack-quant/core/src/money/types.rs)
support the Decimal representation, explicit conversion, and non-finite-input
rejection. This does not certify every downstream monetary calculation.

### 1.2 Arithmetic and aggregation

**Decisions:** [0020](docs/decisions/financial/conventions/0020-numerical-representation-and-rounding.md)
is accepted for arithmetic boundaries;
[0042](docs/decisions/standards/0042-numerical-accumulation-and-comparisons.md)
is accepted for floating-point accumulation and its finite-input requirements.
[Clause map M06–M09](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

**Required:**

- Use Decimal arithmetic where rounding affects a contractual amount, posted
  balance, settlement instruction, regulatory submission, or dispute.
- `f64` aggregation is acceptable for model PV, risk, and analytical totals
  when the boundary and rounding policy are documented.
- For floating-point totals, use compensated summation for mixed-sign cash
  flows, large dynamic ranges, or totals whose floating-point error can affect
  a reported result. Use
  `finstack_quant_core::math::summation::{neumaier_sum, NeumaierAccumulator}`.
  Use `OnlineStats` for online mean and variance, not as a substitute for
  compensated totals. Required Decimal monetary paths remain Decimal.
- Validate finiteness before Kahan/Neumaier accumulation. Compensation does
  not make `NaN` or infinity safe.

**Evidence:** the shared [summation](finstack-quant/core/src/math/summation.rs)
and [statistics](finstack-quant/core/src/math/stats.rs) modules provide the
numerical primitives. Their existence does not establish that every caller
validates finiteness or uses the required arithmetic. The concrete monetary
paths needing classification and correction are recorded in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

---

## 2. Determinism and floating point

**Authority:** [ADR 0021](docs/decisions/standards/0021-reproducibility-guarantees.md)
defines the reproducibility guarantees;
[ADR 0014](docs/decisions/project/0014-financial-reference-evidence.md) governs
reference evidence and tolerances. The stream, ordering, reduction, and
comparison rules below retain their detailed scope.

### 2.1 Reproducibility tiers

**Detailed decision:** [0043](docs/decisions/standards/0043-deterministic-ordering-and-monte-carlo-execution.md)
is accepted for target-specific baselines and concrete result/error ordering under
accepted ADR 0021. [Clause map D01–D06](docs/decisions/INVARIANT_MAP.md#2-determinism-and-floating-point).

Every public stochastic API declares the bit-reproducible, seed-reproducible,
or statistically reproducible guarantee defined in
[ADR 0021](docs/decisions/standards/0021-reproducibility-guarantees.md), including
its supported target and execution conditions.

**Per-target reproducibility (native vs wasm32).** A guarantee within one
build target does not establish cross-target identity. Native (Python) and wasm32 (WASM) builds use different
platform implementations of transcendental functions (`exp`, `ln`, `powf`,
statrs CDFs, nalgebra decompositions), so any `f64` result that uses them may
differ at the ulp level between targets. Monte Carlo and finite-difference or
bump metrics can amplify this. A Greek dominated by numerical noise (for
example a vega bumped off a near-zero vol) can be exactly zero on one target
and non-zero on the other. Calibration
diagnostics (iteration counts, multi-start winners) can differ the same way.
Therefore:

- a Python↔WASM behavioural parity test MUST compare `f64` outputs with an
  explicit justified tolerance unless cross-target bit identity is demonstrated
  for the supported conditions and mathematical-library implementations;
- baselines captured on one target (for example UI fixtures captured from the
  wasm32 build) must be recaptured on that same target, recording the relevant
  build, runtime, and execution conditions needed to interpret or reproduce
  the baseline; the target name alone does not establish reproducibility;
- what MUST agree across targets is everything that is not `f64` rounding:
  result structure, keys, ordering, error kinds, and **which item an error
  names**. Parallel maps MUST return the lowest-index failing item's error on every
  target and thread count (`finstack_quant_core::parallel::try_map_ordered`),
  and hash-map iteration order (which can differ between 64-bit native and
  wasm32) MUST NOT determine results, floating-point reductions or error selection.

**Evidence:** [ordered parallel mapping and its tests](finstack-quant/core/src/parallel.rs)
implement lowest-index error selection. The
[EWMA implementation](finstack-quant/features/src/timeseries.rs) uses
`libm::hypot` to reduce target-dependent behavior; that choice alone does not
prove all cross-target guarantees. Coverage and previously reported numerical
differences are tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

### 2.2 Random number generation

**Decisions:** [0021](docs/decisions/standards/0021-reproducibility-guarantees.md)
is accepted for explicit seeds and declared guarantees;
[0043](docs/decisions/standards/0043-deterministic-ordering-and-monte-carlo-execution.md)
is accepted for the concrete RNG and execution conditions.
[Clause map D07–D09](docs/decisions/INVARIANT_MAP.md#2-determinism-and-floating-point).

**Required:** library code MUST use explicit seeds. Do not use
`thread_rng()` or `rand::random()`.

**Required for the primary Monte Carlo engine's bit-identity guarantee:**
within the supported target and fixed declared inputs and configuration,
bit-identical serial/Rayon results require all of the following:

- a splittable stream such as `PhiloxRng::split(path_id)` or
  `substream(path_id)`;
- a work partition determined by path count and declared chunk configuration,
  never by worker or thread count. Default chunking is a pure function of path
  count; a caller-selected chunk size is held constant for the comparison;
- a fixed merge order independent of rayon thread count; and
- adaptive confidence-interval stopping disabled.

Philox alone is not the guarantee. Non-splittable sequences such as the
current Sobol implementation do not satisfy this parallel tier. Halton
multi-start calibration is deterministic by construction.

**Evidence:** the [Philox stream](finstack-quant/models/src/monte_carlo/rng/philox.rs),
[engine partition and merge logic](finstack-quant/models/src/monte_carlo/engine/pricing.rs),
and [serial/parallel and thread-count tests](finstack-quant/models/src/monte_carlo/engine/tests.rs)
support the named same-target conditions. The tests also reject quasi-random
streams in the generic engine; this is not a claim of support for every RNG
or proof of cross-machine or native/WASM bit identity.

### 2.3 Map ordering

**Detailed decision:** [0043](docs/decisions/standards/0043-deterministic-ordering-and-monte-carlo-execution.md)
is accepted for ordered public maps and private hash-map use.
[Clause map D10–D11](docs/decisions/INVARIANT_MAP.md#2-determinism-and-floating-point).

**Required:**

- `std::collections::HashMap` uses a per-instance random seed and exposes
  arbitrary iteration order.
- `finstack_quant_core::FxHashMap` uses deterministic hashing
  for a given insertion sequence, but its iteration order is still not a
  serialization or API contract.
- Any map order visible through serialization, files, public iterators,
  snapshots, or golden tests MUST use `IndexMap` for insertion order or
  `BTreeMap` for sorted order. Populate insertion-ordered maps in a
  deterministic logical order; inserting from hash iteration or worker
  completion order does not satisfy this requirement.

Hash maps MAY be used for internal caches, intern tables, and lookups whose
order cannot escape.

**Evidence:** the [core exports](finstack-quant/core/src/lib.rs) expose
`FxHashMap` and `FxHashSet`. Public ordering and reduction coverage require
caller-level evidence; the hasher's identity is not an ordering guarantee.

### 2.4 Reductions

**Detailed decisions:** [0042](docs/decisions/standards/0042-numerical-accumulation-and-comparisons.md)
is accepted for accumulation accuracy;
[0043](docs/decisions/standards/0043-deterministic-ordering-and-monte-carlo-execution.md)
is accepted for deterministic execution under the promised reproducibility tier.
[Clause map D12–D14](docs/decisions/INVARIANT_MAP.md#2-determinism-and-floating-point).

Floating-point addition is commutative for finite ordinary operands but is
not associative. A parallel `+` fold is therefore not automatically
deterministic.

**Required:**

- Reductions promising bit identity across supported execution modes or
  thread counts MUST use deterministic partitions and a fixed merge tree
  for the same declared inputs and configuration.
- `OnlineStats::merge`, Welford/Chan statistics, and compensated sums still
  depend on merge order; use them inside that fixed tree when such bit
  identity is promised.
- Compensated summation improves accuracy. It does not independently provide
  bit reproducibility.
- Do not use an arbitrary term-count threshold as a substitute for analysing
  sign cancellation, dynamic range, and materiality.

### 2.5 Comparisons

**Decisions:** [0014](docs/decisions/project/0014-financial-reference-evidence.md)
is accepted for justified tolerances and
[0038](docs/decisions/standards/0038-substantive-api-documentation.md) for accurate
API references; [0042](docs/decisions/standards/0042-numerical-accumulation-and-comparisons.md)
is accepted for tolerance semantics, exact numerical equality, and bitwise
comparison of bit-identity guarantees.
[Clause map D15–D17](docs/decisions/INVARIANT_MAP.md#2-determinism-and-floating-point).

**Required:** approximate model results MUST use a documented tolerance with
explicit semantics: absolute, relative, notional-scaled, probability, rate,
percentage, money half-unit, or another named domain tolerance.

Exact numerical `f64` equality is allowed only when its IEEE semantics are
intended, including exact-zero sentinels and values known to come from the
same deterministic operation. Bit-reproducibility tests MUST compare
`to_bits()` representations; ordinary `==` does not establish bit identity.
Do not cite a shared comparison helper unless that helper exists in the
public API.

### 2.6 Portfolio evaluation execution

**Authority:** [ADR 0022](docs/decisions/architecture/0022-portfolio-evaluation-state.md).
**Detailed decision:** [0046](docs/decisions/architecture/0046-portfolio-execution-operational-contract.md)
accepts the operational conditions below.
[Clause map P01–P13](docs/decisions/INVARIANT_MAP.md#26-portfolio-evaluation-execution).

**Scope:** portfolio valuation, metrics-based attribution, scenario P&L,
scenario batches, replay, and factor-stress endpoint P&L. Ordinary position
valuation MUST use the request-scoped portfolio evaluation executor.
Public entry points and host-language bindings are adapters into
that executor; they MUST NOT own a second position-pricing loop, valuation
assembler, calibration cache, or scheduling policy.

**Required:**

- A request registers immutable market states separately from immutable
  portfolio/instrument states. A changed market, effective valuation date, or
  instrument set receives a distinct state identity.
- Each market state creates fresh rate and hazard recalibration caches. Cloned pricing
  options MAY share those caches only within the same immutable state; caches
  MUST NOT cross market states or requests.
- Evaluation jobs are distinguished by their exact requested metric menu, strict-risk
  policy, and base-currency policy. Each position computes its structurally
  applicable subset; inapplicable metrics MUST be explicitly reported, never
  represented as successful zeroes. Strict risk fails when an applicable
  requested metric fails. An empty explicit metric menu is PV-only and
  MUST NOT be promoted to the standard risk set. A degraded best-effort result
  MUST NOT satisfy a strict request. A workflow that explicitly rejects
  cross-currency positions MUST validate native currency before any FX lookup.
- A caller MAY explicitly supply an immutable prior result to seed a new
  request. Selective reuse requires compatible evaluation profiles, portfolio
  shape and relevant pricing configuration, plus an established relationship
  between parent and updated markets and an authoritative change set. If
  compatibility or the change set cannot be established, perform fresh
  valuation. This input is distinct from an executor retaining valuation
  caches across requests.
- FX invalidation MUST distinguish instrument repricing from
  base-currency conversion refresh so a reused native PV never leaves a stale
  base PV. Without an authoritative FX path manifest, any changed FX quote
  conservatively invalidates every position with any FX dependency because a
  cross may be triangulated through that quote. When conversion inputs change,
  refresh base-currency conversion for every reused position; proven unchanged
  conversion inputs do not require redundant refresh.
- One portfolio execution wave selects at most one Rayon axis: states, positions, or
  factors. Nested state-by-position or factor-by-position fan-out is forbidden.
  Instrument-internal Monte Carlo threading remains a separate coordination
  and performance-verification concern.
- Parallel work collects indexed results before a serial logical-order scan.
  Position/state order, first-error selection, and Neumaier aggregation order
  therefore remain deterministic.
- Evaluation state and caches are request-local. Do not add a global,
  thread-local, process-wide, persistent, or cross-request valuation cache.
  Immutable decoded-instrument input caches, static immutable registries, and
  UI presentation caches are outside this market-dependent financial-cache scope.
- Python adapters MAY hold typed borrow guards while extracting plain Rust
  references outside detached closures; native closures MUST capture only Rust
  state. This boundary does not require cloning whole typed portfolios.
  Native computation, JSON parsing, portfolio materialization, and native result
  serialization MUST release Python attachment after copying any required Python
  string input. Python object extraction and construction remain attached.
- A migrated workflow MUST delete its superseded compute path. Do not retain a
  runtime legacy/new switch, dormant fallback engine, or duplicate scheduler.
  Financially distinct reducers may remain, but any ordinary portfolio
  valuation they require enters through the shared executor.

**Evidence:** the [executor](finstack-quant/portfolio/src/evaluation/executor.rs),
[request-owned state](finstack-quant/portfolio/src/evaluation/state.rs), and
[evaluation keys and invalidation inputs](finstack-quant/portfolio/src/evaluation/plan.rs)
provide implementation evidence. Reuse checks do not independently establish
pricing-configuration or originating-market compatibility. The shared
`deserialize_json_with_context` helper also detaches without the required
string copy; this is a policy-compliance gap, not a demonstrated safety defect.
Complete workflow, cache-invalidation, model-internal scheduling, and binding
coverage remains subject to the
[portfolio follow-up](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

---

## 3. Sign and perspective conventions

**Authority:** [ADR 0007](docs/decisions/financial/conventions/0007-units-and-sign-conventions.md).
**Scope:** the perspectives and boundary conversions below; an operation's
explicit convention governs its inputs and outputs.

Signs describe a perspective, not an intrinsic property of an amount.
Interfaces MUST name their perspective and MUST NOT pass an amount between
perspectives without an explicit conversion.

### 3.1 Canonical perspectives

**Detailed decision:** [0044](docs/decisions/financial/conventions/0044-financial-perspectives-and-boundary-conversions.md)
accepts the named perspectives and explicit boundary conversions.
[Clause map S01–S02](docs/decisions/INVARIANT_MAP.md#3-sign-and-perspective-conventions).

These are canonical conceptual labels; they are not all Rust types yet.

| Perspective | Positive means | Primary uses |
|-------------|----------------|--------------|
| `EconomicCashFlow` | cash received by the entity or holder being valued | NPV, XIRR, DCF, instrument and Monte Carlo cash flows |
| `PortfolioExternalFlow` | client contribution into the portfolio | Modified Dietz / GIPS-style performance |
| `StatementMagnitude` | non-negative magnitude; the formula carries direction | CapEx, opex, tax, dividends, increase/decrease buckets |
| `RollForwardActivity` | signed additive change in `ending = beginning + Σ change` | corkscrews and additive reconciliations |
| `RegulatoryCollateral` | net collateral held by the bank | SA-CCR variation margin and NICA |

`AmountOrScalar` distinguishes currency amounts from unitless scalars; it does
not encode a sign perspective.

### 3.2 Required boundary conversions

**Detailed decision:** [0044](docs/decisions/financial/conventions/0044-financial-perspectives-and-boundary-conversions.md)
accepts each conversion and side convention below. Accepted
[0005](docs/decisions/standards/0005-python-wasm-parity.md),
[0007](docs/decisions/financial/conventions/0007-units-and-sign-conventions.md), and
[0038](docs/decisions/standards/0038-substantive-api-documentation.md) govern
equivalent host polarity; [0010](docs/decisions/financial/valuations-framework/0010-metric-identifiers.md)
governs metric meaning.
[Clause map S03–S12](docs/decisions/INVARIANT_MAP.md#3-sign-and-perspective-conventions).

- Dietz flows use `PortfolioExternalFlow`: contribution in is positive.
  XIRR/MWR uses `EconomicCashFlow`: contribution in is negative. Conversion
  requires `xirr_amount = -dietz_amount`.
- Real-estate CapEx schedules contain positive `StatementMagnitude` outflows.
  The pricer owns the single negation into `EconomicCashFlow`. Passing
  already-negative CFS CapEx into that schedule is invalid because it
  double-negates the outflow. `RealEstateAsset::validate()` rejects negative
  `capex_schedule` amounts.
- Retained-earnings dividends default to `MagnitudePositive`: a non-negative
  magnitude is subtracted by the identity. An explicitly selected
  `InflowPositive` convention instead adds the signed dividend directly.
  Dividend `+10` under the default and `-10` under the signed convention both
  reduce retained earnings by `10`. Callers MUST NOT infer the convention from
  the value or silently apply `abs()`. Corkscrew equity reductions are negative
  `RollForwardActivity`.
- Roll-forward `increases` and `decreases` are positive magnitude buckets;
  converting to a corkscrew produces `+increase` and `-decrease`.
- SA-CCR collateral and NICA are signed net amounts. Positive means the bank
  holds collateral. Margined replacement cost uses
  `max(V - C, TH + MTA - NICA, 0)`, where `C` is total recognized net collateral
  after applicable adjustments and already includes NICA. Component inputs
  MUST be explicitly defined and non-overlapping; NICA MUST NOT be added to
  total `C` a second time. The public `collateral` field is variation margin
  excluding NICA, and the kernel adds `nica` to form `C`. Full
  margined/unmargined methodology qualification remains open.
- CDS option Call means payer protection; Put means receiver protection.
- Portfolio quantity greater than zero means long. P&L attribution is
  positive for a gain to the long holder. Sensitivity signs follow the
  relevant `MetricId` documentation.

Binding documentation MUST state the same polarity as Rust.
Avoid asset-class-ambiguous `long`/`short` aliases for `PayReceive`; prefer
`pay`, `receive`, or an explicit protection side.

**Evidence:** [real-estate validation](finstack-quant/valuations/src/instruments/equity/real_estate/types.rs),
[retained-earnings conventions](finstack-quant/statements/src/checks/builtins/retained_earnings.rs),
[corkscrew magnitude buckets](finstack-quant/statements-analytics/src/extensions/corkscrew/mod.rs),
and [SA-CCR replacement cost](finstack-quant/margin/src/regulatory/sa_ccr/replacement_cost.rs)
provide operation-specific implementation evidence. Retained earnings supports
the accepted explicit dividend selector, but negative magnitude inputs currently
produce warnings rather than rejection; enforcement remains a follow-up. The
SA-CCR public `collateral` field is documented as variation margin excluding
NICA, matching the kernel, which adds `config.nica`; a caller who supplies total
collateral in `collateral` would still double count NICA, so the kernel does
not enforce non-overlap. The kernel also implements
unmargined `max(V - C, 0)`; accepting the sign policy does not qualify either
branch's complete methodology. Non-overlap verification, named conversion
helpers or typed wrappers where needed, and cross-boundary verification remain
tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

---

## 4. Date, day-count, and time conventions

**Authority:** [ADR 0007](docs/decisions/financial/conventions/0007-units-and-sign-conventions.md)
and [ADR 0008](docs/decisions/financial/conventions/0008-financial-defaults-and-cagr.md)
protect explicit conventions and defaults. The clock roles and boundary
rules below retain their existing scope; these links do not adopt every
instrument model or observed default.

A single instrument can require distinct clocks. Do not infer one clock from
another.

### 4.1 Clock roles

**Detailed decision:** [0045](docs/decisions/financial/conventions/0045-financial-clocks-and-date-boundaries.md)
accepts separate clocks and their conventions; ADR 0008's dated-CAGR
default remains accepted.
[Clause map C01–C06](docs/decisions/INVARIANT_MAP.md#4-date-day-count-and-time-conventions).

| Clock | Rule |
|-------|------|
| Discount | Obtain the exact relative discount factor from curve dates and the curve's own base date/day-count. |
| Projection | Locate dates on the projection curve's clock; express forwards using the contractual index accrual fraction. |
| Accrual | Compute coupon accrual from the instrument or leg day-count. |
| Option/volatility | Use the calibration's reference date and time basis, with calendar context where required. |

Act/360 is commonly an accrual convention; it is not a universal rates-option
time basis. The full date-to-expiry mapping MUST be explicit through surface
metadata or a documented model contract, including its reference date and time
basis. Metadata representation and missing-metadata behavior remain open;
do not infer a universal fallback. Existing rate-option, equity, FX, and commodity defaults
require operation-specific evidence rather than an inferred universal basis;
the default inventory is tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

The accepted default for dated-window CAGR is specified by
[ADR 0008](docs/decisions/financial/conventions/0008-financial-defaults-and-cagr.md). It does
not redefine frequency-based annualization, XIRR, or an instrument's model
clock; implementation reconciliation is tracked with that decision.

### 4.2 Discount-factor bridge

**Detailed decision:** [0045](docs/decisions/financial/conventions/0045-financial-clocks-and-date-boundaries.md)
accepts the discount-factor bridge within the conditions below.
[Clause map C07](docs/decisions/INVARIANT_MAP.md#4-date-day-count-and-time-conventions).

For curve base date `b`, valuation date `v`, and payment date `T`, obtain the
relative discount factor from the supplied curve. When a single-time model
needs a continuously compounded rate on model time `t_model`, derive it from
that factor:

```text
relative_df = D(b, T) / D(b, v)
r_model = -ln(relative_df) / t_model
exp(-r_model * t_model) = relative_df
```

Both curve factors, `relative_df`, and `t_model` MUST be finite and positive.
The bridge preserves the factor mathematically; comparisons use justified floating-point tolerances.
It does not establish equivalence of entire model dynamics. Zero-time, expired,
and invalid-input behavior MUST follow explicit operation-specific contracts.
Discount with the curve factor directly whenever the model permits.

**Evidence:** `zero_rate_from_df` in
[the shared pricing helpers](finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs)
and the [discount curve's date APIs](finstack-quant/core/src/market_data/term_structures/discount_curve/curve.rs)
provide the conversion primitives. The rate helper does not validate finite
model time and returns zero for `t <= 0` before validating the factor. Its
presence does not establish input enforcement or adoption by every pricer;
coverage is tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

### 4.3 Date boundaries

**Detailed decisions:** [0045](docs/decisions/financial/conventions/0045-financial-clocks-and-date-boundaries.md)
accepts dates, settlement, fixing-cutoff requirements, survival alignment and
the numerical-evidence requirement below;
[0048](docs/decisions/project/0048-reference-provenance-and-version-pinning.md)
accepts the detailed provenance, measurement-evidence and version-pinning standards.
[Clause map C08–C13](docs/decisions/INVARIANT_MAP.md#4-date-day-count-and-time-conventions).

**Required:**

- Valuation `as_of`, curve `base_date`, trade date, spot/settlement date,
  fixing date, and payment date MUST remain distinct.
- Rebase seasoned curves with relative date-based discount factors.
- Contractual settlement MUST apply the relevant spot lag, calendars,
  business-day convention, and end-of-month rule.
- Fixing requirements MUST follow an explicit valuation/publication cutoff.
  Required historical fixings MUST use the configured observations; missing
  required fixings MUST fail rather than silently project from a curve.
  Same-day treatment remains operation-specific and requires reconciliation.
- Credit survival time and discount time MAY use different day-counts and
  MUST be bridged through date-based survival probabilities and discount
  factors.

**Evidence:** the [autocallable](finstack-quant/valuations/equity/src/autocallable/pricer.rs)
and [cliquet](finstack-quant/valuations/equity/src/cliquet_option/pricer.rs)
Monte Carlo pricers measure model/vol time on the instrument day count and
use date-based curve discount factors. The named regression
`test_vol_clock_independent_of_curve_day_count` in
[the autocallable tests](finstack-quant/valuations/equity/tests/autocallable/test_day_count_basis.rs)
provides a bounded example, not complete clock-independence or cliquet
coverage. The cashflow fixing helper uses an available same-day fixing, while
the term cap/floor pricer assumes start-of-day and projects it; acceptance does
not settle that discrepancy. Verification gaps and the full volatility
date-to-expiry mapping are
tracked in [FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

Do not publish numerical error bands such as basis-point drift unless a
reproducible benchmark or golden test defines the setup and provenance.
Published measurements require recorded execution evidence identifying the
revision, command, inputs, comparator, error units and relevant build, runtime
and execution conditions. Distinguish measured error, sampling uncertainty and
test tolerance. Evidence for a named setup does not establish an accuracy bound
over a broader domain.

---

## 5. Error handling

**Authority:** [ADR 0036](docs/decisions/standards/0036-safe-rust-and-recoverable-failures.md)
defines safety and recoverable failures;
[ADR 0006](docs/decisions/standards/0006-structured-errors.md) defines error
meaning across interfaces. **Scope:** project library and binding code.

### 5.1 Enforced lints

**Decisions:** [0036](docs/decisions/standards/0036-safe-rust-and-recoverable-failures.md)
and [0034](docs/decisions/standards/0034-code-standards-authority.md) are accepted
for safety and shared enforcement;
[0049](docs/decisions/standards/0049-explicit-public-error-matching.md) is accepted
for explicit error classification and canonical delegation.
[Clause map E01–E03](docs/decisions/INVARIANT_MAP.md#5-error-handling).

**Evidence:** the [core library attributes](finstack-quant/core/src/lib.rs)
forbid unsafe code and deny `unwrap_used`, `expect_used`, `panic`, and
`unreachable` on production paths. The
[Python](finstack-quant-py/src/lib.rs) and [WASM](finstack-quant-wasm/src/lib.rs)
binding attributes forbid unsafe code and deny `unwrap_used`, `expect_used`,
and `panic`; they do not currently deny `unreachable`.
[Workspace configuration](Cargo.toml) denies `match_wild_err_arm`. That lint
checks `Err(_)` arms taking drastic actions such as panic; it does not enforce
exhaustive matching of arbitrary error enums. Broader classification coverage
remains a verification and enforcement follow-up.

**Required:** explicitly name project-owned public error variants where code
assigns their meaning, including category, structured code, required diagnostic
fields, conversions and specialized handling. Named variants may share a
branch. Both `_` and catch-all bindings such as `other` must not silently
assign a default meaning to future variants. Canonical classifiers and
conversions in the owning crate must enumerate their variants, including
nested error variants where those determine the mapping.

Unchanged propagation through `?`, forwarding, formatting and delegation to
Rust-owned `kind()`/`code()` contracts do not require duplicate variant tables.
A `#[non_exhaustive]` enum requires a catch-all outside its defining crate,
including sibling workspace crates. Keep the explicit classification in the
owner and preserve it through downstream delegation; do not claim that adding
a variant must break every consumer's compilation. Independently specialized
handling retains its own review responsibility.

Required unknown-variant handling for third-party non-exhaustive errors must
have a narrow, documented failure mapping appropriate to the operation and
retain original diagnostic/source context. It must not silently return
success, infer a known cause from message text, or turn a recoverable failure
into a panic. Existing generic adapter fallbacks are not approved merely
because a wildcard is required by the language.

Intentional panic-based assertions in test, example, and benchmark targets
follow ADR 0036. Crate inheritance and complete panic-path coverage are
separate verification questions tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

### 5.2 Required failure behavior

[Clause map E04–E05](docs/decisions/INVARIANT_MAP.md#5-error-handling) records
the accepted decisions for the requirements below.

Return the recoverable domain errors and diagnostic context required by
[ADR 0036](docs/decisions/standards/0036-safe-rust-and-recoverable-failures.md),
preserving their structured meaning under ADR 0006. Error documentation names
the public error callers can handle under
[ADR 0038](docs/decisions/standards/0038-substantive-api-documentation.md).

Use the actual public enums, such as
[`PricingError`](finstack-quant/valuations/core/src/pricer/errors.rs) and
[`finstack_quant_scenarios::Error`](finstack-quant/scenarios/src/error.rs).
Do not describe an error type that does not exist. Centralized host adapters
are defined in [Python errors](finstack-quant-py/src/errors.rs) and
[WASM utilities](finstack-quant-wasm/src/utils/mod.rs).

### 5.3 Indexing

**Decision:** [0036](docs/decisions/standards/0036-safe-rust-and-recoverable-failures.md)
is accepted for checked bounds and locally established kernel invariants.
[Clause map E06](docs/decisions/INVARIANT_MAP.md#5-error-handling).

**Required:** caller-controlled shapes and indices require checked access or
explicit precondition validation under ADR 0036. Use `.get()`,
`.first()`, checked split operations, or explicit precondition validation.
Numerical kernels may index directly only when the invariant is established
locally and covered by tests.

**Evidence status: partial.** `indexing_slicing` is not denied workspace-wide;
panic-family lints alone do not establish bounds safety. The required audit
and regression work is tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

---

## 6. Test discipline

Testing policy is maintained in the governing decisions:

| Policy | Authority |
| --- | --- |
| Behavior and invariant coverage; regressions that fail without the fix and identify the failure mode | [ADR 0039](docs/decisions/standards/0039-behavior-and-invariant-tests.md) |
| Independently justified expected values and protected tolerances | [ADR 0014](docs/decisions/project/0014-financial-reference-evidence.md) |
| Evidence for every promised execution mode and thread-count class; tolerance tests do not prove bit identity | [ADR 0021](docs/decisions/standards/0021-reproducibility-guarantees.md) |
| Shared definitions for scoped and full verification | [ADR 0034](docs/decisions/standards/0034-code-standards-authority.md) |
| Case-attributable golden metadata and permitted fixture/provenance locations | [ADR 0048](docs/decisions/project/0048-reference-provenance-and-version-pinning.md) |

[Clause map T01–T07](docs/decisions/INVARIANT_MAP.md#6-test-discipline) records
the accepted testing policy and reference details; enforcement remains subject
to verification.

The detailed reference-evidence conventions retained here are:

- Golden values MUST record provenance, methodology/version, valuation date,
  conventions, inputs, tolerances and their rationale, and expected units.
  Metadata must be attributable to each case. It may be inherited from fixture
  metadata, the test header, or the golden-suite README, with explicit
  case-specific overrides. Identify genuinely inapplicable fields with a
  reason rather than inventing dates or versions; methodology, inputs, units
  and tolerance rationale remain required.
- Rust and Python golden suites may share fixtures. Do not require all goldens
  to live directly under a Rust `<crate>/tests/` directory. Shared host runs do
  not create independent reference sources, and fixture location or metadata
  completeness does not establish financial correctness.

The [canonical tasks](mise.toml) implement scoped and full test runs;
[CONTRIBUTING.md](CONTRIBUTING.md) describes the workflow. Remaining evidence
and coverage work is tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

---

## 7. Public API, bindings, and deprecation

### 7.1 Host-surface parity

**Authority:** [ADR 0005](docs/decisions/standards/0005-python-wasm-parity.md)
defines equivalent host capabilities and the protected zero-backlog policy;
[ADR 0003](docs/decisions/standards/0003-canonical-public-names.md) defines
canonical names. Explicit justified host exceptions remain supported.

[Clause map H01–H04](docs/decisions/INVARIANT_MAP.md#7-public-api-bindings-and-deprecation)
also identifies the accepted documentation and independent-protection decisions.

A financial capability selected for host exposure has corresponding Python
and WASM interfaces under ADR 0005. This does not require exposing every
internal Rust function. Coordinated changes update the affected Rust API,
PyO3 registration, Python stubs and exports, WASM bindings and facade
declarations, examples, and parity contracts in the same change set.

**Evidence:** the [parity ledger](finstack-quant-py/parity_contract.toml),
[ledger tests](finstack-quant-py/tests/parity/test_parity_ledger.py), and
[WASM facade surface](finstack-quant-wasm/facade-surface.json) provide surface
coverage. Behavioral coverage and protection against self-modified ceilings
remain separate requirements under ADRs 0005 and 0016, tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

### 7.2 Deprecation policy

**Authority:** [ADR 0004](docs/decisions/project/0004-development-compatibility.md).
Approved development replacements remove obsolete paths rather than retaining
callable compatibility layers. Changes to established contracts still require
the authorization defined by [ADR 0015](docs/decisions/project/0015-authorized-change-scope.md).

[Clause map H05–H06](docs/decisions/INVARIANT_MAP.md#7-public-api-bindings-and-deprecation)
records the accepted replacement policy and required final-release decision.

The final-release compatibility strategy remains a separate decision required
before the first final release. Its unresolved support window, deprecation,
and migration mechanisms are tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

**Evidence status: legacy enforcement remains.** The
[annotation checker](scripts/check_deprecated_annotations.py) still runs from
`rust-doc`; its annotation-format checks do not implement the accepted
development replacement policy. Its remaining reconciliation is tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#reconciliation-items).

---

## 8. Credit factor model schema versioning

**Authority:** [ADR 0012](docs/decisions/standards/0012-serialized-contracts.md)
governs the wire contract, [ADR 0004](docs/decisions/project/0004-development-compatibility.md)
governs development replacements, and
[ADR 0041](docs/decisions/standards/0041-type-invariants-and-validation-boundaries.md)
governs declared validity and explicit validation boundaries.

**Detailed decision:** [0047](docs/decisions/architecture/0047-credit-factor-wire-and-validation-contract.md)
accepts the concrete wire and loader contract, including the required typed
`schema` field and marker rejection during deserialization. Implementation
and semantic-validation coverage remain subject to verification.
[Clause map W01–W09](docs/decisions/INVARIANT_MAP.md#8-credit-factor-model-schema-versioning).

The retained `CreditFactorModel` contract uses a namespaced string marker
rather than the numeric marker used by other result types:

```
finstack_quant.credit_factor_model/1
```

**Required:**

- Require the serialized `schema` field, typed as `CreditFactorModelSchema`,
  with `CreditFactorModelSchema::CURRENT` identifying the exact marker above.
  Reject missing, malformed and unsupported markers during deserialization;
  this does not promise first-error ordering when other fields are malformed.
  Do not retain `schema_version` as an alias or a second format.
- The v1 root is closed: unknown root fields are rejected by the Rust wire
  type and schema. The marker identifies this contract version exactly.
- The schema remains at
  [credit_factor_model.schema.json](finstack-quant/models/schemas/factor_model/1/credit_factor_model.schema.json),
  with the published URI under `https://finstack_quant.dev/schemas/factor_model/1/`.
  Moving Rust ownership must not change that stable identifier.
- Ordinary Rust deserialization does not call `CreditFactorModel::validate()`
  or establish complete semantic validity. Consumers MUST validate the current
  state, or use a validating loader, before financial use and after relevant
  mutation. Python/WASM JSON loaders and calibration entry points MUST retain
  this boundary. `from_json` performs semantic validation; `from_slice_strict`
  additionally applies resource limits and structured contract diagnostics.
  Semantic validation alone does not establish resource-limited loading.
- Authorized development root changes replace v1 in
  place and MUST update every in-workspace consumer and generated artifact in
  the same slice. No compatibility branch or alternate marker is retained.
  Final-release compatibility remains the separate ADR 0004 decision.
- Only explicitly open nested extension types that omit both
  `deny_unknown_fields` and schema-level `additionalProperties: false` may
  accept new keys without a root version bump. `CreditCalibrationDiagnostics`
  is one such extension point: unknown keys are accepted and discarded, without
  arbitrary extension-data round-trip preservation. Known fields retain their
  requiredness and meaning; `fold_ups` is used during decomposition. Additions
  that change computation require a coordinated semantic contract change.
- Field removal, type change, semantic change, required-field addition, or
  root-key addition MUST be made at the canonical Rust serde type, followed by
  deterministic v1 schema regeneration and complete in-workspace propagation.

**Evidence:** the [factor hierarchy](finstack-quant/factor/src/credit/hierarchy.rs)
currently uses `CreditFactorModelSchema::CURRENT` in the typed `schema` field;
plain deserialization rejects unsupported marker values. The same source's
`from_slice_strict` and `from_json` loaders also call semantic validation.
Its unit tests and the [schema-contract tests](finstack-quant/factor/tests/schema_contract.rs)
exercise closed roots and intentionally open diagnostics.

ADR 0047 supersedes the former `SCHEMA_VERSION`, `schema_version`, and
validation-only marker-rejection descriptions. Remaining work includes
current-state validation at consumer boundaries and after mutation, coverage
of computational `fold_ups` fields, and guidance reconciliation. The inspected
validator does not check `diagnostics.fold_ups`; decomposition checks an
out-of-range level later. Located validation calls and tests do not prove
complete enforcement. The Serde open-object guidance, Python macro wording
and root-field test wording now follow ADR 0047; the remaining enforcement
work is recorded in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#reconciliation-items).

---

## 9. Authority and enforcement

The ownership model is defined by
[ADR 0013](docs/decisions/project/0013-documentation-sources.md) and
[ADR 0034](docs/decisions/standards/0034-code-standards-authority.md):

| Source | Responsibility |
| --- | --- |
| Accepted ADR | Agreed policy, scope, and rationale. |
| This catalogue or the designated API documentation | Precise operational contracts at the appropriate scope. |
| Canonical Rust wire types, parity ledger, and derived schemas | The designated structural sources under [ADR 0012](docs/decisions/standards/0012-serialized-contracts.md) and [ADR 0005](docs/decisions/standards/0005-python-wasm-parity.md), with their generated representations. |
| Types, lints, configuration, and tests | Implementation and enforcement evidence for those requirements. |
| [FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md) | Outstanding implementation, reconciliation, verification, and unresolved scope. |

**Detailed evidence decision:**
[0048](docs/decisions/project/0048-reference-provenance-and-version-pinning.md)
accepts the catalogue-wide evidence attribution and status rules below.

Each detailed rule has one canonical home; other documents summarize and link
to it. A specifically accepted decision resolves a conflicting rule within
its scope. Where no accepted decision resolves the conflict, preserve the
competing evidence and obtain the scoped decision required by
[ADR 0015](docs/decisions/project/0015-authorized-change-scope.md). Code,
generated output, or passing self-modified tests cannot authorize a change.

Evidence claims identify their backing type, lint, schema, or test and the
conditions covered. Keep policy acceptance, implementation observation and
executed validation distinct. Source inspection or a located test does not
establish a passing run or complete enforcement. This distinction does not
require methodology-style status headers in every document.
The [clause map G01–G09](docs/decisions/INVARIANT_MAP.md#9-authority-and-enforcement)
records the corresponding decisions, including the catalogue's evidence and
scope rules. Every detailed invariant has an accepted correspondence; a related
link does not expand that decision's scope or approve unrelated methodologies.
Implementation, reconciliation and verification remain in FOLLOW_UPS.md.

Incomplete enforcement is not behavior callers can assume already holds.
Keep migration tasks and temporary verification status in FOLLOW_UPS.md, with
a short limitation here when it affects safe use of the contract. Independent
protection of contracts and their checks remains governed by
[ADR 0016](docs/decisions/project/0016-independent-contract-protection.md).

---

## 10. References

**Decisions:** [0014](docs/decisions/project/0014-financial-reference-evidence.md)
and [0026](docs/decisions/financial/governance/0026-methodology-decisions-and-inventory.md)
are accepted for financial evidence and methodology documentation;
[0048](docs/decisions/project/0048-reference-provenance-and-version-pinning.md)
is accepted for exact edition, effective calibration/parameter, convention
and benchmark pinning. A bibliography entry does not adopt a methodology or
authorize upgrading the implementation to that edition.
[Clause map R01–R02](docs/decisions/INVARIANT_MAP.md#10-references).

- [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) — public
  API and pre-1.0 compatibility semantics.
- [NVIDIA, Floating Point and IEEE 754](https://docs.nvidia.com/cuda/archive/12.4.1/floating-point/index.html)
  — non-associativity, FMA, and reduction-order reproducibility.
- [JSON Schema object reference](https://json-schema.org/understanding-json-schema/reference/object)
  — `additionalProperties` and closed-object behavior.
- [BCBS 279, *The standardised approach for measuring counterparty credit risk exposures*](https://www.bis.org/publ/bcbs279.pdf),
  paragraphs 143–145 — NICA and margined replacement cost.
- [2020 Global Investment Performance Standards for Firms](http://www.gipsstandards.org/wp-content/uploads/2021/03/2020_gips_standards_firms.pdf)
  — external cash flows and time-weighted performance.
- [ISDA SIMM v2.8+2512](https://www.isda.org/2026/06/12/isda-publishes-isda-simm-methodology-version-2-8-2512/)
  — effective 11 July 2026. SIMM code and goldens MUST pin their implemented
  methodology and effective parameter set, including any overrides, rather
  than relying on “current”. Use a published calibration tag where available;
  otherwise identify the exact edition and immutable source/parameter set.
  The implemented and test-pinned edition is historical v2.6; citing v2.8+2512
  here does not adopt it or authorize a methodology upgrade.
- [AFMA, *Interest Rate Options Conventions* (June 2025)](https://www.afma.com.au/getattachment/Standards/Market-Conventions/Sections/Content/Interest-Rate-Options-Conventions-2025-06.pdf?lang=en-AU)
  — market-specific Actual/365 quotation practice; not a universal global
  volatility-time rule.
- Hagan, P. S., & West, G. (2006) — monotone-convex interpolation.
- Brigo & Mercurio (2006), Andersen (2008) — HW1F and QE-Heston.

Model-specific modules and golden fixtures MUST cite the exact edition,
calibration, convention, and benchmark used, with genuinely inapplicable
fields identified explicitly. Parameter provenance includes the effective
overrides or an immutable identity for the resolved parameter set. A wire-format
schema version does not identify the financial methodology or calibration;
this policy does not require a new runtime metadata API. The inspected SIMM
implementation exposes historical v2.6; reconciliation of bibliography and
provenance guidance remains in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
