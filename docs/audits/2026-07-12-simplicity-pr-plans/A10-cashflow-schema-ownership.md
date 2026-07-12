# Consolidation Plan A10: Move Cashflow Schema Ownership into cashflows

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F16.
- Risk tier: Tier 4 — generated schema location, resolver ownership, and downstream contract tooling.
- Estimated net change: -250 to +100 LOC.
- Dependencies: A08 and A09.
- Suggested branch: `codex/a10-cashflow-schema-ownership`.
- Parallel and merge safety: safe beside A02, A05-A07, and A12. Conflicts with A08/A09 schema regeneration and A11 generated wire artifacts; merge after A09 and before A11.
- Atomicity: schema-owner move exception. Asset moves, include paths, generator ownership, resolver wiring, and golden path updates must land together; no duplicate checked-in schema directory remains after the commit.

## Exact Files and Filesets

- `finstack-quant/cashflows/Cargo.toml`
- `finstack-quant/cashflows/src/lib.rs`
- New `finstack-quant/cashflows/src/schema.rs`
- New `finstack-quant/cashflows/src/bin/gen_schemas.rs`
- Move the exact fileset `finstack-quant/valuations/schemas/cashflow/1/*.schema.json` to `finstack-quant/cashflows/schemas/cashflow/1/`
- `finstack-quant/valuations/src/schema.rs`
- `finstack-quant/valuations/src/bin/gen_schemas.rs`
- `finstack-quant/valuations/tests/integration/schema/parity.rs`
- `finstack-quant/valuations/tests/schema_audit.rs`
- `finstack-quant/cashflows/tests/cashflows/schema_roundtrip.rs`
- `finstack-quant-py/tests/golden/pricing_validation.py`

## Scope

- Make `finstack-quant-cashflows` generate, embed, and expose its own cashflow schemas.
- Move all seven cashflow schema assets from valuations into the cashflows crate with their stable public schema IDs unchanged.
- Make valuations consume cashflow schema resources through the cashflows crate rather than local `include_str!` paths or copied generator branches.
- Remove cashflow type matching, definition rewriting, and generation branches from the valuations schema generator.
- Update Rust and Python schema tests to resolve the canonical owner path.

## Non-Goals

- No instrument/common/calibration schema move.
- No schema ID or version bump solely because the physical owner changes.
- No hand-editing generated schema content except correcting stale provenance/descriptions through the Rust source and regeneration.
- No envelope deletion; A11 owns it.

## Implementation Steps

1. Add the minimal schema-generation dependencies and a cashflows-owned schema module/binary.
2. Move the cashflow schema assets without changing stable `$id` URLs.
3. Move cashflow-specific definition selection/ref rewriting from the valuations generator to the new owner.
4. Expose parsed schema resources from cashflows and have valuations register those resources through the dependency.
5. Delete valuations' cashflow includes/generator cases and update parity/audit/golden paths.
6. Regenerate from a clean tree and require zero duplicate cashflow schema assets under valuations.

## Tests to Add or Update

- Cashflows schema round-trip test reads assets from the cashflows crate.
- Valuations schema parity resolves external cashflow `$ref` values through the cashflows resource provider.
- Generator check proves a clean regeneration has no diff.
- Python pricing validation locates the canonical owner and validates a representative fixed, floating, and step-up spec.

## Full Verification

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
rtk mise run python-build -- --release
rtk mise run python-lint
rtk mise run python-test
rtk mise run wasm-build
rtk mise run wasm-lint
rtk mise run wasm-test
rtk env UV_CACHE_DIR=/private/tmp/finstack-uv-cache uv run pytest finstack-quant-py/tests/parity -x
rtk mise run gen-check
rtk mise run rust-check-schemas
rtk mise run goldens-test
```

## Binding, Parity, and Serde Impact

- Python/WASM runtime symbols and serde payloads do not change.
- Parity topology does not change.
- Schema physical ownership and generation commands change; public `$id` URLs remain stable.
- Generated TypeScript may be refreshed from the canonical assets but must not gain valuation-owned duplicate definitions.

## Rollback

Revert the entire move, generator extraction, resolver update, and test paths together. Do not leave duplicate asset directories as a rollback shortcut.

## Done Criteria

- No `valuations/schemas/cashflow` directory exists.
- The valuations generator contains no cashflow-spec type table or generation branch.
- Cashflows owns generation, embedding, tests, and public schema resources.
- Stable external schema IDs resolve in valuations and bindings.
- `gen-check` is clean from a clean worktree.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` over schema generators, include paths, assets, and tests. Accept only when cashflow schemas have one physical owner and one generator, valuations is a consumer only, and no copied cashflow definition map or stale owner path remains.
