# Methodology governance

[ADR 0026](0026-methodology-decisions-and-inventory.md) accepts the process for
inventorying, documenting, adopting and maintaining financial methodologies.
Use the [record template](METHODOLOGY_TEMPLATE.md) for a meaningful new choice.

Place product-specific ADRs alongside their owning [catalogue](../README.md#coverage).
Keep cross-domain contracts in [conventions](../conventions/README.md), and
link shared choices from every affected catalogue. Preserve the independent
decision, implementation and validation statuses.

Accepted convention ADRs own the agreed contracts. Draft selection ADRs own
caller selection, scope, proposed rationale and pending adoption choices;
they link to detailed records for formulas, numerical controls, failure branches
and evidence. Keep each substantive definition at its owning record instead
of repeating it in an overview.

The [source map](../SOURCE_COVERAGE.md) and per-scope ledgers describe inspected
algorithm ownership. Governing and overview ADRs are indexed separately;
their relocation or acceptance does not add algorithm records to those ledgers.
The [audit boundary](../AUDIT_BOUNDARY.md) owns the current exclusions and
qualification limits.

Record open work in the [algorithm backlog](../FOLLOW_UPS.md) and coordinate
cross-domain changes in the [central follow-ups](../../FOLLOW_UPS.md#financial-methodology-audit).
