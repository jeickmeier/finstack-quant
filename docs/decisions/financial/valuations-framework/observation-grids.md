# VALUATIONS-FRAMEWORK-07 — Variance sampling and barrier monitoring grids

- Decision status: **Accepted — complete checked observation schedules and monitoring grids package, 2026-11-13**.
- Implementation status: **Observed, with reconciliation pending** across variance and barrier products, model routes and hosts.
- Validation status: Source and primary references inspected. Independent bounded schedule, sample and monitoring examples checked. Runtime, product-price and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked observation schedules and monitoring grids

1. **Compile one contractual variance schedule.** Require ordered raw
   endpoints `start<maturity`, a positive explicit cadence, a qualified
   observation calendar and business-day adjustment, and the applicable
   roll and terminal stub terms. A single calendar or an explicitly
   selected joint base/quote calendar must resolve by identity. The joint
   calendar is open only when both components are open. Keep it separate
   from settlement and from the authority for a fixing or session.
   Business-day cadence counts every `k`th open date from the adjusted
   first observation. Stop at the adjusted terminal date without seeking
   an unused later date. Include a shortened last interval only under
   admitted Short Back terms. Week, month and year cadence use the
   original roll anchor, explicit Natural or applicable EOM and the
   supported Short Back rule. EOM requires a month/year cadence and
   a raw month-end anchor. Adjust the resulting dates once. Reject
   irrelevant controls, unsupported stubs and missing calendar coverage.
   [Accepted rolling](../core-conventions/006-business-day-adjustments.md#accepted-decision-rolling-and-joint-calendar-conventions),
   [accepted schedule grids](../core-conventions/008-schedule-generation.md#accepted-decision-schedule-grids-events-and-failure-boundaries).

2. **Preserve observation identities, phases and counts.** The generated
   shared variance profile has one observation slot per actual date.
   Reject adjusted duplicates, reversals and collapsed required sample
   geometry. Preserve raw/adjusted dates, observation source and phase,
   and each slot's role, including a distinct anchor or terminal fixing
   where applicable. Unadjusted dates need a qualified observation on
   the literal date. Check calendar work and cap the resulting schedule
   at 100,000 dates inclusive. Contract-specific amendments, market
   disruptions and separate actual versus expected denominators need the
   owner's explicit rule and provenance. A missing source row never
   rewrites the expected schedule or produces a zero return by itself.
   Cboe's variance futures illustrate a listing close anchor, a special
   terminal opening quotation and an exchange-owned disruption rule.
   [Cboe contract specification](https://cdn.cboe.com/resources/participant_resources/SP_500_Variance_Futures.pdf).

3. **Require explicit contractual annualization.** Supply one finite
   positive annualization factor in target samples per year. Preserve
   an applicable contract's frequency convention through that value.
   A contract specifying 252 trading days per year and two-business-day
   sampling can supply `126`. The framework uses the supplied factor
   across holidays, stubs and partial realization. Remove the implicit
   252 and the framework's automatic tenor-to-factor recipe. Use the
   same factor for realized calculations and compatible forecast and
   risk measures. Keep annualization, expected sample count, elapsed
   model time and quotation units distinct.
   [Accepted realized variance](../core-numerics/realized-variance.md#accepted-decision-explicit-log-returns-and-realized-variance).

4. **Count complete estimator samples on the agreed layout.** With `N`
   ordered closes, close-to-close has `N−1` target intervals. Each needs
   its two actual adjacent observations. In a complete OHLC layout
   count every target bar, including the first. Require the separately
   identified immediately preceding close for OHLC close-to-close and
   Yang–Zhang. Range methods use every complete bar, with no anchor.
   Resolve the contractual bar window and a common price, adjustment and
   clock basis. A daily bar sampled weekly is still a daily bar. The
   final contract requires at least one target for close-to-close and
   range methods, and at least two complete target bars for Yang–Zhang.
   Check the full schedule's capacity to supply that layout before
   returning any fraction, including at maturity. A partial Yang–Zhang
   history may contribute one target to progress with its estimator
   still unavailable. Keep progress separate from estimator availability.

5. **Resolve publication before counting or settling.** At the frozen
   valuation phase, require exact eligible observations and their
   authorized source versions. Count only completed sample identities,
   using their required close or complete bar and any needed predecessor.
   Keep a qualified first close when it stands alone. Missing required
   eligible data or unknown qualification errors. An explicitly pending
   input remains pending even on the same civil day. Preserve gaps and
   their identities. A forecast that needs a contiguous history must
   decline an unresolved gap. A live spot used in the forecast has its
   own role. Report `completed/contractual` and the integer counts on
   the admitted common denominator. Return one only when the required
   sample set is complete. Keep final observation, payable settlement
   and settled status distinct. A qualified known unpaid claim avoids
   unused forward, spot and volatility queries. A confirmed settled
   position may skip inactive observations after contract admission.
   [Qualified exact observations](../core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings),
   [Cboe observation timing](https://www.cboe.com/document/tech-spec/document/technical-specifications/sp-500-variance-futures-variance-calculator-user-guide/).

6. **Use the shared schedule in variance pricing and risk.** Equity and
   FX pricing, expected and remaining variance, expiry, settlement, risk
   and export use the same admitted observation basis. Forecasts and
   blending rules remain with the product owner. A count fraction alone
   supplies no estimator forecast or vega. Additive per-sample methods
   can qualify count-weighted contributions. Yang–Zhang uses full-window
   centered terms and a count-dependent weight, so a live forecast needs
   a separately qualified method. Reject unsupported live estimator
   branches. Retain equity replication and FX interval forecast as
   separate product decisions.
   [Equity variance](../valuations-equity-and-composition/15-variance-swap.md),
   [FX variance](../valuations-fx-and-exotics/fx-variance.md).

7. **Require explicit barrier observations.** Admit the accepted
   continuous window with explicit start and inclusive expiry, or an
   explicit nonempty, strictly ordered discrete schedule no later than
   expiry. Discrete dates define their observation window without a
   separate start. Monitor discrete expiry only when scheduled. Resolve
   the exact observable and observation phase at each eligible event.
   Remove the generic Continuous default. Apply inclusive touch at an
   eligible observation under the accepted barrier classification.
   Reject inconsistent terms and unsupported phase or window models.
   [Barrier classification](../core-conventions/020-barrier-classification.md#accepted-decision-barrier-classification-touch-and-conditional-payment-timing),
   [Broadie, Glasserman and Kou on discrete monitoring](https://www.columbia.edu/~mnb2/broadie/Assets/bgk_mf.pdf).

8. **Resolve historical monitoring and cash before new path work.**
   Identify an explicitly fresh window or require qualified no-hit
   coverage through the effective phase for elapsed monitoring. Resolve
   any remaining eligible origin observation separately. A past hit
   remains latched with its event provenance. Apply earned intrinsic,
   rebate and settlement status on the admitted product and model basis.
   Resolve a known hit or completed monitoring window first and run
   only any remaining dynamics the resulting claim requires. Require
   qualified known data for a past observation, with an explicit owner
   policy for still-pending past information. A simulated horizon can
   use qualified scenario observations with hypothetical provenance.
   Held spot alone supplies neither authoritative history nor evidence
   of payment. Product owners retain entitlement and cutoff rules.
   [Accepted dated barrier boundary](../models-analytic/barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash).

9. **Bind live events to exact stored nodes.** Map dates with the
   selected model clock and full context. Keep civil event identity
   separate from numeric time, including at the origin. Form the exact
   knot union, then find each event by exact equality on the stored
   grid. Remove the tolerance lookup. Close distinct times remain
   distinct. Equal-time events retain their identities when the selected
   route can honor them. Coalesce observations only when the terms make
   them interchangeable. Reject a clock collapse when distinct event
   states are required. Continuous monitoring tests its start node on
   its own and examines only later adjacent monitored cells. Insert
   required model and rebate discount knots without adding discrete
   barrier observations. Resolve an eligible origin-only state directly.
   Live Monte Carlo requires a positive horizon, explicit work and the
   accepted model-specific execution controls.
   [Exact simulation grids](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots),
   [Monte Carlo monitoring](../models-monte-carlo/18-barrier-monitoring.md#accepted-decision-for-checked-monte-carlo-barrier-monitoring-and-rebates).

10. **Share event resolution and retain model ownership.** Reconcile
    asset GBM, Heston QE-M, FX GBM and applicable analytic/PDE barrier
    routes on the same dated eligibility. Each route retains its own
    supported windows, crossing, first passage and payoff behavior.
    Use the accepted GBM cell law or explicitly qualified Heston bridge
    approximation when applicable. Discrete lookback can reuse exact
    date binding with its own extrema and history contract. Prepare and
    validate in Rust, including restore, and reconcile existing native,
    registry, risk, diagnostic, Python and WASM instrument routes and
    wire schemas, including the WASM FX variance and barrier envelope
    wrappers. Keep prepared grids Rust-only. Check actual counts
    and propagate precise construction, missing data, model and range
    errors. Qualify the actual route before claiming pricing accuracy.
    [Conditional continuous crossings](https://business.columbia.edu/sites/default/files-efs/pubfiles/4311/one-step_survival.pdf),
    [accepted lookback extrema](../models-monte-carlo/19-lookback-monitoring.md#accepted-decision-for-checked-monte-carlo-lookback-extrema).

## Observed behavior

### Choice, scope, and rationale

The common observation adapters distinguish contractual sample counts from elapsed calendar time and require simulation grids to contain barrier observation dates. Variance payoff estimators and barrier crossing kernels are implemented in core/models, outside these adapters. This record covers the date-to-sample/time mapping that materially changes those results.

### Algorithm and selection

`variance_observation_dates` resolves a strict single calendar or joint base/quote calendar. Daily tenors mean business-day steps: adjust start/end, walk the calendar by tenor count and explicitly append adjusted maturity if the last step missed it. Weekly/monthly/yearly dates come from the core schedule builder with Short Back stub and caller-selected end-of-month, then each is business-day adjusted. Sort and deduplicate all adjusted dates. Zero tenor is rejected. The daily branch does not independently reject reversed start/maturity before appending its endpoint; callers must establish the date range.

Annualization is `12/month_count` for month/year tenors, `52/week_count` for weeks, and `trading_days_per_year/day_count` for days. It is contractual frequency annualization, not measured spacing after holidays. `realized_sample_fraction` counts observations on `as_of` as realized. Close-to-close and Yang–Zhang subtract one anchor from both realized and total observations; Parkinson, Garman–Klass and Rogers–Satchell count bars. Empty schedule gives zero; as-of at/after final observation gives one before checking sample count. Otherwise a zero sample denominator fails. Thus a one-date close-to-close schedule can return one at maturity despite having no return interval.

Barrier `Monitoring` defaults to Continuous. Continuous monitoring adds the optional future start time, or time zero if monitoring already started, as a required simulation knot. Discrete monitoring converts dates on/after as-of to times using the instrument day count. `PathDependentPricerConfig::build_time_grid` inserts required times into the simulation grid; each is found with tolerance `1e−12×max(|t|,1)`, otherwise internal error. The adapter then supplies continuous start-step or explicit discrete observation-step indices to the model payoff. It does not reconstruct prior barrier breaches.

### Limitations and alternatives

Calendar-day daily sampling, actual-spacing annualization and time-weighted seasoned variance are different methods, not selected alternatives here. Barrier continuous monitoring accuracy depends on the chosen process/payoff crossing method and simulation grid; the enum alone is not evidence of exact continuous monitoring.

## Evidence

- [variance_observations.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/variance_observations.rs): algorithms and inspected inline tests `day_tenor_uses_holiday_calendar_and_business_endpoints`, `monthly_schedule_preserves_end_of_month_anchor`, and unknown-calendar rejection.
- [monitoring.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/monitoring.rs): required-knot and observation-step mapping; core/models own grid construction and crossing probabilities.
- [Demeterfi et al.](../../../REFERENCES.md#demeterfi-1999-volatility-swaps), [Broadie–Glasserman–Kou](../../../REFERENCES.md#broadie-glasserman-kou-1997), and [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) provide canonical conceptual context, not direct acceptance of these calendar rules.
- The live variance routes are [equity](../../../../finstack-quant/valuations/equity/src/variance_swap/pricer.rs) and [FX](../../../../finstack-quant/valuations/fx/src/fx_variance_swap/pricer.rs). Barrier use includes [asset GBM](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/pricer.rs), [asset Heston](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/heston_mc_pricer.rs), [asset PDE](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/pde_pricer.rs) and [FX GBM](../../../../finstack-quant/valuations/fx/src/fx_barrier_option/pricer.rs).

### Independent review evidence

- With July 4, 2025 closed and weekends excluded, a two-business-day
  cadence from July 3 to July 9 reaches July 3 and July 8, then adds
  July 9 under Short Back. An authored annualization of 126 applies
  across the stub. Close-to-close progress on July 8 is `0/2` before
  the close qualifies and `1/2` afterward. Under monthly EOM and
  Following, the raw May 31 and June 1, 2025 dates both map to June 2.
  The generated one-slot-per-date profile rejects the collision.
- Under an explicitly authorized Cboe-style zero-return disruption
  rule, closes `100,110`, a disrupted slot, and terminal opening value
  `99` give three contractual samples and annual decimal variance
  about `1.6955289653` at factor 252. Dropping the slot changes the
  denominator to two and the result to `2.5432934479`. The source
  contract has its own quote scale and rounding.
  [Cboe contract specification](https://cdn.cboe.com/resources/participant_resources/SP_500_Variance_Futures.pdf).
- Four flat OHLC bars with overnight logs `[0,a,0,a]` have zero
  intraday and range components. Their whole-window Yang–Zhang
  estimate before annualization is `a²/3`. Two separate two-bar
  estimates are each `a²/2`, so weighting these by sample count
  changes the estimator.
- Both `.5` and its next representable float belong in the exact
  grid. The old tolerance can resolve the latter to `.5`. Distinct
  January 30 and January 31 dates can share a 30E/360 coordinate.
  That equality cannot establish a common eligible observation.
- An up barrier at `105` with origin spot `112`, later monitoring
  start `99` and expiry `102` misses along the deterministic affine
  log path within the monitored window. Testing at origin would hit.
  For a separate qualified stochastic cell with endpoint spots `90`
  and `95`, up barrier `100` and log variance `.04`, both discrete
  observations miss while the log bridge crossing probability is
  about `.7632158454`.
  [Conditional continuous crossings](https://business.columbia.edu/sites/default/files-efs/pubfiles/4311/one-step_survival.pdf).

Production qualification requires dated calendar and fixing fixtures,
observation and settlement phase tests, independent admissible forecasts,
barrier window and history cases, applicable model price evidence, and
runtime and host checks.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
