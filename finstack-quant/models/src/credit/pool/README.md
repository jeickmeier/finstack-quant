# Structured-Credit Pool Models

`finstack_quant_models::credit::pool` owns reusable collateral-pool stochastic
engines. The module is independent of deal instruments, tranche waterfalls,
calibration registries, and valuation results.

Public specifications and kernels include:

- `StochasticDefaultSpec` and `StochasticDefault`;
- `StochasticPrepaySpec` and `StochasticPrepayment`;
- `CorrelationStructure`;
- `PerNameCopulaDefault` and `PoolGranularity`.

The serializable spec tags are unchanged. Deal-type presets are intentionally
absent: valuations reads the v1 structured-credit assumptions registry and
constructs explicit specs with the constructors in this module.

```rust
use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec};
use finstack_quant_models::credit::pool::{
    CorrelationStructure, StochasticDefaultSpec, StochasticPrepaySpec,
};

let prepay = StochasticPrepaySpec::factor_correlated(
    PrepaymentModelSpec::constant_cpr(0.12),
    0.4,
    0.2,
);
let default = StochasticDefaultSpec::factor_correlated(
    DefaultModelSpec::constant_cdr(0.03),
    0.5,
    0.3,
);
let correlation = CorrelationStructure::flat(0.20, -0.15)?;
# let _ = (prepay, default, correlation);
# Ok::<(), Box<dyn std::error::Error>>(())
```
