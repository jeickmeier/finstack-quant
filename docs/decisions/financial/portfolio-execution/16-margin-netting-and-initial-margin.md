# PORTFOLIO-EXECUTION-016 — Margin membership, sensitivity netting and initial-margin dispatch

- **Decision status:** Accepted 2026-11-16 — complete scoped netting, explicit IM dispatch, method-qualified inputs, strict coverage and provenance package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent partition, calculation-currency, net-to-gross and gross proxy examples verified. Runtime, host and model qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio netting and initial margin

1. **Bind each applicable holding to an exact netting scope.** Bilateral
   scope requires the legal counterparty and applicable agreement and
   netting partition. Cleared scope requires the exact CCP and applicable
   account, clearing member or service partition. Resolve one dated terms
   version. Reject missing, ambiguous or conflicting required membership.
   Keep collateral account identity separate. Exclude only established
   inapplicability.
   [Margin identity](../valuations-framework/margin-bridge.md#accepted-decision-for-checked-margin-repricing-and-identity).

2. **Select IM before extracting inputs.** Each scope elects no IM with a
   reason, SIMM with an exact edition and parameter source, a qualified
   Schedule profile or a named CCP route. Bind the valuation and information
   cut, applicable horizon, model, settings, provenance and currencies.
   Retire implicit SIMM and CCP selection, first-member term inheritance
   and unused SIMM work. Unsupported methods require their dedicated owner.

3. **Net SIMM within the elected scope and product class.** Apply signed
   holding size once to complete owner-qualified sensitivities. Retain
   units, exact factor, vertex, subcurve and applicable credit identity.
   Accumulate compatible signed risk with checked compensated arithmetic.
   Separate calculation and reporting currencies. Require qualified FX
   sensitivities on the elected calculation basis. An empty FX delta map
   does not establish valid currency remapping. Apply the elected MPOR rule
   once. Bundled `v2.6` is a historical USD calculator. An unavailable
   requested edition returns a typed error.
   [SIMM sensitivity ownership](../margin-collateral-im/03-sensitivity-inputs.md).

4. **Run Schedule IM on complete qualified inputs.** Use the elected
   profile's effective notional, asset class, maturity convention and
   signed replacement marks before collateral terms. Size notional by
   absolute holding and marks by signed holding. Compute gross charges and
   apply the profile's net-to-gross ratio once to the eligible set. Require
   an explicit rule for zero gross replacement cost. Calendar expiry alone
   is insufficient evidence of exposure extinguishment. Remove the generic
   square-root MPOR rescaling in the adapter.
   [Schedule netting](../margin-collateral-im/13-schedule-net-gross.md).

5. **Separate supplied CCP margin and elected proxies.** Admit a qualified
   external amount already sized for its exact account, service, cut,
   currency and horizon. An expressly elected exposure-rate proxy uses
   qualified unsigned exposure and absolute holding size, then sums
   standalone amounts without offsets. Label the proxy as an estimate.
   Remove substring routing, automatic proxy fallback and blanket MPOR
   scaling.
   [CCP inputs and proxy](../margin-collateral-im/14-ccp-external-and-proxy.md).

6. **Default to strict coverage across methods.** Shared admission failures
   always abort. Explicit diagnostic mode may retain complete independent
   results with typed holding issues. Withhold an affected set's IM and
   any headline requiring it. Distinguish explicit no IM, a computed zero,
   an inapplicable contribution and an unavailable calculation.

7. **Preserve checked amounts and method provenance.** Reduce Money in
   deterministic holding and scope order. Carry method, edition, parameter
   source, effective date, currency, MPOR, approximation status and
   component coverage through the existing hosts. Keep gross model IM,
   contractual collateral and transfers distinct. Record 017 owns VM,
   collateral account arithmetic and result restoration.
   [VM and collateral](17-margin-collateral-and-wire.md).

## Independent calculation checks

- At a one percent Schedule charge, two USD 10,000 notionals with marks
  of `+100` and `−60` give gross IM of 200, net-to-gross ratio of `.4` and
  IM of 128. Omitting the second holding gives 100. With no positive
  replacement cost, an elected ratio of one gives 200 and a ratio of zero
  gives 80 on the same gross IM.
- In a one-factor example, equal and opposite weighted sensitivities of
  `+100` and `−100` cancel in one qualified product class. When split across
  product classes their separate charges sum to 200.
- EUR 100 at `1.25 USD/EUR` acquires USD 1.25 of translation delta for a
  positive one percent FX move even if the original EUR-basis risk vector
  has no EUR row. Two equal and opposite holdings of two lots each require
  four times the standalone per-lot amount under the gross proxy.

## Observed choice and scope at the audit revision

The portfolio margin adapter groups eligible instruments into netting sets and delegates SIMM, schedule or clearing-house calculations to the margin crate. It applies signed holdings to sensitivities and absolute holdings to standalone collateral requirements. It is not a regulatory certification: product proxies and incomplete extraction remain explicitly relevant.

## Membership and sensitivity stages

Only instruments exposing `as_marginable()` and a netting-set ID are members. Validate each supplied OTC margin specification. Within a set, supplied CSA and clearing-status terms must agree; a set may inherit its first supplied spec. Across sets, the same CSA ID must retain the same counterparty and terms. Instruments without membership remain in `positions_without_margin`.

A default-version SIMM calculator is constructed before method dispatch. Native builds calculate member sensitivities in indexed parallel order; WASM is serial. When an instrument has a spec whose chosen methodology is not SIMM, its contribution is an empty sensitivity set. Otherwise extract product SIMM sensitivities, multiply by signed position scale and convert the calculation currency to portfolio base. Cross-currency rebasing with any nonempty FX-delta map is rejected because scaling alone does not define the required FX calculation-currency remap. Other sensitivities can be uniformly FX-scaled. Successful sets merge in member order; extraction/conversion failures become degraded positions and are omitted from sensitivity aggregation.

## Initial-margin selection and formulas

A supplied spec with no IM parameters produces zero IM. Otherwise use its selected methodology, falling back to ClearingHouse for cleared sets and SIMM for bilateral sets without a method. Unsupported methodologies error.

- **SIMM:** calculate from aggregated sensitivities using the CSA MPOR if present, otherwise the calculator default. Missing aggregate sensitivities produce zero IM. The margin crate owns bucket/correlation/concentration formulas; the portfolio does not reconstruct them.
- **Schedule:** require regulatory exposure notional and contractual expiry. Expired positions are skipped. Remaining years are calendar days/365. Supported classification is IRS and fixed-income-index TRS→interest rates; CDS/CDS index→credit; equity TRS→equity. Other products error. Use absolute position scale on regulatory notional, signed scaled MTM, base-currency conversion, and the margin crate's net-to-gross-ratio schedule calculation. Rescale its result by `sqrt(target MPOR/source MPOR)`; zero source MPOR errors.
- **Clearing house:** choose the CCP proxy by netting-set counterparty. Calculate each standalone requirement, multiply by absolute scale and `sqrt(target/source MPOR)`, spot-convert and add. No portfolio offsets are granted. Calculation/conversion failures are degraded and omitted; it is not the same failure policy as schedule IM.

## Failure, approximation and aggregation limits

SIMM extraction failures can leave a partially populated or zero IM while the call succeeds with degradation metadata. Schedule input/calculator errors abort; CCP per-position failures can yield a partial total. VM counts positions through its separate MTM pass, so `position_count` is not necessarily the count of complete IM contributions. `positions_without_margin` counts untracked or degraded positions, even if a partial number exists. The first degradation message per position is retained, potentially hiding additional failure reasons.

`is_approximate` is set for cleared sets or when a SIMM breakdown is present. A false flag on schedule/zero-IM output is not evidence of exact regulatory compliance. SIMM model/version, schedule classification and CCP-proxy adoption require the [margin methodology scope](../margin-collateral-im/README.md); current external rule applicability was not researched here.

A live source check found no qualified Schedule or CCP exposure-base
implementation in the built-in valuation products. The portfolio CCP path
does not attach an external amount provider. Published ISDA material lists
`2.8+2512` as effective from July 2026 and `2.9+2606` from December 5, 2026.
The bundled historical edition does not establish either implementation.
[ISDA July edition](https://www.isda.org/2026/06/12/isda-publishes-isda-simm-methodology-version-2-8-2512/)
and [ISDA December edition](https://www.isda.org/a/ZYxiE/ISDA-SIMM_v2.92606_PUBLIC.pdf).

Netting-set traversal and several monetary accumulations use hash-map iteration and plain f64 addition. Those mixed-sign/dynamic-range and deterministic-order choices differ from [existing invariants](../../../../INVARIANTS.md#12-arithmetic-and-aggregation); the observation is not an approved exception. No numerical impact was reproduced. A strict all-or-error margin policy and richer per-position completeness report are possible alternatives requiring explicit behavior design.

## Evidence

[aggregator.rs](../../../../finstack-quant/portfolio/src/margin/aggregator.rs), [netting_set.rs](../../../../finstack-quant/portfolio/src/margin/netting_set.rs) and [results.rs](../../../../finstack-quant/portfolio/src/margin/results.rs). [Margin aggregation tests](../../../../finstack-quant/portfolio/tests/margin_aggregation.rs) use instrument probes for quantity, sign, FX and total reconciliation; [bilateral IRS test](../../../../finstack-quant/portfolio/tests/margin_bilateral_irs.rs) uses a product path. These are bridge/invariant evidence. [ISDA SIMM](../../../REFERENCES.md#isda-simm) and [BCBS/IOSCO](../../../REFERENCES.md#bcbs-iosco-uncleared-margin) are unversioned conceptual references here, not proof of current compliance.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
