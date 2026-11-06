# VALUATIONS-FRAMEWORK-02 — Volatility, fitted model inputs, drift shape and simulation controls

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the shared resolution paths cited below; product and model kernels have separate records.
- Validation status: Evidence inspected/located, not executed; no external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and algorithm

Shared helpers resolve the economic inputs before the selected product model runs. They do not calibrate a missing model during pricing.

1. Resolve a volatility source from the market (cube, then ordinary surface, then FX-delta surface). An explicit scalar implied-volatility override plus explicit model convention may work without a source; automatic convention selection still needs source metadata. A present source supplies its expiry/tenor convention. Explicit lognormal with a shifted source retains the source shift; two explicit shifts differing by more than `1e−12` fail. An explicit normal model reads normal quotes, while lognormal/shifted models read Black quotes. This is model-aware retrieval, not arbitrary quote conversion. Sigma must be finite and nonnegative. Normal accepts finite signed forward/strike; Black requires both displaced coordinates strictly positive. The equity/FX `resolve_sigma_at` helper requires a strike-axis surface when no scalar override is active, clamps lookup, and rejects displaced sources for its unshifted consumer.
2. Scalar Hull–White resolution requires a complete positive finite `(kappa,sigma)` pair. Instrument override wins, then a complete constructor-supplied pair, then market scalars named by curve and cap/floor versus swaption family. Partial overrides fail even when a complete fallback exists. A volatility schedule fails in this scalar-only path. Market scalars can be unitless or monetary numeric values; invalid or absent values produce missing/partial-parameter errors. This differs from the explicit deterministic-zero permission of particular bond lattices. No volatility surface fitting or default short-rate parameters occur here.
3. Closed-form Heston resolution requires all five unitless market scalars `HESTON_KAPPA`, `HESTON_THETA`, `HESTON_SIGMA_V`, `HESTON_RHO`, `HESTON_V0`; missing or monetary entries fail. Caller-supplied continuous rates and dividend yield are combined through the model constructor, which owns parameter admissibility.
4. Optional dividend yield defaults to zero only when no identifier was supplied. A configured missing or monetary scalar fails. The helper itself does not independently reject nonfinite unitless yield; downstream model validation is relevant.

## Term-structure drift adapter

For GBM paths, sample at `max(num_steps,1)+1` evenly spaced knots over `[0,T]`. Let `u0` be the signed curve time at valuation and `D0=DF(u0)`, which must be positive finite. Form `C(t)=−ln[DF(u0+t)/D0]`. If `|C(T)|>1e−12`, multiply the curve shape by `a=rT/C(T)`; otherwise set `a=1`. The cumulative drift is `M(t)=aC(t)−qt`. The model's drift-schedule constructor validates the result. This retains term-structure shape while matching the supplied maturity-effective rate when the scaling denominator is usable. It samples additive model time on the curve clock, not a date for every interior knot. When `|C(T)|≤1e−12`, terminal matching is not forced if supplied `rT` differs.

## Simulation and numerical controls

The common path helper defaults to 100,000 paths, 252 steps/year, and 100 steps for rough-volatility adapters, with an allocation ceiling of 5,000,000 paths. Actual pricers can supply different defaults through the embedded pricer registry. `resolve_mc_paths` treats absent/zero override as the caller default and rejects the resulting count above the ceiling; its direct default argument can itself be zero. In contrast `ModelConfig::validate` rejects an explicit zero `mc_paths`. The merged path configuration changes only path count and optional antithetic flag. Seed derivation uses instrument ID plus scenario label (default `base`); product-specific consumption of CI targets and other settings is not implied by their presence on the shared config.

Model configuration defaults to clamped volatility extrapolation and no explicit tree model; callers interpret no tree model as Hull–White. Tree steps must be nonzero; shared volatility, mean-reversion and call-friction fields are finite nonnegative; LMM loading scale and CI target must be positive finite; rate-credit correlation lies in `[-1,1]`. Product paths impose stricter combinations. HW sigma is absolute annual short-rate volatility; BDT/BK sigma is relative log-rate volatility; option implied volatility is a different quote. Call friction is cents per 100 par and changes the exercise decision, not contractual redemption. Continuous or positive-periodic OAS compounding is accepted; simple compounding is rejected.

The inspected [product-default JSON](../../../../finstack-quant/valuations/core/data/defaults/product_mc_defaults.v1.json) sets LMM Bermudan to 25,000 paths, seed 42, degree-three basis, antithetics and at least four steps between exercises; Merton PIK to 5,000 estimators, antithetics and 12 steps/year; revolver to 10,000 estimators without antithetics. Loading verifies contract/version plus positive LMM path/basis/step counts, positive Merton paths/grid density and at least two revolver estimators. The separate `embedded_defaults_or_panic` entry treats corrupted embedded assets as a panic; the fallible loader returns validation errors. These values do not imply equal estimator meanings or effective seed policy across every consuming engine.

## Alternatives, evidence

Direct surface calibration, automatic scalar unit conversion, flat maturity drift, and automatic model substitution are distinct methods; only the explicitly described branches are implemented here.

- [vol_resolution.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/vol_resolution.rs), [market volatility bridge](../../../../finstack-quant/valuations/core/src/market/volatility.rs), [hw1f_params.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/hw1f_params.rs), [heston_market.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/heston_market.rs), [helpers.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs), [pricing_overrides.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing_overrides.rs), and [defaults.rs](../../../../finstack-quant/valuations/core/src/pricer/defaults.rs) own these policies. Inline tests cover missing/partial inputs, scalar overrides, axes and schedule rejection; this is behavioral evidence, not an independent numerical benchmark.
- [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) supplies simulation context. The default path counts and drift adapter are implementation choices awaiting agreement.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
