# Market

Valuation-owned market conventions and option-volatility lookup policy.

Quote DTOs, quote-to-instrument construction, date resolution, calibration, and
recalibration live in `finstack-quant-calibration`. This module deliberately
contains no quote ingestion or calibration engine.

## Layout

| Path | Visibility | Contents |
|------|------------|----------|
| `conventions/` | public | Convention definitions, typed IDs, and the global registry |

## Conventions

`conventions::ConventionRegistry::try_global()` returns a lazily built,
process-wide singleton loaded from JSON embedded at compile time from
`../../data/conventions/`.

| Registry | Data file | Key type |
|----------|-----------|----------|
| Rate index | `rate_index_conventions.json` | `finstack_quant_core::types::IndexId` |
| CDS | `cds_conventions.json` | `conventions::ids::CdsConventionKey` |
| Swaption | `swaption_conventions.json` | `conventions::ids::SwaptionConventionId` |
| Inflation swap | `inflation_swap_conventions.json` | `conventions::ids::InflationSwapConventionId` |
| IR future | `ir_future_conventions.json` | `conventions::ids::IrFutureContractId` |
| Cross-currency | `xccy_conventions.json` | `conventions::ids::XccyConventionId` |

Lookups are strict. `require_rate_index` and its siblings return
`InputError::NotFound` when an ID is absent. CDS schedule lookup goes through
`resolve_cds`: meta clauses map to their regional family (`Au`/`Nz` use Asia),
exact restructuring clauses stay on the instrument, and `ANY` rows are
loader-only fallbacks behind explicit currency entries. Calibration builders
depend on these valuation-owned conventions.

`conventions::ids` also defines typed identifiers referenced by instruments
and calibration quotes, including `SwaptionConventionId`.

## Dependency boundary

The permanent direction is:

```text
finstack-quant-calibration -> finstack-quant-valuations -> core/models/cashflows
```

Valuations exposes the object-safe recalibration contract in
`crate::recalibration`; calibration implements it. Pricing receives that
service through `PricingOptions` and never imports the calibration crate.
