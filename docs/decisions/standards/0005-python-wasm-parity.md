# ADR 0005: Preserve Python and WASM financial parity

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Code standards

## Decision

A financial capability selected for host-language exposure has corresponding
Python and WASM interfaces with equivalent calculations, defaults,
validation, and error categories. Host-specific exceptions are explicit and
justified. This does not require exposing every internal Rust function.

The parity backlog remains zero under the agreed host-exclusion rules.
Increasing the permitted backlog or relaxing those rules requires explicit
agreement.

Ordinary host-language differences are permitted when they preserve meaning,
including Python exception types versus structured JavaScript errors.

## Rationale

Choosing a host language should not silently change financial behavior or
remove a selected financial capability. Explicit exclusions retain useful
host-specific conveniences without hiding missing computation interfaces.

## Examples

| Change | Treatment |
|---|---|
| Expose a new Rust risk calculation in Python | Provide its WASM counterpart in the same change |
| Convert results into a pandas DataFrame | Legitimate documented Python-specific capability |
| Represent a plain Python data constructor as a TypeScript object literal | Allowed only when it performs no calculation, validation, or defaulting |
| Apply different default lookback windows in the two hosts | Violates this decision |
| Classify a calculation as a `literal` to avoid its WASM interface | Violates this decision |

## Enforcement

The parity ledger checks surface coverage. Shared behavioral cases must also
verify results, defaults, validation, and error meaning. Protect backlog
ceilings and exclusion rules against unapproved relaxation.

The existing backlog test reads its ceiling from the candidate ledger. It
does not independently prevent a patch from raising both backlog and ceiling.
Trusted-baseline enforcement under ADR 0016 remains necessary.

## Implementation notes and references

- [Parity ledger](../../../finstack-quant-py/parity_contract.toml) records
  host mappings, `literal` exclusions, and zero backlog ceilings.
- [Parity ledger tests](../../../finstack-quant-py/tests/parity/test_parity_ledger.py)
  check surface coverage and the candidate-tree backlog ceiling.
- [ADR 0001](../architecture/0001-rust-owned-financial-behavior.md) assigns semantics to Rust.
- [ADR 0003](0003-canonical-public-names.md) defines name correspondence.
- [ADR 0006](0006-structured-errors.md) defines equivalent failure meaning.
