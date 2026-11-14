# VALUATIONS-EQUITY-AND-COMPOSITION-13: Autocallable seasoning, coupon memory and redemption

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-13`.
- **Decision status:** Accepted 2026-11-14 — complete dated autocallable seasoning, coupon memory, redemption and risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Filed terms and source inspected. Independent replay, redemption, dated cash, horizon and live risk examples checked. Production pricing, risk, replay and host qualification remain pending.

## Accepted decision for checked dated autocallable pricing and risk

1. **Use immutable single-underlying terms and one row per observation.**
   Require exact signed Money principal, a qualified positive strike-set
   level `S0`, one maturity redemption variant and an explicit memory
   choice where coupons exist. Each strictly ordered observation row
   has independently optional coupon and call terms. Specify
   last-row call eligibility the same way. Coupon fractions are finite
   and signed, and barrier ratios are finite and nonnegative. A
   payable row has one contractual payment date for its earned coupon
   and any called principal. An observation-only row needs no payment
   date. Use a separate maturity payment date for uncalled principal.
   Applicable row payments are nondecreasing, each from observation
   through maturity payment inclusive. Require qualified mapping of
   the actual term sheet, including applicable barrier rounding.
   [Accepted payoff terms](../models-monte-carlo/20-autocallable.md),
   [filed RBC issue terms](https://www.sec.gov/Archives/edgar/data/1000275/000095010325000733/dp223672_424b2-jpmeln550.htm).

2. **Replay qualified actual fixings before requesting live inputs.**
   Retain the established strike-setting fixing even before the first
   payoff observation. Today's spot can establish `S0` at an
   explicitly qualified strike-setting event. Use an explicit
   information cut and a unique authoritative positive fixing per
   required consumed row, including a published same-day fixing.
   Keep an unpublished required row pending. If the event lacks a
   qualified model state, its price is unavailable. Use one checked
   transition for replay and simulation. Replay the eligible prefix
   through first call and carry the consumed prefix, discrete downside
   touch, contingent memory and earned dated cash into the remaining
   valuation. Restore that checked state on each simulated leg.
   [Accepted fixing qualification](../conventions/0045-financial-clocks-and-date-boundaries.md),
   [accepted path preparation](12-path-gbm.md).

3. **Process the coupon first and the independent call second.**
   A fixing at or above the coupon level earns the current signed
   fraction plus eligible memory on the row payment date. A qualifying
   explicit zero coupon can release memory. An absent coupon leaves memory untouched.
   A miss accumulates only under elected memory terms. Test the call
   independently with inclusive equality. A literal zero call threshold
   triggers for positive spot and an absent threshold disables calling.
   The first call earns one principal on the row date, keeps previously
   earned coupons and ends future observations. Unreleased memory
   lapses at call or maturity after processing that row's coupon test.
   [Accepted sequence and thresholds](../models-monte-carlo/20-autocallable.md).

4. **Use the last contractual fixing and the selected redemption law.**
   At the last row process coupon and any enabled call before uncalled
   redemption. On survival freeze performance `x=S_final/S0` there.
   Capital protection pays `max(floor,p min(x,cap))` with finite
   nonnegative floor, finite signed participation and finite
   positive cap. Participation pays `1+p max(min(x,cap)−1,0)`
   with finite nonnegative participation and finite positive cap.
   Knock-in put pays par unless an eligible contractual fixing
   touched its finite nonnegative downside barrier inclusively.
   On touch it pays `max(1−max(k−x,0),0)` for finite positive
   strike ratio `k`. Monitor only contractual observations while
   uncalled, including replayed observations and the final fixing.
   Apply checked level comparisons and complete payoff arithmetic.
   [Accepted redemption formulas](../models-monte-carlo/20-autocallable.md).

5. **Resolve settlement separately for each earned leg.** Keep
   identity, signed amount, original due date, entitlement and payment
   or discharge status for each coupon and principal. Include an
   entitled unpaid same-day leg at factor one when the elected cash
   phase includes it. Exclude paid or discharged legs on evidence.
   Require qualified collection and valuation treatment for overdue
   outstanding amounts. A called or fully observed note prices its
   remaining claims directly on their actual dated cash basis. For
   a live note use the fixed-budget `atm_term_gbm` route under
   `monte_carlo_gbm` through the last required underlying fixing.
   Use maturity payment `P` as engine cash reference. For each included
   payment at `d`, calculate checked equivalent Money using
   `D(V,d)/D(V,P)`, then discount the total with `D(V,P)` once.
   Export actual earned legs with their status and require complete
   attribution for a full cashflow export. Simulated captures retain
   diagnostic provenance.
   [Accepted cash and dated discounting](../models-monte-carlo/20-autocallable.md),
   [accepted cashflow export](../valuations-framework/cashflow-export.md).

6. **Use selected-model risk and one horizon holder scenario.**
   Hold the contractual strike anchor, observed history and locked
   state across same-cut risks. Rebuild responsive ATM inputs and
   require checked paired stencils on complete logical groups. Apply
   a DV01 shock once per actual source across its carry and payment
   roles. Retire the `rho` alias for curve DV01. Known unpaid claims
   retain actual dated discount risk. For horizon theta require
   explicit opening and closing information and payment cuts, an
   elected held-opening-spot scenario for crossed observations, an
   explicit settlement policy and qualified closing active market
   views. Keep crossed hypothetical fixings private and labelled,
   preserve actual history and stop replay at first call. Derive
   signed period cash and the closing claim from the same replay,
   keeping pending rights in closing PV. Return `C+V_H−V_0`
   on the admitted holder basis. Reconcile raw price, risk, typed
   diagnostics and exact provenance through existing Rust, Python,
   WASM and schema routes.
   [Accepted selected-model risk](08-selected-risk-and-implied-vol.md),
   [accepted holder theta](../valuations-framework/carry-theta-breakeven.md).

## Observed behavior

### Choice and rationale

The adapter replays past discrete observations before simulating the remaining note. Coupon barriers are independent from autocall barriers, and payment-date DF ratios preserve coupon/redemption settlement lags.

### Scope and input domain

Nonempty increasing observations, matching barrier/coupon/payment arrays, nondecreasing payments between each observation and expiry. Initial levels are positive when supplied; any past or same-day observation requires an explicit initial level and exact fixing. Barrier ratios are nonnegative; coupons are finite signed return amounts.

### Algorithm

1. After expiry return zero. Replay all observations ≤ as-of. Update the discrete minimum; when the coupon barrier is met, release that coupon plus accumulated memory, retaining only payments after as-of. Otherwise accrue the coupon only when memory is enabled.
2. At first satisfied autocall barrier, return zero if paid, otherwise discount principal and all fixed unpaid coupons to their actual payment dates.
3. If every observation is past without autocall, compute final redemption from the last fixing and past observed minimum, payable at expiry. CapitalProtection is `max(floor, participation×min(Sfinal/S0,cap))`; Participation is `1+participation×max(min(Sfinal/S0,cap)−1,0)`; KnockInPut pays `max(1−max(strike_ratio−Sfinal/S0,0),0)` when observed minimum ≤ S0×barrier, otherwise one.
4. Otherwise simulate remaining observations under record 12, seeding historical minimum, memory and fixed coupons. Encode each payment as `Dpayment/Dexpiry` in the payoff and discount the resulting path total by Dexpiry. Final fixing freezes at the last observation even when expiry/payment is later.

### Selection, controls and conventions

`MonteCarloGBM` is the product default. Final barrier monitoring is discrete at contractual observation dates, not every simulation step. Spot risk pins omitted initial level; vega/volga and rate risks delegate to shared bump calculators. Theta-state construction fills newly crossed observations with the scenario spot and sorts them; that is scenario state, not historical data repair.

### Limitations, failures and alternatives

Unreleased memory coupons are not automatically paid at maturity unless their coupon barrier is met. Negative coupons/participation settings have variant-specific acceptance. Continuous knock-in monitoring and a different capital-protection payoff would be distinct methods. The current final-payoff names alone do not establish a particular issuer term sheet.

### Evidence

[structured-product reference](../../../REFERENCES.md#overhaus-2007-equity-derivatives); [test_seasoned.rs](../../../../finstack-quant/valuations/equity/tests/autocallable/test_seasoned.rs), [test_settlement_gap.rs](../../../../finstack-quant/valuations/equity/tests/autocallable/test_settlement_gap.rs) and [test_day_count_basis.rs](../../../../finstack-quant/valuations/equity/tests/autocallable/test_day_count_basis.rs) locate seasoning, fixed payment and clock regressions. Shared payoff tests independently assert simple deterministic paths but do not establish calibrated-market accuracy.

## Independent numerical checks

- With principal 1,000 and strike-set level 100, a fixing of 70
  misses the .80 trigger for a 4% coupon. A fixing of 80 at an
  explicit zero-coupon row releases 40 of memory. The subsequent
  fixing of 100 meets the 1.00 call threshold and misses the 1.10
  trigger for a 2% coupon. The residual memory lapses. The earned
  40 and 1,000 dated at payment factors .98 and .95 give PV 989.20.
  On the coupon date just after receipt, a held dated discount
  basis gives closing principal PV `1,000·.95/.98=969.387755102`
  and theta `40+969.387755102−989.20=20.187755102`.
- A second exact fixture with positive notional 1,000 includes a
  20 same-day earned coupon and pending called principal at .97.
  Its PV is 990 before coupon discharge and 970 afterward.
  An uncalled variant with observed inclusive .60 downside touch,
  final performance .85, strike ratio 1 and maturity factor .94
  gives 819 including the same coupon.
- Under the selected lognormal law, a one-row live note with
  spot and anchor 100, principal 1,000, coupon .02, coupon
  level 95, call level 102, half-year horizon, volatility .20,
  yield .01, fixing factor .99, row payment factor .98 and
  uncalled par payment factor .95 gives PV 975.2331331982.
  Its analytic fixed-anchor spot delta is 1.3574220758 and
  a separate 1-unit centered price stencil gives 1.3564139899.

Repository production pricing, payoff replay, risk and host suites
remain unrun for this review.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/autocallable/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/equity/src/autocallable/pricer.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/autocallable/metrics.rs)
