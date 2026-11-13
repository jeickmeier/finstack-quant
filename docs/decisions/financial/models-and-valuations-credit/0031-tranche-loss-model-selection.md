# ADR 0031: Synthetic tranche loss and copula selection

**Decision status:** Draft — observed implementation; adoption and rationale
pending agreement.

**Implementation status:** Observed for `CdsTranche` and the standalone
`CdsTranchePricer`; structured-credit waterfall simulation is outside scope.

**Validation status:** Evidence located and inspected; tests and reference
generators not rerun for this documentation audit.

**Audit date:** 2026-11-05

**Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`

## Observed choice and rationale

The [accepted CREDIT-17 package](17-tranche-loss-distribution.md#accepted-decision-for-checked-finite-pool-tranche-losses)
owns pool and copula selection and the numerical loss contract.
The dispatch descriptions below record observed behavior.

`CdsTranche` uses base-correlation equity-tranche decomposition with a one-factor
Gaussian copula by default. Recovered principal amortizes senior tranche
notional, while only losses trigger protection payments. The
[loss-distribution record](17-tranche-loss-distribution.md) owns the decomposition
and finite-pool calculations, including exact small-pool state enumeration to
avoid rounding across payoff kinks.

Proposed adoption rationale, awaiting agreement: finite-pool conditional loss
calculations preserve issuer granularity, while factor integration gives
deterministic valuation without Monte Carlo sampling noise. Source explanations
support the observed implementation; they do not establish model governance.

## Scope and selection

Inputs are tranche terms, a discount curve and `CreditIndexData`, including
index hazard, constituent count, recovery and base correlation. Supplied issuer
curves require complete coverage and notional-preserving weights. For live
pricing, index and issuer credit curves must be based on `as_of`, as specified
in the [loss input domain](17-tranche-loss-distribution.md#scope-and-input-domain).
This does not impose the same base-date restriction on discounting.

`CdsTranche::base_value` and cashflow projection construct
`CdsTranchePricer::new()`, selecting Gaussian. The registered instrument key
`ModelKey::HazardRate` alone does not identify the copula. Explicit standalone
`CdsTranchePricer::with_copula(CopulaSpec)` supports Gaussian, Student-t, random
factor loading and multi-factor variants. These are not separate registered
instrument model keys, and a standalone selection does not change default
projection or all specialized [risk calculations](19-tranche-risk.md).

Issuer inputs select homogeneous versus heterogeneous loss calculation; small
pool enumeration and bounded-grid convolution are internal dispatch choices,
not caller-selected alternative engines. Recovery is an index or per-issuer
input, not simulated stochastic recovery. Cash securitization waterfalls,
dynamic correlation calibration, counterparty risk and full portfolio credit
migration are outside scope.

## Methodology ownership

| Choice or algorithm | Detailed owner |
| --- | --- |
| Conditional distributions, factor representations and generic quadrature | [Copula kernels](09-copula-kernels.md) |
| Finite-pool dispatch, realized-state erosion, smoothing, resource/accuracy controls and loss repairs | [Tranche loss distribution](17-tranche-loss-distribution.md) |
| Premium and protection timing, realized settlement and the native valuation trace | [Accepted tranche projection](18-tranche-projection.md#accepted-decision-for-checked-tranche-cashflows-and-settlement) |
| Quote/payoff risk, copula propagation and issuer-recovery limitations | [Tranche risk](19-tranche-risk.md) |
| Stored correlation interpolation, shocks and shape diagnostics | [Base-correlation representation](../core-market/base-correlation.md) |
| Fixed-maturity correlation fitting and single-tranche tail-parameter fitting | [Base-correlation calibration](../calibration-targets/15-base-correlation.md) and [Student-t calibration](../calibration-targets/16-student-t.md) |

The owners retain formulas, units, numerical settings, failure branches and
evidence. Generic copula integration and its tranche-specific integration
override are distinct methods; neither record's settings supersede the other's.
Adjacent calibration records remain Draft and do not resolve model adoption.

## Alternatives and tradeoffs

Student-t adds tail dependence; random loadings and multiple factors add latent
structure and integration cost. Their existence does not establish intended
instrument uses or sufficient calibration inputs. A large homogeneous pool
approximation would reduce finite-name cost but discard concentration; Monte
Carlo could support other joint dynamics but introduce sampling error. Neither
is an automatic fallback at this engine's resource or integration limits. Grid
convolution has discretization error avoided by sufficiently small exact pools.

## Limitations and evidence

Maturity loss errors, interim floors, recovery-side floors and monotonicity/
outstanding-notional repairs have different contracts in the
[loss owner](17-tranche-loss-distribution.md#limitations-failure-and-fallback);
none establishes general base-correlation arbitrage freedom or exact limiting
correlation behavior.

The [loss reference evidence](17-tranche-loss-distribution.md#evidence)
retains its independent-generator descriptions and unqualified provenance/domain
limits under [ADR 0014](../../project/0014-financial-reference-evidence.md).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
