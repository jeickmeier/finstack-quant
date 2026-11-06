# Follow-ups and reconciliations

This file tracks implementation gaps, conflicting guidance, and unresolved
scope associated with the [project decisions and drafts](README.md). Listing
work here does not mark it complete or approve a new exception.

This register owns mutable work for the numbered decisions: implementation
discrepancies, guidance reconciliation, investigations, missing evidence and
pending policy questions. ADRs retain the decision, rationale, scope, lasting
limitations and verification criteria, with links here instead of duplicated
action lists. Detailed financial-methodology work is owned by the
[scoped algorithm backlog](financial/FOLLOW_UPS.md); cross-domain coordination
belongs here. Keep each action in one register entry and link related records.

## Reconciliation items

These items identify conflicting guidance or unresolved scope. Listing an item
does not complete its reconciliation or approve a new exception. Where an
accepted ADR defines the intended correction, apply that decision; otherwise
resolve the stated scope question explicitly.

1. **Legacy deprecation enforcement and remaining guidance.**
   [INVARIANTS.md section 7.2](../../INVARIANTS.md#72-deprecation-policy) follows
   [ADR 0004](project/0004-development-compatibility.md): approved development
   replacements remove obsolete paths. The
   [annotation checker](../../scripts/check_deprecated_annotations.py) still
   runs in `rust-doc` and cites section 7.2 while requiring `since = "X.Y.Z"`,
   a replacement pointer or retention rationale, and an earliest removal
   release in each deprecation note. It checks annotation content, not the
   former notice window. Reconcile that check, its diagnostic wording, and
   remaining guidance with the accepted policy without silently disabling a
   protected gate. The former rule retaining APIs through `0.(y+1)` after
   deprecation in `0.y`, with removal in `0.(y+2)`, is superseded; it is not the
   final-release strategy. That strategy remains a separate lifecycle task.
2. **Python/WASM parity guidance and coverage.**
   [INVARIANTS.md section 7.1](../../INVARIANTS.md#71-host-surface-parity) follows
   [ADR 0005](standards/0005-python-wasm-parity.md). Audit remaining guidance,
   ledger exclusions, and behavioral coverage against corresponding interfaces
   for selected financial capabilities, explicit justified host exceptions,
   and the protected zero-backlog requirement. Preserve the distinction from
   exposing every internal Rust function. Surface checks alone do not prove
   semantic parity or independent protection of their own ceilings.
3. **WASM error instructions.**
   [AGENTS.md](../../AGENTS.md) and the simplify skill references
   ([refactor-tactics.md](../../.agents/skills/finstack-simplify/references/refactor-tactics.md),
   [slop-patterns.md](../../.agents/skills/finstack-simplify/references/slop-patterns.md))
   now specify `crate::utils::to_js_err`, matching the
   [boundary checker](../../scripts/check_wasm_boundary.py) and
   [ADR 0006](standards/0006-structured-errors.md). Audit remaining contributor
   and binding docs for any stale bare-string error examples.
4. **Cross-target reproducibility claims.**
   The [models README](../../finstack-quant/models/README.md), under
   "Determinism and parallelism", claims native/WASM bit identity.
   The [Monte Carlo run-metadata documentation](../../finstack-quant/models/src/monte_carlo/results.rs)
   and [engine pricing documentation](../../finstack-quant/models/src/monte_carlo/engine/pricing.rs)
   also promise host-independent bit reproduction; include them in the same
   reconciliation. The inspected serial/parallel and thread-count tests
   establish evidence for same-target conditions, not every host or machine.
   [INVARIANTS.md](../../INVARIANTS.md), section 2.1, limits reproducibility
   tiers per target and requires justified tolerances across hosts unless
   stronger guarantees are tested.
   [ADR 0021](standards/0021-reproducibility-guarantees.md) now defines the accepted
   policy: limit bit-identity promises to demonstrated target and execution
   conditions, and verify numerical parity with justified tolerances otherwise.
   Correct the guidance and reconcile its evidence; that work remains pending.
   This conflict is not proof of a numerical defect, and ADR 0005 does not
   imply blanket bit-identical floating-point results.
5. **Option-bearing bond CS01 scope.**
   The [bond CS01 implementation](../../finstack-quant/valuations/fixed-income/src/bond/metrics/cs01.rs)
   has a model-specific OAS branch when the active option-bearing model does
   not consume a credit curve. Clarify its relationship to the supported-bond
   z-spread rule in [ADR 0011](financial/valuations-bonds/0011-bond-cs01-without-hazard-curve.md).
   The existing branch is neither silently approved as an exception nor
   classified as a defect by this record.
6. **Code-standard guidance and configuration.**
   Align contributor and agent guidance with the accepted policy and shared
   executable definitions under
   [ADR 0034](standards/0034-code-standards-authority.md). The
   [Rust standards guidance](../../.agents/rules/rust/code-standards.md)
   now reflects the [ADR 0036](standards/0036-safe-rust-and-recoverable-failures.md)
   unsafe-code prohibition and fallible constructor validation, and the
   [error guide](../../.agents/rules/rust/error-handling.md) now specifies the
   centralized, kind-aware binding adapters required by
   [ADR 0006](standards/0006-structured-errors.md) and
   [ADR 0049](standards/0049-explicit-public-error-matching.md). The
   Rust standards guidance has also been reconciled with
   [ADR 0004](project/0004-development-compatibility.md) (compatibility),
   [ADR 0020](financial/conventions/0020-numerical-representation-and-rounding.md),
   [ADR 0021](standards/0021-reproducibility-guarantees.md) and
   [ADR 0042](standards/0042-numerical-accumulation-and-comparisons.md)
   (determinism and summation), and ADR 0034 (lint definitions live in the
   workspace manifest). The error guide's crate inventory is now a snapshot
   to re-check, and the
   [testing guidance](../../.agents/rules/rust/testing-standards.md) now
   follows [ADR 0048](project/0048-reference-provenance-and-version-pinning.md)
   for fixture location and provenance and names the actual `ResultsMeta`
   fields. Continue
   auditing remaining contributor guidance and crate-level lint configuration for
   any further drift; ADR 0034 does not independently approve every legacy
   instruction or current configuration setting.

<a id="credit-factor-guidance"></a>

7. **Credit factor schema guidance and extension scope.**
   [ADR 0047](architecture/0047-credit-factor-wire-and-validation-contract.md)
   is accepted on 2026-11-05: require typed `schema` with
   `CreditFactorModelSchema::CURRENT`, reject missing, malformed or unsupported
   markers during deserialization, and retain no `schema_version` alias.
   Rejection before successful decoding does not promise first-error ordering.
   The former representation/failure-timing discrepancy is settled; enforcement
   and guidance corrections remain open under
   [INVARIANTS.md section 8](../../INVARIANTS.md#8-credit-factor-model-schema-versioning).
   Reconcile the [Serde policy's](../SERDE_STABILITY.md#object-strictness)
   extension-map wording with the narrow, explicitly open
   `CreditCalibrationDiagnostics` object. Unknown diagnostic keys are accepted
   and discarded; known fields retain their requiredness and meaning, including
   computational `fold_ups`. Additions affecting computation require a
   coordinated semantic contract change. Qualify the Python
   [`rust_json` macro documentation](../../finstack-quant-py/src/bindings/macros.rs)
   so unknown-field rejection applies according to each object's declared
   closure. The [hierarchy's](../../finstack-quant/factor/src/credit/hierarchy.rs)
   unknown-root-field test still says an additive root field requires a new
   schema version; align that wording with authorized development v1
   replacement while retaining rejection of unrecognized fields. Preserve the
   stable schema path/URI and the separate final-release compatibility decision.

## Implementation and lifecycle follow-up

- **Binding ownership coverage:** under
  [ADR 0001](architecture/0001-rust-owned-financial-behavior.md), audit bindings
  for duplicated financial calculations, business validation, and defaults.
  Existing boundary and parity checks provide partial enforcement; complete
  shared behavioral coverage and review semantic duplication that mechanical
  checks cannot reliably detect.
- **Dependency-policy enforcement:** under
  [ADR 0002](architecture/0002-crate-boundaries.md), create the complete checked-in
  dependency policy and wire validation of Cargo's graph, including optional
  features and explicit test-only allowances, into required checks. Audit the
  current graph; accepting the policy does not establish that this gate is
  installed or that the graph has been exhaustively reviewed.
- **Canonical naming coverage:** under
  [ADR 0003](standards/0003-canonical-public-names.md), extend Rust-to-host naming
  checks across namespaces and their documented exceptions. The existing
  [Rust-name parity test](../../finstack-quant-py/tests/parity/test_rust_names.py)
  checks the Python `models` surface; passing host-to-host parity alone does not
  establish complete correspondence with canonical Rust names. Preserve
  contract-change reporting even when the two host surfaces agree.

<a id="cagr-default"></a>

- **CAGR default:** [ADR 0008](financial/conventions/0008-financial-defaults-and-cagr.md)
  explicitly chooses Actual/365 Fixed. The
  [dated-CAGR reconciliation](financial/FOLLOW_UPS.md#dated-cagr-default) owns
  implementation, dependent-calculation and regression work.
- **Final-release compatibility:** before the first final release, revisit
  [ADR 0004](project/0004-development-compatibility.md) and establish
  compatibility across a couple of releases. The exact support window,
  deprecation process, and migration strategy remain open.
- **Independent enforcement:** the trusted-baseline and independently
  authorized protection required by
  [ADR 0016](project/0016-independent-contract-protection.md) still need
  implementation and verification. These docs do not establish that live
  repository permissions or checks provide it.
  The [parity backlog test](../../finstack-quant-py/tests/parity/test_parity_ledger.py)
  reads its ceiling from the same candidate
  [ledger](../../finstack-quant-py/parity_contract.toml), so raising both can pass
  that individual check. Generation checks in [mise.toml](../../mise.toml)
  verify source/output consistency rather than change authorization.
  [CONTRIBUTING.md](../../CONTRIBUTING.md) documents a single-maintainer setup
  without required approving reviews, and [CODEOWNERS](../../.github/CODEOWNERS)
  notes the limitation when the owner authors the change. Verify the live
  repository settings and the independent approval boundary; this documentation
  cleanup did not inspect or change those settings.
- **Independent library and presentation use:** audit the existing headless
  and installed-consumer checks against
  [ADR 0017](project/0017-library-first-product.md), including display-only
  components that do not require WASM or a computation provider. Fill any
  coverage gaps. The [UI removal guide](../REMOVING_THE_UI.md) still documents
  test and generation seams that must be adjusted during removal; this ADR
  does not establish that deleting the UI directory alone works.
- **Complete consumer support matrix:** under
  [ADR 0018](project/0018-platform-and-runtime-support.md), specify consumer
  version commitments and verification levels across Rust, Python, WASM, and
  UI surfaces, including the still-open browser and Node versions. Preserve
  the initial Python wheel baseline and its explicit Linux arm64 build-only
  qualification. Promoting a target's verification level requires supporting
  runtime evidence; development-tool pins alone do not define that commitment.
- **Exact final-artifact qualification:** define the concrete release
  criteria, financial validation, and blocking findings required by
  [ADR 0019](project/0019-final-release-qualification.md). Connect validation
  and approval to the exact source revision and shipped artifact identities,
  including final-version rebuilds. Verify enforcement for candidate promotion
  and direct-from-`master` releases; existing source-level gates alone do not
  demonstrate this requirement is complete.

<a id="monetary-boundaries"></a>

- **Monetary precision and rounding:** classify and resolve the existing
  `f64` paths under
  [ADR 0020](financial/conventions/0020-numerical-representation-and-rounding.md) and the
  detailed rules in [INVARIANTS.md section 1](../../INVARIANTS.md#1-money-and-model-numerics):
  cashflow schedule splitting and aggregation after the Decimal accrual
  product; portfolio and margin totals aggregating `Money::amount()` in `f64`
  and re-wrapping with `Money::new` (including portfolio valuation and schedule
  IM); and statement evaluator arithmetic on `f64` even when node values are
  `AmountOrScalar::Amount(Money)`. Identify the affected functions and either
  carry Decimal through finalization or document an explicitly justified model
  boundary with its conversion and rounding policy. Verify contractual
  rounding stages, compensated aggregation where required, and finiteness
  before accumulation. The listed paths are work to classify and resolve,
  not evidence of compliant monetary arithmetic.
  The 2026-11-05 amendment also requires verification of sub-minor-unit
  construction precision, explicit currency-scale rounding and non-finite
  monetary-input rejection across supported construction boundaries.
- **Accumulation and comparison coverage:** under accepted
  [ADR 0042](standards/0042-numerical-accumulation-and-comparisons.md), review
  floating-point totals for the mixed-sign, dynamic-range and materiality
  criteria and validate inputs before compensated accumulation. Keep online
  mean/variance separate from totals and preserve required Decimal paths under
  ADR 0020. Verify domain-specific tolerance semantics and distinguish exact
  numerical equality from `to_bits()` checks for bit-identity promises.
  Acceptance records the standard; caller compliance, meaningful numerical
  cases and protected-tolerance review remain open.
- **Reproducibility coverage:** inventory and verify the declared tier,
  supported targets, execution settings, and test evidence for each public
  stochastic API under [ADR 0021](standards/0021-reproducibility-guarantees.md)
  and the accepted execution rules in
  [ADR 0043](standards/0043-deterministic-ordering-and-monte-carlo-execution.md).
  Verify deterministic insertion into public ordered maps and lowest-index
  error selection. For promised bit identity, check partitions and merge
  order at fixed declared inputs/configuration, including explicit chunk
  sizes, independently of worker/thread count. Keep the adaptive-stopping
  restriction scoped to the primary engine's bit-identity mode. Verify that
  same-target baselines also record the relevant build/runtime/execution
  conditions; the target alone does not establish reproducibility.
  Address missing repeatability, thread-count, or cross-host coverage for the
  guarantees actually promised. Reconciliation item 4 tracks the conflicting
  native/WASM bit-identity wording separately. The former catalogue reported
  approximately `1e-8` relative Monte Carlo PV differences and `2e-4` relative
  carry/bucketed-DV01 differences without a pinned reproducible setup. Recover
  the inputs, target builds, and provenance or retire those unverified
  observations; do not adopt them as tolerances or error bounds. Verify
  cross-target guarantees for the `libm::hypot` EWMA path independently of the
  implementation choice. Preserve calibration-diagnostic and near-zero-Greek
  sensitivity to rounding when designing justified comparisons.

<a id="portfolio-execution"></a>

- **Portfolio execution and invalidation coverage:** audit the workflows and
  adapters covered by [ADR 0022](architecture/0022-portfolio-evaluation-state.md)
  and accepted [ADR 0046](architecture/0046-portfolio-execution-operational-contract.md)
  against the shared-executor boundary. Verify request/state isolation for
  both rate and hazard recalibration caches; distinguish executor-retained
  financial caches from explicitly supplied immutable prior results and
  immutable decoded-instrument input caches. Inspect the
  [recalibration provider](../../finstack-quant/calibration/src/recalibration/provider.rs)
  and [instrument artifact cache](../../finstack-quant/portfolio/src/materialization/cache.rs)
  against those distinct lifetimes.
  Establish complete selective-reuse compatibility, including relevant pricing
  configuration and the parent-to-updated-market relationship plus an
  authoritative change set. The public
  [selective revaluation entry point](../../finstack-quant/portfolio/src/valuation.rs)
  accepts a prior valuation and creates a new request plan. Current
  [reuse checks](../../finstack-quant/portfolio/src/evaluation/plan.rs) cover
  dates, metric/risk profiles, base currency and portfolio identity/shape, but
  stored provenance does not record originating market identity or pricing
  configuration. These checks do not establish complete compatibility. Perform
  fresh valuation when compatibility cannot be proved.
  Compare selective and fresh evaluation after changes to configuration,
  markets, effective dates, instruments, metric menus, risk policies and FX
  conversion dependencies.
  Verify each position's applicable metric subset and explicit inapplicability
  diagnostics, strict failure for applicable requested calculations, exact
  empty-menu PV-only behavior, and rejection of best-effort reuse for strict
  requests. Do not report inapplicable metrics as successful zeroes or infer
  approval of every observed metric default.
  Exercise conservative FX triangulation invalidation and conversion refresh
  for every reused position when conversion inputs change; retain reuse of
  conversion results only when their inputs are established unchanged. The
  [dependency index](../../finstack-quant/portfolio/src/dependencies.rs) is an
  evidence location for conservative dirty-set handling, not proof of complete
  conversion-input coverage.
  Verify the one-axis portfolio scheduler and ordered errors/reductions across
  promised execution modes. Instrument-internal Monte Carlo threading needs
  separate coordination and performance evidence; the portfolio scheduler
  does not establish that coverage.
  Inspect Python adapters for native-only detached closures, guards retained
  outside the closure, and attached Python object extraction/construction.
  The [shared JSON helper](../../finstack-quant-py/src/bindings/json_bridge.rs)
  `deserialize_json_with_context` detaches without first copying its incoming
  string, unlike `deserialize_json`. Reconcile it with the accepted copy rule
  and verify affected callers. This is a policy-compliance gap, not a
  demonstrated safety defect. Complete workflow and regression coverage
  remains open; existing executor structures alone do not prove compliance.

<a id="sign-conversion-boundaries"></a>

- **Sign-conversion boundaries:** preserve
  [INVARIANTS.md section 3](../../INVARIANTS.md#3-sign-and-perspective-conventions)
  under [ADR 0007](financial/conventions/0007-units-and-sign-conventions.md) and
  [ADR 0044](financial/conventions/0044-financial-perspectives-and-boundary-conversions.md). Introduce
  named conversion helpers or typed wrappers where needed for the high-risk
  Dietz/XIRR, statement/DCF, and roll-forward/corkscrew boundaries; classify
  their need under ADRs 0037 and 0041 rather than imposing wrappers everywhere.
  Verify conversions and host documentation preserve the authorized polarity.
  Verify both accepted retained-earnings conventions: `MagnitudePositive`
  remains the default, while explicitly selected `InflowPositive` adds signed
  dividends directly. Neither sign inference nor `abs()` is permitted. Negative
  magnitude inputs currently produce warnings in the
  [retained-earnings check](../../finstack-quant/statements/src/checks/builtins/retained_earnings.rs);
  resolve enforcement and test the public boundary without treating those
  warnings as strict rejection.
  Reconcile the SA-CCR [public field's](../../finstack-quant/margin/src/regulatory/sa_ccr/types.rs)
  net-collateral description with the [kernel's](../../finstack-quant/margin/src/regulatory/sa_ccr/replacement_cost.rs)
  VM-only interpretation of `config.collateral` and addition of NICA.
  Total recognized adjusted collateral `C` already includes NICA; define
  non-overlapping inputs and verify NICA is never counted twice. The
  [SA-CCR qualification entry](financial/FOLLOW_UPS.md#saccr-collateral-qualification)
  owns regulatory qualification of the margined and unmargined methodologies.
  Acceptance of the sign policy does not settle the exact public-field
  reconciliation or certify either calculation.

<a id="clock-metadata-defaults"></a>

- **Clock metadata, defaults, and adoption:** audit the detailed clock rules in
  [INVARIANTS.md section 4](../../INVARIANTS.md#4-date-day-count-and-time-conventions)
  under accepted [ADR 0045](financial/conventions/0045-financial-clocks-and-date-boundaries.md).
  The former catalogue described swaption, cap/floor, and CMS option time as
  Act/365F and equity/FX/commodity defaults as often Act/365F. Inventory and
  verify those defaults at their actual selection paths; these observations
  do not approve a universal option clock or settle the methodology drafts.
  Define the full volatility date-to-expiry mapping through metadata or an
  explicit model contract, including the reference date, time basis and any
  required calendar context. Agree representation and missing-metadata behavior
  without a universal fallback: current
  [VolSurface](../../finstack-quant/core/src/market_data/surfaces/vol_surface.rs)
  and [VolCube](../../finstack-quant/core/src/market_data/surfaces/vol_cube.rs)
  carry expiry grids without reference-date or day-count/time-basis fields.
  Verify projection uses curve date coordinates while annualizing forwards
  with the contractual index accrual fraction, as expressed by
  [rate_between_dates](../../finstack-quant/core/src/market_data/term_structures/forward_curve/curve.rs).
  Verify date-based relative discounting `D(b,T)/D(b,v)` and
  `zero_rate_from_df` adoption across pricers. The
  [rate helper](../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs)
  does not enforce finite model time and returns zero for `t <= 0` before
  validating the factor. Establish enforcement for finite positive factors
  and model time, with explicit expiry and invalid-input contracts; test the
  bridge with justified tolerances without claiming full model-dynamics
  equivalence. The old `common_impl::two_clock` reference has no current source
  counterpart.
  Define valuation/publication cutoffs and reconcile same-day fixing behavior:
  the [cashflow helper](../../finstack-quant/cashflows/src/builder/rate_helpers.rs)
  uses an available same-day observation, while
  [term cap/floor projection](../../finstack-quant/valuations/rates/src/cap_floor/pricing/projection.rs)
  assumes start-of-day and projects it. Required historical fixings must fail
  when missing; acceptance does not choose a universal same-day policy.
  Strengthen evidence for model-clock independence: the existing autocallable
  `test_vol_clock_independent_of_curve_day_count` uses a bounded comparison,
  not a general proof, and does not supply cliquet coverage. Preserve
  historical fixing, settlement, credit-survival, and separate clock rules
  while resolving any operation-specific discrepancies.
- **Tabular export and adapter coverage:** audit public tabular exports and
  their conversions under
  [ADR 0023](architecture/0023-host-neutral-tabular-exports.md). Verify column
  names/order, types, missing values, roles, and required metadata; identify
  competing contracts or dependency violations rather than assuming uniform
  compliance. Arrow import, IPC support, and zero-copy guarantees remain
  separate scope questions; the existing adapter is export-only.
- **Extension interface and persistence coverage:** document and verify which
  custom instrument/pricer capabilities work in native calls, persisted
  portfolios, and host bindings under
  [ADR 0024](architecture/0024-explicit-extension-boundaries.md). Verify
  duplicate-registration rejection, deliberate replacement, and unsupported
  persisted-type errors. Native extension support does not establish custom
  persistence or host exposure; runtime plugins, host callbacks, and external
  deserialization registries require a separate architectural decision.
- **Audit/replay evidence coverage:** inventory the saved or shared valuation
  and risk workflows covered by
  [ADR 0025](project/0025-auditable-results-and-replay.md). Define and verify
  the required input/snapshot references, effective configuration, requested
  and effective dates, producing build, and applicable stochastic settings.
  Check archival round trips, metadata preservation, reproduction under the
  declared guarantee, and explicit missing-input diagnostics. Existing
  optional provenance and snapshot types do not establish complete replay
  coverage; archival storage remains caller-owned.
- **Shared code-standard enforcement:** under
  [ADR 0034](standards/0034-code-standards-authority.md), verify that every
  crate inherits applicable standards and that local and CI checks use shared
  definitions with the required target and feature coverage. Review existing
  lint exceptions for permitted scope and specific justification. Record and
  close gaps without treating narrower check selection as an exemption.
  Independent protection remains the separate ADR 0016 follow-up; the presence
  of shared configuration does not establish that protection.
- **Public API structure and coverage:** under
  [ADR 0035](standards/0035-deliberate-public-api.md), audit public paths,
  re-exports, helper visibility, construction entry points, and parameter
  grouping. Preserve intentional umbrella module mounts and distinguish
  purposeful input variants from redundant aliases. Review the
  [public-path checker](../../scripts/check_single_path.py) and its invocation
  coverage: reporting duplicates and detecting lost items in comparison mode
  do not establish full enforcement of the standard. Propose changes to
  established public contracts within explicitly authorized scope; recording
  the standard does not itself authorize their removal or renaming.
- **Recoverable failure and bounds coverage:** under
  [ADR 0036](standards/0036-safe-rust-and-recoverable-failures.md), audit public
  library and binding paths for input-triggered panics, including unchecked
  indices, shape assumptions, and assertions that existing lints may miss.
  Verify typed error categories and diagnostic context for invalid inputs,
  market data, model state, and ordinary numerical failures. Establish and
  test the local bounds invariants used by directly indexed numerical kernels.
  [INVARIANTS.md](../../INVARIANTS.md), section 5.3, still records incomplete
  indexing enforcement; the current lint configuration does not establish
  complete compliance. Keep intentional test assertions separate from library
  failure behavior.

<a id="error-classification"></a>

- **Explicit error classification and adapter coverage:** under accepted
  [ADR 0049](standards/0049-explicit-public-error-matching.md), enumerate
  project-owned error variants wherever code assigns categories, structured
  codes, required diagnostics, conversions or specialized handling. Replace
  the [core classifier's](../../finstack-quant/core/src/error/mod.rs)
  catch-all validation branch, including its implicit treatment of remaining
  `InputError` variants, with explicit named groups. Reconcile the
  [scenario conversion](../../finstack-quant/scenarios/src/error.rs), whose
  `other` branch silently maps new variants to validation despite the adjacent
  exhaustive `kind()` method. Apply the same review to
  [portfolio `code()`](../../finstack-quant/portfolio/src/error.rs), which
  defaults remaining variants to no structured code.
  Establish the canonical Rust-owned classification/diagnostic contract for
  [ContractError](../../finstack-quant/core/src/contract/diagnostics.rs), which
  currently lacks owner-side kind/code methods. Reconcile its
  [Python](../../finstack-quant-py/src/errors.rs) generic contract-validation
  fallback and [WASM](../../finstack-quant-wasm/src/utils/mod.rs) validation/no-code
  defaults; required cross-crate wildcards do not authorize loss of meaning.
  Preserve ordinary propagation and canonical delegation without duplicating
  enum tables in every adapter. Verify independently specialized handling and
  required host fields separately. Document narrow third-party unknown-error
  mappings with operation-appropriate failure meaning and original diagnostic
  context; do not treat every current catch-all as approved.
  Correct the [workspace lint comment](../../Cargo.toml), which overstates
  `match_wild_err_arm`: that lint checks `Err(_)` arms taking drastic actions,
  not general enum-classification exhaustiveness. Establish suitable focused
  enforcement and coverage, including review of new owner/nested variants and
  cross-host error meaning. Do not promise compiler rejection in every consumer
  of a cross-crate non-exhaustive enum. These are policy and future-variant
  coverage gaps, not demonstrated misclassification of existing variants.
- **Implementation and reuse review:** under
  [ADR 0037](standards/0037-simple-modular-implementation-and-reuse.md), align
  contributor and agent guidance with the accepted simplicity, modularity,
  dependency-choice, and working-increment requirements. During relevant
  implementation work, record existing capabilities considered and the concrete
  need for added dependencies, custom replacements, or abstractions. Verify
  target and dependency-policy fit alongside behavioral checks. Existing
  dependency audits do not establish that all implementations meet this design
  standard; broad cleanup remains subject to the authorized task scope.
- **Public API documentation coverage:** under
  [ADR 0038](standards/0038-substantive-api-documentation.md), verify public
  documentation coverage and semantic accuracy across Rust, Python, and WASM.
  Review exact input names, units and conventions, defaults, missing-data
  behavior, outputs, public errors, and state or ownership effects. Verify the
  applicable example coverage and execution against supported public
  interfaces. Preserve the authoritative sources under ADR 0013; documentation
  checker and doctest success alone does not establish that every contract is
  explained correctly.
- **Behavioral test and regression coverage:** under
  [ADR 0039](standards/0039-behavior-and-invariant-tests.md), review test
  guidance and relevant suites for meaningful public workflows, internal
  invariants, invalid inputs, and boundary cases. Verify bug-fix regressions
  fail without their fixes, remove incidental clock or external-service
  dependencies, and distinguish performance measurements from correctness
  assertions. Use coverage reports to locate gaps without treating counts or
  percentages as proof of correctness. Preserve financial reference evidence
  under ADR 0014 and scoped reproducibility guarantees under ADR 0021; this
  record does not establish that existing suites already meet every requirement.
- **Performance evidence and budget scope:** under
  [ADR 0040](standards/0040-measured-performance-decisions.md), review material
  performance claims for representative inputs, comparable before-and-after
  conditions, repeated measurements, and reproducible commands and revisions.
  Distinguish kernel gains from end-to-end gains, report material resource
  tradeoffs, and verify the relevant correctness contracts alongside speed.
  Identify the workloads and conditions covered by each regression budget;
  existing materialization thresholds do not define library-wide limits.
  Retain reviewable evidence without assuming every exploratory benchmark has
  a committed acceptance record or a required CI gate.
- **Type invariants and validation boundaries:** under
  [ADR 0041](standards/0041-type-invariants-and-validation-boundaries.md),
  classify relevant public types by their promised local validity, raw input
  status, and remaining contextual checks. Verify constructors, builders,
  supported deserialization, alternate input paths, and mutation preserve those
  promises, including the valid state left after failed updates. Confirm raw
  input and contextual validation runs before computation relies on it.
  Preserve the explicit `CreditFactorModel` validation boundary described in
  [INVARIANTS.md](../../INVARIANTS.md), section 8; successful deserialization
  alone does not establish its validity. Identify any changes to existing
  contracts explicitly, without assuming all types need private fields,
  automatic deserialization validation, or additional wrapper layers.

<a id="credit-artifact-validation"></a>

- **Credit artifact semantic-validation coverage:** under accepted
  [ADR 0047](architecture/0047-credit-factor-wire-and-validation-contract.md),
  audit each consumer's preconditions so validation applies to the current
  artifact before financial use and after relevant mutation. The
  [validator](../../finstack-quant/factor/src/credit/hierarchy.rs) does not
  inspect `diagnostics.fold_ups`; the
  [decomposition consumer](../../finstack-quant/factor/src/credit/decomposition.rs)
  checks an out-of-range fold-up level later. Establish validation responsibility
  for all computational fields and add malformed-artifact and post-mutation
  evidence. Inspected Rust/Python/WASM JSON loaders and the
  [calibrator](../../finstack-quant/factor/src/credit/calibration/calibrator.rs)
  call semantic validation, but that does not prove every invariant or consumer
  is covered. Verify ordinary `from_json` separately from `from_slice_strict`'s
  resource limits and structured diagnostics; host semantic validation does
  not imply the bounded loading contract. Verify missing/malformed/unsupported
  markers, closed roots, discarded unknown diagnostic keys, known-field
  preservation, deterministic schema generation and coordinated host behavior.
  Carry guidance corrections in reconciliation item 7 above.

<a id="reference-provenance"></a>

- **Reference provenance and golden guidance:** under accepted
  [ADR 0048](project/0048-reference-provenance-and-version-pinning.md), audit
  case-attributable metadata, inherited suite/header/README values, explicit
  case overrides and reasons for genuinely inapplicable fields. Preserve
  methodology, inputs, output units and tolerance rationale, including when
  fixtures are shared between hosts. Reconcile the
  [core golden guide](../../finstack-quant/core/tests/golden/README.md), which
  promotes fixtures to `certified` after unspecified review and permits a skip
  with an explanation, with ADR 0014's independent evidence and authorization
  requirements. The [realized-variance fixture](../../finstack-quant/core/tests/golden/data/realized_variance.json)
  names the implementation as its generator while expected-value notes claim
  hand calculation. Trace the independent derivation before counting it as
  financial correctness evidence; this review establishes no numerical defect.
  The [metadata test](../../finstack-quant/core/tests/golden/variance_tests.rs)
  checks nonempty labels and `certified` status, not independent derivation,
  tolerance rationale or review. Shared host runs do not create additional
  independent references. Preserve protected expectations and tolerances.

<a id="methodology-pins"></a>

- **Methodology and effective-parameter pins:** under ADR 0048, identify the
  exact methodology, available calibration tag, and resolved parameter set
  used by each reference case, including overrides. Where no separate
  calibration tag exists, use the exact edition and immutable source/parameter
  identity without inventing a tag or requiring a new runtime metadata API.
  The [SIMM calculator](../../finstack-quant/margin/src/calculators/im/simm.rs)
  exposes historical v2.6 and accepts configuration overlays, while `version()`
  reports only the methodology enum. Record effective overlay/parameter
  provenance after [registry overlays](../../finstack-quant/margin/src/registry/mod.rs);
  the registry's wire-format version is a separate identity.
  Historical [v2.6 table tests](../../finstack-quant/margin/tests/simm_schedule_parity.rs)
  still select `SimmVersion::default()` in several cases; pin `V2_6` explicitly.
  Reconcile the [margin data guide](../../finstack-quant/margin/data/margin/README.md),
  including its six-file count versus five embedded files, stale v2.5 listing,
  removed `SimmV26GoldenValues` reference and
  procedure description, with the actual tests and
  [v2.6 provenance record](../../finstack-quant/margin/tests/fixtures/simm_v26_provenance.md).
  Distinguish the implemented historical pin from the catalogue's v2.8+2512
  bibliography entry and the unversioned [central reference](../REFERENCES.md#isda-simm).
  These documentation corrections do not authorize a methodology upgrade or
  certify regulatory applicability.

<a id="numerical-accuracy-evidence"></a>

- **Published numerical-accuracy evidence:** under ADR 0048, record executed
  evidence with revision, command, inputs, comparator, error units and relevant
  build/runtime/execution conditions, including seeds where applicable. Apply
  ADR 0043's reproducibility conditions and retain the distinction between
  measured error, sampling uncertainty, test tolerance and a broader bound.
  The [SABR documentation](../../finstack-quant/models/src/volatility/README.md)
  claims approximately 5–10 bp error for expiries above ten years without a
  linked reproducible setup, comparator or clearly specified error scale.
  Supply that evidence or remove the numerical claim while retaining justified
  qualitative limitations. Audit other published accuracy claims for the same
  evidence requirements; locating a test or source citation does not establish
  an executed result or complete coverage.
- **Detailed invariant enforcement and evidence:** the
  [clause map](INVARIANT_MAP.md) provides an accepted decision for every
  normative catalogue clause. Review of decisions 0042–0049 is complete under
  [invariant decision review](#invariant-decision-review); complete their scoped
  implementation, reconciliation and evidence work above. Acceptance does not
  approve unrelated observed alternatives or financial methodology drafts.
  Source links and existing test names are not proof that every stated
  invariant is enforced.

## Invariant decision review

The [invariant map](INVARIANT_MAP.md) completes documentary correspondence.
Existing catalogue requirements remain in force. Acceptance does not certify
implementations or authorize unrelated changes.

**Reviewed 2026-11-05:**
[ADR 0042](standards/0042-numerical-accumulation-and-comparisons.md) is accepted
for floating-point accumulation and comparisons, with separate roles for
Neumaier totals and OnlineStats and explicit bitwise checks for bit identity.
Its monetary-construction and non-finite monetary-input rules are accepted
as an amendment to [ADR 0020](financial/conventions/0020-numerical-representation-and-rounding.md).
Their implementation and verification follow-ups remain open above.

[ADR 0043](standards/0043-deterministic-ordering-and-monte-carlo-execution.md)
is also accepted on 2026-11-05, including deterministic insertion order,
tier-scoped fixed execution with declared chunk configuration, and relevant
baseline build/runtime/execution conditions. Its compliance and evidence
checks remain in the reproducibility follow-up above.

[ADR 0044](financial/conventions/0044-financial-perspectives-and-boundary-conversions.md)
is accepted on 2026-11-05 for the named perspectives and explicit conversions,
including the dividend magnitude default with an explicitly selected signed
alternative. Its retained margined replacement-cost formula uses total
recognized adjusted collateral `C`, already including NICA. Dividend
enforcement, SA-CCR public-field reconciliation, full margined/unmargined
methodology qualification, and cross-boundary checks remain in the sign
follow-up above.

[ADR 0045](financial/conventions/0045-financial-clocks-and-date-boundaries.md)
is accepted on 2026-11-05 for separate clocks and date roles, explicit
volatility date-to-expiry mapping, contractual forward annualization,
discount-factor bridges with finite positive inputs, and explicit fixing cutoffs.
Metadata representation and omission behavior, projection-basis coverage,
same-day fixing reconciliation, helper enforcement and caller verification
remain in the clock follow-up above. No instrument model or universal option
clock is adopted by this decision.

[ADR 0046](architecture/0046-portfolio-execution-operational-contract.md)
is accepted on 2026-11-05 for the detailed portfolio execution contract,
including explicitly supplied prior results with complete reuse compatibility,
metric-menu applicability, conditional FX conversion refresh, rate/hazard cache
isolation, one portfolio scheduling axis and native Python boundaries.
Configuration/market provenance, selective-versus-fresh coverage, inner-model
threading coordination and shared JSON copy compliance remain in the portfolio
follow-up above. Acceptance does not approve every observed default or certify
implementation coverage.

[ADR 0047](architecture/0047-credit-factor-wire-and-validation-contract.md)
is accepted on 2026-11-05 for required typed `schema`, marker rejection during
deserialization without first-error priority, stable closed-root schema identity,
explicit open diagnostics and current-state semantic validation. Unknown
diagnostic keys are discarded; computational `fold_ups` retains its meaning.
Ordinary and resource-limited loaders retain distinct contracts. Consumer and
post-mutation validation, fold-up coverage, and Serde/Python/test guidance
corrections remain in the credit-artifact follow-up and reconciliation item 7
above. Acceptance does not certify complete validation coverage.

[ADR 0048](project/0048-reference-provenance-and-version-pinning.md)
is accepted on 2026-11-05 for case-attributable applicable metadata, shared
fixture locations, exact methodology/effective-parameter pins, and recorded
execution evidence for measured accuracy. Inherited metadata permits explicit
case overrides; genuinely inapplicable fields require an explanation. Policy,
implementation and validation statuses remain distinct. Golden derivation and
guidance, SIMM version/override provenance and historical test selection, and
the SABR accuracy claim remain in the reference follow-ups above. Acceptance
does not approve a methodology upgrade, certify existing references or change
protected expectations and tolerances.

[ADR 0049](standards/0049-explicit-public-error-matching.md)
is accepted on 2026-11-05 for explicit error classification where meaning is
assigned, including categories, codes, diagnostics and conversions. Ordinary
propagation and canonical Rust delegation remain allowed. Cross-crate
non-exhaustive enums require appropriate downstream handling, including within
the workspace; third-party unknown-error mappings need a documented boundary.
Owner classifier/conversion gaps, `ContractError` adapter ownership and focused
enforcement remain in the error follow-up above. The current Clippy lint does
not establish exhaustive classification or complete compliance.

All invariant-detail decisions 0042–0049 are accepted. Their implementation
discrepancies remain in the monetary, reproducibility, portfolio, sign, clock,
schema, error and reference follow-ups. Completion of this decision review
does not close those checks or accept the separate financial methodology drafts.

## Financial methodology audit

The [2026-11-05 inventory](financial/README.md#coverage) links algorithm-level
records for core, cashflows, models, valuations, calibration, analytics,
statements, statements-analytics, scenarios, portfolio, margin, factor,
attribution, covenants, features and Arrow interchange. The
[source map](financial/SOURCE_COVERAGE.md) accounts for their
declared Rust scope. The [scoped algorithm backlog](financial/FOLLOW_UPS.md)
preserves findings from all audited catalogues, with links to the individual
methods and their evidence. Use that backlog for detailed reconciliation;
the items below connect it to broader decisions and qualification boundaries.

[ADR 0026](financial/governance/0026-methodology-decisions-and-inventory.md) accepts the
documentation process. Neither overview drafts 0027–0033 nor the detailed
algorithm records approve their observed implementations. Numerical suites
were not rerun. Static source discrepancies, suspected numerical defects
requiring reproduction, adoption choices and missing validation are distinct.

### Scoped algorithm reconciliation

- **Core and cashflows:** reconcile monetary conversion/finalization boundaries,
  curve and surface validation, fixing and overnight period semantics,
  event ordering, and numerical fallback policies. The source records explain
  the actual branches; deciding whether they are intended remains separate.
- **Models:** resolve process-parameter usage, tree/PDE exercise boundaries,
  Monte Carlo stepping and regression choices, error-estimation conventions,
  and unsupported combinations. Establish independent convergence and
  reference evidence for each intended domain before qualification.
- **Product valuation:** review Merton calibration/coupon treatment, credit
  projection versus PV and maturity behavior, copula/recovery propagation
  through risk, selected-model Greek consistency, overnight engine differences,
  and structured-credit OAS, recovery, trigger and correlation behavior.
  The detailed backlog preserves each product-specific finding and the
  reproduction or adoption decision needed; these observations do not
  authorize silently changing models.
- **Calibration:** reconcile target units and parameter domains, solver versus
  returned-curve conventions, clamped residual/Jacobian behavior, diagnostic
  aggregation, fit acceptance and recalibration replay. Independent evidence
  must distinguish parameter recovery from identification and market accuracy.
- **Statements and analytics:** preserve the previously agreed reported-cost
  D&A convention, dated-CAGR/return-metric directions and period-close
  waterfall FX policy. Review period windows, forecast dependence, dated debt
  balances, check/skip semantics, ECL reversion approximations and valuation
  assumptions against the detailed records.
- **Scenarios, portfolios and attribution:** reconcile shock coordinate units,
  ordered versus parallel partial failure, state/dependency reuse, aggregation
  and FX boundaries, factor routing and P&L allocation meaning. A complete
  total-P&L reconciliation does not establish equivalent attribution factors.
- **Margin and factor models:** review sensitivity/container validation,
  parameter and registry versions, collateral/netting scope, risk allocation,
  estimator and horizon conventions, and regulatory/XVA approximation domains.
  Their implemented formulas and labels do not establish current compliance.
- **Covenants, features and Arrow interchange:** review effective metric and
  threshold resolution, episode/forecast state, panel and window semantics,
  nonfinite/missing-value behavior, numerical transform domains, and export
  schema/precision/metadata contracts. Keep checked-construction guarantees
  distinct from public-field, deserialization and downstream validation paths.

### Adoption decisions

- **Options and exercise:** agree the vanilla option domains, dividend and
  lifecycle treatment, tree choice and accuracy targets in
  [0027](financial/valuations-equity-and-composition/0027-vanilla-option-model-selection.md). Agree the callable
  bond default and supported rate-only alternatives in
  [0028](financial/valuations-bonds/0028-callable-bond-model-selection.md), and the quotation/
  exercise selection boundary in
  [0029](financial/valuations-rates/0029-rate-option-exercise-and-volatility.md). Joint stochastic
  rates-credit, LSMC policy, HW versus LMM, settlement annuities, and stochastic
  volatility require their own scoped choices.
- **Credit conventions and dependence:** agree the CDS convention bundles,
  absent-calendar/date-inversion behavior and numerical accuracy domain in
  [0030](financial/models-and-valuations-credit/0030-cds-survival-and-integration.md). Agree tranche
  copula/calibration scope, settlement overrides, correlation clipping, and
  interim loss/monotonicity repairs in
  [0031](financial/models-and-valuations-credit/0031-tranche-loss-model-selection.md). Observed repairs are
  not approved fallbacks. Standalone copula configuration also needs a host
  reachability and result-provenance assessment.
- **Curve construction:** agree the distinct direct-builder and calibration
  interpolation defaults, permitted global polishing/seed adoption, fit
  acceptance and tail behavior in
  [0032](financial/calibration-targets/0032-curve-interpolation-and-calibration.md). Distinct hazard,
  inflation, cross-currency, SABR/SVI, HW and base-correlation calibration
  choices remain pending, including approximate-knot acceptance.
- **Tail risk:** agree historical summary selection, R-7 quantiles versus
  fractional-boundary empirical ES tails, horizon interpretation and unavailable-sample
  behavior in [0033](financial/analytics/0033-value-at-risk-method-selection.md).
  Preserve the prior agreed explicit-error direction for invalid
  Cornish–Fisher estimates; adopting its current Gaussian substitution is not
  an open default choice.

### Implementation reconciliation

- **Prior analytics directions:** the 2026-10-11 directions require explicit
  invalid Cornish–Fisher errors, strictly positive up-capture, cash-adjusted
  Martin, modified Burke across all episodes, and an actual initial valuation
  date. Current [tail-risk kernels](../../finstack-quant/analytics/src/risk_metrics/tail_risk.rs),
  [capture selection](../../finstack-quant/analytics/src/benchmark.rs),
  [scalar metrics](../../finstack-quant/analytics/src/performance/scalar.rs), and
  [return-panel construction](../../finstack-quant/analytics/src/performance/mod.rs)
  still conflict with those directions. Reconcile the affected Rust and host
  contracts, documentation, and protected expectations together. The existing
  CAGR item above remains a separate discrepancy under ADR 0008.
- **Selection wording:** clarify the private Ho-Lee/200-step tree configuration
  versus the public bond HW/100-step resolver documented in 0028; a private
  default does not describe public selection. Reconcile the negative-rate
  conversion promised by `CapFloorVolType::Auto` and test-module prose with
  the executable rejection behavior linked in 0029. Correct the
  [credit-family README](../../finstack-quant/valuations/credit/README.md)
  Black option wording against the live `BloombergCdso` dispatch recorded in
  the inventory. Reconcile Bloomberg CDS par-denominator source descriptions
  with the [leg calculation](financial/models-and-valuations-credit/12-cds-legs.md#selection-and-controls):
  both clean and full-premium variants include accrual-on-default; clean
  subtracts accrued and full-premium does not. These findings do not authorize
  silently changing models.
- **Related scope:** retain the option-bearing CS01/OAS reconciliation above.
  The inventory's statement DCF path uses ACT/365.25; the dated-CAGR decision
  does not establish a DCF time-basis change. Different fixing/overnight paths
  need explicit scope decisions before their behavior is harmonized.

### Validation and coverage

- **Independent evidence:** assess provenance and justified tolerances of the
  linked cases. Vendor-labelled fixtures, internal formula comparisons,
  round trips, and broad range checks establish different things. Preserve
  the recorded CDSO benchmark differences; establish independent option/tree,
  curve-fit, CDS integration, loss-convolution, and tail-risk evidence for the
  intended domains. Slow/ignored convergence tests need explicit execution
  evidence before being counted as validated. Complete missing technical
  citations identified in the draft records.
- **Remaining adoption and audit scope:** detailed records cover the declared
  crates across numerical frameworks, calibration, valuation, analytics,
  statements, scenarios, portfolios, attribution, margin and factor models.
  Covenant engines, panel features and Arrow conversions also have detailed
  records. Their adoption remains open. The
  [audit boundaries](financial/AUDIT_BOUNDARY.md) identify separate root-package
  and host surfaces and outstanding numerical qualification. Embedded
  policy names and rule versions do not establish accounting or regulatory
  qualification.
- **Coverage boundary:** the source ledger is complete for its declared Rust
  scope, including production macro and calendar-generation code. It does not
  establish complete host-surface tracing, every supported feature combination,
  independent reference provenance, or numerical accuracy. Qualify those
  separately, and keep the ledger aligned with source changes. Do not infer
  product adoption or accuracy from a shared numerical engine's availability.
