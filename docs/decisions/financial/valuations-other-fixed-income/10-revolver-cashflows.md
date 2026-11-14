# VALUATIONS-OTHER-FIXED-INCOME-10: Revolver contractual state, utilization accrual and fee timing

- **Decision status:** Accepted 2026-11-14 — full checked revolver contractual cashflows package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below. Reconciliation to the accepted contract is pending.
- **Validation status:** Live source and primary references inspected. Independent arithmetic checked elapsed accrual, phase admission, overnight growth, fees and modeled funding. Contract, model, runtime and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked revolver contractual cashflows

1. **Bind the actual lender position and source history.** Require
   an explicit economic and information cut and one authoritative
   opening state with date, phase, currency, commitment, funded
   principal and outstanding letters of credit (LCs) at the
   modeled share. Replay all covered actual movements needed to
   establish the anchor and every open claim. A seasoned snapshot
   can start the future replay directly when it agrees with the
   authoritative elapsed cash and exposure state. Identify which
   movements are already in the opening stock. Reject duplicate
   and conflicting events and out-of-coverage balance queries.
   Historical data and authorized future instructions keep
   different provenance. The current drawn amount supplies future
   state only. It supplies neither original funding nor elapsed
   balances. A new facility's funding is a dated authored obligation
   or event, with its own economic and cash phases.

   For each elapsed but unpaid interest or fee parent require the
   covered history of economic balances, terms and observations
   or an authoritative covered parent state sufficient to reproduce
   the future payable cash. A known completed payable is admissible
   on its own amount and paid or unpaid authority. A started,
   still compounding parent may need the growth state as well as
   a cash prefix. Missing elapsed state fails dependent results.
   Keep observed elapsed state, projected accrual, actual partial
   payoff and quote accrued separate. Derive no entitlement from
   a date or a current balance alone.
   [Dated opening stock and atomic phases](../cashflows/03-principal-and-emission-order.md),
   [floating replay and partial earning](../cashflows/08-floating-replay.md),
   [loan settlement and quote authority](06-loan-quotes-eir.md).

2. **Replay exact revolving capacity and signed principal.** In the
   admitted single-currency facility keep commitment `C`, funded
   balance `D`, outstanding eligible LC use `L`, its sublimit
   and the independently authored availability window. Validate
   exact nonnegative amounts, `D + L ≤ C` and the LC sublimit
   at every committed phase. Zero commitment requires zero drawn
   and LC use in this profile. Limit steps take effect on their
   authored phases. Repayment restores capacity only while future
   draws are authorized. An accordion increases the limit without
   funding cash. Stopping new draws does not itself retire funded
   par, end every fee, or imply a funded term conversion.

   Admit draws only on qualified dates and phases, including the
   availability cutoff. Preserve each identified draw and repayment,
   with cash negative for lender funding and positive for lender
   repayment. Bind the paired par effect and its authored economic
   date independently of cash settlement. Validate reductions
   against live exposure. Same-date offsetting events can use one
   atomic batch only when contractual simultaneity authorizes it.
   Otherwise require authoritative order, including repayment,
   LC release, limit change and any new extensions, and check
   each committed phase. Capacity breaches require an authorized
   repayment or other supported resolution. Do not infer a principal
   payment, an overadvance, LC cash collateral, or a limit change
   from a balance mismatch.
   [Filed repayment and reborrowing rights](https://www.sec.gov/Archives/edgar/data/1597672/000119312523008974/d436551dex101.htm),
   [filed capacity reduction, repayment and LC collateral](https://www.sec.gov/Archives/edgar/data/92116/000162828023024157/exhibit102-executedcredi.htm).

3. **Separate loan termination, accrued claims and payable cash.**
   End availability and running coupon accrual at their authored
   boundaries. At the terminal economic principal phase retire
   the remaining balance once after authorized same-phase
   principal movements. Set a separately authorized redemption
   cash date and phase. Sharing the adjusted interest date requires
   contractual authority. Preserve any positive unpaid redemption
   as its own claim through settlement, with no extra interest or
   capacity from a payment delay alone. Recognize prepayment
   interest, breakage, termination and delayed compensation only
   as owner-qualified separate economics. When an early repayment
   makes earned interest payable, resolve its partial cash and
   avoid a second payment on the original parent. Unsupported
   payoff terms fail the requested complete strip.

   Resolve same-day known cash, actual payment status and lender
   or buyer entitlement explicitly. Keep pending lender claims
   after legal maturity as long as they are covered. Use zero for
   a proven closed claim. LC issuance, amendment, expiry and
   cancellation change contingent capacity with zero funded
   principal and zero loan settlement cash. A reimbursement that
   actually becomes a loan requires its own linked funded event
   and releases or reclassifies the corresponding LC use once.
   Other LC reimbursement, cash collateral, surviving participation
   and term-out need separately qualified claim treatment.
   [Validated lifecycle](../cashflows/12-schedule-lifecycle.md),
   [cashflow and valuation export](../valuations-framework/cashflow-export.md),
   [filed LC disbursements and participations](https://www.sec.gov/Archives/edgar/data/1620459/000162045925000090/exhibit101-creditagreement.htm).

4. **Build original contractual schedules and term observations.**
   Require checked raw accrual grids, stub and roll/EOM choice,
   payment reference and delay, covered calendars by purpose and
   actual payment phases. Zero lag and any weekdays-only mode
   are explicit. Retain original coupon context for ACT/365L,
   ACT/ACT ICMA and other contextual counts. A balance or fee
   change can split the integration while preserving the
   underlying original parent. Payment adjustment leaves the
   authorized accrual boundary intact.

   Fixed and term coupons use the sum of `D_i × R_i × ΔA_i`
   over exact economic-state and rate slices. Here `A` is the
   original-parent cumulative accrual measure. Preserve exact
   contractual margin changes and rate controls. Term reset
   cadence is independent of the payment grid and index tenor.
   Each authorized reset binds exact index identity, fixing
   and publication version, actual value dates and index accrual
   fraction. Multiple term resets per payment require qualified
   windows. Resolve known observations before projecting any
   eligible unresolved observations. Use actual index value
   intervals, and no projection lookup for a fully fixed claim.
   A rate or balance step cannot create a new fixing without
   its own contractual reset authority. Check the total and
   restore Money once per payable parent under the accepted
   amount rules. An average displayed rate is a diagnostic
   and does not reproduce variable-balance cash by multiplying
   by opening par.
   [Checked schedule grids](../core-conventions/008-schedule-generation.md),
   [original-parent coupon measure](../cashflows/05-coupon-cash-and-pik.md),
   [resolved term observations](../cashflows/06-floating-rate-projection.md),
   [publication and revisions](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates).

5. **Use one original overnight growth and allocation law.**
   Require an explicit covered overnight profile, including
   observation calendar and basis, simple or compounded law
   and its supported lookback, observation shift or cutoff,
   gearing, spread and daily or period bounds. Keep each
   fixing's original observation block, genuine value period
   and allocated weight. Use the accepted cumulative growth
   and full-parent rate-to-cash allocation for the admitted
   profile. For variable balances allocate the differences of
   the original parent's contribution measure to the state on
   each economic slice. Do not restart interest compounding,
   roll every balance cut, or apply a final annualized rate
   uniformly to all slices. An unsupported intra-parent change
   to nonlinear whole-period controls needs a qualified rule
   or independently authorized parents. Known elapsed growth
   can be available while a future-dependent bound or bridge
   leaves partial payable cash unresolved. Deterministic and
   modeled routes consume the same observation and allocation
   law. Sampled overnight rates require a qualified original
   block rate path and owner-approved mapping under record 11.
   [Accepted overnight growth and controls](../cashflows/07-overnight-compounding.md),
   [ARRC syndicated loan conventions](https://www.newyorkfed.org/medialibrary/Microsites/arrc/files/2020/ARRC_SOFR_Synd_Loan_Conventions.pdf).

6. **Give each running fee its own lawful base and schedule.**
   For the covered fee recipes use `C−D−L` for unused commitment,
   `D` for funded usage, `C` for facility fees and the applicable
   eligible LC face for the participation and issuer fronting
   fees. Each enabled fee has an explicit entitled recipient,
   eligibility window, original accrual parent and day count,
   payment reference and phase. Accrual boundaries and payable
   dates can differ from interest. Integrate exact exposure and
   contractual-rate changes using original-parent cumulative
   differences. Restore Money once per payable fee. Preserve
   distinct claims even when dates coincide. No automatic switch
   from commitment to a surviving exposure base after termination.

   Flat and tiered rates use checked exact basis points. For the
   covered contemporaneous tier profile use `(D+L)/C` and the
   greatest inclusive threshold, with exact comparisons. Require
   strictly increasing thresholds, explicit coverage below the
   first applicable tier and a declared disabled or enabled fee.
   At zero commitment the ratio is undefined and no tier is
   selected. Emit any required admitted zero fee under its
   applicable parent. Dated signed deltas cumulate at their
   authorized phases. Ordinary creditor running fees require
   nonnegative effective rates after any explicitly contracted
   floor. Remove the hidden zero clamp and negative-fee-as-absent
   convenience. Period-average tier triggers, strict thresholds,
   different usage components or other fee bases require a
   separately qualified profile.
   [Accepted fee parents and tier law](../cashflows/09-fees.md),
   [filed daily unused base and separate payment dates](https://www.sec.gov/Archives/edgar/data/717605/000093041323001421/c106287_ex10-1.htm),
   [filed fee and LC bases](https://www.sec.gov/Archives/edgar/data/1288469/000128846926000060/a93020268-kexhibit101.htm).

7. **Elect LC rates and carry event fees once.** LC participation
   elects an independently fixed rate or an explicitly linked
   contractual live margin stream, with its identity and effective
   phases. A live link does not read the index, effective coupon
   or modeled credit spread. Issuance-frozen rates require qualified
   LC lot history or are unavailable. Fronting is separate issuer
   economics and enters only the entitled modeled position. Cash
   charges for issuance and other documented events use their own
   authorized dates and amounts.

   Emit qualified upfront, amendment and other scheduled amounts
   once under their own identified cash claims. A proportional
   upfront amount uses the authorized original opening commitment.
   An elected reduction fee uses covered positive reduction
   `(C_before−C_after) × bp / 10000` with exact terms. A step-up
   cannot trigger that formula. Keep fixed charges, signed authored
   rebates where admitted, and third-party amounts under their
   actual ownership. A lender fee has no funded principal effect.
   Remove separate insertion of the same upfront payment in each
   caller. Credit treatment of a future fee and origination EIR
   use their respective owner rules.
   [Shared fee and original EIR basis](06-loan-quotes-eir.md),
   [filed LC and lender fee terms](https://www.sec.gov/Archives/edgar/data/1620459/000162045925000090/exhibit101-creditagreement.htm),
   [filed commitment reduction fee](https://www.sec.gov/Archives/edgar/data/1315657/000119312513010596/d364901dex108.htm).

8. **Use an explicit dated funding law for simulated balances.**
   Select a checked funding grid with an exact anchor, ordered
   distinct dated nodes and phase identities for coincident events.
   Require positive explicit maximum calendar spacing over the
   modeled availability interval and a positive node/work bound.
   Include all dates that change modeled capacity or terminate
   funding. Keep coupon, observation, fee and reporting cuts in
   the integration grid. Adding a reporting cut alone must not
   change projected funding. Record 11 owns model stepping and
   the exact mapping to the selected funding grid.

   Hold funded dollars fixed between funding nodes. Apply eligible
   simulated changes at their right endpoint, before accrual over
   the next interval. At a coincident term change, the admitted
   projection first applies the modeled change under incoming
   terms, then the ordered contractual phases. The closing phase
   suppresses any discretionary modeled update on the terminal
   boundary. Validate the incoming and resulting states. A contract
   requiring another event policy needs a qualified mapping. Use
   the same actual modeled balance for interest, fee tiers and
   capacity. A change of limits or LC
   outstanding does not leak into an earlier accrual interval.
   A pure increase of capacity leaves dollar principal unchanged.
   If a reduction or new LC makes existing funded par infeasible,
   use only an explicitly qualified dated repayment or scenario
   rule or fail. Keep modeled and contractual movements separately
   identifiable. Preserve the separately dated terminal cash.

   Verify path anchor, date/time mapping, finite values, exact
   restored amount and capacity. Restore each admitted modeled
   balance once to Money and compute lender cash as the exact
   negative balance increment. Carry all authorized nonzero
   changes and reconcile exact principal, funding and redemption.
   Remove whole-coupon net funding, synthetic midpoint cash,
   hidden clamps and sub-cent suppression. An intra-coupon draw
   followed by repayment produces both dated cash rows. The
   chosen held-balance law is a discrete approximation to
   continuous utilization. A projection with no modeled changes
   shares the deterministic cashflow law under the same terms.
   [Checked grid behavior](../core-numerics/time-grids.md),
   [revolver simulation owner](11-revolver-monte-carlo-credit.md).

9. **Qualify timing and preserve claim identity across consumers.**
   Before relying on the modeled schedule for a published purpose,
   qualify funding timing under nonflat discount and credit,
   changes in capacity and tier, original coupon clocks and
   terminal and pending cash. Require explicit error budgets in
   the report's units for the affected valuation or schedule
   measures and compare appropriately refined funding grids with
   paired paths or an independent reference. Report discretization
   evidence separately from sampling uncertainty. Record the
   admitted scope of the timing qualification so results can use
   applicable existing evidence. Mark dependent model results
   outside that scope unsupported.
   Record 11 owns path construction, discount and survival
   integration, default exposure and expected-schedule estimation.
   It must consume the same prepared pre- and post-phase exposure
   and the dated cash rows. Record 12 owns revolving loan quotes,
   rate and credit risk, and commitment-normalized cost. Record 06
   owns EIR and the general loan settlement strip.

   Use one validated Rust preparation for deterministic valuation,
   realized and modeled ledgers, metrics and cashflow export.
   Retain opening state, original parent and event identity,
   matched cash and economically effective dates, payment and
   entitlement state, fee recipient and observed or projected
   provenance. Keep expected schedules identifiable as model
   results and preserve amount and rate diagnostic semantics.
   Reject invalid restoration or compilation atomically.
   Reconcile the direct and registered Rust APIs, typed Python
   and WASM, generated schemas and UI, scenario exposure,
   portfolio and statement consumers. Hosts transport canonical
   values, identities and errors.
   [Cashflow export and owner boundaries](../valuations-framework/cashflow-export.md),
   [current Rust engine](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/cashflow_engine.rs).

## Observed behavior

### Choice and rationale

[CashflowEngine](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/cashflow_engine.rs) shares fee and coupon machinery between deterministic event replay and simulated utilization paths. The current drawn balance represents state at `max(issue_date,as_of)` and also supplies elapsed balance in an unpaid period where earlier movements are unavailable. [Types](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/types.rs), [validation](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/validation.rs) and [utilities](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/utils.rs) define material input and timing behavior.

### Algorithm

1. Build contractual unadjusted accrual periods, adjusted payments, term-index resets and observation dates. Observation dates include accrual boundaries, reset dates and dated commitment/margin/fee/LC changes. Default stub is ShortFront; calendar resolution and lag rules are explicit in utilities.
2. Deterministic events are sorted, then all events on/before the anchor reject. Replay future draws/repayments with capacity and balance checks. Initial funding is a negative cashflow only when issue is after valuation. Split coupon periods at events, resets and dated terms. Period-end events affect subsequent accrual, while dated principal flows settle on their own event dates.
3. On each deterministic subinterval charge `drawn × all_in_rate × year_fraction`. Fixed rates include cumulative margin deltas. Term floating resets before `as_of` require exact fixings; future resets project the index and apply gearing/floor/cap/spread rules. Overnight rates use daily compounding/projection and the fixing machinery. Actual/365L retains coupon-period metadata.
4. Fees use commitment minus drawn minus LC for undrawn, drawn for usage, total commitment for facility fees, and LC balance for LC/fronting fees. Fee tier utilization includes LC usage. Select the highest reached ordered threshold, add cumulative dated fee deltas and floor the resulting fee rate at zero. LC fee may inherit the running floating margin. Add dated fixed and commitment-reduction fees. Aggregate coupon/fee amounts to adjusted period payment date.
5. Stochastic paths use mean endpoint utilization on each observation subinterval. Future floating rates come from the reset observation, plus a deterministic index-versus-short-rate basis when needed; historical fixings override simulation. Deterministic forward mode replaces the raw baseline with the contractual projected index while preserving path shocks. Overnight MC cashflows compound the whole coupon window and reuse it across subintervals with equal margins.
6. Changes due to commitment steps book principal on the step date. The remaining utilization-driven net balance change over each coupon period books at its calendar midpoint, or midpoint from `as_of` for the active period. Sub-cent changes carry into subsequent periods. Final drawn balance repays on the adjusted final coupon date.

### Scope and conventions

Currency is the commitment currency; rates are annual decimals, fee/margin inputs basis points. Loan receipts are positive and draws negative. Deterministic history must be rebased into `drawn` before valuation. Current-period interest implicitly uses available anchor state for elapsed pieces; no full historic utilization path is reconstructed. Interest diagnostics are time-weighted rate averages and do not imply that opening balance times displayed rate reproduces a variable-balance coupon.

### Limitations, failure and alternatives

Validation rejects invalid currencies, dates, commitments, LC capacity, fees and incompatible process specifications. Repayment exceeding balance and draw exceeding capacity error. Stochastic utilization is capped for LC availability. Flat-fee convenience construction treats nonpositive commitment/usage fee rates as absent tiers; dated reductions floor fees at zero. Mean endpoint utilization and midpoint net funding do not reconstruct the timing of multiple draws and repayments inside an observation interval. A full daily utilization/funding process is an unimplemented alternative; exact dated draws are the supported deterministic alternative.

### Evidence

[Cashflow tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/cashflows.rs), [fixing tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/fixings.rs), [review regressions](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/review_fixes.rs), and [fee tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/metrics/utilization_fee.rs) provide contractual examples and parity evidence, not an external servicing reconciliation. [ISDA](../../../REFERENCES.md#isda-2006-definitions) supports date-convention context.

## Independent arithmetic and qualification

- In a 90-day ACT/360 fixed parent at 12%, funded balance 100
  for days 0–20 and 200 thereafter gives full interest `16/3`.
  Actual day-30 accrued is `4/3`. Carrying today's 200 backward
  gives day-30 `2`; linear allocation of known full interest
  gives `16/9`. The latter is a separate hypothetical quote rule.
- From commitment 100, funded 80 and LC 10, a same-date
  repayment 20, limit cut to 85 and draw 15 is admissible in
  that order. Starting with either the cut or the draw exceeds
  capacity. Final funded 75 plus LC 10 fits the limit. Preserve
  both gross principal cash events even for a contractually
  authorized atomic batch.
- Two one-day ACT/360 compound-rate blocks at 3.6% and 7.2%,
  with par 100 then 200, give `.050004` before financial
  rounding on unshifted weights and with neutral final controls.
  Independent compounding restarts give `.05` and applying a
  full-window average annualized rate gives `.045003`.
- On a 90-day ACT/360 fee window with commitment 1,000 and
  30-day states `(D,L) = (400,100), (500,150), (500,0)`,
  inclusive thresholds 0%, 50%, 60% at 25, 50, 100 bp
  select 50, 100, 50 bp. Commitment, 40 bp usage, 20 bp
  facility, 200/250/250 bp live LC margin and 12.5 bp issuer
  fronting produce respectively `17/24`, `7/15`, `1/2`,
  `23/48` and `5/192` before rounding.
- A modeled draw of 60 on day 10 and repayment of 60 on day
  20 has lender funding PV about `−.16370945` at 10% continuous
  ACT/365F. Coupon-net funding gives zero. A balance of 80 for a
  30-day ACT/360 interval at 10% generates interest `2/3`
  even if an LC capacity change requires a 40 repayment at the
  right boundary. Applying that later cap to the previous
  endpoint average would give `5/12`.

These checks exercise exact arithmetic and isolated funding timing.
Full qualification needs executed agreement and servicing fixtures
with same-date and phased amendments, pending cash and LC lifecycle,
exact fixing and parent-dependent fee checks, paired model refinement,
and native and host agreement. Repository pricing and host suites
were not run.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
