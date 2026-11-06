# ADR 0026: Financial methodology decisions and inventory

**Decision status:** Accepted

**Decision date:** 2026-11-05

**Implementation status:** Documentation structure and source catalogues
established for the declared financial/numerical crates; excluded domains and
implementation reconciliation remain open.

**Validation status:** Evidence mapped for the declared audit scope; this
policy does not certify any calculation or validation suite.

**Category:** Financial methodology governance

## Decision

A reference explains a method. A methodology decision records why we use it,
where it applies, and what behavior callers can rely on.

Maintain [Financial methodologies](../README.md) under `docs/decisions/financial/`
with two complementary pieces:

- A [methodology inventory](../README.md#coverage) mapping calculations and supported
  instruments to implemented models, material variants, selection behavior,
  decision records, owning code, validation evidence, and open issues.
- One decision record per meaningful methodology choice. Share a record
  across instruments only where assumptions and caller-visible behavior are
  genuinely common. A common engine name alone is insufficient.

Existing [references](../../../REFERENCES.md) remain the technical source.
Records explain adoption and scope, linking rather than duplicating the
technical derivation. Inventory entries may point to a draft or identify a
missing record; neither implies approval.

## Required content

Use the [methodology record template](METHODOLOGY_TEMPLATE.md). Each record
answers:

1. **Choice and rationale:** what was chosen, why, meaningful alternatives,
   and tradeoffs. Distinguish documented rationale from proposed rationale
   that still needs agreement; do not invent historical reasons from code.
2. **Scope:** instruments, features, assumptions, input domain, and intended
   use, including exclusions.
3. **Algorithm:** ordered computation stages, key equations and material
   branches sufficient to identify the implemented method. Include state/event
   ordering, projection, integration, exercise, inverse solves and risk
   construction where applicable; link full derivations rather than copying
   them. A family-level capability list alone is not an algorithm audit.
4. **Selection:** actual caller entry points, defaults, explicit overrides,
   automatic dispatch, feature gates, and unsupported combinations. Trace
   runtime selection; an enum variant or registered engine is not proof that
   the ordinary valuation path uses it.
5. **Conventions:** units, time basis, calibration inputs, exercise treatment,
   and numerical settings that materially affect results.
6. **Limitations:** approximations, failure behavior, and any permitted
   fallback with its trigger and resulting method. An observed fallback is
   not automatically a permitted fallback.
7. **Evidence:** existing references, owning implementation, relevant
   validation cases, provenance and tolerances, and limits of that evidence.
8. **Follow-up tracking:** links to the owning register entries for pending
   choices, implementation discrepancies, investigations, and missing evidence.
   Keep mutable action lists in the register; retain material limitations and
   concise adoption qualifications in the record.

## Independent statuses

| Dimension | Meaning |
| --- | --- |
| Decision | **Draft** records an observed or proposed choice awaiting agreement. **Accepted** records an agreed choice within its stated scope. **Superseded** identifies the replacing decision and scope. |
| Implementation | State whether the relevant path is untraced, observed, partial, or reconciled with the accepted decision. Name the covered entry points and link tracked gaps. |
| Validation | Distinguish missing evidence, evidence located, evidence reviewed, and checks executed. Record the revision, configuration, coverage, outcome, and unresolved limitations when claiming execution or qualification. |

An accepted model may have incomplete implementation or validation. Located
tests do not establish a passing run, independent financial correctness, or
complete coverage. A passing test does not accept a methodology. Apply
[ADR 0014](../../project/0014-financial-reference-evidence.md) to independent
expectations and numerical tolerances.

## Audit and adoption

1. Inventory calculations and trace their actual defaults and dispatch paths.
   Identify public alternatives separately from internal mechanisms.
2. Match implementation to existing references and validation evidence.
   Record input conventions and evidence limitations.
3. Draft records separating observed behavior, documented rationale, and
   proposed reasoning or choices requiring agreement.
4. Accept the intended methodology choices explicitly. Carry discrepancies,
   missing evidence, and unresolved choices into
   the [algorithm backlog](../FOLLOW_UPS.md), with cross-domain work owned by
   [central follow-ups](../../FOLLOW_UPS.md#financial-methodology-audit).
   Keep one authoritative entry per action and link it from the record.
   Acceptance of this documentation policy
   does not accept all inventoried implementations or any draft model record.
5. Reconcile implementation and validation against each accepted choice,
   updating their statuses independently.

The initial audit is dated and scoped in the inventory. Untraced families and
missing records remain visible there and in the follow-ups; do not describe
the inventory as exhaustive until coverage supports that claim.

## Maintenance and enforcement

Material methodology changes update the relevant decision and inventory in
the same change as code, tests, and references. This includes model selection,
defaults, calibration, exercise handling, input conventions, numerical
settings affecting results, approximations, and failure or fallback behavior.
Explain whether the change implements an accepted choice or proposes a new
one. Preserve explicit authorization for changes to accepted contracts under
[ADR 0015](../../project/0015-authorized-change-scope.md).

The [contribution workflow](../../../../CONTRIBUTING.md#documentation) requires
these updates during review. No automatic check is claimed to establish
methodology approval, evidence independence, or complete semantic coverage.

## Rationale and alternatives

A bibliography alone cannot tell a caller which model is used, why it was
adopted, or which limitations matter. Putting all of that in source comments
fragments the decision across engines and instrument families. Conversely,
one record for every function duplicates common assumptions and obscures
the substantive choices. A calculation inventory plus scoped records provides
both a navigable implementation map and an explicit adoption history.
