# Market Bootstrap Reference Envelopes

A catalog of `CalibrationEnvelope` JSON examples: the canonical user-facing
examples for the "build a `MarketContext` from quotes" workflow. Between them
they exercise six of the thirteen `plan.steps` kinds — `discount`, `forward`,
`hazard`, `vol_surface`, `swaption_vol`, `base_correlation` — plus the
snapshot-only inputs (`fx_spot`, `price`, `dividend_schedule`). There is no
reference envelope yet for `inflation`, `parametric`, `hull_white`,
`cap_floor_hull_white`, `svi_surface`, `xccy_basis`, or `student_t`.

Envelopes are chained rather than seeded: where a step needs a discount or hazard
curve, that curve is produced by an upstream step in the same plan, so a quote
shock propagates through the whole chain instead of hitting a frozen snapshot.
The two credit envelopes (`05`, `12`) are the exception — they carry placeholder
hazard and base-correlation entries in `prior_market` because the `credit_indices`
context entry must reference an existing curve at load time and the base
correlation solver needs a seed. Their own plan steps overwrite both. Each file's
`plan.description` states exactly which entries are placeholders and why.

## Envelope shape

```jsonc
{
  "schema": "finstack_quant.calibration/1",
  "plan": {
    "id": "...",
    "quote_sets": { "usd_quotes": ["USD-SOFR-DEP-3M", "..."] },  // ID lists only
    "steps":      [ { "id": "USD-OIS", "quote_set": "usd_quotes", "kind": "discount", ... } ],
    "settings":   {}
  },
  "market_data":  [ { "kind": "rate_quote", "id": "USD-SOFR-DEP-3M", ... } ],  // the quotes
  "prior_market": [ /* pre-built curves/surfaces the plan reads but does not produce */ ]
}
```

`plan.quote_sets` holds named lists of quote **IDs**; the quote bodies live once
in the envelope-level `market_data` array. See
[`../../schemas/README.md`](../../schemas/README.md) for the full field
reference.

## Editor autocomplete and validation

Each file declares `$schema` pointing at
[`schemas/calibration/1/calibration.schema.json`](../../schemas/calibration/1/calibration.schema.json).
Modern editors (VS Code, JetBrains, Vim+coc, anything with JSON LSP) pick
this up automatically and provide:

- Autocomplete on every field, every step `kind`, every quote `class`.
- Inline validation of bad fields before the envelope hits the calibrator.

For project-wide coverage of calibration JSON outside this directory, add a
`json.schemas` mapping to your own `.vscode/settings.json` — see the IDE
Autocompletion section of [`../../schemas/README.md`](../../schemas/README.md).

## The catalog

Each envelope is loaded and exercised by an integration test in
[`../../tests/calibration/reference_envelopes.rs`](../../tests/calibration/reference_envelopes.rs),
which runs it through the engine and asserts that the resulting `MarketContext`
answers a typical analyst accessor query.

| File | Track | Purpose |
|---|---|---|
| `01_usd_discount.json` | A | USD-OIS discount from deposit + IRS quotes (foundational). |
| `02_usd_3m_forward_curve.json` | A | USD-SOFR-3M forward layered on a chained USD-OIS step. |
| `03_single_name_hazard.json` | A | Single-name CDS hazard layered on chained USD-OIS. |
| `04_cdx_ig_hazard.json` | A | CDX.NA.IG.46 index hazard with realistic par spreads. |
| `05_cdx_base_correlation.json` | A | CDX tranche base correlation chained on discount + hazard. |
| `06_cdx_index_vol.json` | A | CDX index option (CDSO) SABR vol surface. |
| `07_swaption_vol_surface.json` | A | USD swaption normal-vol cube on chained discount + forward. |
| `08_equity_vol_surface.json` | A | AAPL equity SABR vol; chained discount + spot/dividends in `market_data`. |
| `09_fx_matrix.json` | B | FX cross rates supplied as `fx_spot` entries in `market_data`. |
| `10_bond_prices.json` | B | Bond clean prices as `price` entries in `market_data`. |
| `11_equity_spots_dividends.json` | B | Equity spots + dividends in `market_data`. |
| `12_full_credit_desk_market.json` | A composite | Chained discount → hazard → base correlation, FX in `market_data`. |

**Track A** envelopes carry quotes in `market_data` and run `plan.steps` to
bootstrap curves from them. **Track B** envelopes carry snapshot-only
inputs (FX spots, prices, dividend schedules) in `market_data`, no
calibration steps.

## How to use one

```rust
use finstack_quant_core::contract::LoadLimits;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::schema::CalibrationEnvelope;

let envelope_json = std::fs::read_to_string("01_usd_discount.json")?;
let (envelope, _load_report) =
    CalibrationEnvelope::from_slice_strict(envelope_json.as_bytes(), &LoadLimits::default())?;
let result = engine::calibrate(&envelope)?;
let market = MarketContext::try_from(result.result.final_market)?;
let curve = market.get_discount("USD-OIS")?;
println!("DF(1y) = {}", curve.df(1.0));
```

`engine::calibrate` returns a structured `ExecuteError` (`worst_quote_id`,
tolerance, and the rest) on solver non-convergence. `From<ExecuteError>` maps
that payload to `finstack_quant_core::Error` so `?` still works in
`core::Result` functions. Static validation is fail-fast; `dry_run` lists
every static error without solving.

Same pattern in Python and JavaScript:

```python
result = finstack_quant.calibration.calibrate(envelope_json)  # -> CalibrationResult
market = result.market                                        # -> MarketContext
```

```javascript
const result = calibration.calibrate(envelope);  // plain JS object
const state = result.result.final_market;
```

## Verification

```bash
cargo nextest run -p finstack-quant-calibration --test calibration reference_envelopes
```

The envelopes are hand-maintained, not generated: edit the JSON and re-run that
test. A change to the calibration schema that these files do not follow fails at
deserialization inside `load_envelope`, naming the file.
