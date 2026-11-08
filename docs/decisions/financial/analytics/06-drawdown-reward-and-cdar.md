# ANALYTICS-006: Calmar, Martin, Sterling, Burke, Pain and conditional drawdown risk

**Decision status:** Accepted for the complete metric aggregation, parameter,
availability and failure package below (2026-11-07). Path/episode construction
and broader summary policies retain the scope of their owning records.

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: drawdown reward and CDaR

**Accepted:** 2026-11-07.

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
  its observed drawdown. Empty CDaR is undefined.
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

[Martin's own Ulcer/UPI description](https://www.tangotools.com/ui/ui.htm)
defines RMS percentage drawdown and cash-adjusted return over that risk.
This API explicitly chooses decimal units and dated CAGR for its annual
reward. The [Kestner bibliographic entry](../../../REFERENCES.md#kestner2003)
records the correct 2003 edition as practitioner context, without asserting
that every Sterling variant matches this API.

The modified-Burke observation-count normalization follows the
[package authors' documented definition](https://search.r-project.org/CRAN/refmans/PerformanceAnalytics/html/BurkeRatio.html).
An episode-RMS alternative instead divides by episode count. With
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

## Implementation evidence

[drawdown.rs](../../../../finstack-quant/analytics/src/drawdown.rs), [performance/scalar.rs](../../../../finstack-quant/analytics/src/performance/scalar.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
