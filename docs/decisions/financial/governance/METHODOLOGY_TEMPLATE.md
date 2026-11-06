# Methodology decision template

Copy this structure into a record with a stable identifier for one meaningful
choice. Use
[ADR 0026](0026-methodology-decisions-and-inventory.md) for the status definitions
and adoption process. Replace every prompt with evidence or a concise adoption
qualification linked to the follow-up register before listing the record as
ready for agreement.

## Metadata

- **Decision status:** Draft / Accepted / Superseded. Name the accepted scope
  and replacing record where applicable.
- **Decision date:** Date of acceptance, or pending for a draft.
- **Audit date and source revision:** Identify the source inspected and any
  relevant local changes.
- **Implementation status:** Untraced / Observed / Partial / Reconciled, with
  covered entry points and a link to tracked discrepancies.
- **Validation status:** Missing / Evidence located / Evidence reviewed /
  Checks executed, with coverage, result, configuration, revision, and limits.

## Choice and rationale

State the intended decision or the observed implementation awaiting adoption.
Identify existing documented rationale with links. Label proposed rationale
as requiring agreement; code presence is not an adoption rationale.

## Scope and assumptions

Identify calculations, instruments, features, intended use, input domains,
assumptions, and exclusions. Explain why any shared scope has common behavior.

## Algorithm and formulation

Explain the ordered computation stages and key equations needed to identify
the implemented method and material variants. Include projection, interpolation,
integration, exercise, inverse solves and metric construction where applicable.
Trace state changes, event ordering and transformations that affect results.
Link references for full derivations; a product or engine name alone does not
describe its algorithm.

## Selection and unsupported combinations

Trace ordinary caller entry points to effective methods. Record defaults,
explicit overrides, automatic dispatch, configuration and feature gates,
internal mechanisms, and rejected combinations. Distinguish available engines
from engines actually used by the ordinary path.

## Conventions and numerical settings

Document units, time basis, compounding, currencies and curve roles,
calibration inputs and objectives, exercise treatment, discretization,
convergence tolerances, random settings, and other material controls.
Explain which are fixed, defaulted, calibrated, or caller-supplied.

## Alternatives and tradeoffs

Describe meaningful alternatives, whether implemented or only considered.
Explain the accuracy, stability, domain, cost, and usability tradeoffs that
support the intended choice. Do not imply rejected alternatives were evaluated
historically without evidence.

## Limitations and failure behavior

State approximations, incomplete coverage, insufficient-input and convergence
errors, and any permitted fallback with its trigger and effective method.
Separate observed fallback behavior from an agreed permission to use it.

## Implementation and validation evidence

Link the owning dispatch, implementation, canonical reference anchors, and
specific tests or benchmark cases. Identify what each case demonstrates, its
independence and tolerance basis, and what it does not demonstrate. Record
executed checks separately from located evidence.

## Follow-up tracking

Link the owning section or entries in the [algorithm backlog](../FOLLOW_UPS.md)
and, when applicable, [central follow-ups](../../FOLLOW_UPS.md#financial-methodology-audit).
Keep implementation discrepancies, investigation plans, pending choices, missing
validation, progress, and completion criteria in those registers, with one owner
per action and a link back to this record. Do not duplicate task lists here.

Briefly identify any uncertainty that prevents adoption or wider scope. Keep
formulas, observed behavior, material limitations, and evidence qualifications
in the sections above; moving a task does not establish approval or compliance.
