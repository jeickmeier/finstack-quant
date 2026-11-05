# Adding an instrument

The one checklist for a new instrument type. Every other README that mentions
the task links here. Paths are relative to `finstack-quant/valuations/`;
`<family>` is one of `rates`, `fixed-income`, `credit`, `equity`, `fx`,
`exotics`, `structured-credit`.

Each step names the check that fails when it is skipped.

## 1. The type

Add `<family>/src/<name>/`, following a neighbour (for example
`fixed-income/src/bond/` or `rates/src/irs/`). The struct needs
`id: InstrumentId`, `attributes: Attributes`, `Clone`, and an `example()`
constructor; a long-dated example rolls forward from
`common_impl::example_constants::FAR_EXPIRY`. Reuse the `common_impl/parameters/`
leg and underlying types and the `instruments::pricing` kernels instead of
writing parallel ones.

## 2. The `InstrumentType` variant

In `core/src/pricer/keys.rs`, add the variant with the next free discriminant
(never renumber), and its arm in `InstrumentType::as_str`. Add
`#[serde(rename = ...)]` only when the wire tag must differ from the snake_case
variant name, and then `as_str` returns that same string.

## 3. `impl Instrument`

```rust
impl Instrument for Foo {
    impl_instrument_base!(InstrumentType::Foo, crate::RATES);
    // ...
}
```

`impl_instrument_base!` takes the instrument type and the family static and
supplies `id`, `key`, `attributes` and `family`. Listed future options use
`impl_future_option_instrument!` or `define_future_option!`
(`core/src/instruments/common_impl/listed/future_option/instrument_impl.rs`),
which generate the whole `impl Instrument`; that is why
`grep 'impl Instrument for'` does not find them.

- An instrument that accepts pricing, metric or scenario overrides stores the
  three override bags and adds `impl_focused_pricing_overrides!()`.
- An instrument whose native model is not `ModelKey::Discounting` overrides
  `Instrument::default_model`, so `price_with_metrics` and the JSON `"default"`
  model selector reach its pricer.

## 4. Cashflows

Implement `finstack_quant_cashflows::CashflowScheduleSource`; `CashflowProvider`
is blanket-implemented from it. A product whose waterfall is intentionally
empty uses `impl_empty_cashflow_provider!(Foo, representation)`.

## 5. Pricer and metrics registration

In `<family>/src/family.rs`:

- `register_pricers`: one line for an instrument that discounts its own
  cashflows,

  ```rust
  register_generic!(registry, InstrumentType::Foo, crate::foo::Foo);
  ```

  the four-argument form `register_generic!(registry, InstrumentType::Foo, crate::foo::Foo, ModelKey::Bar)`
  for another model, or a hand-written `Pricer` registered with
  `registry.register(...)`. A hand-written pricer downcasts with
  `expect_inst::<Foo>(instrument, InstrumentType::Foo)?` and returns the
  unshocked base PV. Add a `ModelKey` only when no existing one describes the
  model.
- `register_metrics`: call `register_foo_metrics(registry)`, defined in
  `<family>/src/foo/metrics.rs`.

## 6. The JSON row

In `src/instruments/json_loader.rs`, add the `use` for the type and one row to
`with_instrument_json_registry!`: kind (`plain`, or `boxed` for a large
payload), `InstrumentJson` variant, type, wire tag (equal to
`InstrumentType::as_str`), schema category, example, and a distinct
one-sentence schema-index summary.

The row embeds `schemas/instruments/1/<category>/<tag>.schema.json` with
`include_str!`, so the crate does not compile until that file exists. Create it
containing `{}`; step 9 replaces it with the generated schema.

## 7. Persistence manifest

Add an `[[instrument]]` entry to `tests/instruments/coverage_manifest.toml`:
`tag`, `required_keys`, `no_market_dependencies` and a `rationale`. Its fixture
is `json_examples/<tag>.json`, which step 9 writes. When the type does not
implement `PartialEq`, name the field that blocks it in `partial_eq_blocker`
(`Type.field: NestedType`); the rationale must quote it. When it does, leave
`partial_eq_blocker` out: `InstrumentJson::semantic_eq` then compares the
round-tripped values automatically.

`registry_coverage.rs` checks the manifest, the registry and the fixture
directory against each other and enforces this grammar.

## 8. Tests

Add `tests/instruments/<name>/` covering construction, cashflows, pricing,
metrics and validation (see `tests/instruments/README.md`), with a
`#[path = ...] mod` entry in `tests/instruments.rs` when the directory is new.
Every expected number needs provenance (`tests/instruments/README.md`,
"Expected-value provenance").

## 9. Regenerate

```bash
mise run gen-write
```

This writes the instrument's schema and the schema index, the canonical
fixture `tests/instruments/json_examples/<tag>.json`, the WASM contract types,
the UI's generated files, the pinned registry snapshot
(`tests/data/registry_snapshot.json`), the WASM facade surface, and the
generated-artifact manifest. Review the diff: the snapshot lists the new
instrument's models and metrics.

## 10. Bindings (optional)

An instrument is priced from Python and WASM through its JSON wire form with no
binding change. A typed wrapper (a Python class or a WASM handle) is optional;
when you add one, update `finstack-quant-py/parity_contract.toml` and the WASM
`exports/*.js` in the same change, and run `mise run gen-write` again for the
WASM declarations (`finstack-quant-wasm/index.d.ts`).

## 11. The gate

```bash
mise run all
```

It runs exactly what CI runs on a pull request.
