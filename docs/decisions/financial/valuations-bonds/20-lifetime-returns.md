# VALUATIONS-BONDS-20: Issue-cost MOIC/XIRR and worst-exit lifetime returns

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and input basis

These metrics are issue-to-exit contractual returns, not returns on today's quoted purchase price. Cost basis is issue date and return-floor issue-price specification (par by default, otherwise its amount/OID resolution); currency must match and cost must be strictly positive. Lifetime holder cashflows retain past payments, including amounts before `as_of`; floating history therefore depends on available schedule/fixing inputs. Return floors are early-call protection and do not guarantee the held-to-maturity return.

## Algorithms and selection

MOIC sums positive holder amounts strictly after issue and divides by cost. Negative post-issue holder payments are ignored by MOIC. XIRR instead forms `(issue,−cost)` followed by **all signed** post-issue holder amounts, then delegates to core XIRR with its default solver. Day count is ACT/365F unless an XIRR return floor supplies another; a MOIC floor's day count does not select XIRR annualization.

Worst-exit metrics first lower any floor into effective call rights, build lifetime flows, and enumerate call/put dates from `as_of` onward using the same workout candidates as record 11. For each path retain issue-to-exit flows including same-date distributions and add redemption, including accrued after those distributions. Take the minimum across held-to-maturity and all call/put exits. MOIC again uses only positive amounts. XIRR maturity-path failure propagates, but an early-exit XIRR failure is silently skipped. Therefore the reported worst XIRR is the minimum among successfully solved candidates, not necessarily every contractual candidate.

## Limits, alternatives and evidence

The full schedule is a current contractual projection; retaining past rows does not turn it into a verified realized cash ledger. The maturity path is unfloored; a floor target above natural maturity return can coexist with a lower `to_worst`. Treating voluntary put exits as worst cases follows enumeration semantics and differs from optimal holder action. Skipped candidates limit the coverage of the reported worst return. A current-purchase return, signed-net MOIC, or per-candidate validity result would be different methods.

Owning code: [cost basis, lifetime flows and MOIC](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/return_metrics/moic.rs), [XIRR paths and skip](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/return_metrics/xirr.rs), [exit redemption](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/yield_price.rs), [floor construction](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/return_floor.rs). Inline floor property/mutation tests locate early-exit floor checks, while metric tests in these modules inspect lifetime versus remaining-cash semantics. Core solver validation does not prove economically unique IRRs for arbitrary signed streams. Reference: [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook) is general background; exact lifetime/worst-exit convention provenance remains unestablished.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
