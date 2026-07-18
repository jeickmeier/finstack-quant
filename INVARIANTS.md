# finstack Workspace Invariants

This file is the authoritative source for cross-crate financial and
numerical contracts. It distinguishes rules that hold today from migration
targets and review practices:

- **Enforced** — backed by types, lints, schemas, or tests.
- **Required** — normative for new or modified code.
- **Migration target** — the intended contract is not yet workspace-wide.
- **Process policy** — a review or release rule rather than runtime behavior.

When code and this file disagree, do not silently follow either one. Determine
the intended public contract, then update the code, tests, bindings, schemas,
and this file together.

---

## 1. Money and model numerics

### 1.1 Representation

**Enforced:** `finstack_quant_core::money::Money` stores
`rust_decimal::Decimal` plus a `Currency`. `Money::new` and
`Money::try_new` accept `f64`; `Money::amount()` returns `f64`.
`Money::from_decimal` and `Money::amount_decimal()` provide the lossless
Decimal path.

**Required:**

- Curves, rates, volatilities, correlations, returns, greeks, optimizers, and
  Monte Carlo paths MAY use `f64`.
- **New** accounting, ledger, settlement, regulatory-reporting, and
  margin-dispute paths MUST use `Decimal` or `Money` without an intermediate
  `f64` round-trip. Existing `f64` aggregation paths are enumerated migration
  targets (below), not license for new f64 settlement-grade code.
- Wrapping an `f64` result in `Money` gives it currency semantics and Decimal
  storage; it does **not** make the preceding calculation Decimal-exact.
- `Money::new` and `Money::try_new` preserve sub-minor-unit precision. They do
  not imply ISO-4217 quantization. Currency-scale rounding MUST be requested
  through configuration or applied explicitly when an amount is finalized.
- Non-finite monetary inputs MUST be rejected.

### 1.2 Arithmetic and aggregation

**Required:**

- Use Decimal arithmetic where rounding affects a contractual amount, posted
  balance, settlement instruction, regulatory submission, or dispute.
- `f64` aggregation is acceptable for model PV, risk, and analytical totals
  when the boundary and rounding policy are documented.
- Use compensated summation for mixed-sign cash flows, large dynamic ranges,
  or totals whose floating-point error can affect a reported result. Use
  `finstack_quant_core::math::{neumaier_sum, NeumaierAccumulator}` or
  `OnlineStats` as appropriate.
- Validate finiteness before Kahan/Neumaier accumulation. Compensation does
  not make `NaN` or infinity safe.

**Migration targets (existing `f64` paths, tracked per function):**

- cashflow schedule splitting and aggregation after the Decimal accrual
  product (`finstack-quant-cashflows` aggregation);
- portfolio and margin totals that aggregate `Money::amount()` in `f64` and
  re-wrap with `Money::new` (portfolio valuation, schedule IM);
- statement evaluator arithmetic, which computes on `f64` even when node
  values are `AmountOrScalar::Amount(Money)`.

For each path, either carry Decimal through finalization or document it as an
intentional `f64` model boundary with its rounding policy.

---

## 2. Determinism and floating point

### 2.1 Reproducibility tiers

Every public stochastic API MUST document one of these tiers:

1. **Bit-reproducible** — identical bits for supported serial/parallel modes
   and thread counts.
2. **Seed-reproducible** — stable for a documented seed, execution mode, and
   platform; other execution modes may differ numerically.
3. **Statistically reproducible** — distributional properties are tested, but
   bit identity is not promised.

Do not claim cross-host bit identity unless it is tested across the supported
targets and mathematical-library implementations.

### 2.2 Random number generation

**Required:** library code MUST use explicit seeds. Do not use
`thread_rng()` or `rand::random()`.

**Enforced for the primary Monte Carlo engine:** bit-identical
serial/rayon results require all of the following:

- a splittable stream such as `PhiloxRng::split(path_id)` or
  `substream(path_id)`;
- a work partition that is a pure function of the path count;
- a fixed merge order independent of rayon thread count; and
- adaptive confidence-interval stopping disabled.

Philox alone is not the guarantee. Non-splittable sequences such as the
current Sobol implementation do not satisfy this parallel tier. Halton
multi-start calibration is deterministic by construction.

### 2.3 Map ordering

**Required:**

- `std::collections::HashMap` uses a per-instance random seed and exposes
  arbitrary iteration order.
- `finstack_quant_core::HashMap` is an FxHash map with deterministic hashing
  for a given insertion sequence, but its iteration order is still not a
  serialization or API contract.
- Any map order visible through serialization, files, public iterators,
  snapshots, or golden tests MUST use `IndexMap` for insertion order or
  `BTreeMap` for sorted order.

Hash maps MAY be used for internal caches, intern tables, and lookups whose
order cannot escape.

### 2.4 Reductions

Floating-point addition is commutative for finite ordinary operands but is
not associative. A parallel `+` fold is therefore not automatically
deterministic.

**Required:**

- Reproducible parallel reductions MUST use deterministic partitions and a
  fixed merge tree.
- `OnlineStats::merge`, Welford/Chan statistics, and compensated sums still
  depend on merge order; use them inside a fixed tree.
- Compensated summation improves accuracy. It does not independently provide
  bit reproducibility.
- Do not use an arbitrary term-count threshold as a substitute for analysing
  sign cancellation, dynamic range, and materiality.

### 2.5 Comparisons

**Required:** approximate model results MUST use a documented tolerance with
explicit semantics: absolute, relative, notional-scaled, probability, rate,
percentage, money half-unit, or another named domain tolerance.

Exact `f64` equality is allowed only when exact IEEE semantics are intended,
including bit-reproducibility tests, exact-zero sentinels, and values known to
come from the same deterministic operation. Do not cite a shared comparison
helper unless that helper exists in the public API.

---

## 3. Sign and perspective conventions

Signs describe a perspective, not an intrinsic property of an amount.
Interfaces MUST name their perspective and MUST NOT pass an amount between
perspectives without an explicit conversion.

### 3.1 Canonical perspectives

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

- Dietz flows use `PortfolioExternalFlow`: contribution in is positive.
  XIRR/MWR uses `EconomicCashFlow`: contribution in is negative. Conversion
  requires `xirr_amount = -dietz_amount`.
- Real-estate CapEx schedules contain positive `StatementMagnitude` outflows.
  The pricer owns the single negation into `EconomicCashFlow`. Passing
  already-negative CFS CapEx into that schedule is invalid because it
  double-negates the outflow. **Enforced:** `RealEstateAsset::validate()`
  rejects negative `capex_schedule` amounts.
- Retained-earnings dividends are positive magnitudes and are subtracted by
  the identity. Corkscrew equity reductions are negative
  `RollForwardActivity`.
- Roll-forward `increases` and `decreases` are positive magnitude buckets;
  converting to a corkscrew produces `+increase` and `-decrease`.
- SA-CCR collateral and NICA are signed net amounts. Positive means the bank
  holds collateral. Replacement cost uses
  `max(V - C, TH + MTA - NICA, 0)`.
- CDS option Call means payer protection; Put means receiver protection.
- Portfolio quantity greater than zero means long. P&L attribution is
  positive for a gain to the long holder. Sensitivity signs follow the
  relevant `MetricId` documentation.

**Migration target:** introduce named conversion helpers or typed wrappers for
the high-risk Dietz/XIRR, statement/DCF, and roll-forward/corkscrew
boundaries. Binding documentation MUST state the same polarity as Rust.
Avoid asset-class-ambiguous `long`/`short` aliases for `PayReceive`; prefer
`pay`, `receive`, or an explicit protection side.

---

## 4. Date, day-count, and time conventions

A single instrument can require distinct clocks. Do not infer one clock from
another.

### 4.1 Clock roles

| Clock | Rule |
|-------|------|
| Discount | Obtain the exact relative discount factor from curve dates and the curve's own base date/day-count. |
| Projection | Obtain forwards from the projection curve's date-based APIs and conventions. |
| Accrual | Compute coupon accrual from the instrument or leg day-count. |
| Option/volatility | Use the time basis on which the volatility surface or model was calibrated. |

Act/360 is commonly an accrual convention; it is not a universal rates-option
time basis. Current swaption, cap/floor, and CMS option paths use Act/365F for
option time. Equity/FX/commodity defaults are often Act/365F, but the
surface-declared basis is authoritative.

### 4.2 Discount-factor bridge

When a single-time model needs a continuously compounded rate on model time
`t_model`, derive it from the exact curve discount factor:

```text
r_model = -ln(df_curve) / t_model
exp(-r_model * t_model) = df_curve
```

Discount with the exact curve factor whenever the model permits. Shared
plumbing currently lives under
`finstack_quant_valuations::instruments::common_impl::two_clock` and related
date-based pricing helpers; it is crate-private and not yet adopted by every
pricer.

### 4.3 Date boundaries

**Required:**

- Valuation `as_of`, curve `base_date`, trade date, spot/settlement date,
  fixing date, and payment date MUST remain distinct.
- Rebase seasoned curves with relative date-based discount factors.
- Contractual settlement MUST apply the relevant spot lag, calendars,
  business-day convention, and end-of-month rule.
- Past resets MUST use the configured fixing series; do not silently project
  a historical fixing from a curve.
- Credit survival time and discount time MAY use different day-counts and
  MUST be bridged through date-based survival probabilities and discount
  factors.

**Enforced:** autocallable and cliquet Monte Carlo pricers measure model/vol
time on the instrument day count and bridge exact date-based curve discount
factors onto model time (regression:
`test_vol_clock_independent_of_curve_day_count`).

**Migration target:** volatility surfaces should carry explicit time-basis
metadata so the calibration clock is data, not convention.

Do not publish numerical error bands such as basis-point drift unless a
reproducible benchmark or golden test defines the setup and provenance.

---

## 5. Error handling

### 5.1 Enforced lints

- Rust library crates deny `unwrap_used`, `expect_used`, `panic`, and
  `unreachable` through crate-level attributes on library targets.
- The Python and WASM binding crates deny `unwrap_used`, `expect_used`, and
  `panic`; they do not currently deny `unreachable`.
- Workspace lint configuration denies `match_wild_err_arm`. Match public error
  enums explicitly so a new variant forces review.

Tests, examples, and benchmarks may use panic-based assertions where that is
the purpose of the target.

### 5.2 Required failure behavior

- Library code MUST NOT panic for invalid user input, market data, model
  state, or ordinary numerical failure.
- Public fallible operations MUST return a domain error with enough context to
  identify the attempted operation, the failed condition, and the input or
  identifier needed to diagnose it.
- Error documentation and binding mappings MUST name the public error callers
  should handle.
- Do not refer to error type names that do not exist. Apply this structural
  rule to the actual public enums, such as `PricingError` and
  `finstack_quant_scenarios::Error`.

### 5.3 Indexing

**Migration target:** `indexing_slicing` is not denied workspace-wide.
Unchecked indexing can still panic despite the panic-family lints. New code
reachable from user-controlled shapes or indices SHOULD use `.get()`,
`.first()`, checked split operations, or explicit precondition validation.
Numerical kernels may index directly only when the invariant is established
locally and covered by tests.

---

## 6. Test discipline

This section is **process policy**, not a runtime invariant.

- A bug fix MUST include a regression test that fails without the fix.
- The test name, doc comment, or assertion message MUST state the failure mode
  being protected.
- Run the smallest targeted checks while iterating. Run workspace-wide checks
  once after targeted checks are clean when the change scope requires them.
- Golden values MUST record provenance, methodology/version, valuation date,
  conventions, inputs, tolerances, and expected units. Provenance may live in
  fixture metadata, the test header, or the golden-suite README.
- Rust and Python golden suites may share fixtures. Do not require all goldens
  to live directly under a Rust `<crate>/tests/` directory.
- Determinism claims require tests over every promised execution mode and
  thread-count class. Tolerance tests do not establish bit reproducibility.

---

## 7. Public API, bindings, and deprecation

### 7.1 Host-surface parity

**Required:**

- Rust is the canonical API design.
- A public Rust change MUST update every published host surface that already
  exposes the changed symbol in the same change set: PyO3 registration,
  Python stubs and exports, WASM bindings and facade declarations, examples,
  and parity contracts as applicable.
- Python tracks the broad Rust surface defined by
  `finstack-quant-py/parity_contract.toml`.
- WASM is an opt-in subset. A Rust or Python addition outside the declared
  WASM subset does not require a new WASM export. Changes to the subset MUST
  update `[wasm_core_subset]` or the relevant parity section.
- Host-language names and behavior MUST follow the Rust contract unless an
  explicit, documented host-language constraint requires a difference.

### 7.2 Deprecation policy

Semantic Versioning treats `0.y.z` APIs as unstable. finstack applies a
stronger project policy to reduce avoidable consumer breakage.

**Enforced** (`scripts/check_deprecated_annotations.py`, run by
`mise run rust-doc`): every `#[deprecated]` annotation MUST state:

1. the release in which the deprecation began;
2. the replacement API (or an explicit retention rationale); and
3. the earliest planned removal release.

```rust
#[deprecated(
    since = "0.6.0",
    note = "use `replacement_api` instead; removable in 0.8.0"
)]
pub fn legacy_api(...) -> Result<Output> { ... }
```

Before 1.0, the default notice window is one intervening minor release:
deprecated in `0.y`, callable through `0.(y+1)`, and removable in
`0.(y+2)`. A shorter window requires a CHANGELOG migration note explaining
why continued exposure is unsafe, materially incorrect, or impossible to
maintain.

At 1.0 and later, incompatible removal requires the next major version unless
the published compatibility policy explicitly permits otherwise.

---

## 8. Credit factor model schema versioning

`CreditFactorModel` uses a string schema identifier rather than the `u32`
pattern used by result types. The canonical version is:

```
finstack_quant.credit_factor_model/1
```

**Enforced:**

- `CreditFactorModel::SCHEMA_VERSION` lives in
  `finstack_quant_factor_model::credit::hierarchy`.
- The v1 root Rust type uses `#[serde(deny_unknown_fields)]`.
- The v1 root JSON schema uses `"additionalProperties": false` and a
  `"const"` schema-version value.
- The schema file is
  `finstack-quant/valuations/schemas/factor_model/1/credit_factor_model.schema.json`.
- Compatibility semantics are locked by unit tests: an unknown root key fails
  to deserialize, an unknown `CalibrationDiagnostics` key succeeds, and a
  wrong `schema_version` deserializes but fails `validate()`
  (`unknown_root_key_is_rejected_but_diagnostics_extension_is_accepted`,
  `wrong_schema_version_deserializes_but_fails_validate`).

**Required:**

- Rust deserialization does not call `CreditFactorModel::validate()`
  automatically. Consumers MUST call `validate()` or use a public loader that
  does so before trusting the model. Python/WASM JSON loaders and calibration
  entry points MUST retain this validation.
- Adding a root key is a forward-compatibility break for older v1 readers,
  even when the new Rust field is optional and has `#[serde(default)]`.
  Root additions therefore require a new schema version.
- Only explicitly open nested extension types that omit both
  `deny_unknown_fields` and schema-level `additionalProperties: false` may
  accept new keys without a root version bump. `CalibrationDiagnostics` is
  one such extension point.
- Field removal, type change, semantic change, required-field addition, or
  root-key addition requires a new identifier such as
  `finstack_quant.credit_factor_model/2`, a schema under
  `finstack-quant/valuations/schemas/factor_model/2/`, migration guidance,
  and compatibility tests.
- `#[serde(default)]` lets newer readers consume older payloads. It does not
  make older closed readers accept newer payloads.

`docs/SERDE_STABILITY.md` MUST use the same compatibility terminology and
canonical module paths.

---

## 9. Authority and enforcement

Use the narrowest applicable source of truth:

1. public financial or serialization contract in this file;
2. public API documentation and type system;
3. parity contract or JSON schema;
4. executable contract, regression, and golden tests;
5. migration notes and process policy.

An **Enforced** claim in this file MUST identify a backing type, lint, schema,
or test. A **Migration target** MUST not be presented as behavior callers can
rely on today. Snapshot counts and temporary implementation status belong in
issues or plans, not permanent invariants.

---

## 10. References

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
  methodology and calibration tag rather than relying on “current”.
- [AFMA, *Interest Rate Options Conventions* (June 2025)](https://www.afma.com.au/getattachment/Standards/Market-Conventions/Sections/Content/Interest-Rate-Options-Conventions-2025-06.pdf?lang=en-AU)
  — market-specific Actual/365 quotation practice; not a universal global
  volatility-time rule.
- Hagan, P. S., & West, G. (2006) — monotone-convex interpolation.
- Brigo & Mercurio (2006), Andersen (2008) — HW1F and QE-Heston.

Model-specific modules and golden fixtures MUST cite the exact edition,
calibration, convention, and benchmark used.
