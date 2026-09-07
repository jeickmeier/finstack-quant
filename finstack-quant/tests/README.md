# finstack-quant integration tests

Integration tests for the umbrella crate. The umbrella re-exports the thirteen
domain crates and owns exactly one piece of behaviour of its own — the
workspace-wide JSON Schema registry in [`../src/schema.rs`](../src/schema.rs) —
so this directory holds the tests that need every crate's schema registry
visible at once.

## Layout

| File | Covers |
|------|--------|
| `schema_projection.rs` | Whole-corpus properties of the LLM schema projection |

Per-pass projection behaviour (single-document rewriting, budget arithmetic,
handle substitution) is unit-tested inside `finstack_quant_core::schema`. These
tests assert only the properties that emerge once every domain registry is
merged. Nine registry domains publish schema artifacts — `core`,
`attribution`, `cashflows`, `factor_model` (owned by `models`), `margin`, `portfolio`, `scenarios`,
`statements` and `valuations` — and `domain_registries()` in `../src/schema.rs`
is the list.

## What `schema_projection.rs` asserts

The corpus is assembled from `finstack_quant::schema::documents_by_id()` and
`finstack_quant::schema::artifacts()`, then run through
`finstack_quant_core::schema::project_llm` with a default `LlmProfile`.

- **Self-containment.** Published artifacts reference each other by absolute
  `$id` on a host that does not resolve. Projection must leave zero non-fragment
  `$ref`s across the whole corpus.
- **Determinism.** Projecting the same artifact twice yields identical JSON.
- **Union collapse.** The `Currency` union of 159 `const` branches, which is
  inlined wherever `Money` appears, must become a flat `enum` more than 4x
  smaller while keeping every ISO code.
- **Handle substitution.** An oversized reference (the instrument union reached
  from `portfolio_materialization.schema.json`) must be replaced by a
  `RESOLVES_FROM_KEYWORD` handle rather than inlined.
- **Not a validator.** A payload the canonical artifact accepts must be
  *rejected* by its projection. This is deliberate: the projection is both
  stricter and looser than the runtime contract, and a caller that reached for
  it as a schema would silently mis-validate.
- **Still JSON Schema.** Every projected artifact must compile under
  `jsonschema::validator_for`. An array-form `items` once broke this.
- **Inline budget boundary.** `DEFAULT_MAX_INLINE_BYTES` is a boundary in the
  corpus, not a tuning knob: `money`, `currency`, `day_count`, `date`,
  `decimal` and `tenor` must sit below it; `instrument_pricing_overrides` and
  `metric_pricing_overrides` must sit above it.
- **Both sides of that boundary on one artifact.** A projected `fx_forward`
  must keep `Currency` inline as a flat `enum` (a payload author needs the codes
  in front of them) while `InstrumentPricingOverrides` survives only as a
  `RESOLVES_FROM_KEYWORD` handle with no `properties`, and the whole document
  must stay under 24 KB.
- **Examples.** Every published artifact must carry at least one `examples`
  entry, and every example must validate against its own artifact.

### The pinned artifact count

`every_published_artifact_carries_a_valid_example` asserts the published
registry is non-empty and that every artifact ships at least one example that
validates against its own schema. Adding a schema artifact to any domain crate
will fail this test until that artifact includes a valid example.

## Running

`mise run rust-test` covers this directory as part of the workspace nextest run
(`--lib --test '*'`). To run only these tests:

```bash
cargo nextest run -p finstack-quant
```

Do not invoke `cargo test` directly for the dev loop; it also runs doc tests,
which the workspace gates separately through `mise run rust-doc`. The doc tests
on `finstack_quant::schema` are exercised there.

Schema artifacts themselves are generated from Rust types and checked for drift
by `mise run rust-check-schemas` (regenerate with `mise run rust-gen-schemas`);
`mise run gen-check` wraps both plus an idempotency digest.

## See also

- [`../src/schema.rs`](../src/schema.rs) — the registry, the `$ref` resolver,
  and the union-drilling validator these tests exercise
- [`../../docs/CONTRACTS.md`](../../docs/CONTRACTS.md) — what the published
  schema contracts guarantee
- [`../../INVARIANTS.md`](../../INVARIANTS.md) — determinism, currency-safety
  and serde-stability invariants
- [`../../.agents/rules/rust/testing-standards.md`](../../.agents/rules/rust/testing-standards.md)
  — workspace testing conventions
