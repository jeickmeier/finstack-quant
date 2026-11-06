# VALUATIONS-EQUITY-AND-COMPOSITION-18: Private-fund dated hurdles, catch-up, promote and clawback

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-18`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

The waterfall allocates dated contributions/distributions through explicit ordered tranches. European mode pools all events; American mode maintains deal-level state and can reconcile carry at fund end. Dated LP hurdle balances use fixed-rate future value, avoiding an inverse IRR solve as the allocation gate.

## Scope and input domain

One currency, nonnegative finite event magnitudes, explicit contribution/distribution/proceeds kinds, nonempty deal IDs in American mode. Hurdles exceed -100%; shares and holdbacks lie in [0,1], promote shares sum to one within 1e-6. An ordered caller-supplied tranche sequence determines priority.

## Algorithm

1. Sort events chronologically, with contributions preceding distributions on the same date. Contributions increase outstanding LP capital and negative LP cashflow history. European state is pooled; American state is separate per deal.
2. ReturnOfCapital pays the smaller of remaining cash and unreturned capital. PreferredIrr computes the cash required to make the dated signed LP history meet the stated compounded hurdle at the distribution date, subtracting LP allocations already made within that distribution and flooring at zero.
3. CatchUp targets the next promote GP share when present, otherwise its own share. Full mode sends catch-up cash wholly to GP; partial mode uses its own GP share and the complementary LP share. Required gross catch-up is the target shortfall divided by `(catchup_GP_share−target_GP_share)` when positive; otherwise the branch consumes available cash. Holdback reduces GP cash paid while cumulative carry tracks gross accrual.
4. PromoteTier first satisfies its LP hurdle, then splits residual cash until the next promote hurdle's LP requirement is met (or consumes all remaining cash in the last tier). Remaining unallocated cash above 1e-6 errors rather than being discarded.
5. Optional fund-end clawback settles on the last event date. Compute allowed GP carry as the smaller of chronological whole-fund European replay and a timing-reconciled aggregate replay. Escrow equals proceeds less paid LP/GP; transfer the GP over/underpayment and release the balancing LP amount without moving earlier cashflow dates.

## Selection, controls and conventions

Waterfall spec selects European/American, full/partial catch-up, day count and optional clawback/holdback. Default day count is ACT/365F. LP ledger cashflows merge dated contributions/allocations; small entries below the ledger tolerance are omitted. Row IRR is a reporting solve and can be absent; allocation hurdle calculation uses the fixed-rate balance.

## Limitations, failures and alternatives

Observed discrepancy: the partial-catch-up branch records cash paid to LP but does not increment `lp_allocated_in_call_so_far`. A following promote hurdle and that row's LP IRR can omit this same-distribution cash. Conservation tests alone do not prove the hurdle calculation is correct. GP clawback/replay conventions are contractual choices; a generic European/American label does not establish compliance with a fund agreement.

## Evidence

Inline tests in [waterfall.rs](../../../../finstack-quant/valuations/equity/src/pe_fund/waterfall.rs) cover order invariance, precise catch-up, promote thresholds, clawback/escrow conservation, fixed-rate dated hurdles and partial-catch-up allocations. These are local formula/ledger invariants; no executed regression or independent legal-agreement reference was established. [private-equity context](../../../REFERENCES.md#rosenbaum-pearl-2020) does not specify this implementation's negotiated waterfall terms.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [waterfall.rs](../../../../finstack-quant/valuations/equity/src/pe_fund/waterfall.rs)
