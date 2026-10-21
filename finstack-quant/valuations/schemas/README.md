# Valuations JSON Schemas

This directory contains JSON Schema Draft 2020-12 definitions owned by the
valuations crate: instruments, valuation results, and shared definitions needed
by those schemas. The Rust serde types and strict loaders are authoritative.

Cashflow schemas are owned under [`../../cashflows/schemas/`](../../cashflows/schemas/);
calibration and market-quote schemas under
[`../../calibration/schemas/`](../../calibration/schemas/);
portfolio materialization schemas under [`../../portfolio/schemas/`](../../portfolio/schemas/);
credit factor-model schemas under
[`../../models/schemas/factor_model/`](../../models/schemas/factor_model/).
Valuations schemas may reference those separately generated artifacts by `$id`,
but must not regenerate them.

Wire-format stability rules for these documents are in
[`docs/SERDE_STABILITY.md`](../../../docs/SERDE_STABILITY.md); the strict-loader
contract for each envelope is in [`docs/CONTRACTS.md`](../../../docs/CONTRACTS.md).

## Regenerating Schemas

```bash
# Regenerate every registry-owned Rust schema
mise run rust-gen-schemas

# Regenerate only valuations-owned schemas, the schema index, and the
# canonical instrument fixtures (all three come from one binary)
cargo run -p finstack-quant-valuations --bin gen_schemas -- --write

# Run the other owning generators directly when needed
cargo run -p finstack-quant-cashflows --bin gen_cashflow_schemas -- --write
cargo run -p finstack-quant-portfolio --bin gen_portfolio_schemas -- --write

# Validate schema parity and the checked-in schema audit
mise run rust-check-schemas

# Regenerate all maintained schema/binding artifacts and check path/content drift
mise run gen-check
```

The `gen_schemas` binary ([`src/bin/gen_schemas.rs`](../src/bin/gen_schemas.rs))
accepts `--write`, `--check`, `--list`, and `--output-root <path>`. It owns three
outputs and reconciles all of them in one pass, deleting artifacts that no longer
have a source type:

| Output | Contents |
|--------|----------|
| `schemas/<root>/1/*.schema.json` | One artifact per registered serde type, across the five roots below |
| `schemas/index.json` | Machine-readable catalog of every valuations schema artifact |
| `../tests/instruments/json_examples/*.json` | One canonical `finstack_quant.instrument/1` fixture per registry tag |

Every artifact is generated directly from its runtime serde type and registry
metadata. Examples come from deterministic Rust providers. Never hand-edit a
checked-in schema; update the owning Rust type, registry metadata, or example
provider and regenerate. JSON Schema validation is supplementary: strict Rust
loaders still own resource limits and semantic checks.

## Directory structure

```
schemas/
  index.json               # Catalog of every artifact below ($id, path, kind, title, summary, bytes)
  common/1/                # Shared component schemas referenced by generated artifacts
    attributes / currency / date / day_count / decimal / id / money / tenor
    business_day_convention / diagnostic / validation_report
    instrument_pricing_overrides / metric_pricing_overrides / scenario_pricing_overrides
  instruments/1/           # Financial instrument definitions (v1)
    instrument.schema.json # Standalone envelope: inlined 70-branch oneOf over every type
    fixed_income/          # Bonds, loans, structured credit, MBS
    rates/                 # Swaps, swaptions, caps/floors, futures
    credit_derivatives/    # CDS, CDS indices, tranches, options
    equity/                # Equities, options, autocallables, PE funds
    fx/                    # FX spots, forwards, options, barriers
    commodity/             # Commodity forwards, options, swaps
    exotics/               # Asian, barrier, lookback, basket, range accrual, TARN, snowball
  results/1/               # Valuation result schema (v1)
```

### `index.json`

`index.json` is generated alongside the schemas and is the entry point for tools
that need to enumerate the surface without walking the tree. Each entry carries
`$id`, a crate-relative `path` (`schemas/...`), `title`, a one-line `summary`,
`bytes`, and a `kind` of `input` (a document a caller submits), `output` (a
document the library returns), or `component` (a shared `$defs`-style fragment
referenced by others).

`index.json` is the on-disk projection of the Rust-side registry returned by
`finstack_quant_valuations::schema::artifacts_slice()`. The umbrella crate's
`finstack_quant::schema` module merges that registry with every other domain
crate's, so cross-document `$ref` resolution and whole-corpus lookup
(`finstack_quant::schema::find` / `validate`) work against one view.

## Using Schemas

### IDE Autocompletion (VS Code)

Add to your `.vscode/settings.json`:

```json
{
  "json.schemas": [
    {
      "fileMatch": ["**/instruments/**/*.json"],
      "url": "./finstack-quant/valuations/schemas/instruments/1/instrument.schema.json"
    }
  ]
}
```

### Constructing Instrument JSON

Every instrument uses the envelope format. This is the checked-in canonical
fixture [`../tests/instruments/json_examples/bond.json`](../tests/instruments/json_examples/bond.json),
emitted by `gen_schemas` from the registry's own `example()` provider:

```json
{
  "schema": "finstack_quant.instrument/1",
  "instrument": {
    "type": "bond",
    "spec": {
      "id": "US912828XG33",
      "notional": { "amount": "1000000", "currency": "USD" },
      "issue_date": "2024-01-15",
      "maturity": "2034-01-15",
      "cashflow_spec": {
        "fixed": {
          "coupon_type": "cash",
          "frequency": { "count": 6, "unit": "months" },
          "day_count": "act_act_isma",
          "calendar_id": "sifma",
          "rate": "0.0425",
          "business_day_convention": "following",
          "end_of_month": false,
          "payment_lag_days": 0,
          "stub": "short_front"
        }
      },
      "discount_curve_id": "USD-TREASURY",
      "credit_curve_id": null,
      "call_put": null,
      "settlement_days": 1,
      "ex_coupon_days": 0,
      "ex_coupon_calendar_id": "sifma",
      "attributes": {}
    }
  }
}
```

Key conventions:
- **`notional.amount`** is a string (decimal precision)
- **Rates** (`rate`, `spread_bp`, `strike`) are strings when the Rust field is
  `rust_decimal::Decimal`; `_bp` fields are in basis points, unsuffixed rate
  fields are decimals (`"0.0425"` = 4.25%)
- **Dates** are ISO 8601 strings (`"2024-01-15"`)
- **Enums and enum variant tags** use `snake_case` (`"modified_following"`,
  `"call"`, `"european"`, `"fixed"`)
- **`attributes`** is `{"tags": [...], "meta": {...}}` for scenario tagging
- Unknown fields are rejected: generated schemas set `additionalProperties: false`
  and the Rust types carry `#[serde(deny_unknown_fields)]`

Rather than hand-writing a payload, start from the canonical fixture for the
instrument type you want — there is exactly one per registered tag under
[`../tests/instruments/json_examples/`](../tests/instruments/json_examples/).

### Instrument Types

The `instrument.type` field must be one of the registry discriminators (70 as of
this writing, one per per-type schema under `instruments/1/`). See
`instrument.schema.json` for the full union, or enumerate them at runtime:

```rust
use finstack_quant_valuations::schema::{instrument_schema, instrument_types};

let types = instrument_types()?;        // Vec<String> of registry tags
let bond = instrument_schema("bond")?;  // serde_json::Value for one type
```

### Schema Structure

Each instrument schema has:
- **`examples`** — one or more fully-populated JSON examples from actual Rust serialization
- **`properties.instrument.properties.spec`** — typed property definitions with:
  - Field types, descriptions, and defaults
  - `required` arrays for mandatory fields
  - Enum variants with descriptions and standards references
- **Root-level `$defs`** — nested types (enums, structs) referenced from the spec

`instruments/1/instrument.schema.json` validates both the common envelope and
the typed `instrument.spec` payload through a 70-branch `oneOf` at
`$defs/InstrumentJson`. That union is **inlined**, not assembled by `$ref` to
the per-type files: the envelope document carries its own copy of every
instrument's definitions in `$defs`, so it is a standalone artifact rather than
an index over its siblings. The Rust helper
`validate_instrument_envelope_json()` still runs a second per-type validation
step so callers get discriminator-specific error messages.

### Shared References

Generated schemas use canonical external refs for repeated shapes:
- `common/1/*.schema.json` covers shared core types such as money, currency,
  IDs, attributes, pricing overrides, tenors, dates, decimals, day-count
  conventions, and business-day conventions.
- `../../cashflows/schemas/cashflow/1/*.schema.json` covers standalone cashflow component specs. The
  generator only externalizes unambiguous cashflow definitions; overloaded names
  such as instrument-specific `AmortizationSpec` remain local.

Validators must resolve these `$id` URIs. Offline validators should register the
checked-in files as in-memory resources keyed by their `$id` values. The Rust
runtime validation helpers do this automatically for embedded schemas.

`exotics/basket.schema.json` is the single canonical basket schema; there is no
separate "basket with instruments" artifact.

## Versioning

Schema versions are encoded in directory paths; every artifact in this tree is
currently at `/1/`, and a breaking change adds a sibling `/2/` rather than
mutating `/1/`. On database-oriented paths, the strict Rust loaders enforce the
`schema` field
(for example, `"finstack_quant.instrument/1"`). Raw `serde_json` deserialization and generic
JSON Schema validators only check what their caller invokes; they do not
replace strict loader version, resource-limit, migration, or semantic checks.

`schema_version` is reserved for internal model/data payloads whose Rust type
owns that field. The valuation-result schema is the exception in this tree;
credit factor-model artifacts live in the owning factor-model crate. Public
envelopes should use `schema`.

## Validation

Schemas can be used with any JSON Schema Draft 2020-12 validator:

```python
import jsonschema, json

schema = json.load(open("schemas/instruments/1/fixed_income/bond.schema.json"))
instance = json.load(open("my_bond.json"))
jsonschema.validate(instance, schema)
```

For schemas containing external `$ref`s, configure your validator with the
referenced files from `common/1` and the cashflows-owned schema directory. Those
are the only two namespaces any instrument document reaches: no instrument
schema `$ref`s another instrument schema, and neither does
`instrument.schema.json`, whose union is inlined. Registering `common/1` plus
`cashflow/1` is therefore sufficient for every artifact under `instruments/1/`,
the envelope included.

In Rust, use `finstack_quant_valuations::schema::validate_instrument_envelope_json()`
for runtime validation against the embedded schemas, or
`validate_instrument_type_json()` when the discriminator is already known. The
umbrella crate's `finstack_quant::schema::validate()` resolves cross-crate
`$ref`s and drills into `oneOf` failures so the reported error names the field
that is wrong rather than the whole union.

For persistence loading, use
`finstack_quant_valuations::instruments::InstrumentEnvelope::from_slice_strict`
or the contract-specific strict loader listed in
[`docs/CONTRACTS.md`](../../../docs/CONTRACTS.md).

## Verification

```bash
# Fail if any checked-in schema, index entry, or canonical fixture has drifted
cargo run -p finstack-quant-valuations --bin gen_schemas -- --check

# Full schema gate: serde audit, every owning generator, and the parity tests
mise run rust-check-schemas
```
