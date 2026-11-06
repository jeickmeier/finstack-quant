# MODELS-MONTE-CARLO-11: Bounded utilization, short-rate and CIR spread joint process

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in process/credit_line.rs and discretization/credit_line.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

The three coordinates are utilization fraction, decimal short rate and nonnegative credit spread. Utilization has positive mean reversion, target in [0,1], nonnegative additive volatility and optional spread-linked target sensitivity. Rates are fixed, Hull–White stochastic, or deterministically interpolated forward levels; spreads are CIR. Optional 3×3 driver correlation is Cholesky-factorized.

## Algorithm and controls

Apply correlation once in the dedicated scheme when supplied. First set the utilization target to `clamp(theta+sensitivity*(spread/spread0-1),0,1)` using the old spread; disable that adjustment if sensitivity=0 or spread0≤0. Evolve an exact OU transition over the step and clamp utilization to [0,1]. Next preserve fixed rate, integrate the Hull–White theta/sigma kernels at `time_offset+t`, or interpolate deterministic rate at the endpoint. Finally update spread using shared QE threshold 1.5. Deterministic rates use linear interpolation and flat tails; the helper returns zero for empty arrays and truncates mismatched lengths, though validated specifications reject malformed curves.

## Limitations and alternatives

Clamping an OU draw creates endpoint probability mass and changes transition moments; it is not a reflected diffusion. The generic process drift omits spread-target coupling and generic Euler does not clamp utilization or refresh deterministic rates. Unlike dividends/LMM, this process does not declare a mandatory scheme, so low-level generic-engine pairing can silently select a materially different process. A Jacobi or reflected process is an unimplemented alternative. Joint shock correlation is not exact finite-step joint transition correlation when components have different mean-reversion kernels.

## Evidence

[Process](../../../../finstack-quant/models/src/monte_carlo/process/credit_line.rs), [dedicated stepping](../../../../finstack-quant/models/src/monte_carlo/discretization/credit_line.rs); tests locate exact-OU mean, old-spread coupling inputs, CIR tail monotonicity and curve-clock theta integration. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) supports numerical primitives. No canonical economic source or calibration/adoption evidence for the utilization/spread coupling was found in the central references; this is an explicit open methodology choice.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
