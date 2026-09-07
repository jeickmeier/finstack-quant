# Finstack Quant 0.8.0

**Release Date**: 2026-09-06

**Bump type**: minor (pre-1.0; intentional breaking Rust, Python, WASM, and JSON changes)
**Status**: Prepared, not yet tagged (`v0.7.0` is the latest git tag)

## Executive Summary

0.8.0 gives reusable quantitative engines one canonical owner:
`finstack-quant-models`. Core retains neutral data and infrastructure,
valuations retains instruments and pricing orchestration, and portfolio retains
position and workflow policy. The release also removes every implicit 40%
recovery fallback.

This is a clean break. Old crates, modules, host namespaces, re-exports, and
fallback parsing are removed rather than deprecated.

## Who Should Upgrade

- Rust callers importing mathematical, credit, volatility, factor, liquidity,
  Hull-White, or structured-credit stochastic engines from non-model crates.
- Python callers using the former top-level factor-model namespace, core credit
  model exports, core DTSM, or portfolio liquidity functions.
- WASM callers using the corresponding old facade paths.
- Any caller building hazard curves, credit indices, calibration envelopes,
  Merton Monte Carlo inputs, or XVA inputs without an explicit recovery.

## Breaking Changes

### 1. Reusable engines live in `finstack-quant-models`

Canonical Rust destinations are:

| Removed owner | Canonical owner |
|---|---|
| characteristic-function engines formerly in core | `models::fourier::characteristic_function` |
| dynamic term-structure engines formerly in core | `models::rates::dtsm` |
| computational credit modules formerly in core | `models::credit` |
| standalone factor-model crate | `models::factor` |
| pure portfolio factor-risk kernels | `models::factor::{risk, credit}` |
| portfolio liquidity engines | `models::liquidity` |
| valuation-owned Hull-White equations | `models::rates::hull_white` |
| structured-credit stochastic engines | `models::credit::pool` |

For example:

```rust
use finstack_quant_models::liquidity::days_to_liquidate;
use finstack_quant_models::rates::dtsm::DieboldLi;
```

The former standalone factor-model workspace package is gone. Factor
configuration schema IDs remain `finstack_quant.factor_model_config/1`; only
their owning crate and generated-file location changed.

### 2. Volatility computation is models-owned

Core retains serializable `VolSurface`, `VolCube`, and `FxDeltaVolSurface`
artifacts. SABR, SVI, Heston, rough-Heston, local-volatility, implied-volatility,
fitting, interpolation, extrapolation, and Black/Bachelier formulas live in
models.

`VolProvider` and `MarketContext::get_vol_provider` are removed. Use
`models::volatility::VolSource` for evaluation; valuation pricers obtain one
through the valuation-owned market resolver.

### 3. Recovery is required

Any calculation whose result depends on recovery requires a finite decimal in
`[0.0, 1.0]`. Missing recovery is a validation error; explicit zero is valid.
The global credit-assumptions recovery field and the 40% fallback are removed.

```rust
// before: recovery could be omitted and silently became 40%
let input = HazardCurveInput { recovery_rate: None, /* ... */ };

// after: the caller supplies the economic assumption
let input = HazardCurveInput { recovery_rate: 0.40, /* ... */ };
```

Because the project is pre-release, contracts were corrected in place and
remain version 1. Do not change persisted markers: calibration stays
`finstack_quant.calibration/1`, credit assumptions stay
`finstack_quant.credit_assumptions/1`, and all other affected schema directories
remain `/1`.

### 4. Host namespaces follow Rust ownership

Python model APIs are under:

```python
from finstack_quant.models import credit, factor, liquidity, rates, volatility
from finstack_quant.models.rates import dtsm
```

WASM callers use the corresponding facade paths:

```javascript
models.factor;
models.rates.dtsm;
models.volatility;
models.credit;
models.liquidity;
```

The former top-level factor-model, core credit-model, core DTSM, portfolio
liquidity, and valuation-owned model namespaces do not resolve. Structured
credit pool model engines were not previously bound, so `models::credit::pool`
remains Rust-only in this release.

### 5. Fail-closed constructors after the August RC cut

These landed after the original 0.8.0 changelog date and are part of this tag:

- Asian call/put constructors return `Result` and require at least one future
  or historical fixing.
- Portfolio margin-result JSON uses `SimmSensitivitiesJson` tuple arrays.
- FX barrier, digital, and touch options reject a monitoring start after
  `as_of`.
- Black-Scholes and related model entry points reject non-positive spot or
  strike and non-finite numerical inputs.

`CashFlowAccrual` also gained optional `coupon_period` and
`end_is_termination_date` fields.

### 6. Calibration crate

Quote ingestion, market construction, calibration, and replay caches live in
`finstack-quant-calibration`. Valuations keep instruments, pricing, and the
recalibration port. Python and WASM keep the existing `calibration` namespaces.

### 7. More fail-closed constructors and removed shims

- `Rate` conversion, percentage construction, and rate negation return
  `Result`.
- Callable bonds reject `discounting`; use `tree` or `rates_credit`.
- `PvDiscountSource::Market` is gone; pass an explicit discount source.
- Compatibility shims listed in `CHANGELOG.md` under post-RC surface cuts
  were deleted rather than deprecated.

## Architecture Result

The intended dependency direction is:

```text
core / analytics / cashflows -> models -> calibration / valuations -> portfolio
```

Core owns neutral types and observed market-data artifacts. Models owns
product-independent engines. Calibration owns quote ingestion, market
construction, calibration, and replay caches. Valuations owns instruments,
market resolution, the recalibration port, presets, and results. Portfolio owns
positions, assignment, valuation-based sensitivities, allocation policy,
what-if workflows, and reporting adapters.

## Deprecated

None. No compatibility aliases or deprecated paths are retained.

## Migration Checklist

1. Replace imports from removed core, portfolio, valuation, and standalone
   factor-model paths with the canonical `models` paths above.
2. Update Python imports to `finstack_quant.models.*` and WASM access to
   `models.*`.
3. Supply recovery explicitly everywhere it affects a calculation; validate it
   as a decimal in `[0.0, 1.0]`.
4. Replace volatility provider trait usage with concrete `VolSource`
   evaluation or the valuations market resolver.
5. Import `HullWhiteParams` and Hull-White pricing kernels from
   `models::rates::hull_white`; keep quote preparation and calibration calls in
   valuations.
6. Import structured-credit stochastic specifications from
   `models::credit::pool`; continue to construct and price deals through
   valuations.
7. Keep every persisted contract marker at version 1.
8. Replace `PvDiscountSource::Market` with an explicit discount source.
9. Handle `Result` from `Rate` and percentage constructors.
10. Price callable bonds with `tree` or `rates_credit`, not `discounting`.

## Numerical Behavior

The ownership moves preserve the existing numerical kernels and seeded-path
discipline. The deliberate numerical-input change is recovery: calculations no
longer manufacture a 40% assumption when the caller omitted it.

## Known Limitations

- Structured-credit pool stochastic engines remain Rust-only because no such
  engine types were bound before this move.
- Core volatility artifacts provide data and structural validation only;
  computational evaluation requires the models crate.
- Five Python golden fixtures still fail and must be resolved before tag:
  hazard-curve QuantLib bonds (NPV/DV01), a callable OAS QuantLib bond that
  still requests `discounting`, the Hagan CMS floorlet regression, and the
  term-loan B CS01/DV01 regression. Do not tag `v0.8.0` until those pass or
  are explicitly re-baselined.

See [`CHANGELOG.md`](CHANGELOG.md) for the complete itemized release history.
