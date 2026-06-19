# Finstack JSON Schemas

JSON Schema (Draft 7) definitions for all Finstack data types. These schemas are **auto-generated from Rust types** via [schemars](https://github.com/GREsau/schemars) and should not be edited by hand.

## Regenerating Schemas

```bash
# Regenerate all typed schemas from Rust serde types
mise run rust-gen-schemas

# Or directly:
cargo run -p finstack-quant-valuations --bin gen_schemas
```

## Directory Structure

```
schemas/
  instruments/1/           # Financial instrument definitions (v1)
    instrument.schema.json # Envelope schema (tagged union of all types)
    fixed_income/          # Bonds, loans, structured credit, MBS
    rates/                 # Swaps, swaptions, caps/floors, futures
    credit_derivatives/    # CDS, CDS indices, tranches, options
    equity/                # Equities, options, autocallables, PE funds
    fx/                    # FX spots, forwards, options, barriers
    commodity/             # Commodity forwards, options, swaps
    exotics/               # Asian, barrier, lookback, basket options
  calibration/2/           # Frozen calibration schema (v2)
  calibration/3/           # Current calibration schema (v3)
  cashflow/1/              # Cashflow component specs (v1)
  factor_model/1/          # Credit factor-model schemas (v1)
  market/1/                # Market quote schemas (v1)
  results/1/               # Valuation result schema (v1)
```

## Using Schemas

### IDE Autocompletion (VS Code)

Add to your `.vscode/settings.json`:

```json
{
  "json.schemas": [
    {
      "fileMatch": ["**/instruments/**/*.json"],
      "url": "./finstack-quant/valuations/schemas/instruments/1/instrument.schema.json"
    },
    {
      "fileMatch": ["**/calibration/**/*.json"],
      "url": "./finstack-quant/valuations/schemas/calibration/3/calibration.schema.json"
    }
  ]
}
```

### Constructing Instrument JSON

Every instrument uses the envelope format:

```json
{
  "schema": "finstack_quant.instrument/1",
  "instrument": {
    "type": "bond",
    "spec": {
      "id": "UST-10Y",
      "notional": { "amount": "1000000", "currency": "USD" },
      "issue_date": "2024-01-15",
      "maturity": "2034-01-15",
      "cashflow_spec": {
        "Fixed": {
          "coupon_type": "Cash",
          "freq": { "count": 6, "unit": "months" },
          "dc": "ActActIsma",
          "calendar_id": "sifma",
          "rate": "0.0425",
          "bdc": "following",
          "stub": "None"
        }
      },
      "discount_curve_id": "USD-TREASURY",
      "attributes": {}
    }
  }
}
```

Key conventions:
- **`notional.amount`** is a string (decimal precision)
- **Rates** (`rate`, `spread_bp`, `strike`) are strings when using `rust_decimal::Decimal`
- **Dates** are ISO 8601 strings (`"2024-01-15"`)
- **Enums** use `snake_case` (e.g., `"modified_following"`, `"call"`, `"european"`)
- **`attributes`** is `{"tags": [...], "meta": {...}}` for scenario tagging

### Instrument Types

The `instrument.type` field must be one of the supported discriminators. See `instrument.schema.json` for the full list, or use:

```rust
use finstack_quant_valuations::schema::instrument_types;
let types = instrument_types()?; // Vec<String>
```

### Schema Structure

Each instrument schema has:
- **`examples`** — one or more fully-populated JSON examples from actual Rust serialization
- **`properties.instrument.properties.spec`** — typed property definitions with:
  - Field types, descriptions, and defaults
  - `required` arrays for mandatory fields
- Root-level `$defs` for nested types (enums, structs) referenced from the spec
  - Enum variants with descriptions and standards references

### Calibration JSON

Calibration uses a plan-based approach:

```json
{
  "schema": "finstack_quant.calibration/3",
  "plan": {
    "id": "my-calibration",
    "quote_sets": {
      "usd_rates": [
        { "type": "rates", "id": "USD-3M-SOFR", ... }
      ]
    },
    "steps": [
      {
        "id": "USD-OIS",
        "kind": "discount",
        "quote_set": "usd_rates",
        "curve_id": "USD-OIS",
        "currency": "USD",
        "base_date": "2025-01-15",
        "interpolation": "log_linear"
      }
    ]
  }
}
```

## Versioning

Schema versions are encoded in directory paths (`/1/`, `/2/`, `/3/`). The `schema` field in envelopes (for example, `"finstack_quant.instrument/1"` and `"finstack_quant.calibration/3"`) enforces version compatibility at parse time.

## Validation

Schemas can be used with any JSON Schema Draft 7 validator:

```python
import jsonschema, json

schema = json.load(open("schemas/instruments/1/fixed_income/bond.schema.json"))
instance = json.load(open("my_bond.json"))
jsonschema.validate(instance, schema)
```

In Rust, use the `finstack_quant_valuations::schema::validate_instrument_envelope_json()` function for runtime validation against the embedded schemas.
