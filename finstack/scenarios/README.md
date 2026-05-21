# Finstack Scenarios

`finstack-scenarios` applies deterministic shocks and time rolls to market data,
financial statement models, and instrument collections. Scenarios are described as
serde-friendly specs and executed by a single engine.

## Core types

| Type | Role |
|------|------|
| `ScenarioSpec` | Named scenario with metadata and ordered `OperationSpec` list |
| `OperationSpec` | Shock or roll operation (curves, FX, equity, vol, statements, instruments, time) |
| `ScenarioEngine` | Composes and applies specs |
| `ExecutionContext` | Mutable market data, statement model, optional instruments, `as_of` |
| `ApplicationReport` | Applied operation count and warnings |
| `RateBindingSpec` | Links statement nodes to curves via `OperationSpec::RateBinding` |

## Operation families

- **Market data**: FX, equity, discount/forward/hazard/inflation curves, base correlation,
  volatility surfaces.
- **Statements**: forecast percent/assign, rate bindings.
- **Instruments**: price and spread shocks by type or attribute selector.
- **Time**: horizon roll-forward with carry/theta-aware paths.

## Composition

Scenarios merge deterministically with stable operation ordering and priority-based
conflict resolution. Market, statement, and time operations share one application path.

## Dependencies

- `finstack-core` — market data and dates
- `finstack-statements` — statement models
- `finstack-valuations` — pricing-aware rolls and instrument shocks

Bindings: `finstack-py`, `finstack-wasm`.

## Usage

1. Build a `ScenarioSpec` from `OperationSpec` values (or load a template from
   `templates`).
2. Fill `ExecutionContext` with market data, an optional statement model, and `as_of`.
3. Call `ScenarioEngine::apply`.
4. Read `ApplicationReport` and use the mutated context for downstream pricing.

See crate-level rustdoc for a runnable example.

## Tests

```bash
cargo test -p finstack-scenarios
```

## License

MIT OR Apache-2.0
