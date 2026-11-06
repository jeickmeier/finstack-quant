# CORE-NUMERICS-14: Marsaglia–Tsang Gamma sampling and beta-ratio fallback

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/distributions.rs::sample_beta` and private Gamma sampler: positive finite shape parameters and a caller-supplied `RandomNumberGenerator`. Beta outputs lie on [0,1] under the assumed RNG contract; this is a generic distribution, not a chosen recovery/severity model.

## Choice and rationale

Beta draws are ratios of independent Gamma draws. Gamma shape≥1 uses Marsaglia–Tsang acceptance/rejection; shape<1 uses shape augmentation and a uniform power. The bounded rejection loop protects callers from pathological RNG behavior. The endpoint fallback is an observed numerical approximation, not an accepted economic convention.

## Algorithm

1. Validate alpha,beta finite positive. If both equal 1 exactly, return one uniform draw.
2. Gamma shape a<1 returns `Gamma(a+1)*max(U,1e−300)^(1/a)`.
3. For a≥1 set d=a−1/3 and c=1/sqrt(9d). Draw standard normal X, set v=(1+cX)^3 when its base is positive, and draw U. Accept if `U<1−0.0331 X^4` or `log(max(U,1e−300))<X²/2+d[1−v_safe+log(v_safe)]`, with v_safe=max(v,1e−300). Return d v.
4. After 100,000 unsuccessful attempts, return a validation error. A shape-augmentation call shares that same attempt cap.
5. Draw X~Gamma(alpha), Y~Gamma(beta). If X+Y is finite positive, return X/(X+Y). Otherwise draw a Bernoulli endpoint: return 1 if U<alpha/(alpha+beta), else 0.

## Selection, controls and conventions

The RNG and seed are caller-selected; rejection consumption is path-dependent. The Gamma sampler is private. There is no option to disable the endpoint approximation or request a log-Gamma beta ratio. Shapes are dimensionless, and Gamma scale is one.

## Limitations, failure and fallback

Tiny shapes can underflow both Gamma draws and trigger the endpoint approximation. Large finite shapes can overflow their sum; alpha+beta in the fallback probability can also overflow, making its computed ratio zero. Thus the fallback is not established as uniformly correct over all accepted finite shapes. RNG outputs are trusted; no uniform/normal validation occurs here.

## Alternatives and tradeoffs

A log-domain Gamma ratio or specialized beta sampler could avoid the endpoint approximation but is not implemented. The supplied uniform special case avoids unnecessary Gamma work. Fallback changes distributional behavior; its permitted consumer domain is not adopted.

## Evidence

- [Implementation and inline tests](../../../../finstack-quant/core/src/math/distributions.rs).
- `gamma_rejection_loop_has_a_hard_limit` uses a pathological RNG to verify bounded failure. Beta/Gamma statistics tests compare sampled moments; they are stochastic sanity checks, not deterministic tail or extreme-shape accuracy guarantees. Small-shape cases establish that endpoint-valued results can occur.
- [Monte Carlo reference context](../../../REFERENCES.md#glasserman-2004-monte-carlo). A direct Marsaglia–Tsang primary-reference anchor is absent from the inspected catalog.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
