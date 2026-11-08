# ANALYTICS-006: Calmar, Martin, Sterling, Burke, Pain and conditional drawdown risk

**Decision status:** Accepted for the complete metric aggregation, parameter,
availability and failure package below (2026-11-07). Path/episode construction
and broader summary policies retain the scope of their owning records.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, scope and conventions

Reward/drawdown ratios combine the dated compounded CAGR with a path or episode denominator. CAGR's current ACT/365.25 default conflicts with accepted ACT/365F as described in [record 02](02-wealth-and-annualization.md). Drawdown depths are negative decimal fractions; risk denominators are positive magnitudes. Cash parameters on Sterling/Burke/Pain are annual rates subtracted directly from dated CAGR, unlike the geometric per-observation cash conversion used by arithmetic Sharpe.

The [accepted cash-rate decision](02-wealth-and-annualization.md#accepted-decision-cash-rates)
owns cash representation, subtraction and validation. Invalid-cash NaN
propagation below requires reconciliation.

## Accepted decision: drawdown reward and CDaR

**Accepted:** 2026-11-07. **Implementation:** Partial; selected source
inspected at `5140b499cc`. Martin cash, Burke selection/normalization,
Sterling inputs, empty-history outcomes and numerical-failure propagation
require reconciliation. No runtime estimator checks were executed.

This package covers method, population, parameter, availability and failure
choices in this record. Previously accepted cash conventions, ACT/365F dated
CAGR, actual initial valuation dates, cash-adjusted Martin, all-episode
modified Burke and zero-risk division rules remain constraints.

Let `G` be available dated CAGR under record 02, `R` explicit annual-effective
cash, `T` total compounded return, `d_i <= 0` the observed percentage drawdown
path, `N` its return-observation count, `M = max(abs(d_i))`, and `D_j` the
detected episode valley depths. These decisions concern aggregation
over path/episode inputs, rather than choosing their construction rules.

| Metric | Accepted complete convention |
| --- | --- |
| Calmar | `G / M` over the caller-selected active analysis window. No automatic three-year truncation or cash subtraction. |
| Martin | `(G - R) / sqrt(sum(d_i^2) / N)`. The cash adjustment is already agreed; specify the endpoint RMS denominator explicitly. |
| Recovery factor | `T / M`, using compounded total return without annualization or cash subtraction. It does not measure recovery duration. |
| Sterling | `(G - R) / mean(abs(D_j))` for the deepest up to explicitly requested `n` episodes. No fixed ten-percentage-point penalty or annual-block-maxima variant. |
| Modified Burke | `(G - R) / sqrt(sum(D_j^2) / N)` over all detected episodes. `N` counts return observations, not episodes. All-episode selection remains agreed; observation-count normalization is accepted here. Remove the obsolete episode-count parameter. |
| Pain ratio | `(G - R) / (sum(abs(d_i)) / N)`, including zero drawdowns. |
| CDaR | Signed exact fractional empirical lower-tail ES of the observed percentage drawdown path at explicit confidence `c`; include zero drawdowns and preserve the already accepted nonpositive sign. |

### Population and parameter rules

- For Martin, Pain and CDaR, use one equally weighted drawdown at each active
  return-ending observation, including observed zeros and observations within
  an unrecovered episode. Exclude an extra synthetic initial-zero sample.
  Calendar gaps do not change these observation weights. For modified Burke,
  `N` counts that same complete return history, including zero-return periods.
- Sterling and Burke include an open episode's observed valley. Sterling
  requires explicit integer `n >= 1`; reject zero and remove the implicit
  five-episode host default. If fewer than `n` episodes exist, retain all
  available episodes and divide by the actual retained count. No zero padding
  or minimum-episode-count gate is introduced.
- Cash remains explicit only where used: Martin, Sterling, Burke and Pain.
  Calmar, recovery and CDaR require no unused cash parameter. Validate cash,
  confidence and the Sterling count before per-ticker availability.
- CDaR uses `m = N * (1 - c)`: average exactly that much worst-tail mass,
  fractionally weighting the boundary observation. Ties do not expand the
  selected mass. Permit any positive `m`; below one observation, the result is
  the deepest observed drawdown. No hard tail-count cutoff is introduced.
- Drawdown denominators and CDaR receive no annualization or horizon
  multiplier. CAGR-based numerators use their already accepted annual clock;
  recovery uses total compounded return.

### Availability and calculation failures

- At least one valid active return/drawdown observation is required. An empty
  or unavailable history yields undefined per ticker. CAGR-based ratios also
  require available dated CAGR under record 02, with a known actual initial
  valuation date and positive elapsed span. This package adds no one-year or
  three-year reporting minimum.
- For available history with zero detected episodes, Sterling and Burke have
  zero episode-based risk. Apply the accepted division rules after checking
  reward availability: finite nonzero reward/zero risk gives signed infinity;
  zero reward/zero risk is undefined. This does not change raw path drawdowns
  or infer zero maximum drawdown from the episode list.
- Available all-zero drawdowns give CDaR zero. A singleton CDaR sample returns
  its observed drawdown. Empty CDaR becomes undefined, replacing the current
  zero substitution.
- Invalid requests retain explicit validation errors. Nonfinite computed
  reward, path or risk denominator, or a nonfinite ratio computed with a finite
  positive denominator, is a calculation error rather than missing-history NaN, zero risk or
  a substituted estimator. Intentional signed infinity from valid finite
  nonzero reward and exactly zero measured risk remains available. Do not add
  another near-zero denominator cutoff in these metric compositions.
  Request/calculation errors fail the affected metric call; unavailable
  histories remain undefined per ticker. Whole-summary failure behavior is
  separately scoped.

### Variant interpretation and scope

The modified-Burke observation-count normalization follows the
[package authors' documented definition](https://search.r-project.org/CRAN/refmans/PerformanceAnalytics/html/BurkeRatio.html).
The current episode-RMS denominator instead divides by episode count. With
100 return observations and four episodes, the accepted ratio is five times
the episode-RMS ratio for identical reward and depths. Observation-normalized
modified Burke is sampling-dependent; it must not be presented as invariant
to resampling or universally comparable across frequencies.

The accepted Sterling method is specifically the worst-episode-average
variant. The
[PerformanceAnalytics Calmar/Sterling documentation](https://search.r-project.org/CRAN/refmans/PerformanceAnalytics/html/CalmarRatio.html)
describes a different Sterling denominator, so no cross-library Sterling
equivalence is claimed. Calmar's caller-selected window also needs an explicit
label rather than an implication of automatic 36-month selection.

CDaR is an empirical compounded-percentage-drawdown reporting variant.
[Chekhlov, Uryasev and Zabarankin, section 3](https://www.math.columbia.edu/~chekhlov/ChekhlovUryasevZabarankin--03-2004.pdf)
motivate equally weighted endpoint tail averaging and exclusion of the
initial zero, but study absolute drawdowns of uncompounded returns. Their
optimization properties are not established here for the percentage variant.
For observed drawdowns `[0, -10%, -20%, 0, -5%]` and explicit 70% confidence,
tail mass is 1.5 and CDaR is approximately `-16.6667%`.

The [accepted record-05 contract](05-drawdown-paths-and-episodes.md#accepted-decision-drawdown-paths-and-episodes)
owns wealth/peak construction, rebased windows, strict episode recovery,
date alignment, metadata removal, durations and standalone path-measure
availability. This record owns the reward/CDaR aggregation over those inputs.
Broader summary signatures, non-tail summary selection and whole-report
partial-output behavior also remain outside this record. Source inspection,
primary references and illustrative arithmetic do not establish numerical or
statistical accuracy; independent implementation qualification remains open.
Reconciliation is owned by the
[record-06 follow-up](../FOLLOW_UPS.md#analytics-drawdown-reward-and-cdar).

## Implemented algorithms and selection

- Calmar = CAGR / absolute maximum drawdown over the current active range. No fixed Young-style 36-month lookback is enforced; the caller must select that window if required.
- Martin = CAGR / Ulcer index. The current public method takes no cash-rate argument. This conflicts with the previously agreed cash-adjusted Martin direction; the omission is not a proposed default.
- Recovery factor = compounded total return / absolute maximum drawdown; it is not annualized.
- Sterling = (CAGR−cash) / absolute mean of the n deepest episode valleys. It does not add a fixed 10-percentage-point penalty or average annual calendar maxima.
- Burke first selects the n deepest episode valleys, then computes `(CAGR−cash)/sqrt(sum(depth²)/m)`, m being the actual number retained. This is a selected-episode RMS denominator. The accepted rule above requires all episodes and normalization by return-observation count, so both selection and normalization require reconciliation. No episodes currently returns zero before division; invalid cash or NaN CAGR still yields NaN.
- Pain ratio = (CAGR−cash) / mean absolute drawdown across observation dates.
- CDaR takes the empirical expected shortfall of all drawdown observations at supplied confidence, including zero-drawdown dates. It uses exact fractional tail mass as in [record 07](07-historical-tail-risk.md); output is a signed nonpositive drawdown average. It is neither the average of worst episode minima nor a path-simulation optimization constraint. Empty drawdown input returns zero.

Other zero-risk ratios use the common finite-input division helper: signed infinity for nonzero/zero and zero for 0/0. In particular a positive all-up path can produce infinite Calmar/Martin. Nonfinite cash is propagated before zero-risk outcomes. `n=0` is not rejected: Sterling then has a zero denominator and Burke an empty-episode result of zero. Confidence alone is validated strictly inside (0,1).

Zero substitutions above require reconciliation with the
[accepted history/zero-risk outcomes](04-return-risk-ratios.md#accepted-decision-insufficient-history-and-zero-risk).
Episode selection, Sterling's explicit positive count and CDaR methodology
are accepted above; conflicting source behavior requires reconciliation.

CDaR confidence follows the
[accepted explicit-confidence contract](01-panels-windows-and-wire.md#accepted-decision-explicit-tail-confidence):
supplied finite `0.5 < c < 1`, without an implicit 95% default. The observed
wider validator range above requires reconciliation. CDaR's endpoint
population, fractional ES estimator and empty-history result are accepted
in this record. The
[accepted tail-output sign convention](07-historical-tail-risk.md#accepted-decision-signed-tail-outputs)
retains nonpositive CDaR, including zero for a valid observed path with no
drawdown; it does not authorize a zero result for unavailable history.

The [accepted historical return VaR/ES formulas](07-historical-tail-risk.md#accepted-decision-historical-var-and-fractional-empirical-es)
own the shared fractional empirical tail definition. This record separately
accepts applying that definition to its drawdown population and specifies
CDaR availability.

## Rationale, alternatives and limitations

The implemented ratios describe reward per historical path distress but mix two clocks: elapsed-date CAGR and equally weighted observed drawdowns. Choice of active-window rebasing, episode count and missing initial date materially affects outputs. Existing reference names do not settle variants such as Sterling's penalty or original versus modified Burke. Supported parameters are cash, n where exposed and CDaR confidence; there is no optional Martin cash input or all-episodes Burke mode selected automatically.

The complete accepted package requires implementation and host-contract reconciliation, rather than renewed choice between its variants. Record 05 still owns path construction and episode mechanisms. Time-weighted underwater risk, annual-block drawdowns and simulation-based conditional drawdown distributions are not selected by this package.

## Implementation and validation evidence

- [Public metric composition](../../../../finstack-quant/analytics/src/performance/scalar.rs) and [episode/ratio helpers](../../../../finstack-quant/analytics/src/drawdown.rs) establish actual selection.
- Inspected [audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) `burke_uses_the_worst_episode_depths_without_date_metadata` explicitly protects n=1/2/3/5 RMS outcomes for 10%/20%/30% episodes. It is hand-formula evidence for the current conflicting behavior. Cash/zero-risk regressions and tied/fractional CDaR cases were inspected; none was run.
- Canonical references: [Calmar](../../../REFERENCES.md#youngCalmar1991), [Martin/Ulcer](../../../REFERENCES.md#martinUlcer1987), [Sterling variants](../../../REFERENCES.md#kestner1996), [Burke](../../../REFERENCES.md#burke1994), [CDaR](../../../REFERENCES.md#chekhlov2005). Source-to-variant and application-domain reconciliation remains incomplete.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
