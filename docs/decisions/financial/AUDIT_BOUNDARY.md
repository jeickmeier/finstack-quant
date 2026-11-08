# Methodology audit boundaries and qualification

The [algorithm catalogue](README.md) covers core, cashflows,
models, valuations, calibration, analytics, statements, statements-analytics,
scenarios, portfolio, margin, factor, attribution, covenants, features and
Arrow interchange. The former deferred domains now have detailed records,
source inventories and follow-ups.

## Source boundary

The coverage ledger enumerates all Rust files under the named crates' `src/`
trees and their Rust build generators. It includes every production valuations
family, the valuations umbrella and its builder macro crate, including runtime
items after interleaved test declarations. Embedded data and defaults that
materially select a method are inspected and linked in the owning record.
The SHA-256 ledger covers Rust source; it does not certify external inputs,
registry provenance or future data freshness.

The [root package entrypoints](../../../finstack-quant/src), Python/WASM
bindings, UI adapters and downstream host workflows are separate surfaces.
Selected caller traces may cite these surfaces, but they are outside exhaustive
source enumeration and independent host qualification. Their existence or
delegation to an audited kernel does not establish complete API, serialization
or behavioral parity. Dev-only `valuations/test-support` and `test-utils`, fuzz
targets, examples, benchmarks and external tests are supporting evidence rather
than additional production algorithm owners in this ledger.

## Evidence and adoption boundary

Detailed records distinguish decision, implementation and validation status.
Source inspection documents the implemented algorithm and its controls; it
does not accept the methodology or certify its numerical accuracy. Existing
accepted policies and prior agreed directions retain their stated scope.
Detailed algorithm records retain their individually stated decision status.
The [15 analytics records](analytics/README.md#accepted-scopes-in-detailed-records)
and [ADR 0033](analytics/0033-value-at-risk-method-selection.md) are accepted,
with [implementation evidence](analytics/README.md#implementation-evidence)
recorded separately from the original audit. The governing and convention
ADRs retain their separately
stated statuses; describing an observed method does not approve it.

Selected tests and references were inspected; this does not claim review of
every test body. Numerical suites, simulation experiments and vendor comparisons
were not executed for these documentation audits. Independent semantic reviews
sample records; they do not mathematically certify every implementation or
qualify a release. The first batch's core reference-data consistency check compares
maintained data with generated tables; it does not verify official holidays.
Accounting/regulatory labels and embedded versions do not establish current
compliance. Independent provenance, convergence, domain coverage and justified
tolerances remain specific follow-ups in the
[review register](FOLLOW_UPS.md).

Maintain these boundaries under
[ADR 0026](governance/0026-methodology-decisions-and-inventory.md). Trace actual callers,
update the record and source mapping after substantive changes, and qualify
consumers separately where their units, defaults, state or failure behavior
differ from the shared kernel.
