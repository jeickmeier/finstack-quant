# MODELS-MONTE-CARLO-03: Whole-path Sobol points, Brownian bridge and replicate uncertainty

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in pricer/path_dependent.rs and rng/sobol.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

`PathDependentPricer` offers explicitly selected randomized Sobol pricing for GBM payoffs. One Sobol point supplies the entire Gaussian path. `with_sobol(true)` also enables Brownian-bridge construction; a subsequent override can disable it. Pseudorandom pricing remains the registry default. The bridge flag alone has no effect in the Philox branch.

## Algorithm and controls

Dimension is steps times factors without a bridge, or steps with a single-factor bridge. Reject zero/excessive core Sobol dimension, parallel Sobol and antithetic Sobol. Split the requested path budget among `min(16,num_paths)` independently seeded scrambles; the first remainder replicates receive one extra path. Scramble seeds combine root seed, a domain salt and a Weyl multiple, then replace zero by one. Draw an entire normal point; optionally construct Brownian levels on actual grid times and recover standardized increments `(W[i+1]-W[i])/sqrt(dt[i])`. Payoff uniforms use a separate salted path-indexed Philox stream, not reused Sobol coordinates. Use exact GBM and ordinary payoff/capture machinery.

The reported mean weights all individual paths equally. Standard error and Student-t half-width come from unweighted replicate means; displayed path count and standard deviation refer to individual paths. This distinction is financially material: the CI is across scrambles, not an i.i.d. interval over Sobol points.

## Failure behavior and alternatives

No power-of-two size is imposed. For unequal replicate sizes, reported mean and uncertainty use different weighting, including tiny/non-divisible budgets. Capture retains the generic expected-count hash sample. Independent auxiliary uniforms can limit QMC variance reduction for barrier/lookback payoffs. Alternatives are equal power-of-two replicate budgets, weighted replicate variance, and joint QMC auxiliary coordinates; these are not implemented choices.

## Evidence

[Implementation](../../../../finstack-quant/models/src/monte_carlo/pricer/path_dependent.rs) contains `price_with_sobol`, `fill_sobol_increments`, `SobolPathStream`; tests `test_sobol_rqmc_stderr_covers_true_error`, irregular-grid bridge, dimension rejection and multiple-path capture are located. The coverage test is a seeded numerical diagnostic, not universal coverage proof. [Owen scrambling](../../../REFERENCES.md#owen-1995-scrambling), [Sobol](../../../REFERENCES.md#sobol-1967) and [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) are background references; core generator/bridge kernels are outside this owned slice.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
