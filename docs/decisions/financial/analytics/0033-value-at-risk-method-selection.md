# ADR 0033: Return-series VaR and expected-shortfall method selection

**Decision status:** Draft — observed implementation; adoption and rationale
pending agreement.

**Implementation status:** Observed in the `Performance` scalar and summary
paths, their Rust kernels, and Python/WASM argument defaults.

**Validation status:** Evidence located and inspected, not rerun; independent
reference coverage and tolerance justification remain incomplete.

**Audit date:** 2026-11-05

**Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`

## Observed choice and rationale

`Performance::value_at_risk`, `expected_shortfall` and the standard summary
select the [historical estimators](07-historical-tail-risk.md).
[Gaussian and Cornish–Fisher VaR](08-moments-gaussian-and-cornish-fisher.md)
are separately named, explicit alternatives; selecting either does not change
the summary's estimator. This record owns that selection boundary and its
proposed rationale, not the algorithms defined in those detailed records.
The broader methodology choices remain unadopted.

**Prior agreed direction (2026-10-11):** invalid Cornish–Fisher estimates
return an explicit error, rather than a per-ticker `NaN` or Gaussian
substitution. The current fallback is an implementation discrepancy against
that direction; its presence and regression test do not reopen the decision.

The algorithm rationales and evidence are recorded in the detailed owners
below. They do not establish an agreed reason to make historical VaR the
summary choice.

Proposed rationale for agreement: separately named estimators let callers
choose empirical tails or distributional assumptions explicitly. Historical
summary metrics avoid selecting a fitted distribution for every panel.
These are proposed tradeoffs, not a recovered adoption decision.

## Scope and assumptions

The scope is per-ticker risk on the active `Performance` window under the
[panel input and observation-span contract](01-panels-windows-and-wire.md).
There is no instrument pricing, position revaluation, or currency conversion
inside these estimators; portfolio and factor-model VaR need separate records.

Historical estimation preserves the observed shape but has limited tail
resolution for short samples. Gaussian estimation is compact and admits the
implemented horizon scaling, but excludes skew and excess kurtosis.
Cornish–Fisher includes those moments without fitting a full distribution,
but noisy moments and a non-monotonic expansion can invalidate its quantiles.
The detailed records own the sampling assumptions, numerical conventions and
unselected alternatives for each estimator.

## Selection and unsupported combinations

| Entry point | Effective method and selection |
| --- | --- |
| `value_at_risk(confidence)` | [Historical VaR](07-historical-tail-risk.md). |
| `expected_shortfall(confidence)` | [Exact-mass empirical ES](07-historical-tail-risk.md). |
| `summary(risk_free_rate, confidence)` | Historical VaR and ES; no model override. |
| `parametric_var(confidence, horizon_periods)` | Explicit [Gaussian alternative](08-moments-gaussian-and-cornish-fisher.md). |
| `cornish_fisher_var(confidence, horizon_periods)` | Explicit [moment-adjusted alternative](08-moments-gaussian-and-cornish-fisher.md); its current fallback conflicts with the prior direction above. |

The [host-default contract](01-panels-windows-and-wire.md) traces omitted
confidence and horizon arguments; the [horizon algorithm](08-moments-gaussian-and-cornish-fisher.md)
defines their observation-period meaning. Historical VaR and ES expose no
horizon-scaling argument. There is no Gaussian or Cornish–Fisher ES selector
in these methods. The [public dispatch and summary](../../../../finstack-quant/analytics/src/performance/scalar.rs)
establish this selection; available alternatives do not change the summary.

## Algorithm and evidence owners

| Owner | Defined method and supporting evidence |
| --- | --- |
| [ANALYTICS-001](01-panels-windows-and-wire.md) | Panel validation, active spans, wire behavior and shared host defaults, with the traced Rust/Python/WASM caller paths. |
| [ANALYTICS-007](07-historical-tail-risk.md) | R-7 quantiles, fractional empirical ES, signed outputs, tail ratio, unavailable samples and historical evidence. |
| [ANALYTICS-008](08-moments-gaussian-and-cornish-fisher.md) | Moments, Gaussian/CF horizons, the observed CF discrepancy, invalid-horizon behavior, formula evidence and validity limits. |

The [analytics inventory](README.md) maps all 21 production Rust source
files. Its source enumeration does not establish complete host parity.
The detailed records distinguish independent arithmetic examples from
same-kernel comparisons and located tests from executed checks. Their
reference and tolerance gaps remain open.

## Follow-up tracking

The historical-summary rationale and broader estimator domains remain Draft.
Implementation reconciliation, adoption questions and missing evidence are owned by the
[analytics follow-ups](../FOLLOW_UPS.md#analytics); the
[central follow-ups](../../FOLLOW_UPS.md#financial-methodology-audit) coordinate cross-domain work.
