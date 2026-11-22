# VALUATIONS-EQUITY-AND-COMPOSITION-18: Private-fund dated hurdles, catch-up, promote and clawback

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-18`.
- **Decision status:** Accepted 2026-11-14 — complete dated private-fund waterfall, catch-up, promote and final clawback package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent order, partial catch-up, stepped promote and final settlement checks completed. Production allocation, agreement mapping and host qualification remain pending.

## Accepted decision for checked private-fund waterfalls and final clawback

1. **Validate and order the complete event ledger.** Admit one
   currency, unique event identities, nonnegative Money amounts
   and explicit contribution, distribution or proceeds kinds.
   At the same date process contributions before receipts and
   require an authoritative sequence within each phase. Keep
   original dates, event kind, identity and actual or projected
   status. Elect whole-fund pooling or deal-by-deal allocation
   with exact deal IDs. Preserve each deal's state and the common
   fund provenance. Validate the complete event stream and
   waterfall terms at every owning entry point.
   [ILPA model agreement templates](https://ilpa.org/industry-guidance/templates-standards-model-documents/model-limited-partnership-agreement/).

2. **Use dated fixed-rate hurdle balances.** Apply the authored tier
   sequence. A return-of-capital tier pays up to the outstanding
   balance. For preferred or promote hurdles, compound signed LP
   cash flows to the distribution date at the elected finite annual
   rate greater than −100% and on explicit ACT/365F or ACT/360.
   Pay the positive shortfall from available cash. Include every
   LP allocation already made in that distribution, including
   capital and both sides of the partial catch-up split in their
   respective investor balances. Post the LP's share before each
   subsequent test. Use row IRR as separately labelled reporting
   on the complete eligible LP cash prefix, through canonical
   XIRR and the qualified annual quote basis. Report genuine
   unavailable results and computational failures explicitly.
   [Accepted IRR and XIRR](../core-conventions/017-irr.md).

3. **Require a reachable catch-up target and checked promote
   transitions.** Use the next authored promote tier's GP share
   `p` as the target. Choose full catch-up with GP share `c=1`
   or an explicit partial share, with `c>p`. With eligible
   cumulative gross profit `P` after contributed capital and
   previously allocated gross GP carry `G`, cap the gross
   catch-up at `max(pP−G,0)/(c−p)` and available cash. Commit
   both investor shares before the next hurdle. Pay the current
   promote tier's LP hurdle first, then apply its split until
   the next strictly higher hurdle or exhaustion. Advance if
   the next hurdle is already met. If its LP shortfall remains
   positive and the current tier gives LP zero, allocate the
   available cash under the elected current split.
   [ILPA whole-fund waterfall, section 14.3](https://ilpa.org/wp-content/uploads/2020/07/ILPA-Model-Limited-Partnership-Agreement-Whole-of-Fund-Waterfall-July-2020.pdf).

4. **Conserve each receipt and track holdback explicitly.** Author
   one GP share and derive its LP complement. Use checked Money
   and reconcile LP allocation, GP paid cash and GP carry held
   against each receipt. Preserve distinct gross carry, paid carry
   and escrow. Remove share-sum tolerance and cash thresholds
   that suppress balances. A nonzero unallocated balance fails.
   Keep the event and tier detail through reporting, with exact
   signed same-date netting only in the derived LP cash view.
   [Accepted Money arithmetic](../core-conventions/012-money-decimal-precision.md).

5. **Elect final-only clawback with distinct liquidation and
   settlement.** Require a qualified completed liquidation cut
   or explicit terminal scenario and a declared settlement date.
   Interim ledgers retain holdback. The carry allowance is the
   smaller of chronological whole-fund carry and the
   timing-reconciled lifetime replay. The latter retains actual LP
   receipt dates for compounding and offsets their nominal total
   at liquidation before reallocating lifetime proceeds. The
   synthetic offset remains inside the replay. Use escrow first.
   Cap a positive GP release by the available escrow and report
   any unfunded allowance. Excess paid GP carry creates a signed
   recovery due, with actual settlement recognized separately.
   Allocate the balancing escrow and any recovery to LP without
   changing prior cash dates. Preserve outstanding principal
   classification on the qualified ledger. Reconcile terms,
   dates, diagnostics and settled or projected status through the
   existing Rust and host routes. Record 19 owns residual PV, NAV,
   aggregate performance and sensitivity.
   [ILPA deal-by-deal clawback terms](https://ilpa.org/wp-content/uploads/2020/07/ILPA-Model-Limited-Partnership-Agreement-Deal-By-Deal.pdf),
   [private-fund valuation](19-private-fund-valuation.md).

## Observed behavior

### Choice and rationale

The waterfall allocates dated contributions/distributions through explicit ordered tranches. European mode pools all events; American mode maintains deal-level state and can reconcile carry at fund end. Dated LP hurdle balances use fixed-rate future value, avoiding an inverse IRR solve as the allocation gate.

### Scope and input domain

One currency, nonnegative finite event magnitudes, explicit contribution/distribution/proceeds kinds, nonempty deal IDs in American mode. Hurdles exceed -100%; shares and holdbacks lie in [0,1], promote shares sum to one within 1e-6. An ordered caller-supplied tranche sequence determines priority.

### Algorithm

1. Sort events chronologically, with contributions preceding distributions on the same date. Contributions increase outstanding LP capital and negative LP cashflow history. European state is pooled; American state is separate per deal.
2. ReturnOfCapital pays the smaller of remaining cash and unreturned capital. PreferredIrr computes the cash required to make the dated signed LP history meet the stated compounded hurdle at the distribution date, subtracting LP allocations already made within that distribution and flooring at zero.
3. CatchUp targets the next promote GP share when present, otherwise its own share. Full mode sends catch-up cash wholly to GP; partial mode uses its own GP share and the complementary LP share. Required gross catch-up is the target shortfall divided by `(catchup_GP_share−target_GP_share)` when positive; otherwise the branch consumes available cash. Holdback reduces GP cash paid while cumulative carry tracks gross accrual.
4. PromoteTier first satisfies its LP hurdle, then splits residual cash until the next promote hurdle's LP requirement is met (or consumes all remaining cash in the last tier). Remaining unallocated cash above 1e-6 errors rather than being discarded.
5. Optional fund-end clawback settles on the last event date. Compute allowed GP carry as the smaller of chronological whole-fund European replay and a timing-reconciled aggregate replay. Escrow equals proceeds less paid LP/GP; transfer the GP over/underpayment and release the balancing LP amount without moving earlier cashflow dates.

### Selection, controls and conventions

Waterfall spec selects European/American, full/partial catch-up, day count and optional clawback/holdback. Default day count is ACT/365F. LP ledger cashflows merge dated contributions/allocations; small entries below the ledger tolerance are omitted. Row IRR is a reporting solve and can be absent; allocation hurdle calculation uses the fixed-rate balance.

### Limitations, failures and alternatives

Observed discrepancy: the partial-catch-up branch records cash paid to LP but does not increment `lp_allocated_in_call_so_far`. A following promote hurdle and that row's LP IRR can omit this same-distribution cash. Conservation tests alone do not prove the hurdle calculation is correct. GP clawback/replay conventions are contractual choices; a generic European/American label does not establish compliance with a fund agreement.

### Evidence

Inline tests in [waterfall.rs](../../../../finstack-quant/valuations/equity/src/private_markets_fund/waterfall.rs) cover order invariance, precise catch-up, promote thresholds, clawback/escrow conservation, fixed-rate dated hurdles and partial-catch-up allocations. These are local formula/ledger invariants; no executed regression or independent legal-agreement reference was established. [private-equity context](../../../REFERENCES.md#rosenbaum-pearl-2020) does not specify this implementation's negotiated waterfall terms.

## Independent numerical checks

- With 1,000 contributed and 1,500 received one ACT/365F year later,
  use return of capital, 12% preferred return, a 50% GP catch-up
  targeting 20%, and promote hurdles at 18% and 30% with GP
  shares 20% and 30%. The partial catch-up of 80 allocates 40
  each. Posting the LP's 40 before the 18% hurdle gives total
  LP 1,391 and GP 109, and catch-up row LP IRR 16%. The
  observed accumulator omission gives LP 1,403 and GP 97,
  with catch-up row LP IRR 12%. Both totals conserve 1,500.
- With contributions of 100 on 2017-01-01 and 10 on 2019-01-01,
  a receipt of 150 on 2018-01-01, liquidation 2020-01-01,
  an 8% ACT/365F hurdle and 80/20 promote, chronological
  whole-fund carry is 8.40 and timing-reconciled carry is
  7.358208. At 50% holdback the earlier GP paid cash and escrow
  are each 4.20. Terminal allocations are 3.158208 to GP and
  1.041792 to LP. At zero holdback GP owes 1.041792.
- Against an opening return of capital balance of 100, same-date
  receipts 60 and 70 with an 80/20 residual split give aggregate
  LP 124 and GP 6 in either order. The authorized order determines
  which receipt owns the 6 of carry.

The checked fixtures cover the stated allocation policies. Production
allocation, agreement mapping and host qualification remain pending.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [waterfall.rs](../../../../finstack-quant/valuations/equity/src/private_markets_fund/waterfall.rs)
