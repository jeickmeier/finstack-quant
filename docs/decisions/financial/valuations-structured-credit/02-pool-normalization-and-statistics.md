# VALUATIONS-STRUCTURED-CREDIT-02: Collateral normalization and pool statistics

- **Decision status:** Accepted 2026-11-15 — complete collateral normalization, cut-off par, statistics, concentration, tranche partition and reporting package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent balance-bridge, heterogeneous pool, weighted statistics and exact tranche-partition calculations checked. Production pricing, reporting and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for collateral normalization and pool statistics

1. **Resolve one dated collateral snapshot.** Use one authoritative source
   of asset rows, representative lines or typed instruments. Validate unique
   identities, common currency, balances and required terms before and after
   normalization. Keep the source and snapshot dates. A representative line
   expands one-to-one, with a qualified age anchor and explicit rate and
   amortization terms, including applicable floating index and spread.
   Resolve a closing-based integer age with checked direct month arithmetic
   from closing, preserving its source date at later valuations. Carry
   missing metadata as missing and keep normalized data subordinate to
   the authored source. Representative-line results approximate the stated
   collateral population and require qualification for heterogeneous pools.
   [Accepted dispatch and assumptions](01-dispatch-and-assumptions.md#accepted-decision-for-structured-credit-dispatch-and-assumptions).

2. **Reconstruct original par only for a qualified closed runoff pool.**
   Require complete, disjoint principal tallies. Add current nondefaulted
   par, gross defaulted par, prepayments and scheduled principal repayments.
   A retained defaulted balance already counted in gross defaults is
   excluded from current par in this bridge. Recovery cash is separate.
   Require explicit cut-off par for replenishing pools, new draws or
   incomplete histories. Match a qualified bridge exactly when both
   sources are available. Current par can exceed cut-off par after
   admitted additions. Use checked same-currency arithmetic.

3. **Use explicit populations and units for weighted statistics.** WAC
   weights positive performing fixed-rate par by contractual annual
   decimal coupon. WAS weights positive performing floating-rate par
   by its signed contractual spread in bp. Require a coherent rate
   kind and inputs. WAM weights outstanding par, including retained
   defaulted balances, by remaining contractual ACT/365F years with
   matured terms contributing zero years. Pool age uses outstanding
   par and completed calendar months from qualified age anchors, with
   a fractional weighted mean. Consumer-specific aging discretization
   stays with the consumer. An entire-population statistic is unavailable
   when required coverage is missing.
   [Accepted tenor and date arithmetic](../core-conventions/007-tenor-date-arithmetic.md).

4. **Require a sourced rating-factor profile for WARF.** Weight the
   qualified factors by positive performing par. Freeze the rating
   observations and mapping against the selected immutable profile and
   its source. Distinguish explicit `NR` from absent rating data. An
   explicitly qualified contractual unrated treatment can supply a
   factor. Incomplete coverage leaves full WARF unavailable. A required
   contractual calculation errors on missing inputs. Generic ratings
   have no implicit factor table.
   [Moody's WARF methodology](https://ratings.moodys.com/api/rmc-documents/420962),
   [Accepted rating semantics](../core-conventions/019-credit-ratings.md).

5. **Report concentration and effective obligor count.** Group positive
   outstanding par by qualified obligor ID for `1/sum(weight^2)`, using
   balance weights normalized over identified par. Label the measure
   effective obligor count and report the identified fraction of full
   par. Concentration tables weight full current par, retain explicit
   missing-key buckets and sort by descending balance and canonical
   key. Preserve checked currency amounts including qualifying undrawn
   commitments. Undefined ratios are unavailable. A measured zero is
   reported as zero.
   [HHI definition](https://www.justice.gov/atr/herfindahl-hirschman-index).

6. **Derive structural bands from original face and explicit loss order.**
   Use one checked same-currency original tranche face total. Derive
   common adjacent endpoints from cumulative original face in junior-to-
   senior loss order. The original stack defines the 0% to 100% scale.
   Retire authored percentages as an independent input. Sequential notes
   of the same grade require explicit order. Genuine pari-passu loss
   notes share a group band and have no individual disjoint bands.
   Admit live co-ranked pricing only under qualified waterfall, coverage
   and loss rules. Fail a requested percent view if rounding collapses
   a positive-width layer.
   [Waterfall claims](11-waterfall-claims-and-rounding.md),
   [Accepted coverage tests](12-coverage-and-cures.md#accepted-decision-for-coverage-tests-and-cure-diversion),
   [Loss recognition](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

7. **Use one resolved Rust result across consumers.** Apply the same
   dated populations and availability rules to direct queries, registered
   metrics and reports in Rust, Python and WASM. Propagate lookup, currency
   and arithmetic failures. A known tranche with an empty senior set
   has zero senior balance in its currency. An unknown tranche ID errors.

## Independent calculations

- Current nondefaulted par 75, gross defaults 8, prepayments 10 and
  scheduled principal 7 reconstruct cut-off par 100. Adding retained
  defaulted par 5 would give 105. Recovery cash does not enter the
  bridge. For two equal-face cohorts with monthly prepayment 0% and
  20%, retained par after two months is 82 on 100 original face,
  compared with 81 from one representative line at 10%.
- A pool with 60 fixed par at 4%, 40 floating at 200 bp, 30 fixed at 8%,
  20 defaulted floating, and 50 floating at 100 bp gives WAC 5.333333% and
  WAS 144.444444 bp. Remaining terms in the same order of 2, 1, 3, 0 and
  4 ACT/365F years give WAM 2.25 years. The first two positions are one
  obligor and the next two a second obligor. The last lacks an ID. Effective
  obligor count is 1.8 on 75% identified par. Full WARF is unavailable
  when the last position also lacks a qualified factor.
- With senior faces 50 and 20, mezzanine 25 and equity 5, explicitly
  sequential loss rank gives bands 50%–100%, 30%–50%, 5%–30% and
  0%–5%. Genuine equal senior loss rank gives one 30%–100% senior band.

## Observed implementation and scope

The engine converts representative lines to asset rows and builds a pool view for statistics and simulation. Instrument-backed pools follow a separate projection source. The following sections describe the observed paths. The accepted contract above governs reconciliation.

## Scope, algorithm and conventions

Rows, representative lines and instrument collateral are intended as exclusive authored sources. The current normalization also admits some aligned rows beside typed instruments and replaces them with derived rows. Representative lines carry weighted age/maturity/coupon/default attributes; normalization anchors origination at closing minus age and expands their configured terms into a row. Original balance is explicit when supplied, otherwise reconstructed from current balance plus cumulative defaults, prepayments and scheduled principal. Current validation compares supplied original par with raw current balance using a numeric tolerance.

Pool WAC weights fixed-rate assets only; weighted spread weights floating assets only and remains in basis points. Performing/defaulted treatment differs by statistic and is explicit in each iterator. WAM uses balance-weighted remaining Act/365F years, flooring already matured remaining term at zero. WARF uses canonical rating factors, treating missing ratings as `NR`; callers such as the metric registry return zero when no weighted statistic is available. Diversification score is inverse concentration, `1 / sum(w_obligor²)`, over identified obligors, not a rating-agency diversity-score table. Reports aggregate exposure by asset type, rating, sector, region and obligor, and sort concentrations deterministically.

Attachment/detachment percentages describe the note capital structure. If absent, they are derived from original note balances in reverse payment-priority order; explicit points must tile [0,100] and their thickness must agree with balance share within 0.5 percentage points. Payment priority is seniority then input order, with distinct sequential ranks. These points are not independently calibrated portfolio-loss boundaries.

## Controls, limitations and alternatives

Pool balance, currencies, representation exclusivity, original balance and representative-line shape are validated. Statistics of a representative pool inherit aggregation bias; instrument pools and unrated reinvestment cohorts can lack the row metadata used by some metrics. Empty/absent statistic fallbacks differ by public caller and must not be read as measured zero risk. `senior_balance` silently returns zero if checked currency summation fails; validation is therefore a prerequisite, not an interchangeable reporting policy.

Exact loan-by-loan rows are the supported alternative to representative lines. Exposure-weighted versus equal-name concentration, rating-agency diversity tables, and fully marked asset market values would be distinct methodologies; they are not interchangeable labels for the implemented inverse HHI.

## Implementation and validation evidence

- [structured_credit/types/pool.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pool.rs)
- [structured_credit/types/pool_report.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pool_report.rs)
- [structured_credit/types/pool_state.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pool_state.rs)
- [structured_credit/types/tranches.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/tranches.rs)
- [structured_credit/metrics/pool.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/pool.rs)

Existing evidence: [structured_credit/unit/components/pool_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/components/pool_tests.rs); [structured_credit/unit/attachment_derivation_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/attachment_derivation_tests.rs); [structured_credit/academic_golden.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/academic_golden.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [rating nomenclature](../../../REFERENCES.md#moodys-rating-symbols). These references identify the method family; they do not certify the preset parameters or every implementation branch.

Existing hand-formula WARF/WAS checks do not establish current agency parameter licensing, provenance or fit.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
