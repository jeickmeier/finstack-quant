# Project decisions

These architecture decision records (ADRs) capture agreed decisions and
explicitly labeled drafts. Existing numbers preserve the original decision
sequence; new records receive an identifier when drafted. Directories group
records by subject. An identifier does not imply acceptance.

**Accepted** means the decision is agreed policy. It does not mean all
implementations comply or that the proposed protection checks are deployed.
Known gaps and conflicting guidance are tracked in
[Follow-ups and reconciliations](FOLLOW_UPS.md).
Each record separates the decision, examples, enforcement requirements, and
supporting evidence.

The [invariant-to-decision map](INVARIANT_MAP.md) links every normative clause
in the workspace catalogue to an accepted decision. Review of invariant-detail
decisions 0042–0049 is complete; implementation, reconciliation and verification
remain open in the follow-ups. This does not accept the separate financial
methodology drafts or establish implementation compliance.

Methodology records also state **implementation status** and **validation
status** separately from **decision status**. Draft records describe observed
or proposed choices awaiting agreement; they do not expand accepted policy.

When accepting another ADR, add it to the appropriate table and carry its
known follow-up work or conflicting guidance into
[FOLLOW_UPS.md](FOLLOW_UPS.md). Keep
implementation gaps distinct from questions requiring a further decision;
accepting an ADR does not mark either kind of work complete.

## Record and follow-up boundary

ADRs own the decision, scope, rationale, alternatives, lasting constraints,
consequences, and required verification criteria. Methodology records also
retain formulas, observed behavior, evidence limits, and independent decision,
implementation, and validation statuses. Preserve limitations that affect how
the decision or calculation can be interpreted.

Mutable work belongs in a follow-up register: implementation gaps, fixes,
investigations, guidance reconciliation, pending choices, missing evidence,
owners, progress, and completion criteria. Keep each action in one authoritative
entry, with a link back to its record. Records link to that entry or its scoped
section instead of repeating the action list. A draft may briefly identify an
adoption-blocking uncertainty without duplicating its investigation plan.

[Central follow-ups](FOLLOW_UPS.md) own project-wide and cross-domain work;
[financial follow-ups](financial/FOLLOW_UPS.md) own detailed algorithm work.
Use links between registers for shared work. Completing an implementation task
updates the register and applicable evidence/status; changing an accepted
decision still requires explicit agreement.

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
| [0048](project/0048-reference-provenance-and-version-pinning.md) | Make reference provenance attributable per case, pin effective methodology/parameters, and support measured accuracy with recorded execution evidence and distinct evidence statuses. |

## Code standards

Cross-cutting rules for standards authority, implementation and reuse, public
API design and documentation, safe Rust, naming, host interfaces, errors,
serialized contracts, type invariants, testing, performance, and reproducibility
live in `standards/`.

| ADR | Decision |
| --- | --- |
| [0003](standards/0003-canonical-public-names.md) | Use canonical Rust names across host languages. |
| [0005](standards/0005-python-wasm-parity.md) | Preserve Python/WASM capabilities and behavioral parity with explicit exceptions. |
| [0006](standards/0006-structured-errors.md) | Preserve structured error meaning across interfaces. |
| [0012](standards/0012-serialized-contracts.md) | Use an explicit Rust-owned serialized-data contract. |
| [0021](standards/0021-reproducibility-guarantees.md) | Preserve explicitly scoped, tested reproducibility guarantees and cross-host numerical agreement. |
| [0034](standards/0034-code-standards-authority.md) | Define coding policy in accepted decisions and implement it through shared configuration and canonical checks. |
| [0035](standards/0035-deliberate-public-api.md) | Keep a focused public surface with canonical paths, deliberate visibility, and consistent construction patterns. |
| [0036](standards/0036-safe-rust-and-recoverable-failures.md) | Forbid unsafe project library and binding code; return meaningful errors for recoverable input and computation failures. |
| [0037](standards/0037-simple-modular-implementation-and-reuse.md) | Choose simple, modular implementations, reuse established capabilities, and deliver working increments toward the intended design. |
| [0038](standards/0038-substantive-api-documentation.md) | Document public API contracts substantively, maintain executable examples, and verify semantic accuracy across hosts. |
| [0039](standards/0039-behavior-and-invariant-tests.md) | Protect observable behavior and invariants with meaningful, deterministic tests and defect-detecting regressions. |
| [0040](standards/0040-measured-performance-decisions.md) | Justify performance-driven complexity with representative measurements, preserve contracts, and scope regression budgets explicitly. |
| [0041](standards/0041-type-invariants-and-validation-boundaries.md) | Preserve declared type invariants through construction and mutation, with explicit validation boundaries for raw inputs and contextual conditions. |
| [0042](standards/0042-numerical-accumulation-and-comparisons.md) | Use compensated floating-point totals and appropriate statistical primitives, validate accumulation inputs, and distinguish tolerance, exact numerical and bitwise comparisons. |
| [0043](standards/0043-deterministic-ordering-and-monte-carlo-execution.md) | Preserve deterministic ordering and tier-scoped Monte Carlo/reduction execution, with fixed declared configuration and recorded baseline conditions. |
| [0049](standards/0049-explicit-public-error-matching.md) | Explicitly classify error variants where meaning is assigned, allow canonical delegation, and define cross-crate non-exhaustive and third-party unknown-error boundaries. |

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
| [0046](architecture/0046-portfolio-execution-operational-contract.md) | Define portfolio execution, complete selective-reuse compatibility, metric applicability, state-local caches, conditional FX refresh, and native adapter boundaries. |
| [0047](architecture/0047-credit-factor-wire-and-validation-contract.md) | Require the typed credit-factor schema marker, reject unsupported wire formats during decoding, and preserve closed roots, explicit diagnostic extensions and current-state semantic validation. |

## Financial decisions

| ADR | Decision |
| --- | --- |
| [0007](financial/conventions/0007-units-and-sign-conventions.md) | Treat financial units and sign conventions as public contracts. |
| [0008](financial/conventions/0008-financial-defaults-and-cagr.md) | Set explicit financial defaults; dated-window CAGR defaults to Actual/365 Fixed. |
| [0009](financial/conventions/0009-missing-and-invalid-data.md) | Distinguish invalid inputs, permitted gaps, and undefined results. |
| [0010](financial/valuations-framework/0010-metric-identifiers.md) | Preserve metric identity, meaning, and attribution. |
| [0011](financial/valuations-bonds/0011-bond-cs01-without-hazard-curve.md) | Calculate supported no-hazard bond CS01 through z-spread bump-and-reprice. |
| [0020](financial/conventions/0020-numerical-representation-and-rounding.md) | Separate model numerics from Decimal monetary paths; preserve construction precision, reject non-finite monetary inputs and round explicitly. |
| [0044](financial/conventions/0044-financial-perspectives-and-boundary-conversions.md) | Name financial perspectives and convert explicitly; retain the dividend magnitude default with an explicit signed alternative, and distinguish total collateral from its components. |
| [0045](financial/conventions/0045-financial-clocks-and-date-boundaries.md) | Preserve separately owned clocks, explicit volatility date-to-expiry mapping, contractual forward annualization, bounded discount-factor bridges, and explicit fixing cutoffs. |

## Financial methodologies

[Financial methodologies](financial/README.md) contains the calculation
inventory and adoption records. A reference explains a method; a decision
records why we use it, where it applies, and what callers can rely on.

| Record | Decision status | Scope |
| --- | --- | --- |
| [0026](financial/governance/0026-methodology-decisions-and-inventory.md) | Accepted | Maintain a methodology inventory and scoped decisions; audit existing implementations without automatically approving them. |
| [Inventory](financial/README.md#coverage) | Evidence map | Algorithm-level catalogues for the audited financial/numerical crates, with effective selection, implementation and validation links, and open issues. |
| [Source coverage](financial/SOURCE_COVERAGE.md) | Inspected source map | Every Rust source in the declared audit scope is classified and mapped to methodology records; numerical qualification remains separate. |
| [Draft records](financial/README.md#methodology-records) | Draft unless explicitly stated | Audited choices with rationale, scope, conventions, alternatives, and gaps requiring agreement or reconciliation. |

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

This collection distinguishes agreed decisions from drafts. It does not change calculations,
fixtures, agent guidance outside these records, or repository permissions.
