# ADR 0048: Reference provenance, evidence status, and version pinning

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Project policy

## Decision and scope

[ADR 0014](0014-financial-reference-evidence.md) already requires independently
justified reference results and tolerances.
[ADR 0026](../financial/governance/0026-methodology-decisions-and-inventory.md) governs
methodology documentation and adoption. This decision adopts the detailed
provenance, citation, and catalogue-wide evidence rules below. It does not
accept a financial model or methodology version.

Acceptance does not certify enforcement or weaken the accepted protection of
reference expectations. The [invariant map](../INVARIANT_MAP.md) records the
clause-level correspondence.

## Requirements

1. Golden values record applicable **provenance, methodology and version,
   valuation date, conventions, inputs, tolerances and their rationale, and
   expected units**. Fixture metadata, a test header, or the golden-suite
   README may supply shared evidence, provided it is attributable to each case
   and case-specific overrides are explicit. State a reason when a field is
   genuinely inapplicable; do not invent a valuation date for a calculation
   that has none. This does not remove the need to identify the method,
   inputs, units, assumptions, and justified comparison used by that case.
2. Rust and Python golden suites may share fixtures. A fixture need not live
   directly under a Rust `<crate>/tests/` directory. Its location does not
   establish the independence or validity of its expected result. Multiple host
   runs against one fixture do not create independent reference calculations.
3. Model-specific modules and golden fixtures cite the exact **edition,
   calibration, convention, and benchmark** used. SIMM code and goldens pin
   their implemented methodology version and calibration tag rather than
   referring to “current”. Identify the actual methodology and effective
   calibration or parameter set, including overrides. Use the published
   calibration tag where available; otherwise identify the exact edition and
   an immutable source or parameter-set identity. A methodology version alone
   does not identify an overridden calibration. This is an evidence requirement,
   not a requirement to add a particular runtime API or metadata structure.
   Schema-format versions are distinct from methodology and calibration
   identities. A citation to an available edition does not adopt that edition
   or authorize an update to a different one.
4. Do not publish numerical error bands, including basis-point drift, unless
   a reproducible benchmark or golden test defines the setup and provenance.
   Published measured accuracy requires recorded execution evidence: source
   revision, command, inputs, comparator, error units, and relevant build,
   runtime, and execution settings, including the seed where applicable.
   Apply [ADR 0043](../standards/0043-deterministic-ordering-and-monte-carlo-execution.md)
   to those conditions and ADR 0026 to execution status. Distinguish measured
   error, statistical sampling uncertainty, and assertion tolerances. A result
   for a named setup does not establish a bound over a broader domain.
5. Catalogue evidence claims name their backing type, source, lint, schema or
   test and the conditions it covers. Keep the requirement's policy status
   separate from implementation and validation evidence. Source inspection
   or the existence of a test does not establish a passing run or complete
   enforcement. State partial or unverified coverage explicitly, and do not
   present incomplete enforcement as an achieved guarantee.

These clauses supplement the independence, tolerance rationale, and
authorization requirements of ADR 0014. Metadata completeness alone is not
financial correctness evidence, and a reference citation alone is not the
methodology adoption required by ADR 0026.

Clause 5 establishes the catalogue-wide evidence distinction under the
accepted documentation policies in
[ADR 0013](0013-documentation-sources.md) and
[ADR 0038](../standards/0038-substantive-api-documentation.md). It does not
require ADR 0026's methodology-specific status headings on unrelated records.

## Rationale and alternatives

Versioned sources and reproducible setups let a reviewer distinguish a
calculation difference from different assumptions or calibrations. Keeping
the provenance near its case supports review without requiring one physical
fixture layout across host languages.

A single central fixture directory would simplify location rules but require
unnecessary moves and would not establish independent expectations. A bare
bibliography or an unversioned “current” label is insufficient to identify
which calculation produced a golden value. The decision retains flexible
storage while requiring specific evidence.

## Verification criteria and coverage

| Catalogue clauses | Decision coverage |
| --- | --- |
| [Section 6: golden metadata and shared fixtures](../../../INVARIANTS.md#6-test-discipline) | Clauses 1–2; ADR 0014 continues to govern independent expectations and tolerances. |
| [Section 10: exact citations and SIMM tags](../../../INVARIANTS.md#10-references) | Clause 3; ADR 0026 continues to govern methodology adoption. |
| [Section 4.3: published numerical error bands](../../../INVARIANTS.md#43-date-boundaries) | Clause 4; claimed coverage must match the reproducible setup. |
| [Preamble](../../../INVARIANTS.md) and [section 9: evidence and scope](../../../INVARIANTS.md#9-authority-and-enforcement) | Clause 5; evidence sources, covered conditions and incomplete verification remain distinct from the requirement. |

Enforcement evidence must trace golden-case metadata, sources and tolerance
rationale, and identify implemented editions and effective calibrations,
including overrides. Verified error-band claims require reproduced setups
under the recorded execution conditions. Catalogue evidence statements must
identify their support, covered conditions and verification limits; a source
link alone does not establish coverage.

Acceptance does not establish complete enforcement. Outstanding work is
tracked in the [reference provenance](../FOLLOW_UPS.md#reference-provenance),
[methodology pins](../FOLLOW_UPS.md#methodology-pins), and
[numerical-accuracy evidence](../FOLLOW_UPS.md#numerical-accuracy-evidence)
follow-ups. This decision does not approve the methodology or calibration
versions cited in the catalogue.
