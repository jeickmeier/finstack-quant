# Market Bootstrap Reference Envelopes

A canonical catalog of `CalibrationEnvelope` JSON examples covering every
commonly-needed curve, surface, and snapshot type. Each envelope is
self-bootstrapping: every curve the envelope's calibration steps depend on
is itself produced by an upstream step in the same plan (no hand-entered
snapshot knots that would drift from quote-shocked risk calculations).

## Editor autocomplete and validation

Each file declares `$schema` pointing at
[`schemas/calibration/2/calibration.schema.json`](../../schemas/calibration/2/calibration.schema.json).
Modern editors (VS Code, JetBrains, Vim+coc, anything with JSON LSP) pick
this up automatically and provide:

- Autocomplete on every field, every step `kind`, every quote `class`.
- Inline validation of bad fields before the envelope hits the calibrator.

For project-wide editor coverage of `*.calibration.json` files outside this
directory, copy [`.vscode/settings.json.example`](../../../../.vscode/settings.json.example)
to `.vscode/settings.json`.

## The catalog

Each envelope is loaded and exercised by integration tests in
[`tests/calibration/reference_envelopes.rs`](../../tests/calibration/reference_envelopes.rs).

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
use finstack_quant_valuations::calibration::api::{engine, schema::CalibrationEnvelope};
use finstack_quant_core::market_data::context::MarketContext;

let envelope_json = std::fs::read_to_string("01_usd_discount.json")?;
let envelope: CalibrationEnvelope = serde_json::from_str(&envelope_json)?;
let result = engine::execute(&envelope)?;
let market = MarketContext::try_from(result.result.final_market)?;
let curve = market.get_discount("USD-OIS").expect("USD-OIS calibrated");
println!("DF(1y) = {}", curve.df(1.0));
```

Same pattern in Python (`finstack_quant.valuations.calibrate(json).market`) and
JavaScript (`valuations.calibrate(envelope).result.final_market`).
