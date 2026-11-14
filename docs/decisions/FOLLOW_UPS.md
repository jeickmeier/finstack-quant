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
   runs in `rust-doc`; its docstring and diagnostics now describe it as an
   annotation-format check rather than the replacement policy, and no library
   `#[deprecated]` attribute currently exists. Still open: decide whether its
   `since = "X.Y.Z"`, replacement-or-rationale and earliest-removal-release
   requirements remain, without silently disabling a protected gate. The former
   rule retaining APIs through `0.(y+1)` after deprecation in `0.y`, with
   removal in `0.(y+2)`, is superseded; it is not the final-release strategy.
   That strategy remains a separate lifecycle task.
2. **Python/WASM parity guidance and coverage.**
   [INVARIANTS.md section 7.1](../../INVARIANTS.md#71-host-surface-parity) follows
   [ADR 0005](standards/0005-python-wasm-parity.md). Audit remaining guidance,
   ledger exclusions, and behavioral coverage against corresponding interfaces
   for selected financial capabilities, explicit justified host exceptions,
   and the protected zero-backlog requirement. Preserve the distinction from
   exposing every internal Rust function. Surface checks alone do not prove
   semantic parity or independent protection of their own ceilings.
3. **Cross-target reproducibility claims.**
   [ADR 0021](standards/0021-reproducibility-guarantees.md) and
   [INVARIANTS.md](../../INVARIANTS.md) section 2.1 limit bit-identity promises
   to demonstrated target and execution conditions and verify numerical parity
   with justified tolerances otherwise. The
   [models README](../../finstack-quant/models/README.md),
   [run-metadata documentation](../../finstack-quant/models/src/monte_carlo/results.rs)
   and [engine pricing documentation](../../finstack-quant/models/src/monte_carlo/engine/pricing.rs)
   now scope their promise to thread-count independence on one build target and
   state that native-versus-WASM results agree only within tolerances. Still
   open: no native-versus-WASM comparison or justified tolerance backs that
   statement, and the same-target tests do not cover every machine; add that
   evidence under the reproducibility coverage follow-up. This conflict was
   never proof of a numerical defect, and ADR 0005 does not imply blanket
   bit-identical floating-point results.
4. **Option-bearing bond CS01 scope.**
   The [accepted BONDS-16 scope](financial/valuations-bonds/16-spread-credit-risk.md#accepted-decision-for-checked-bond-spread-and-credit-risk)
   requires an explicitly qualified fixed Z strip for supported option-bearing
   bonds under [ADR 0011](financial/valuations-bonds/0011-bond-cs01-without-hazard-curve.md).
   Reconcile the [live bond CS01 route](../../finstack-quant/valuations/fixed-income/src/bond/metrics/cs01.rs)
   by removing the implicit OAS and parallel-bucket substitutions, and
   qualify fixed-strip admission and no-hazard attribution independently.
5. **Code-standard guidance and configuration.**
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
   workspace manifest). The error guide's crate inventory was re-checked against each
   crate's error module on 2026-11-05 and is accurate, and the
   [testing guidance](../../.agents/rules/rust/testing-standards.md) now
   follows [ADR 0048](project/0048-reference-provenance-and-version-pinning.md)
   for fixture location and provenance and names the actual `ResultsMeta`
   fields. Continue
   auditing remaining contributor guidance and crate-level lint configuration for
   any further drift; ADR 0034 does not independently approve every legacy
   instruction or current configuration setting.

<a id="credit-factor-guidance"></a>

6. **Credit factor schema guidance and extension scope.**
   [ADR 0047](architecture/0047-credit-factor-wire-and-validation-contract.md)
   is accepted on 2026-11-05: require typed `schema` with
   `CreditFactorModelSchema::CURRENT`, reject missing, malformed or unsupported
   markers during deserialization, and retain no `schema_version` alias.
   Rejection before successful decoding does not promise first-error ordering.
   The former representation/failure-timing discrepancy is settled; enforcement
   and guidance corrections remain open under
   [INVARIANTS.md section 8](../../INVARIANTS.md#8-credit-factor-model-schema-versioning).
   The [Serde policy](../SERDE_STABILITY.md#object-strictness), the Python
   [`rust_json` macro documentation](../../finstack-quant-py/src/bindings/macros.rs)
   and the [hierarchy's](../../finstack-quant/factor/src/credit/hierarchy.rs)
   unknown-root-field test message now state the narrow, explicitly open
   `CreditCalibrationDiagnostics` object (unknown diagnostic keys accepted and
   discarded; known fields keep their requiredness and meaning, including
   computational `fold_ups`), unknown-field rejection by each object's declared
   closure, and root additions as a coordinated v1 contract change. Still open:
   enforcement, tracked in the
   [credit artifact validation entry](#credit-artifact-validation), and any
   remaining guidance that still describes `schema_version` or a new schema
   version for root additions. Preserve the stable schema path/URI and the
   separate final-release compatibility decision.

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
  guarantees actually promised. Reconciliation item 3 tracks the remaining
  native/WASM evidence separately. The former catalogue reported
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
  The SA-CCR [public field](../../finstack-quant/margin/src/regulatory/sa_ccr/types.rs)
  documentation (Rust, Python and WASM) now matches the
  [kernel's](../../finstack-quant/margin/src/regulatory/sa_ccr/replacement_cost.rs)
  VM-only interpretation of `config.collateral` and addition of NICA. The
  [accepted complete ADR 0044 package](financial/conventions/0044-financial-perspectives-and-boundary-conversions.md#accepted-amendment-explicit-collateral-components)
  requires the `variation_margin` field, explicit signed VM/NICA components
  and one derived total `C` for all consumers.
  Qualify construction/wire validation and shared-C use; amount-only validation
  cannot prove that an upstream total was not mislabelled as VM. The
  [SA-CCR qualification entry](financial/FOLLOW_UPS.md#saccr-collateral-qualification)
  owns regulatory qualification of the margined and unmargined methodologies.
  Representation is now accepted; runtime reconciliation and independent
  qualification of both calculations remain open.

<a id="clock-metadata-defaults"></a>

- **Clock metadata, defaults, and adoption:** audit the detailed clock rules in
  [INVARIANTS.md section 4](../../INVARIANTS.md#4-date-day-count-and-time-conventions)
  under accepted [ADR 0045](financial/conventions/0045-financial-clocks-and-date-boundaries.md).
  The former catalogue described swaption, cap/floor, and CMS option time as
  Act/365F and equity/FX/commodity defaults as often Act/365F. Inventory and
  verify those defaults at their actual selection paths; these observations
  do not approve a universal option clock or settle the methodology drafts.
  Implement the [accepted complete ADR 0045 package](financial/conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility):
  one reusable `VolatilityClock` (reference date, DayCount and
  existing context state), preserved on volatility artifacts/derived outputs;
  complete active source mapping for dated use, with missing/conflicting source
  declarations rejected and no universal fallback. Model clocks remain distinct;
  supported variance conversions preserve the same physical interval. Different
  calibration origins require an explicitly owned aging/recalibration/model
  contract rather than a relabelled grid. The current
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
  validating the factor. Reconcile already accepted enforcement for finite
  positive factors/model time, leaving lifecycle branches operation-owned; test the
  bridge with justified tolerances without claiming full model-dynamics
  equivalence. The old `common_impl::two_clock` reference has no current source
  counterpart.
  The accepted information-set rules distinguish value dates from
  publication eligibility at an explicitly declared operation cutoff, require
  missing eligible published observations to error, and permit projection of
  unpublished values only under that operation's contract. Preserve snapshot/
  revision provenance without inventing a universal timestamp/cutoff framework.
  Reconcile actual operation policies: the [cashflow helper](../../finstack-quant/cashflows/src/builder/rate_helpers.rs)
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
  Carry the remaining guidance work in reconciliation item 6 above.

<a id="reference-provenance"></a>

- **Reference provenance and golden guidance:** under accepted
  [ADR 0048](project/0048-reference-provenance-and-version-pinning.md), audit
  case-attributable metadata, inherited suite/header/README values, explicit
  case overrides and reasons for genuinely inapplicable fields. Preserve
  methodology, inputs, output units and tolerance rationale, including when
  fixtures are shared between hosts. The
  [core golden guide](../../finstack-quant/core/tests/golden/README.md) now
  ties `certified` promotion and skips to ADR 0014's independent evidence and
  authorization requirements, and records that an independent recomputation
  from the cited formulas reproduced all four realized-variance values. Still
  open: the [realized-variance fixture](../../finstack-quant/core/tests/golden/data/realized_variance.json)
  still names the implementation as its generator and carries approximate
  hand-calculation notes, so store the derivation per case; this review
  establishes no numerical defect.
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
  The historical v2.6 table tests now pin `SimmVersion::V2_6`, the
  [margin data guide](../../finstack-quant/margin/data/margin/README.md) matches
  the five embedded files, tests and
  [v2.6 provenance record](../../finstack-quant/margin/tests/fixtures/simm_v26_provenance.md),
  and INVARIANTS and the [central reference](../REFERENCES.md#isda-simm)
  distinguish the implemented v2.6 pin from the v2.8+2512 bibliography entry.
  These documentation corrections do not authorize a methodology upgrade or
  certify regulatory applicability.

<a id="numerical-accuracy-evidence"></a>

- **Published numerical-accuracy evidence:** under ADR 0048, record executed
  evidence with revision, command, inputs, comparator, error units and relevant
  build/runtime/execution conditions, including seeds where applicable. Apply
  ADR 0043's reproducibility conditions and retain the distinction between
  measured error, sampling uncertainty, test tolerance and a broader bound.
  The SABR documentation no longer publishes an error size without evidence.
  Still open: audit other published accuracy claims for the same evidence
  requirements, including the CMS convexity "5–10 bp" statements in the
  [rates README](../../finstack-quant/valuations/rates/README.md) and the CMS
  swap pricer and module documentation, and the tree-step accuracy table in the
  bond tree configuration. Locating a test or source citation does not
  establish an executed result or complete coverage.
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
enforcement, SA-CCR non-overlap enforcement, full margined/unmargined
methodology qualification, and cross-boundary checks remain in the sign
follow-up above.

[ADR 0045](financial/conventions/0045-financial-clocks-and-date-boundaries.md)
is accepted on 2026-11-05 for separate clocks and date roles, explicit
volatility date-to-expiry mapping, contractual forward annualization,
discount-factor bridges with finite positive inputs, and explicit fixing cutoffs.
The complete 2026-11-07 amendment accepts `VolatilityClock`, active dated
admission, origin/same-interval variance and publication-eligibility requirements.
All six shared financial convention ADRs are now accepted within their scope.
Metadata implementation, projection-basis coverage, operation-owned same-day
fixing reconciliation, helper enforcement and caller verification remain in
the clock follow-up above. No instrument model or universal option clock is
adopted by this decision.

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
post-mutation validation and fold-up coverage remain in the credit-artifact
follow-up; remaining guidance work is in reconciliation item 6 above.
Acceptance does not certify complete validation coverage.

[ADR 0048](project/0048-reference-provenance-and-version-pinning.md)
is accepted on 2026-11-05 for case-attributable applicable metadata, shared
fixture locations, exact methodology/effective-parameter pins, and recorded
execution evidence for measured accuracy. Inherited metadata permits explicit
case overrides; genuinely inapplicable fields require an explanation. Policy,
implementation and validation statuses remain distinct. Golden fixture
derivation metadata, SIMM effective overlay provenance, and the audit of other
published accuracy claims remain in the reference follow-ups above. Acceptance
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
preserves remaining findings from audited catalogues, with links to the individual
methods and their evidence. Use that backlog for detailed reconciliation;
the items below connect it to broader decisions and qualification boundaries.

[ADR 0026](financial/governance/0026-methodology-decisions-and-inventory.md) accepts the
documentation process. [ADR 0033](financial/analytics/0033-value-at-risk-method-selection.md#accepted-decision-fixed-historical-tail-selection)
accepts fixed historical tail-method selection; overview records 0027–0032
remain Draft. Detailed algorithm records identify their accepted scopes;
all 15 [analytics records](financial/analytics/README.md#accepted-scopes-in-detailed-records)
are accepted as of 2026-11-07 and implemented on 2026-11-08, with
[scoped implementation evidence](financial/analytics/README.md#implementation-evidence)
recorded separately from methodology acceptance.
[Core-market record01](financial/core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction),
[record02](financial/core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates),
[record03](financial/core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning),
[record04](financial/core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation),
[record05](financial/core-market/hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery),
[record06](financial/core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification),
[record07](financial/core-market/parametric-discount.md#accepted-decision-validated-nsnss-sources-and-analytic-factor-queries),
[record08](financial/core-market/price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks),
[record09](financial/core-market/basis-spreads.md#accepted-decision-source-derived-dated-zero-rate-differences),
[record10](financial/core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks),
[record11](financial/core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability),
[record12](financial/core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings),
[record13](financial/core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates),
[record14](financial/core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks),
[record15](financial/core-market/fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs),
[record16](financial/core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning),
[record17](financial/core-market/forward-variance.md#accepted-decision-nonnegative-variance-sources-and-explicit-model-time-semantics),
[record18](financial/core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks),
[record19](financial/core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts),
[record20](financial/core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle),
[record21](financial/core-market/hierarchy-selection.md#accepted-decision-typed-selection-and-explicit-ordered-overlap),
and [record22](financial/core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows)
have complete scoped acceptance as of 2026-11-08, with runtime/caller/host
reconciliation and independent qualification pending. **All 22 detailed
core-market records are accepted**, completing this area's methodology review.
Detailed reconciliation work stays in the financial register.
All 20 [core-conventions records 001–020](financial/core-conventions/README.md)
are also accepted within their complete stated scopes, completing that area's
methodology review; runtime reconciliation and independent qualification remain open,
except that record 018 is fully reconciled, record 020's runtime reconciliation
landed on 2026-11-08 (its consumer qualification remains open) and record 003's
runtime reconciliation landed on 2026-11-09 (independent qualification remains open).
[Core-numerics record 01](financial/core-numerics/compounding.md#accepted-decision-explicit-compounding-conversion-and-periodic-spreads)
is accepted within its complete stated scope as of 2026-11-07; runtime/caller/host
reconciliation and independent qualification remain open.
[Record 02](financial/core-numerics/value-interpolation.md#accepted-decision-validated-scalar-interpolation-and-boundary-semantics)
is also accepted within its complete stated scope as of 2026-11-07, with
runtime/host/consumer reconciliation and qualification pending.
[Record 03](financial/core-numerics/forward-interpolation.md#accepted-decision-natural-log-discount-and-hagan-west-interpolation)
is also accepted within its complete scope as of 2026-11-07, with runtime/host/
consumer reconciliation and qualification pending.
[Record 04](financial/core-numerics/quadrature.md#accepted-decision-checked-quadrature-budgets-and-measures)
is accepted within its complete scope as of 2026-11-07, with runtime/caller/table
reconciliation and independent qualification pending.
[Record 05](financial/core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)
has complete scoped acceptance as of 2026-11-07, with runtime/caller
reconciliation and independent qualification pending. [Record 06](financial/core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes)
has complete scoped acceptance as of 2026-11-07, with runtime/caller/host
reconciliation and independent qualification pending. [Record 07](financial/core-numerics/matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission)
has complete scoped acceptance as of 2026-11-08, with runtime/caller/host
reconciliation and independent qualification pending. [Record 08](financial/core-numerics/nearest-correlation.md#accepted-decision-explicit-nearest-correlation-projection-and-repair-limits)
has complete scoped acceptance as of 2026-11-08, with runtime/caller/host
reconciliation and independent qualification pending. [Record 09](financial/core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty)
has complete scoped acceptance as of 2026-11-08, with runtime/caller/host and
independent numerical/statistical qualification pending. [Record 10](financial/core-numerics/realized-variance.md#accepted-decision-explicit-log-returns-and-realized-variance)
has complete scoped acceptance as of 2026-11-08, with runtime/caller/host and
independent numerical/financial qualification pending. [Record 11](financial/core-numerics/covariance-shrinkage.md#accepted-decision-checked-scaled-identity-covariance-shrinkage)
has complete scoped acceptance as of 2026-11-08 for centered population/scaled-identity
shrinkage, stable moment/complement, checked result/error and host/credit-owner
reconciliation, with independent numerical/statistical/financial qualification pending; [Record 12](financial/core-numerics/distribution-functions.md#accepted-decision-checked-special-functions-and-distribution-domains)
has complete scoped acceptance as of 2026-11-08 for checked scalar/distribution
domains, normal/log-tail, bounded Student-t beta and host/caller reconciliation,
with independent numerical/statistical/financial qualification pending;
[Record 13](financial/core-numerics/binomial-mass.md#accepted-decision-checked-full-binomial-probability-mass)
has complete scoped acceptance as of 2026-11-08 for strict binomial domains/
resources, exact-mode/FMA recurrence, normalization, scratch/errors and Rust-only/
credit-owner boundaries, with runtime and independent qualification pending; [Record 14](financial/core-numerics/beta-gamma-sampling.md#accepted-decision-checked-beta-and-gamma-sampling)
has complete scoped acceptance as of 2026-11-08 for checked RNG/shapes, stable
bounded MT/component beta ratios, endpoint/errors and Rust-only/model ownership,
with runtime and independent qualification pending;
[Record 15](financial/core-numerics/chi-squared-inversion.md#accepted-decision-checked-chi-squared-log-tail-inversion)
has complete scoped acceptance as of2026-11-08 for closed endpoints/interior
domain, bounded log-Gamma/Brent, actual-returned-output tail gate and Rust-only/
copula ownership, with runtime and independent qualification pending; [Record 16](financial/core-numerics/correlated-bernoulli.md#accepted-decision-strict-correlated-bernoulli-laws)
has complete scoped acceptance as of2026-11-08 for strict two-event feasibility/
nullable correlation, stable law/conditional/half-open sampling, canonical hosts
and financial ownership, with runtime and independent qualification pending; [Record 17](financial/core-numerics/pseudorandom-and-bridge.md#accepted-decision-checked-random-draws-bridge-and-poisson-counts)
has complete scoped acceptance as of2026-11-08 for explicit PCG/checked normals,
capped stable bridge, bounded Poisson counts and caller/financial ownership,
with runtime and independent qualification pending;
[Record 18](financial/core-numerics/sobol-sequences.md#accepted-decision-finite-period-sobol-points-and-prefix-hash-scrambling)
has complete scoped acceptance as of 2026-11-08 for fixed directions/order,
explicit seed mode, honest prefix hash guarantees, checked whole-point state/
period/buffers, caller reconciliation and estimator ownership, with runtime and
independent qualification pending.
[Record 19](financial/core-numerics/fractional-covariance.md#accepted-decision-checked-fractional-increment-covariance)
has complete scoped acceptance as of 2026-11-08 for typed/unit-normalized fBM,
validated intervals/grids, stable bounded increment arithmetic/cancellation refusal,
checked mirrored matrices and caller/schema/financial ownership, with runtime and
independent qualification pending.
[Record 20](financial/core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots)
has complete scoped acceptance as of 2026-11-08 for canonical knots/actual dts,
strict exact required events, checked lookup/access, bounded geometry/resources,
honest uniform recognition and removal of generic financial snapping with explicit
caller/model/host ownership, with runtime and independent qualification pending.
[Record 21](financial/core-numerics/piecewise-coefficients.md#accepted-decision-checked-piecewise-coefficients-and-exponential-integration)
has complete scoped acceptance as of 2026-11-08 for right-continuous checked
coefficients, stable signed-kappa analytic integration without deadbands, scaled
positive-total range/zero rules and complete model/tree/host error propagation
with explicit financial ownership, with runtime and independent qualification
pending.
[Record 22](financial/core-numerics/expression-evaluation.md#accepted-decision-checked-scalar-expressions-missing-values-and-decimal-quantization)
has complete scoped acceptance as of 2026-11-08 for bounded eager scalar syntax,
explicit exact-shape rows, NaN-only missing/invalid-calculation errors, three-valued
logic, exact binary-input decimal quantization and checked shared helpers with
explicit caller/statements/host/financial boundaries, with runtime and independent
qualification pending.
[Record 23](financial/core-numerics/rounding-and-config.md#accepted-decision-explicit-rounding-config-local-tolerances-and-producer-metadata)
has complete scoped acceptance as of 2026-11-08 for explicit monetary config/
snapshots, local tolerance ownership, fixed embedded validation, optional actual
producer metadata and removal of obsolete global-zero/override/overlay paths,
with coordinated caller/host/replay qualification pending. [Record 24](financial/core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity)
has complete scoped acceptance as of 2026-11-08 for strict symmetric count,
exact monetary/date/sentinel wire boundaries and c2 serialized identity,
including duplicate/lossy-ingress rejection, explicit UTF-8 ordering/byte profile,
bounded single-pass hashing and existing host/cache reconciliation. Runtime and
independent qualification remain pending. [Record 25](financial/core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics)
has complete scoped acceptance as of 2026-11-08 for bounded lossless loading,
exact monetary/owner validation, independent fatal-report status, nullable/exact
table values, bounded trace/error diagnostics, ordered execution and fail-closed
locks. Runtime/caller/host/schema reconciliation and independent qualification
remain pending. [Record 26](financial/core-numerics/schema-publication.md#accepted-decision-canonical-schemas-compact-views-and-bounded-publication)
has complete scoped acceptance as of 2026-11-08 for original-graph/offline-corpus
schemas, exact examples, bounded canonical validation, compact authoring views
and staged publication. **All 26 core-numerics records are accepted**, completing
that area's methodology review; none remain Draft or Proposed. Runtime/caller/
host/schema reconciliation and independent qualification remain pending.
[Cashflows record001](financial/cashflows/01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context)
has complete scoped acceptance as of2026-11-08 for explicit contractual schedule
choices, original nominal references, ICMA/365L/termination context, coincident
events and checked immutable period queries. Its narrow core015 clarification
was applied in place: metadata endpoints describe actual calculation dates,
including explicit adjustment, separately from nominal references.
[Cashflows record002](financial/cashflows/02-program-compilation.md#accepted-decision-coupon-and-payment-program-compilation)
has complete scoped acceptance as of 2026-11-08 for independent atomic windows,
explicit settlement/exchange/cutoff, globally valid overrides, atomic fallible
configuration and exact shared build paths.
[Cashflows record003](financial/cashflows/03-principal-and-emission-order.md#accepted-decision-dated-opening-principal-signed-cash-and-atomic-event-phases)
has complete scoped acceptance as of 2026-11-08 for explicit dated opening
stock, signed settlement cash, atomic same-date principal batches, covered
history, paired terminal dates and exact explicit exchanges.
[Cashflows record004](financial/cashflows/04-amortization.md#accepted-decision-explicit-amortization-laws-live-balance-installments-and-monetary-boundaries)
has complete scoped acceptance as of 2026-11-08 for explicit amortization laws,
exact fractions, protected final targets, stateless live-balance installments,
par effects and monetary/payment boundaries. Its removal of the obsolete owner
installment-update slot was clarified in accepted003 without changing the relative
economic phase order.
[Cashflows record005](financial/cashflows/05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization)
has complete scoped acceptance as of 2026-11-08 for original-parent prefix
weights, once-rounded parent amounts, one exact split share and conserved
cash/PIK allocation, truthful component metadata and capitalization. Accepted003
now explicitly points to005's generated-parent PIK unit/profile, retaining signed
internal contributions and separate explicit events.
[Cashflows record006](financial/cashflows/06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures)
has complete scoped acceptance as of 2026-11-08 for resolved term observations,
publication-qualified exact fixing/projection, explicit exact rate controls and
one shared strict rate path without generic failure substitutes. It inherits
accepted market02/12 and cashflows005;005 now explicitly names006's admitted
Decimal-rate handoff before its separate parent-Money stage.
[Cashflows record007](financial/cashflows/07-overnight-compounding.md#accepted-decision-resolved-overnight-blocks-cumulative-growth-and-parent-allocation)
has complete scoped acceptance as of 2026-11-09 for actual overnight quote
intervals, paired carry/shift blocks, stable frozen-quote growth replay and an
explicit fixed-bridge compound-rate amount allocation with reference-cap limits.
005 now names that overnight contribution/admission authority explicitly.
[Cashflows record008](financial/cashflows/08-floating-replay.md#accepted-decision-parent-bound-replay-exact-full-settlement-and-explicit-timing-ownership)
has complete scoped acceptance as of2026-11-09 for immutable parent-bound state,
independent qualified observation/exact principal capture, shared full-parent
settlement, removal of generic partial/segment monetary paths and explicit
information/event timing ownership.
[Cashflows record009](financial/cashflows/09-fees.md#accepted-decision-explicit-fee-parents-exact-amounts-and-literal-tier-selection)
has complete scoped acceptance as of2026-11-09 for explicit limited fee grids,
opening/integrated original-parent bases, exact whole-parent amounts, literal
fixed cash and strict nullable tier selection.
[Cashflows record010](financial/cashflows/10-prepayment-and-mortality.md#accepted-decision-checked-monthly-mortality-explicit-model-clocks-and-benchmark-boundaries)
has complete scoped acceptance as of2026-11-09 for checked survival conversion,
whole-model admission, one-based monthly assumptions and explicit PSA/ABS/
lockout/vector boundaries, with actual consumer clocks and host reconciliation.
[Cashflows record011](financial/cashflows/11-default-and-recovery-assumptions.md#accepted-decision-explicit-default-targets-strict-capacity-and-one-recovery-authority)
has complete scoped acceptance as of2026-11-09 for explicit conditional/target
interfaces, direct original-reference demand, strict represented capacity,
normalized timing and one recovery authority with truthful source/vintage limits.
[Cashflows record012](financial/cashflows/12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views)
has complete scoped acceptance as of2026-11-09 for complete immutable source
schedules, explicit opening/through principal scope, exact closing replay and
separate native cash views without implicit residual normalization. Restricted
composition and actual product/export/host reconstruction retain their owners.
[Cashflows record013](financial/cashflows/13-accrued-interest.md#accepted-decision-original-parent-cash-quotation-and-inclusive-first-ex-dates)
has complete scoped acceptance as of2026-11-09 for original-parent linear cash
quotation, explicit native Money/currency, inclusive first-ex boundaries and one
final restoration, removing generic compounding/PIK/configuration and inference.
[Cashflows record014](financial/cashflows/14-aggregation-and-life.md#accepted-decision-exact-nominal-reporting-and-explicit-repayment-life)
has complete scoped acceptance as of2026-11-09 for exact native Money/PeriodPlan
reporting, typed cash-year ladders with separately owned qualified PV, and an
explicit ACT/365F repayment-stream statistic with owner-selected weights/dates.
Its correction of universal SIFMA WAL attribution does not adopt product WAL.
[Cashflows record015](financial/cashflows/15-credit-adjusted-pv.md#accepted-decision-product-owned-credit-valuation-and-qualified-pv-reporting)
has complete scoped acceptance as of2026-11-09 removing generic credit-kind/FIFO
recovery valuation and schedule PV shortcuts. Actual product owners supply
qualified value attribution; existing core016 discounting/014 aggregation,
explicit unsupported outcomes and canonical-price reconciliation remain.
[Cashflows record016](financial/cashflows/16-fixing-materialization.md#accepted-decision-qualified-hypothetical-fixing-materialization)
has complete scoped acceptance as of2026-11-09 for explicit required observations,
qualified frozen-forward candidates, exact conflicts and persistent hypothetical
source/role state, replacing date-window and unqualified history insertion.
All16 cashflow records are Accepted; the whole-file cashflow review is complete.
[Analytic-model record01](financial/models-analytic/spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results)
has complete scoped acceptance as of2026-11-09 for checked spot BSM pricing,
raw derivative units, explicit singular Greek availability and composite
calculation behavior. [Analytic-model record02](financial/models-analytic/forward-normal-shifted.md#accepted-decision-unit-forward-prices-and-native-model-greek-bundles)
has complete scoped acceptance as of2026-11-09 for common unit-forward pricing and
native Greek bundles, checked exact boundaries and original displacement identity,
with external product multipliers and explicit unit-output range limits.
[Analytic-model record03](financial/models-analytic/implied-volatility-solvers.md#accepted-decision-explicit-bounded-model-inversion-and-paired-premium-acceptance)
was accepted on 2026-11-10 with one explicit model inverse, five required controls,
strict bounds/identifiable time, bounded core Brent with stable OTM objectives and
paired original-premium/time-value acceptance. Its complete24-point package
retains caller normalization, real product objectives and truthful failures.
[Analytic-model record 04](financial/models-analytic/asian-moments.md#accepted-decision-for-discrete-asian-pricing)
was accepted on 2026-11-10 for one discrete Asian pricing API with prepared
fixing levels and independent payment discount. The decision covers the exact
geometric law, arithmetic moment matching on the future contribution, and
seasoned equity and commodity reconciliation.
[Analytic-model record 05](financial/models-analytic/barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash)
was accepted on 2026-11-10 for checked continuous barrier premiums,
independent first-passage tails and explicit hit/no-hit cash timing with
bounded numerical work.
[Analytic-model record 06](financial/models-analytic/lookback-extrema.md#accepted-decision-for-continuous-lookback-premiums)
was accepted on 2026-11-10 for checked inclusive continuous lookbacks with
qualified extrema, independent carry and discount, positive payoff components
and exact carry limits.
[Analytic-model record 07](financial/models-analytic/quanto-drift.md#accepted-decision-for-fixed-conversion-quanto-premiums)
was accepted on 2026-11-10 for one checked normalized quanto price,
explicit payout-per-asset FX orientation and signed covariance, with
contractual conversion and quantity at the dated owner.
[Analytic-model record 08](financial/models-analytic/heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform)
was accepted on 2026-11-10 for checked parameters, normalized cash and
share contours, the decaying affine logarithm and exact boundary laws.
[Analytic-model record 09](financial/models-analytic/heston-fourier-integration.md#accepted-decision-for-bounded-heston-fourier-pricing)
was accepted on 2026-11-10 for one checked Heston price, exact singular
payoffs, stable OTM inversion, conservative tail enclosure and bounded mesh work.
[Analytic-model record 10](financial/models-analytic/cos-inversion.md#accepted-decision-for-checked-cos-pricing)
was accepted on 2026-11-10 for direct bounded COS payoffs with explicit premium tolerance,
separate window and series bounds, and capped work.
[Analytic-model record 11](financial/models-analytic/jump-characteristic-laws.md#accepted-decision-for-checked-characteristic-laws)
was accepted on 2026-11-10 for checked forward-normalized Black, Merton and variance-gamma
laws with selected contours, exact moment domains and support.
[Analytic-model record 12](financial/models-analytic/rough-heston-riccati.md#accepted-decision-for-checked-rough-heston-pricing)
was accepted on 2026-11-10 for checked rough Heston Lewis pricing with explicit tolerances,
an analytic tail enclosure, separate time and mesh checks, and bounded work.
[Analytic-model record 13](financial/models-analytic/sabr-expansions.md#accepted-decision-for-explicit-checked-sabr-quotes)
was accepted on 2026-11-10 for explicit SABR quote conventions, canonical parameter admission,
exact CEV distance, stable skew and Hagan midpoint time correction.
[Analytic-model record 14](financial/models-analytic/sabr-calibration.md#accepted-decision-for-explicit-checked-sabr-calibration)
was accepted on 2026-11-10 for explicit SABR fitting, native quote-error acceptance,
literal shifts and an observed-ATM pin with a checked alpha bracket.
[Analytic-model record 15](financial/models-analytic/sabr-smile-repair.md#accepted-decision-for-checked-sabr-smile-and-finite-grid-repair)
was accepted on 2026-11-10 for the checked ATM delta proxy, unit call grid diagnostics
and exact down-only repair with move and repricing gates.
[Analytic-model record 16](financial/models-analytic/volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization)
was accepted on 2026-11-10 for typed source evaluation, exact support and explicit boundaries,
native sigma and total-variance interpolation, qualified SABR cube modes,
strict materialization and native-unit reference moves.
[Analytic-model record 17](financial/models-analytic/fx-delta-smiles.md#accepted-decision-for-checked-fx-delta-smiles-and-strike-materialization)
was accepted on 2026-11-10 for checked FX delta strikes and labelled pillars, strict-expiry quote
evaluation and explicit strike wings, plus exact-union materialization
with qualified row forwards.
[Analytic-model record 18](financial/models-analytic/svi-fit-and-density.md#accepted-decision-for-checked-raw-svi-and-slice-fit-acceptance)
was accepted on 2026-11-10 for checked raw SVI, exact shape and wing boundaries, a finite density
scan, one seeded checked fit and separate annual-volatility and sampled-density
acceptance.
[Analytic-model record 19](financial/models-analytic/dupire-local-volatility.md#accepted-decision-for-checked-dupire-extraction-and-local-volatility-grids)
was accepted on 2026-11-10 for checked fixed-moneyness Dupire stencils, explicit extraction
support and smoothing, and checked local sigma grids and simulation
consumption.
[Analytic-model record 20](financial/models-analytic/surface-arbitrage-diagnostics.md#accepted-decision-for-checked-surface-arbitrage-diagnostics-and-coverage)
was accepted on 2026-11-10 for checked sampled call-price, common-support calendar and Dupire
sign checks, with explicit coverage and unfiltered status.
[Analytic-model record 21](financial/models-analytic/variance-replication.md#accepted-decision-for-checked-variance-replication-and-explicit-black-wings)
was accepted on 2026-11-10 for checked forward-split variance replication, an explicit mesh
gate and selected finite-strip or full flat-Black wings.
[Analytic-model record 22](financial/models-analytic/atm-volatility-conversion.md#accepted-decision-for-checked-price-equivalent-atm-volatility-conversion)
was accepted on 2026-11-10 for literal at-the-forward conversion using checked analytic
pricing, direct inversion and premium replay in the smaller boundary
component.
[Analytic-model record 23](financial/models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures)
was accepted on 2026-11-10 for checked scheduled Hull–White moments
and affine bonds, explicit payment-measure caplets and coupons,
exact simple futures bias and a price-equivalent term ATM Normal quote.
[Analytic-model record 24](financial/models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection)
was accepted on 2026-11-10 for checked dated clock mapping, an exact
deterministic curve shift, contractual term-period reconstruction and
active fitted-state rules.
[Analytic-model record 25](financial/models-analytic/hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling)
was accepted on 2026-11-10 for exact joint Hull–White state and log-bank
steps, explicit event and stream controls, and pair-based sampling
uncertainty.
[Analytic-model record 26](financial/models-analytic/hw1f-issuer-lsmc.md#accepted-decision-for-checked-issuer-call-policy-training-and-pricing)
was accepted on 2026-11-10 for dated issuer-call cash, checked
continuation regression and independent pricing of a frozen policy.
[Analytic-model record 27](financial/models-analytic/diebold-li-var.md#accepted-decision-for-checked-nelsonsiegel-factors-and-var-forecasts)
was accepted on 2026-11-10 for explicit observation cadence, checked
Nelson–Siegel factor extraction, stable VAR dynamics and marginal
fitted-yield forecasts.
[Analytic-model record 28](financial/models-analytic/yield-pca.md#accepted-decision-for-checked-yield-change-pca-and-scenarios)
was accepted on 2026-11-11 for checked yield-change PCA, full-spectrum
variance accounting and fitted-basis reconstruction and scenarios.
All 28 analytic-model records are Accepted.
[Numerical-model record 001](financial/models-numerical/equity-binomial-construction.md#accepted-decision-for-checked-crr-and-leisenreimer-lattices)
was accepted on 2026-11-11 for checked CRR and Leisen–Reimer
transitions, explicit lattice counts and exact vanilla payoff limits.
[Numerical-model record 002](financial/models-numerical/dividends-and-lattice-exercise.md#accepted-decision-for-checked-escrowed-dividends-and-lattice-exercise)
was accepted on 2026-11-11 for dated ex-claim values, a common escrow
reserve horizon, exact alignment and product-qualified exercise phases.
[Numerical-model record 003](financial/models-numerical/tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback)
was accepted on 2026-11-11 for checked interval discounting, explicit
nodewise OAS, validated rollback and genuine terminal state.
[Numerical-model record 004](financial/models-numerical/ho-lee-curve-fit.md#accepted-decision-for-checked-holee-curve-fitting)
was accepted on 2026-11-11 for origin-relative Ho–Lee targets,
checked additive row fits and one repricing gate.
[Numerical-model record 005](financial/models-numerical/bdt-lognormal-curve-fit.md#accepted-decision-for-checked-bdt-curve-fitting)
was accepted on 2026-11-11 for strictly decreasing BDT targets,
unclamped log-rate fits and one inclusive replay gate.
[Numerical-model record 006](financial/models-numerical/black-karasinski-trinomial.md#accepted-decision-for-checked-bk-trinomial-fitting)
was accepted on 2026-11-11 for exact conditional OU moments on a
capped trinomial, positive rate fitting and one inclusive replay gate.
[Numerical-model record 007](financial/models-numerical/hull-white-event-grid-and-affine-bonds.md#accepted-decision-for-checked-hullwhite-event-lattices-and-node-bonds)
was accepted on 2026-11-11 for exact Hull–White event grids,
conditional OU moments, Continuous base fitting and explicit
affine exercise valuation.
[Numerical-model record 008](financial/models-numerical/rates-credit-marginal-calibration.md#accepted-decision-for-checked-rates-credit-marginal-calibration)
was accepted on 2026-11-11 for centered binary marginals,
strict variance retention, local integrated-hazard fitting
and checked replay.
[Numerical-model record 009](financial/models-numerical/rates-credit-coupling-and-sampling.md#accepted-decision-for-checked-rates-credit-coupling-and-path-sampling)
was accepted on 2026-11-11 for strict Bernoulli coupling, indexed
Philox midpoints and bound segment replay.
[Numerical-model record 010](financial/models-numerical/rates-credit-coupon-folding.md#accepted-decision-for-checked-joint-credit-rollback-and-coupon-folding)
was accepted on 2026-11-11 for checked joint survival and recovery,
projection on exact index dates and a full coupon fold with explicit
exercise eligibility.
[Numerical-model record 011](financial/models-numerical/pde-grids-interpolation-and-greeks.md#accepted-decision-for-checked-pde-grids-interpolation-and-coordinate-greeks)
was accepted on 2026-11-11 for validated spatial grids, closed-domain
value extraction and interpolated quadratic nodal derivative estimates.
[Numerical-model record 012](financial/models-numerical/pde-spatial-stencils.md#accepted-decision-for-checked-directional-and-mixed-pde-stencils)
was accepted on 2026-11-11 for checked diffusion admission, an inclusive
central-to-one-sided drift switch and an explicit tensor mixed stencil.
[Numerical-model record 013](financial/models-numerical/pde-boundary-elimination.md#accepted-decision-for-checked-pde-boundary-elimination-and-reconstruction)
was accepted on 2026-11-11 for shared checked edge relations, explicit
x-edge corner ownership and stage-consistent reconstruction and cache reuse.
[Numerical-model record 014](financial/models-numerical/theta-time-march-and-thomas.md#accepted-decision-for-checked-theta-marching-and-tridiagonal-solves)
was accepted on 2026-11-11 for exact event-grid theta steps, full-interval
damping and one checked Thomas kernel with a componentwise residual gate.
[Numerical-model record 015](financial/models-numerical/pde-exercise-projection.md#accepted-decision-for-checked-grid-based-pde-exercise)
was accepted on 2026-11-11 for exact post-step payoff comparison,
exact exercise knots and actual-spot immediate exercise at eligible
equity valuations.
[Numerical-model record 016](financial/models-numerical/mcs-adi-time-march.md#accepted-decision-for-checked-modified-craigsneyd-adi)
was accepted on 2026-11-11 for fixed full MCS, checked factorized startup
and atomic stages with application-specific stability and price qualification.
[Numerical-model record 017](financial/models-numerical/black-scholes-and-heston-pde-bridges.md#accepted-decision-for-checked-pde-coefficient-bridges)
was accepted on 2026-11-11 for checked bridge parameters and diffusion,
explicit edge laws, exact deterministic limits and qualified price and
risk boundaries. All 17 trees and finite differences records are accepted.
Runtime/caller/host/state reconciliation and independent qualification remain pending.
The first Monte Carlo package was accepted on 2026-11-11 for explicit
fixed-budget pricing, checked event and discount
handling, deterministic execution and fixed-sample uncertainty.
[Monte Carlo record 01](financial/models-monte-carlo/01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).
The complete Philox stream and seed derivation package was accepted on
2026-11-11. Runtime and host reconciliation remain pending.
[Monte Carlo record 02](financial/models-monte-carlo/02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).
The whole-path Sobol pricing and replicate diagnostics package was
accepted on 2026-11-11. Runtime, host parity and price qualification
remain pending. [Monte Carlo record 03](financial/models-monte-carlo/03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics).
The complete GBM, dated ATM variance and local-volatility package was
accepted on 2026-11-11. Runtime and financial qualification remain pending.
[Monte Carlo record 04](financial/models-monte-carlo/04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths).
The complete coordinatewise Euler and exact Gaussian steps package was
accepted on 2026-11-11. Runtime and model qualification remain pending.
[Monte Carlo record 05](financial/models-monte-carlo/05-generic-steppers.md#accepted-decision-for-checked-coordinatewise-euler-and-exact-gaussian-steps).
The complete discrete-dividend jump package was accepted on 2026-11-11.
Runtime, host and financial qualification remain pending.
[Monte Carlo record 06](financial/models-monte-carlo/06-cash-dividend-jumps.md#accepted-decision-for-checked-discrete-dividend-jumps-on-exact-grid-knots).
The complete Hull–White endpoint and curve-fit package was accepted on
2026-11-11. Runtime, host and product qualification remain pending.
[Monte Carlo record 07](financial/models-monte-carlo/07-hull-white.md#accepted-decision-for-checked-hullwhite-endpoints-and-curve-fit-boundaries).
The complete CIR and CIR++ quadratic-exponential package was accepted on
2026-11-11. Runtime, host and product qualification remain pending.
[Monte Carlo record 08](financial/models-monte-carlo/08-cir-qe.md#accepted-decision-for-checked-cir-and-cir-quadratic-exponential-paths).
The checked Heston QE-M package was accepted on 2026-11-11. Runtime,
host and product qualification remain pending.
[Monte Carlo record 09](financial/models-monte-carlo/09-heston-qe.md#accepted-decision-for-checked-heston-qe-m-paths).
The displaced terminal-measure LMM package was accepted on 2026-11-11.
Runtime, host and product qualification remain pending.
[Monte Carlo record 10](financial/models-monte-carlo/10-lmm.md#accepted-decision-for-checked-terminal-measure-lmm-paths).
The checked revolving-credit path package was accepted on 2026-11-11.
Runtime, host and product qualification remain pending.
[Monte Carlo record 11](financial/models-monte-carlo/11-credit-line.md#accepted-decision-for-checked-revolving-credit-paths).
The checked Schwartz–Smith path and expectation package was accepted on
2026-11-11. Runtime, host and product qualification remain pending.
[Monte Carlo record 12](financial/models-monte-carlo/12-schwartz-smith.md#accepted-decision-for-checked-schwartzsmith-paths-and-expectations).
The fractional-noise and Volterra driver package was accepted on
2026-11-11. Runtime, host and product qualification remain pending.
[Monte Carlo record 13](financial/models-monte-carlo/13-fractional-noise.md#accepted-decision-for-checked-fractional-noise-and-volterra-drivers).
The adapted rough Bergomi and bounded pricing package was accepted on
2026-11-11. Runtime, host and product qualification remain pending.
[Monte Carlo record 14](financial/models-monte-carlo/14-rough-bergomi.md#accepted-decision-for-checked-rough-bergomi-spot-and-pricing).
The checked rough Heston hybrid and pricing package was accepted on
2026-11-11. Runtime, host and product qualification remain pending.
[Monte Carlo record 15](financial/models-monte-carlo/15-rough-heston.md#accepted-decision-for-checked-rough-heston-paths-and-pricing).
The checked rough Cheyette raw path package was accepted on 2026-11-11.
Runtime, host, grid and financial qualification remain pending.
[Monte Carlo record 16](financial/models-monte-carlo/16-rough-cheyette.md#accepted-decision-for-checked-rough-cheyette-paths).
The checked terminal and Asian payoff package was accepted on 2026-11-11.
Runtime, host and product qualification remain pending.
[Monte Carlo record 17](financial/models-monte-carlo/17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs).
The checked Monte Carlo barrier monitoring and rebate package was accepted
on 2026-11-11. Runtime, host and product qualification remain pending.
[Monte Carlo record 18](financial/models-monte-carlo/18-barrier-monitoring.md#accepted-decision-for-checked-monte-carlo-barrier-monitoring-and-rebates).
The checked Monte Carlo lookback package was accepted on 2026-11-11.
Runtime, host and product qualification remain pending.
[Monte Carlo record 19](financial/models-monte-carlo/19-lookback-monitoring.md#accepted-decision-for-checked-monte-carlo-lookback-extrema).
The checked Monte Carlo autocallable payoff package was accepted on
2026-11-11. Runtime, host and dated product qualification remain pending.
[Monte Carlo record 20](financial/models-monte-carlo/20-autocallable.md#accepted-decision-for-checked-monte-carlo-autocallable-payoffs).
The checked Monte Carlo cliquet payoff package was accepted on
2026-11-11. Runtime, host and dated product qualification remain pending.
[Monte Carlo record 21](financial/models-monte-carlo/21-cliquet.md#accepted-decision-for-checked-monte-carlo-cliquet-payoffs).
The checked Monte Carlo range accrual package was accepted on
2026-11-11. Runtime, host and dated product qualification remain pending.
[Monte Carlo record 22](financial/models-monte-carlo/22-range-accrual.md#accepted-decision-for-checked-monte-carlo-range-accrual).
The checked holder exercise package was accepted on 2026-11-11.
Runtime, host and financial qualification remain pending.
[Monte Carlo record 23](financial/models-monte-carlo/23-lsmc-exercise.md#accepted-decision-for-checked-holder-exercise-policy-training-and-pricing).
The checked continuation regression package was accepted on 2026-11-11.
Runtime, host and financial qualification remain pending.
[Monte Carlo record 24](financial/models-monte-carlo/24-regression-policies.md#accepted-decision-for-checked-continuation-regression).
The checked paired spot Greek package was accepted on 2026-11-11.
Runtime, host and financial qualification remain pending.
[Monte Carlo record 25](financial/models-monte-carlo/25-finite-difference-greeks.md#accepted-decision-for-checked-paired-monte-carlo-spot-greeks).
The checked GBM likelihood-ratio package was accepted on 2026-11-11.
Runtime, host and sensitivity qualification remain pending.
[Monte Carlo record 26](financial/models-monte-carlo/26-likelihood-ratio-greeks.md#accepted-decision-for-checked-gbm-likelihood-ratio-greeks).
The checked single-control package was accepted on 2026-11-11.
Runtime, dated product and host qualification remain pending.
[Monte Carlo record 27](financial/models-monte-carlo/27-control-variate.md#accepted-decision-for-checked-single-control-monte-carlo-pricing).
The checked path capture and raw simulation package was accepted on
2026-11-11. This completes adoption of all 28 Monte Carlo records.
Runtime, host and financial qualification remain pending.
[Monte Carlo record 28](financial/models-monte-carlo/28-capture-and-simulation-contracts.md#accepted-decision-for-checked-path-capture-and-raw-simulation).
The checked distress-score package was accepted on 2026-11-11.
Runtime, host and empirical qualification remain pending.
[Credit record 01](financial/models-and-valuations-credit/01-financial-distress-scores.md#accepted-decision-for-checked-financial-distress-scores).
The checked PD transform and rating-data package was accepted on
2026-11-11. Runtime, host, empirical and regulatory qualification remain pending.
[Credit record 02](financial/models-and-valuations-credit/02-pd-scales-and-rating-data.md#accepted-decision-for-checked-pd-transforms-and-explicit-rating-data).
The checked rating migration, generator fit and path package was accepted on
2026-11-11. Runtime, host, numerical and empirical qualification remain pending.
[Credit record 03](financial/models-and-valuations-credit/03-rating-migration.md#accepted-decision-for-checked-rating-migration-generator-fitting-and-paths).
The checked structural default and calibration package was accepted on
2026-11-11. Runtime, host, market and empirical qualification remain pending.
[Credit record 04](financial/models-and-valuations-credit/04-structural-default.md#accepted-decision-for-checked-structural-default-models-and-calibrated-outputs).
The checked leverage hazard and PIK election package was accepted on
2026-11-11. Runtime, host, market and issuer-policy qualification remain pending.
[Credit record 05](financial/models-and-valuations-credit/05-hazard-and-toggle.md#accepted-decision-for-checked-leverage-hazard-and-pik-election-models).
The checked exposure, loss and recovery allocation package and scoped
core beta-quantile amendment were accepted on 2026-11-12. Runtime,
host, empirical and regulatory qualification remain pending.
[Credit record 06](financial/models-and-valuations-credit/06-recovery-and-lgd.md#accepted-decision-for-checked-exposure-loss-and-recovery-allocations),
[core beta-quantile amendment](financial/core-numerics/distribution-functions.md#accepted-amendment-checked-rust-beta-quantile).
The pool default rate and period copula package was accepted on 2026-11-12.
Runtime, host and product qualification remain pending.
[Credit record 07](financial/models-and-valuations-credit/07-pool-defaults.md#accepted-decision-for-checked-pool-default-rates-and-period-copula-resolution).
The checked prepayment and pool correlation package was accepted on
2026-11-12. Runtime, host, market and product qualification remain pending.
[Credit record 08](financial/models-and-valuations-credit/08-prepayment-and-pool-correlation.md#accepted-decision-for-checked-prepayment-and-pool-correlation).
The checked copula laws and latent factors package was accepted on
2026-11-12. Runtime, host and product qualification remain pending.
[Credit record 09](financial/models-and-valuations-credit/09-copula-kernels.md#accepted-decision-for-checked-copula-laws-and-latent-factors).
The checked portfolio loss and correlated recovery package was accepted on
2026-11-12. Runtime, host, calibration and sampling qualification remain pending.
[Credit record 10](financial/models-and-valuations-credit/10-portfolio-loss-and-correlated-recovery.md#accepted-decision-for-checked-portfolio-loss-and-correlated-recovery).
The checked liquidity and execution estimates package was accepted on
2026-11-12. Runtime, host and empirical qualification remain pending.
[Credit record 11](financial/models-and-valuations-credit/11-liquidity.md#accepted-decision-for-checked-liquidity-and-execution-estimates).
The checked CDS legs and quote-basis package was accepted on 2026-11-13.
Runtime, host, calibration and external-reference qualification remain pending.
[Credit record 12](financial/models-and-valuations-credit/12-cds-legs.md#accepted-decision-for-checked-cds-legs-and-quote-basis).
The checked CDS and index risk package was accepted on 2026-11-13.
Runtime, host, calibration and margin qualification remain pending.
[Credit record 13](financial/models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk).
The checked CDS index valuation and cashflows package was accepted on
2026-11-13. Runtime, host and external-reference qualification remain pending.
[Credit record 14](financial/models-and-valuations-credit/14-index-valuation.md#accepted-decision-for-checked-cds-index-valuation-and-cashflows).
The checked CDS option valuation package was accepted on 2026-11-13.
Runtime, host and external-reference qualification remain pending.
[Credit record 15](financial/models-and-valuations-credit/15-cds-option-quadrature.md#accepted-decision-for-checked-cds-option-valuation).
The checked CDS option risk and implied volatility package was accepted on
2026-11-13. Runtime, host, market replay and external qualification remain pending.
[Credit record 16](financial/models-and-valuations-credit/16-cds-option-risk-and-inversion.md#accepted-decision-for-checked-cds-option-risk-and-implied-volatility).
The checked finite-pool tranche loss package was accepted on 2026-11-13.
Runtime, calibration, dated market and host qualification remain pending.
[Credit record 17](financial/models-and-valuations-credit/17-tranche-loss-distribution.md#accepted-decision-for-checked-finite-pool-tranche-losses).
The checked tranche cashflows and settlement package was accepted on
2026-11-13. Runtime, dated market, calibration and host qualification remain pending.
[Credit record 18](financial/models-and-valuations-credit/18-tranche-projection.md#accepted-decision-for-checked-tranche-cashflows-and-settlement).
The checked tranche risk and default scenarios package was accepted on
2026-11-13. Runtime, replay, dated market and host qualification remain pending.
[Credit record 19](financial/models-and-valuations-credit/19-tranche-risk.md#accepted-decision-for-checked-tranche-risk-and-default-scenarios).
The checked exchange and liability-management package was accepted on
2026-11-13, completing review of all 20 detailed credit and liquidity records.
Runtime and host qualification remain pending.
[Credit record 20](financial/models-and-valuations-credit/20-liability-management.md#accepted-decision-for-checked-exchange-and-liability-management-arithmetic).
The checked valuation dispatch and lifecycle package was accepted on
2026-11-13. Runtime, replay and host qualification remain pending.
[Valuation framework record 01](financial/valuations-framework/pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).
The checked valuation model inputs and controls package was accepted on
2026-11-13. Runtime, fit replay and host qualification remain pending.
[Valuation framework record 02](financial/valuations-framework/model-resolution.md#accepted-decision-for-checked-valuation-model-inputs-and-controls).
The checked listed futures and options package was accepted on 2026-11-13.
Runtime, venue settlement, lattice accuracy and host qualification remain pending.
[Valuation framework record 03](financial/valuations-framework/listed-products.md#accepted-decision-for-checked-listed-futures-and-options).
The checked overnight projection and quote risk package was accepted on
2026-11-13. Runtime, dated fixing and host qualification remain pending.
[Valuation framework record 04](financial/valuations-framework/overnight-projection.md#accepted-decision-for-checked-overnight-projection-and-quote-risk).
The checked dated rates-credit valuation package was accepted on 2026-11-13.
Runtime, product-price, risk and host qualification remain pending.
[Valuation framework record 05](financial/valuations-framework/rates-credit-grid.md#accepted-decision-for-checked-dated-rates-credit-valuation).
The checked TRS legs and par spread package was accepted on 2026-11-13.
Runtime, dated product and host qualification remain pending.
[Valuation framework record 06](financial/valuations-framework/trs-legs.md#accepted-decision-for-checked-trs-legs-and-par-spread).
The observation schedules and monitoring grids package was accepted on
2026-11-13. Runtime, product-price and host qualification remain pending.
[Valuation framework record 07](financial/valuations-framework/observation-grids.md#accepted-decision-for-checked-observation-schedules-and-monitoring-grids).
The metric orchestration and reporting package was accepted on 2026-11-13.
Runtime, reporting and host qualification remain pending.
[Valuation framework record 08](financial/valuations-framework/metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).
The finite differences and rate risk package was accepted on 2026-11-13.
Runtime, calibration-replay and host qualification remain pending.
[Valuation framework record 09](financial/valuations-framework/finite-difference-rates.md#accepted-decision-for-checked-finite-differences-and-rate-risk).
The checked credit sensitivities package was accepted on 2026-11-13.
Runtime, calibration-replay and host qualification remain pending.
[Valuation framework record 10](financial/valuations-framework/credit-sensitivities.md#accepted-decision-for-checked-credit-sensitivities).
The checked option and FX sensitivities package was accepted on 2026-11-13.
Runtime, financial accuracy and host qualification remain pending.
[Valuation framework record 11](financial/valuations-framework/option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities).
The checked cross-factor risk package was accepted on 2026-11-13.
Runtime, replay, attribution and host qualification remain pending.
[Valuation framework record 12](financial/valuations-framework/cross-factor-risk.md#accepted-decision-for-checked-cross-factor-risk).
The checked horizon theta, carry and breakeven package was accepted on 2026-11-14.
Runtime, product and host qualification remain pending.
[Valuation framework record 13](financial/valuations-framework/carry-theta-breakeven.md#accepted-decision-for-checked-horizon-theta-carry-and-breakeven).
The checked historical scenario risk package was accepted on 2026-11-14.
Runtime, calibration-replay, financial accuracy and host qualification remain pending.
[Valuation framework record 14](financial/valuations-framework/historical-risk.md#accepted-decision-for-checked-historical-scenario-risk).
The checked cashflow valuation and export package was accepted on 2026-11-14.
Runtime, product and host qualification remain pending.
[Valuation framework record 15](financial/valuations-framework/cashflow-export.md#accepted-decision-for-checked-cashflow-valuation-and-export).
The checked convention selection and dated sources package was accepted on 2026-11-14.
Full row, product, calibration and host qualification remain pending.
[Valuation framework record 16](financial/valuations-framework/convention-selection.md#accepted-decision-for-checked-convention-selection-and-dated-sources).
The checked inflation references and information dates package was accepted on 2026-11-14.
Runtime, product, calibration and host qualification remain pending.
[Valuation framework record 17](financial/valuations-framework/inflation-references.md#accepted-decision-for-checked-inflation-references-and-information-dates).
The checked margin repricing and identity package was accepted on 2026-11-14.
Runtime, replay, margin reporting and host qualification remain pending.
[Valuation framework record 18](financial/valuations-framework/margin-bridge.md#accepted-decision-for-checked-margin-repricing-and-identity).
The checked shared contract and stochastic controls package was accepted on 2026-11-14.
Runtime, product, calibration and host qualification remain pending.
[Valuation framework record 19](financial/valuations-framework/shared-contract-controls.md#accepted-decision-for-checked-shared-contract-and-stochastic-controls).
The generated financial builder package was accepted on 2026-11-14,
completing adoption of all 20 detailed shared valuation framework records.
Macro, consumer, restore and host reconciliation and qualification remain pending.
[Valuation framework record 20](financial/valuations-framework/builder-validation.md#accepted-decision-for-checked-generated-financial-builders).
The checked deposit cashflows and quote inversion package was accepted on 2026-11-14.
Runtime, calibration and host qualification remain pending.
[Rates valuation record 01](financial/valuations-rates/01-deposits.md#accepted-decision-for-checked-deposit-cashflows-and-quote-inversion).
The checked FRA fixing, settlement and risk package was accepted on 2026-11-14.
Runtime, calibration, reporting and host qualification remain pending.
[Rates valuation record 02](financial/valuations-rates/02-fra.md#accepted-decision-for-checked-fra-fixing-settlement-and-risk).
The checked interest-rate swap cashflows and risk package was accepted on 2026-11-14.
Runtime, calibration, margin and host qualification remain pending.
[Rates valuation record 03](financial/valuations-rates/03-interest-rate-swaps.md#accepted-decision-for-checked-interest-rate-swap-cashflows-and-risk).
The checked basis swap cashflows and par spreads package was accepted on 2026-11-14.
Runtime, calibration and host qualification remain pending.
[Rates valuation record 04](financial/valuations-rates/04-basis-swaps.md#accepted-decision-for-checked-basis-swap-cashflows-and-par-spreads).
The checked cap and floor optionlet pricing package was accepted on 2026-11-14.
Runtime, model, calibration and host qualification remain pending.
[Rates valuation record 05](financial/valuations-rates/05-cap-floor-pricing.md#accepted-decision-for-checked-cap-and-floor-optionlet-pricing).
The checked cap and floor risk and implied volatility package was accepted on 2026-11-14.
Runtime, model, calibration and host qualification remain pending.
[Rates valuation record 06](financial/valuations-rates/06-cap-floor-risk.md#accepted-decision-for-checked-cap-and-floor-risk-and-implied-volatility).
The checked European swaption pricing and risk package was accepted on 2026-11-14.
Runtime, model, calibration and host qualification remain pending.
[Rates valuation record 07](financial/valuations-rates/07-european-swaptions.md#accepted-decision-for-checked-european-swaption-pricing-and-risk).
The checked Hull–White swaption exercise and risk package was accepted on 2026-11-14.
Runtime, numerical, calibration and host qualification remain pending.
[Rates valuation record 08](financial/valuations-rates/08-hull-white-swaptions.md#accepted-decision-for-checked-hull-white-swaption-exercise-and-risk).
The checked LMM Bermudan pricing and risk package was accepted on 2026-11-14.
Runtime, numerical, calibration and host qualification remain pending.
[Rates valuation record 09](financial/valuations-rates/09-lmm-bermudans.md#accepted-decision-for-checked-lmm-bermudan-pricing-and-risk).
The checked rate futures and futures option risk package was accepted on 2026-11-14.
Runtime, numerical, calibration and host qualification remain pending.
[Rates valuation record 10](financial/valuations-rates/10-rate-futures.md#accepted-decision-for-checked-rate-futures-and-futures-option-risk).
The checked CMS convexity pricing and risk package was accepted on 2026-11-14.
Runtime, model and host qualification remain pending.
[Rates valuation record 11](financial/valuations-rates/11-cms-convexity.md#accepted-decision-for-checked-cms-convexity-pricing-and-risk).
The checked CMS smile replication and risk package was accepted on 2026-11-14.
Runtime, model, market and host qualification remain pending.
[Rates valuation record 12](financial/valuations-rates/12-cms-replication.md#accepted-decision-for-checked-cms-smile-replication-and-risk).
The checked inflation swap cashflows and risk package was accepted on 2026-11-14.
Runtime, calibration and host qualification remain pending.
[Rates valuation record 13](financial/valuations-rates/13-inflation-swaps.md#accepted-decision-for-checked-inflation-swap-cashflows-and-risk).
The checked inflation option cashflows, expiry and risk package was accepted on 2026-11-14.
Runtime, market and host qualification remain pending.
[Rates valuation record 14](financial/valuations-rates/14-inflation-options.md#accepted-decision-for-checked-inflation-option-cashflows-expiry-and-risk).
The checked repo cashflows, collateral and risk package was accepted on 2026-11-14.
Runtime, market, margin and host qualification remain pending.
[Rates valuation record 15](financial/valuations-rates/15-repo.md#accepted-decision-for-checked-repo-cashflows-collateral-and-risk).
The checked cross-currency swap cashflows, FX resets and risk package was accepted on 2026-11-14.
Runtime, market, calibration and host qualification remain pending.
[Rates valuation record 16](financial/valuations-rates/16-cross-currency-swaps.md#accepted-decision-for-checked-cross-currency-swap-cashflows-fx-resets-and-risk).
The checked CMS spread cashflows, model and risk package was accepted on 2026-11-14.
Runtime, market and host qualification remain pending.
[Rates valuation record 17](financial/valuations-rates/17-cms-spread.md#accepted-decision-for-checked-cms-spread-cashflows-model-and-risk).
The checked bond schedules and holder cash package was accepted on 2026-11-14.
Runtime, market and host qualification remain pending.
[Bond valuation record 01](financial/valuations-bonds/01-schedule-and-holder-cashflows.md#accepted-decision-for-checked-bond-schedules-and-holder-cash).
The checked bond settlement, quote carry and clocks package was accepted on 2026-11-14.
Runtime, market, model and host qualification remain pending.
[Bond valuation record 02](financial/valuations-bonds/02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks).
The checked bond PV selection and fractional recovery of par package was accepted on 2026-11-14.
Runtime, market, model and host qualification remain pending.
[Bond valuation record 03](financial/valuations-bonds/03-pv-models.md#accepted-decision-for-checked-bond-pv-selection-and-fractional-recovery-of-par).
The checked rates-only bond tree and exercise package was accepted on 2026-11-14.
Runtime, fit, convergence, market and host qualification remain pending.
[Bond valuation record 04](financial/valuations-bonds/04-rates-tree-exercise.md#accepted-decision-for-checked-bond-rates-trees-and-exercise).
The checked issue-basis return floor package was accepted on 2026-11-14.
Runtime, contract, path replay and host qualification remain pending.
[Bond valuation record 05](financial/valuations-bonds/05-return-floors.md#accepted-decision-for-checked-issue-basis-return-floors).
The checked joint bond payoff and recovery replay package was accepted on 2026-11-14.
Runtime, stochastic calibration, market, risk and host qualification remain pending.
[Bond valuation record 06](financial/valuations-bonds/06-joint-coupon-replay.md#accepted-decision-for-checked-joint-bond-payoff-and-recovery-replay).
The checked joint bond exercise policy and make-whole package was accepted on 2026-11-14.
Runtime, economic fit, pricing, risk and host qualification remain pending.
[Bond valuation record 07](financial/valuations-bonds/07-lsmc-policy.md#accepted-decision-for-checked-joint-bond-exercise-policy-and-make-whole).
The checked structural bond simulation and outputs package was accepted on 2026-11-14.
Runtime, stochastic convergence, market and host qualification remain pending.
[Bond valuation record 08](financial/valuations-bonds/08-merton-simulation.md#accepted-decision-for-checked-structural-bond-simulation-and-outputs).
The checked cash-equivalent structural calibration package was accepted on 2026-11-14.
Runtime, stochastic fit, market, risk and host qualification remain pending.
[Bond valuation record 09](financial/valuations-bonds/09-merton-calibration.md#accepted-decision-for-checked-cash-equivalent-structural-calibration).
The checked settlement yield and price inversion package was accepted on 2026-11-14.
Runtime, market-specific quote, risk and host qualification remain pending.
[Bond valuation record 10](financial/valuations-bonds/10-quoted-yield.md#accepted-decision-for-checked-settlement-yield-and-price-inversion).
The checked bond spreads and price inversion package was accepted on 2026-11-14.
Runtime, market, workout, risk and host qualification remain pending.
[Bond valuation record 12](financial/valuations-bonds/12-spread-quotes.md#accepted-decision-for-checked-bond-spreads-and-price-inversion).
The checked bond asset-swap package was accepted on 2026-11-14.
Market, runtime, workout and host qualification remain pending.
[Bond valuation record 13](financial/valuations-bonds/13-asset-swaps.md#accepted-decision-for-checked-bond-asset-swaps-and-price-inversion).
The checked quoted-yield bond risk package, including unscaled published convexity, was accepted on 2026-11-14.
Market, runtime, workout and host qualification remain pending.
[Bond valuation record 14](financial/valuations-bonds/14-yield-risk.md#accepted-decision-for-checked-quoted-yield-bond-risk).
The checked curve DV01, key-rate and effective bond risk package was accepted on 2026-11-14.
Market, model, runtime and host qualification remain pending.
[Bond valuation record 15](financial/valuations-bonds/15-curve-risk.md#accepted-decision-for-checked-bond-curve-and-effective-risk).
The checked bond CS01 and spread duration package was accepted on 2026-11-14.
Market replay, model, runtime and host qualification remain pending.
[Bond valuation record 16](financial/valuations-bonds/16-spread-credit-risk.md#accepted-decision-for-checked-bond-spread-and-credit-risk).
The checked bond recovery and jump-to-default package was accepted on 2026-11-14.
Legal-claim, market replay, model, runtime and host qualification remain pending.
[Bond valuation record 17](financial/valuations-bonds/17-default-risk.md#accepted-decision-for-checked-bond-recovery-and-jump-to-default).
The checked bond embedded-option package and currency `embedded_option_vega` were accepted on 2026-11-14.
Model, market, runtime and host qualification remain pending.
[Bond valuation record 18](financial/valuations-bonds/18-embedded-option-risk.md#accepted-decision-for-checked-bond-embedded-option-value-and-vega).
The checked carry and repayment-life package, including retired-par WAL weights, was accepted on 2026-11-14.
Product, market, runtime and host qualification remain pending.
[Bond valuation record 19](financial/valuations-bonds/19-carry-and-life.md#accepted-decision-for-checked-bond-carry-and-repayment-life).
The checked bond issue-to-exit package, including signed-net MOIC and the complete issuer-call minimum, was accepted on 2026-11-14.
Contractual path, historical coverage, runtime and host qualification remain pending.
[Bond valuation record 20](financial/valuations-bonds/20-lifetime-returns.md#accepted-decision-for-checked-bond-issue-to-exit-returns).
Other unadopted choices retain their stated Proposed or Draft status. Acceptance does not certify
observed implementations.
Numerical suites were not rerun in the original documentation audits.
Subsequent analytics implementation checks are recorded in its linked evidence.
Static source discrepancies, suspected numerical defects
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
- **Statements:** preserve the previously agreed reported-cost D&A convention
  and period-close waterfall FX policy. Review period windows, forecast
  dependence, dated debt balances, check/skip semantics, ECL reversion approximations and valuation
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
- **Credit conventions and dependence:** reconcile the accepted single-name CDS
  [leg and quote-basis contract](financial/models-and-valuations-credit/12-cds-legs.md#accepted-decision-for-checked-cds-legs-and-quote-basis) linked from
  [0030](financial/models-and-valuations-credit/0030-cds-survival-and-integration.md). Agree tranche
  copula/calibration scope, settlement overrides, correlation clipping, and
  interim loss/monotonicity repairs in
  [0031](financial/models-and-valuations-credit/0031-tranche-loss-model-selection.md). Observed repairs are
  not approved fallbacks. Standalone copula configuration also needs a host
  reachability and result-provenance assessment.
- **Curve construction:** direct discount selection, tails and checked
  evaluation are accepted in [CORE-MARKET-01](financial/core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction);
  index-specific projection factors and contractual rates are accepted in
  [CORE-MARKET-02](financial/core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates).
  Continuous-zero parameter shocks and dated frozen-source conditioning are
  accepted in [CORE-MARKET-03](financial/core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning).
  Reconcile those choices without reopening them. Agree separately owned
  calibration selections, permitted global polishing/seed
  adoption and fit acceptance in
  [0032](financial/calibration-targets/0032-curve-interpolation-and-calibration.md). Distinct hazard,
  inflation, cross-currency, SABR/SVI, HW and base-correlation calibration
  choices remain pending, including approximate-knot acceptance.

### Implementation reconciliation

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
  crates across numerical frameworks, calibration, valuation, statements,
  scenarios, portfolios, attribution, margin and factor models.
  Covenant engines, panel features and Arrow conversions also have detailed
  records. Analytics methodology adoption is complete; other unadopted
  scopes remain open. The
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
