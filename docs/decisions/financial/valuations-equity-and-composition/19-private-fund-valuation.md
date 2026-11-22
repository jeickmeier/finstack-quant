# VALUATIONS-EQUITY-AND-COMPOSITION-19: Private-fund residual valuation, LP performance and parameter shocks

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-19`.
- **Decision status:** Accepted 2026-11-14 — complete private-fund residual valuation, settled LP performance, carry reporting and prospective risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent NAV and commitment, performance, carry aggregation and prospective shock checks completed. Production valuation, administrator reconciliation and host qualification remain pending.

## Accepted decision for checked private-fund residual value, performance and risk

1. **Elect one residual valuation basis at a qualified cut.**
   Choose projected LP cash flows or a dated attributable net LP
   NAV with a qualified source and scope. On the NAV basis,
   include a separate outstanding distribution or called-but-unpaid
   contribution only when the mark explicitly excludes it. Disclose
   uncalled commitments separately. Include forecast calls only
   in the elected projected cash-flow basis. Require a qualified
   mark, a complete projection or evidence that all rights and
   obligations have closed before reporting residual value.
   [IPEV valuation guidelines, section 4](https://www.privateequityvaluation.com/Portals/0/Documents/Guidelines/2025%20IPEV%20Valuation%20Guidelines.pdf).

2. **Price outstanding cash on the prepared dated ledger.** Freeze
   the information and settlement phases and reuse record 18's
   admitted ledger. Include eligible unpaid same-day cash at
   factor one. Preserve settled cash in history and require
   a qualified collection basis for overdue amounts. Elect a dated
   discount source or an explicitly labelled undiscounted scenario.
   Use checked signed Money, including future calls and clawback
   recoveries. Preserve the original entitlement or obligation
   separately from its actual settlement.
   [Accepted dated waterfall](18-private-fund-waterfall.md),
   [accepted dated discounting](../core-conventions/016-discounting.md).

3. **Use settled LP cash and a qualified ending mark for reported
   performance.** Count actual paid-in and distributed cash before
   same-date netting. Count a recallable distribution when paid
   and a recall when recontributed. DPI is distributions over
   paid-in, RVPI is qualified net residual mark over paid-in, and
   TVPI is their sum. Exact zero paid-in makes the multiples
   unavailable. Since-inception LP XIRR uses the admitted settled
   cash and one qualified same-cut ending NAV or confirmed closed
   zero. Preserve genuine nonzero values through exact dated netting.
   Use canonical ACT/365F XIRR, with the checked exact equal-growth
   ACT/360 quote when elected. Identify any performance result
   using modeled holder PV as a scenario.
   [GIPS performance definitions](https://www.gipsstandards.org/standards/gips-standards-for-firms/gips-standards-handbook-for-firms/),
   [ILPA performance definitions](https://ilpa.org/wp-content/uploads/2025/02/PT-Definitions-Gross-Up.pdf),
   [accepted XIRR](../core-conventions/017-irr.md).

4. **Aggregate carry across deals and respect settlement.**
   `carry_accrued` sums qualifying gross carry entitlement across
   eligible deals, including held carry. Apply a fund-level true-up
   once. `gp_carry_total` reports net actual settled GP cash,
   including repayments recognized on their actual settlement
   basis. Keep escrow and recoveries due as separate balances.
   [Accepted final clawback and escrow](18-private-fund-waterfall.md).

5. **Reprice prospective waterfall parameters and the current NAV
   on their own axes.** Use `carry01` and `hurdle01` for checked
   two-sided reprices of residual LP value per positive bp.
   Require explicit computation widths and valid represented
   endpoints. Hold actual allocations, earned carry, escrow
   and determined cash fixed. Carry moves eligible promote and
   partial catch-up shares, keeping exact complements and full
   catch-up at 100%. Hurdle moves eligible annual rates on their
   contractual clock. Require any terminal replay to honor the
   frozen opening rights and preserve failed reprice provenance.
   `nav01` moves the qualified current LP NAV per positive
   1% relative move, holding ledger cash fixed. An explicitly
   elected projected-proceeds scenario moves only its identified
   unresolved forecast events. Missing or unsupported axes
   remain unavailable.
   [Accepted sensitivity rules](../valuations-framework/finite-difference-rates.md).

6. **Use the prepared ledger for price, metrics and export.**
   Export signed dated cash with event identity, source and
   actual, outstanding or projected status. Report NAV separately
   in the valuation bridge. A complete export requires a checked
   reconciliation to the selected valuation. Retain the actual
   date, yield quote basis, status, signed amount and pricing
   basis through existing Rust, Python, WASM and schema routes.
   [Accepted cashflow export](../valuations-framework/cashflow-export.md).

## Observed behavior

### Choice and rationale

Fund PV is future net LP waterfall cashflows plus an optional present NAV mark. Performance metrics combine realized LP allocations with this residual value. The same request shares its allocation ledger, so metrics use one consistent contractual allocation.

### Scope and input domain

The fund's event and waterfall domains are record 18. Optional NAV must match fund currency and cannot coexist with future distribution/proceeds events; future contributions may remain. A configured discount curve prices future net cashflows; omitted curve deliberately means undiscounted summation.

### Algorithm

PV retains LP cashflows strictly after as-of, discounts them through the configured curve when present, and adds NAV. Fully realized funds with no NAV therefore have zero residual value. LP IRR solves dated historical net LP flows through as-of plus current residual PV at as-of when its magnitude exceeds 1e-6. DPI is paid LP distributions/contributions; TVPI adds residual PV to the numerator. Both return zero when contributions are at most 1e-6. GPCarryTotal sums GP payments through as-of; CarryAccrued reads the last cumulative gross-carry ledger row.

Carry01 shifts every catch-up/promote GP share by ±1 bp, resetting promote LP share to its complement. It returns half the PV difference, using a one-sided difference at [0,1] boundaries and zero if neither side is possible. Hurdle01 moves all preferred/promote hurdles ±1 bp and returns half the PV difference. Nav01 scales every distribution/proceeds event, including historical events, and NAV by ±1%, preserving contributions, reruns the waterfall and returns half the PV difference.

### Selection, controls and conventions

XIRR uses waterfall day count and shared solver defaults. The schedule export returns full net LP ledger flows, labelled contractual with ACT/365F schedule metadata even when waterfall IRR day count differs. The quoted 01 outputs are currency changes for the specified bp or percent shock, not unscaled derivatives.

### Limitations, failures and alternatives

Residual PV can be negative because future contributions remain. LP IRR/TVPI depend on projected cashflow PV and discount choice when explicit NAV is absent. Nav01 is a full realization/NAV scaling scenario, not solely a shock to today's NAV; historical changes can alter future carry. Hurdle01 has no one-sided boundary repair. Alternative performance bases, fee accounting and NAV-only shocks require agreement.

### Evidence

Inline [types.rs](../../../../finstack-quant/valuations/equity/src/private_markets_fund/types.rs) checks realized zero value, projected-flow/NAV modes and currency errors; [metrics.rs](../../../../finstack-quant/valuations/equity/src/private_markets_fund/metrics.rs) includes exact near-zero-rate IRR, fully realized DPI/TVPI identities and request-local ledger sharing. These are local examples and solver consistency, not independent fund administrator reconciliation.

## Independent numerical checks

- Paid-in capital 100, settled distributions 20, net LP NAV 110
  and a separately owed capital call of 30 at factor .95 give
  holder PV 81.50. The reported mark basis gives DPI .20, RVPI
  1.10 and TVPI 1.30. Using holder PV as an explicitly modeled
  ending value gives scenario TVPI 1.015.
- NAV 95 explicitly excluding 12 due today, with a separate
  called obligation of 30 at factor .96, gives holder PV 78.20.
  A mark of 107 already including that same distribution gives
  the identical PV with no additional distribution row.
- With 100 contributed, 95 received historically and 20 of
  projected proceeds, a return-of-capital tier followed by an
  80/20 split gives +.16 per positive 1% when only projected
  proceeds move. The observed experiment that also scales the
  historical receipt gives −.03. Two deal carry balances of
  8.40 and 3.60 at 50% holdback give fund gross carry 12,
  paid carry 6 and escrow 6 on the admitted settlement basis.

Production valuation, agreement and administrator reconciliation,
metric behavior and host qualification remain pending.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/private_markets_fund/types.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/private_markets_fund/metrics.rs)
