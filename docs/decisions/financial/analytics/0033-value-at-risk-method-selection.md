# ADR 0033: Return-series VaR and expected-shortfall method selection

**Decision status:** Accepted for fixed historical tail-method selection
(2026-11-07). Other metric, input and failure policies retain the scope and
status of their owning records.

**Implementation status:** Observed in the `Performance` scalar and summary
paths, their Rust kernels, and Python/WASM argument defaults.

**Validation status:** Evidence located and inspected, not rerun; independent
reference coverage and tolerance justification remain incomplete.

**Audit date:** 2026-11-05

**Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`

## Accepted decision: fixed historical tail selection

**Accepted:** 2026-11-07. **Implementation:** Selection observed in selected
source at `5140b499cc`. The scalar and summary dispatch match this selection;
related confidence, documentation and CF-error reconciliation remain open.
No runtime estimator or cross-host checks were executed.

The canonical `value_at_risk` and `expected_shortfall` methods use the
accepted historical pair: R-7 interpolated VaR and exact fractional empirical
ES. Standard `summary(...)` reports that same pair in its VaR/ES columns,
using each ticker's active return history and explicit confidence. Retain the
accepted native-period, signed-output and historical-availability rules.

Gaussian `parametric_var` and `cornish_fisher_var` remain separately named,
explicit calculations. Calling either does not change the summary's method.
Do not switch estimators automatically because history is short, an estimate
is unavailable, or CF fails. Preserve the selected estimator's undefined or
error outcome. No summary method selector or automatic dispatch is introduced
by this decision.

For example, requesting ten-period Gaussian VaR does not cause a subsequent
summary to report ten-period Gaussian risk: its tail columns remain native-
period historical estimates.

This gives the standard performance report a fixed descriptive interpretation
without selecting Gaussian distribution assumptions or a higher-moment CF
approximation on the caller's behalf. It is a product policy, not a universal
financial requirement. Sample representativeness and tail resolution still
limit historical estimates; the selection does not qualify forecasting
accuracy.

The summary's complete input signature, non-tail metric selection and whole-
report failure behavior follow the separately [accepted record-01 package](01-panels-windows-and-wire.md#accepted-decision-panels-windows-wire-and-summary). In
particular, a calculable historical tail statistic does not imply that CAGR
or another summary statistic is available. Reconciliation and independent
qualification are owned by the
[tail-selection follow-up](../FOLLOW_UPS.md#analytics-tail-method-selection).

## Observed choice and rationale

`Performance::value_at_risk`, `expected_shortfall` and the standard summary
select the [historical estimators](07-historical-tail-risk.md).
[Gaussian and Cornish–Fisher VaR](08-moments-gaussian-and-cornish-fisher.md)
are separately named, explicit alternatives; selecting either does not change
the summary's estimator. This record owns the accepted selection boundary and
rationale above, not the algorithms defined in those detailed records.
Broader summary policies retain their separate scope.

**Prior agreed direction (2026-10-11):** invalid Cornish–Fisher estimates
return an explicit error, rather than a per-ticker `NaN` or Gaussian
substitution. The current fallback is an implementation discrepancy against
that direction; its presence and regression test do not reopen the decision.

The [accepted global CF ordering rule](08-moments-gaussian-and-cornish-fisher.md#accepted-decision-global-cf-quantile-ordering)
now specifies structural admissibility and rejection without automatic repair.
The [classical CF plug-in definition](08-moments-gaussian-and-cornish-fisher.md#accepted-decision-classical-plug-in-cf)
separately selects the expansion inputs. The
[explicit-confidence contract](01-panels-windows-and-wire.md#accepted-decision-explicit-tail-confidence)
requires supplied finite `0.5 < c < 1` without an implicit 95% default.
These choices constrain the accepted selection policy above. The
[observation-period horizon convention](08-moments-gaussian-and-cornish-fisher.md#accepted-decision-observation-period-horizons)
is separately accepted.

The [signed-tail output contract](07-historical-tail-risk.md#accepted-decision-signed-tail-outputs)
is also accepted, preserving valid negative, zero and positive return
estimates. The selected summary method remains historical under the decision
above.

The [historical VaR/ES pair](07-historical-tail-risk.md#accepted-decision-historical-var-and-fractional-empirical-es)
is accepted as R-7 interpolated VaR and exact fractional empirical ES, with
their distinct finite-sample conventions. Summary selection is separately
accepted above. The separately accepted
[historical availability rule](07-historical-tail-risk.md#accepted-decision-historical-availability-and-small-tails)
permits any nonempty valid sample without a hard tail-count cutoff, with
undefined for empty history. It does not certify statistical tail accuracy.

The [Gaussian availability and failure rules](08-moments-gaussian-and-cornish-fisher.md#accepted-decision-gaussian-var-availability-and-failures)
require two returns, preserve constant-history point-mass quantiles and
distinguish numerical errors from unavailable history. Gaussian remains an
explicit alternative and is not substituted for unavailable CF estimates.

The algorithm rationales and evidence are recorded in the detailed owners
below. Their implementation and accuracy qualification remain separate from
the accepted product selection above.

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
| `summary(...)` | Historical VaR and ES; no model override. Full input signature follows accepted record 01. |
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

Fixed historical selection and its rationale are accepted above. Broader
summary policies and estimator qualification remain separately owned.
Implementation reconciliation and missing evidence are owned by the
[analytics follow-ups](../FOLLOW_UPS.md#analytics); the
[central follow-ups](../../FOLLOW_UPS.md#financial-methodology-audit) coordinate cross-domain work.
