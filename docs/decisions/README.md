# Accepted project decisions

These architecture decision records (ADRs) capture decisions. Their numbers preserve the order in which they were agreed; directories group them by subject.

**Accepted** means the decision is agreed policy. It does not mean all
implementations comply or that the proposed protection checks are deployed. Where they don't comply the follow up or reconcilation is recorded later in this README.md.
Each record separates the decision, examples, enforcement requirements, and
known implementation notes.

When accepting another ADR, add it to the appropriate table and carry its
known follow-up work or conflicting guidance into the sections below. Keep
implementation gaps distinct from questions requiring a further decision;
accepting an ADR does not mark either kind of work complete.

## Project decisions

| ADR | Decision |
| --- | --- |
| [0004](project/0004-development-compatibility.md) | Remove obsolete paths during development; revisit compatibility before final release. |
| [0013](project/0013-documentation-sources.md) | Maintain authoritative documentation sources aligned with settled decisions. |
| [0014](project/0014-financial-reference-evidence.md) | Protect independently justified financial references and tolerances. |
| [0015](project/0015-authorized-change-scope.md) | Work autonomously within authorized scope; explicitly authorize decision changes. |
| [0016](project/0016-independent-contract-protection.md) | Protect contracts and checks through a trusted baseline and independent authorization. |
| [0017](project/0017-library-first-product.md) | Keep computation independently usable, with optional presentation layers. |
| [0018](project/0018-platform-and-runtime-support.md) | Protect explicit platform/runtime commitments and distinguish build-only, smoke-tested, and fully tested coverage. |
| [0019](project/0019-final-release-qualification.md) | Qualify the declared release scope and exact shipped artifacts before final publication. |
| [0025](project/0025-auditable-results-and-replay.md) | Preserve audit/replay evidence with caller-owned archives and explicit limits on reproducibility. |

## Code standards

Cross-cutting rules for naming, host interfaces, errors, serialized contracts,
and reproducibility live in `standards/`.

| ADR | Decision |
| --- | --- |
| [0003](standards/0003-canonical-public-names.md) | Use canonical Rust names across host languages. |
| [0005](standards/0005-python-wasm-parity.md) | Preserve Python/WASM capabilities and behavioral parity with explicit exceptions. |
| [0006](standards/0006-structured-errors.md) | Preserve structured error meaning across interfaces. |
| [0012](standards/0012-serialized-contracts.md) | Use an explicit Rust-owned serialized-data contract. |
| [0021](standards/0021-reproducibility-guarantees.md) | Preserve explicitly scoped, tested reproducibility guarantees and cross-host numerical agreement. |

## Specific code decisions

Concrete choices about implementation ownership, crate dependencies, and
execution design live in `architecture/`. These records apply the standards
to the project's chosen structure.

| ADR | Decision |
| --- | --- |
| [0001](architecture/0001-rust-owned-financial-behavior.md) | Rust owns financial calculations, business validation, and financial defaults. |
| [0002](architecture/0002-crate-boundaries.md) | Preserve crate responsibilities and dependency direction. |
| [0022](architecture/0022-portfolio-evaluation-state.md) | Share portfolio evaluation ownership and isolate immutable request state and financial caches. |
| [0023](architecture/0023-host-neutral-tabular-exports.md) | Use a canonical host-neutral table contract with boundary adapters for DataFrame and Arrow integration. |
| [0024](architecture/0024-explicit-extension-boundaries.md) | Support explicit native extension interfaces while keeping persisted instrument types centrally defined and schema-backed. |

## Financial decisions

| ADR | Decision |
| --- | --- |
| [0007](financial/0007-units-and-sign-conventions.md) | Treat financial units and sign conventions as public contracts. |
| [0008](financial/0008-financial-defaults-and-cagr.md) | Set explicit financial defaults; dated-window CAGR defaults to Actual/365 Fixed. |
| [0009](financial/0009-missing-and-invalid-data.md) | Distinguish invalid inputs, permitted gaps, and undefined results. |
| [0010](financial/0010-metric-identifiers.md) | Preserve metric identity, meaning, and attribution. |
| [0011](financial/0011-bond-cs01-without-hazard-curve.md) | Calculate supported no-hazard bond CS01 through z-spread bump-and-reprice. |
| [0020](financial/0020-numerical-representation-and-rounding.md) | Separate floating-point model numerics from Decimal monetary paths and explicit rounding boundaries. |

## Applying the decisions

Read the records relevant to a task and identify their IDs in the change
description. Implementation choices remain autonomous while those contracts
hold. Explicit authorization to change a decision covers its necessary source,
binding, test, and documentation updates; it does not reopen other decisions.

These records govern their stated scope. Existing detailed requirements in
[INVARIANTS.md](../../INVARIANTS.md), [AGENTS.md](../../AGENTS.md), and the
[Serde Contract Policy](../SERDE_STABILITY.md) remain relevant; accepting these
decisions does not independently ratify every current behavior, default,
test expectation, or implementation detail. Reconcile contradictions with the
specific accepted decision and its authorization instead of silently updating
expectations to match code.

## Reconciliation items

These items identify conflicting guidance or unresolved scope. Listing an item
does not complete its reconciliation or approve a new exception. Where an
accepted ADR defines the intended correction, apply that decision; otherwise
resolve the stated scope question explicitly.

1. **Development compatibility guidance.**
   [INVARIANTS.md](../../INVARIANTS.md), section 7.2, still prescribes a
   pre-1.0 deprecation window with callable legacy APIs. Align that guidance
   with [ADR 0004](project/0004-development-compatibility.md): approved
   development replacements remove obsolete paths. Preserve the separate
   requirement to agree a release compatibility strategy before final release.
2. **Python/WASM parity scope.**
   [INVARIANTS.md](../../INVARIANTS.md), section 7.1, still describes WASM as an
   opt-in subset that need not mirror new Python capabilities. Reconcile it
   with [ADR 0005](standards/0005-python-wasm-parity.md): selected financial
   capabilities have corresponding host interfaces, with explicit justified
   host exceptions and a protected zero-backlog requirement. This does not
   require exposing every internal Rust function.
3. **WASM error instructions.**
   [AGENTS.md](../../AGENTS.md) recommends `JsValue::from_str`, while the
   [boundary checker](../../scripts/check_wasm_boundary.py) prohibits that
   path in API bindings. Correct the instruction to the structured `to_js_err`
   path accepted in [ADR 0006](standards/0006-structured-errors.md).
4. **Cross-target reproducibility claims.**
   The [models README](../../finstack-quant/models/README.md), under
   "Determinism and parallelism", claims native/WASM bit identity.
   The [Monte Carlo run-metadata documentation](../../finstack-quant/models/src/monte_carlo/results.rs)
   also promises host-independent bit reproduction; include it in the same
   reconciliation.
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
   z-spread rule in [ADR 0011](financial/0011-bond-cs01-without-hazard-curve.md).
   The existing branch is neither silently approved as an exception nor
   classified as a defect by this record.

## Implementation and lifecycle follow-up

- **CAGR default:** [ADR 0008](financial/0008-financial-defaults-and-cagr.md)
  explicitly chooses Actual/365 Fixed. The inspected
  [Rust default](../../finstack-quant/analytics/src/risk_metrics/return_based.rs)
  and [regression](../../finstack-quant/analytics/tests/correctness_regressions.rs)
  still use 365.25 and require coordinated changes.
- **Final-release compatibility:** before the first final release, revisit
  [ADR 0004](project/0004-development-compatibility.md) and establish
  compatibility across a couple of releases. The exact support window,
  deprecation process, and migration strategy remain open.
- **Independent enforcement:** the trusted-baseline and independently
  authorized protection required by
  [ADR 0016](project/0016-independent-contract-protection.md) still need
  implementation and verification. These docs do not establish that live
  repository permissions or checks provide it.
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
- **Monetary precision and rounding:** classify and resolve the existing
  `f64` monetary migration targets in [INVARIANTS.md](../../INVARIANTS.md),
  section 1, under
  [ADR 0020](financial/0020-numerical-representation-and-rounding.md). Each
  needs a Decimal monetary path or an explicitly justified model boundary
  with its conversion and rounding policy. Verify monetary conversions and
  contractual rounding stages; the accepted distinction does not certify
  existing paths as compliant.
- **Reproducibility coverage:** inventory and verify the declared tier,
  supported targets, execution settings, and test evidence for each public
  stochastic API under [ADR 0021](standards/0021-reproducibility-guarantees.md).
  Address missing repeatability, thread-count, or cross-host coverage for the
  guarantees actually promised. Reconciliation item 4 tracks the conflicting
  native/WASM bit-identity wording separately.
- **Portfolio execution and invalidation coverage:** audit the workflows and
  adapters covered by [ADR 0022](architecture/0022-portfolio-evaluation-state.md)
  against the shared-executor boundary. Verify request/state cache isolation
  and compare selective repricing with fresh evaluation after changes to
  markets, effective dates, instruments, metric sets, risk policies, and FX
  conversion dependencies. Existing executor structures alone do not prove
  complete caller and regression coverage.
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

This collection records the agreed decisions. It does not change calculations,
fixtures, agent guidance outside these records, or repository permissions.
