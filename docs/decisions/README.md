# Accepted project decisions

These architecture decision records (ADRs) capture decisions. Their numbers preserve the order in which they were agreed; directories group them by subject.

**Accepted** means the decision is agreed policy. It does not mean all
implementations comply or that the proposed protection checks are deployed.
Known gaps and conflicting guidance are tracked in
[Follow-ups and reconciliations](FOLLOW_UPS.md).
Each record separates the decision, examples, enforcement requirements, and
known implementation notes.

When accepting another ADR, add it to the appropriate table and carry its
known follow-up work or conflicting guidance into
[FOLLOW_UPS.md](FOLLOW_UPS.md). Keep
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

This collection records the agreed decisions. It does not change calculations,
fixtures, agent guidance outside these records, or repository permissions.
