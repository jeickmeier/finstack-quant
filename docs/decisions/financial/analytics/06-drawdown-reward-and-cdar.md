# ANALYTICS-006: Calmar, Martin, Sterling, Burke, Pain and conditional drawdown risk

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted directions remain reconciliation requirements.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice, scope and conventions

Reward/drawdown ratios combine the dated compounded CAGR with a path or episode denominator. CAGR's current ACT/365.25 default conflicts with accepted ACT/365F as described in [record 02](02-wealth-and-annualization.md). Drawdown depths are negative decimal fractions; risk denominators are positive magnitudes. Cash parameters on Sterling/Burke/Pain are annual rates subtracted directly from dated CAGR, unlike the geometric per-observation cash conversion used by arithmetic Sharpe.

## Implemented algorithms and selection

- Calmar = CAGR / absolute maximum drawdown over the current active range. No fixed Young-style 36-month lookback is enforced; the caller must select that window if required.
- Martin = CAGR / Ulcer index. The current public method takes no cash-rate argument. This conflicts with the previously agreed cash-adjusted Martin direction; the omission is not a proposed default.
- Recovery factor = compounded total return / absolute maximum drawdown; it is not annualized.
- Sterling = (CAGR−cash) / absolute mean of the n deepest episode valleys. It does not add a fixed 10-percentage-point penalty or average annual calendar maxima.
- Burke first selects the n deepest episode valleys, then computes `(CAGR−cash)/sqrt(sum(depth²)/m)`, m being the actual number retained. This is a selected-episode RMS denominator. The prior direction requires modified Burke over **all** episodes, so the n-based selection is an implementation discrepancy. No episodes returns zero before division; invalid cash or NaN CAGR still yields NaN.
- Pain ratio = (CAGR−cash) / mean absolute drawdown across observation dates.
- CDaR takes the empirical expected shortfall of all drawdown observations at supplied confidence, including zero-drawdown dates. It uses exact fractional tail mass as in [record 07](07-historical-tail-risk.md); output is a signed nonpositive drawdown average. It is neither the average of worst episode minima nor a path-simulation optimization constraint. Empty drawdown input returns zero.

Other zero-risk ratios use the common finite-input division helper: signed infinity for nonzero/zero and zero for 0/0. In particular a positive all-up path can produce infinite Calmar/Martin. Nonfinite cash is propagated before zero-risk outcomes. `n=0` is not rejected: Sterling then has a zero denominator and Burke an empty-episode result of zero. Confidence alone is validated strictly inside (0,1).

## Rationale, alternatives and limitations

The implemented ratios describe reward per historical path distress but mix two clocks: elapsed-date CAGR and equally weighted observed drawdowns. Choice of active-window rebasing, episode count and missing initial date materially affects outputs. Existing reference names do not settle variants such as Sterling's penalty or original versus modified Burke. Supported parameters are cash, n where exposed and CDaR confidence; there is no optional Martin cash input or all-episodes Burke mode selected automatically.

The agreed Martin/Burke changes require implementation and host-contract reconciliation, not renewed choice between variants. Time-weighted underwater risk, annual-block drawdowns and simulation-based conditional drawdown distributions are not implemented here.

## Implementation and validation evidence

- [Public metric composition](../../../../finstack-quant/analytics/src/performance/scalar.rs) and [episode/ratio helpers](../../../../finstack-quant/analytics/src/drawdown.rs) establish actual selection.
- Inspected [audit regressions](../../../../finstack-quant/analytics/tests/audit_regressions.rs) `burke_uses_the_worst_episode_depths_without_date_metadata` explicitly protects n=1/2/3/5 RMS outcomes for 10%/20%/30% episodes. It is hand-formula evidence for the current conflicting behavior. Cash/zero-risk regressions and tied/fractional CDaR cases were inspected; none was run.
- Canonical references: [Calmar](../../../REFERENCES.md#youngCalmar1991), [Martin/Ulcer](../../../REFERENCES.md#martinUlcer1987), [Sterling variants](../../../REFERENCES.md#kestner1996), [Burke](../../../REFERENCES.md#burke1994), [CDaR](../../../REFERENCES.md#chekhlov2005). Source-to-variant and application-domain reconciliation remains incomplete.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[analytics follow-ups](../FOLLOW_UPS.md#analytics).
