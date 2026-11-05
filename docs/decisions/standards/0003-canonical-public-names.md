# ADR 0003: Rust defines canonical public names

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Code standards

## Decision

Public names originate in Rust. Python uses the same names; WASM uses the
corresponding camelCase callable names. Canonical type names retain their
spelling. Documented host-language collisions may have explicit exceptions.

The common operation gets the short name. Specialized variants use
descriptive suffixes such as `_from_<input>` or `_with_<thing>`. Accessors use
`get_*` in Rust and Python, with corresponding `get*` names in WASM.

This does not require exposing every internal Rust function. Existing public
names are protected contracts: a cleanup request does not authorize a rename,
even when every language is updated consistently.

## Rationale

A single naming scheme makes capabilities recognizable across languages and
avoids binding-specific aliases becoming competing APIs. Explicitly approved
renames can improve the canonical API without leaving inconsistent consumers.

## Examples

| Example | Treatment |
|---|---|
| `period_stats` in Rust/Python and `periodStats` in WASM | Canonical correspondence |
| `get_discount` in Rust/Python and `getDiscount` in WASM | Accessor convention |
| A specialized Rust-only `period_stats_from_grouped` function | Allowed descriptive variant |
| Rename only Python's `period_stats` to `summarize_returns` | Violates this decision |
| Propose a coordinated rename of an awkward existing public name | Requires explicit agreement, then updates to all affected surfaces |

## Enforcement

Required enforcement checks Rust-to-host naming correspondence and documented
exceptions. Contract-change reporting must expose established public-name
changes even when Python and WASM remain mutually consistent.

Existing naming and parity tests provide coverage, but are not proof of
complete Rust-to-host coverage for every namespace. Extend them where gaps
remain; do not treat passing host-to-host parity alone as sufficient.

## Implementation notes and references

- [Naming strategy](../../../AGENTS.md) defines the canonical naming rules.
- [Rust-name parity test](../../../finstack-quant-py/tests/parity/test_rust_names.py)
  checks the Python `models` surface against Rust sources.
- [Parity ledger](../../../finstack-quant-py/parity_contract.toml) records
  correspondence and exceptions.
- [ADR 0004](../project/0004-development-compatibility.md) governs removal of
  superseded names during development and the pre-release policy review.
