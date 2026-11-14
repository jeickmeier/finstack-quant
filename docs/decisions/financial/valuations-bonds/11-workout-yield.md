# VALUATIONS-BONDS-11: Workout enumeration, yield to worst and inverse workout pricing

**Decision status:** Draft — proposed package for review.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, contractual, market-quote and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Proposed decision for checked bond workouts and yield to worst

1. **Bind one settlement price and one explicit quote law.** Keep the
   original valuation date, information and payment cuts, actual
   buyer settlement `S`, contractual claim, native currency and
   denomination. Select one authorized original price driver.
   Use whole-position dirty Money directly or convert an admitted
   clean amount using matching signed accrued under BONDS-02.
   Where expressly elected, an independently qualified noncircular
   quote conversion or holder-to-buyer bridge may supply dirty on
   the same bond and settlement basis. A model mark requires its
   admitted buyer claim, financing and contingent-delivery treatment.
   It supplies the common purchase amount and does not choose a
   workout. Fetch accrued only for an operation requiring it.
   Preserve the original observation, derived settlement amount and
   selected holder PV as distinct authorities. Remove the clean-pin
   requirement, synthetic clean restamping and default Street
   dispatch. A full YTW uses one comparable BONDS-10 dated
   dirty-price law and declared rate units across all required paths.
   Japanese simple clean yield has a separately scoped price law.
   [Settlement and quote authority](02-settlement-and-clocks.md),
   [selected bond value](03-pv-models.md) and
   [checked yield and price inversion](10-quoted-yield.md).

2. **Define the issuer-controlled minimum and exact opportunity set.**
   The house `ytw` is the lowest admitted yield across scheduled
   maturity under a no-new-voluntary-exercise scenario and every
   still-reachable qualified full issuer call under a no-new-holder-
   put policy. A straight live bond has its scheduled claim. Honor
   a binding prior call or put and any separately vested cash; if
   that election fixes the only reachable terminal path, use it as
   the sole candidate when the buyer's claim qualifies. A prospective
   voluntary put may appear as a labelled elected or diagnostic path
   and is excluded from the issuer minimum. Independently elect any
   fixed workout quote by exact identity. Define reachability from
   actual legal terms and election history, including notice and
   exercise eligibility at the frozen cuts and transfer of the buyer
   rights at settlement. Require exact authorized dates or the
   complete dated events of an admitted window, with explicit
   calendar, boundaries, eligibility and work limits. A later
   adjusted payment does not extend a contractual exercise window.
   No sampling at boundaries, coupon dates or month ends to bypass
   missing coverage. Admit exact deterministic full exits, including
   previously committed deferred cash where fully qualified. Future
   notice, deferred or conditional exercise, partial calls and
   continuously timed rights require a complete qualified dated
   scenario before use. An unresolved reachable issuer right fails
   a requested full YTW. Call friction and model continuation policy
   leave contractual eligibility and payable amounts unchanged.
   This house profile does not supply venue-specific confirmation
   rules, stochastic option value or an issue-return minimum.
   [Exact exercise and notice terms](04-rates-tree-exercise.md),
   [issue-return scenario ownership](20-lifetime-returns.md),
   [MSRB on full issuer calls and maturity](https://www.msrb.org/Rules-and-Interpretations/MSRB-Rules/General/Rule-G-15)
   and [MSRB on separately elected put quotes](https://www.msrb.org/Rules-and-Interpretations/MSRB-Rules/General/Rule-G-12?tab=3).

3. **Prepare the buyer cash and payoff from named legal phases.**
   Identify the scenario and full right identity, notice, exercise,
   principal-effective and actual cash dates. Keep legal maturity,
   last payment and any benchmark endpoint separate. Include only
   the prepared buyer's entitled cash given each authorized phase.
   An eligible known payable settlement-day claim can have factor
   one under the chosen quote law. Keep vested later and co-dated
   independent payments once, and exclude detached or holder-only
   cash from the buyer strip. Prior and senior economic changes
   determine the outstanding before the exit. Keep those changes
   without applying a synthetic maturity principal add-back.
   Replace extinguished continuation and ordinary principal only
   on the exercise path. Pay the authorized dirty redemption once.
   For a clean percent use qualified outstanding times percent plus
   holder-payable accrued. Keep exercise accrual separate from the
   market accrued used for clean/dirty quote conversion. For a
   qualified make-whole resolve the actual reference law and payable
   amount with its own benchmark and deterministic scenario. Apply
   BONDS-05 only to an independently authorized protected call
   with the correct history and paid-through state. No synthetic
   floor call, par covenant or extra accrued on a full dirty payoff.
   Scheduled maturity retains its own contractual cash and actual
   payments without synthetic redemption. Freeze qualified projected
   cash, historical fixings and projected payoff assumptions per
   candidate and distinguish forecasts from known payable amounts.
   Check that all required claim identities and the selected full
   redemption are represented once.
   [Buyer and holder cash](01-schedule-and-holder-cashflows.md),
   [return-floor cash and strike](05-return-floors.md) and
   [filed notice, redemption and coupon terms](https://www.sec.gov/Archives/edgar/data/1216596/000121659610000041/ex41indenturer5.pdf).

4. **Solve and report each admitted candidate's finite yield.**
   Use the elected BONDS-10 law for the exact dated claims. Preserve
   original coupon and quasi-coupon grids, branch rules, applicable
   frequencies and contextual times. An early call or delayed cash
   date is not automatically an admissible Street or Moosmüller
   coordinate. Amount-net only identical full discount-law
   coordinates after preparing the claim map. For inversion require
   on each path a positive dirty target above yield-independent
   cash, nonnegative yield-sensitive net amounts and at least one
   positive yield-sensitive bucket under the admitted monotone law.
   Use BONDS-10's matching analytic or bounded checked solver and
   verify the actual returned yield against the same dirty target
   with the explicit native-currency residual tolerance. All-
   immediate cash identifies no finite yield even when it equals
   price. A target at or below immediate cash with remaining
   positive yield-sensitive cash has no admitted inverse. Preserve
   these distinct causes, as well as invalid amounts, missing cash,
   unavailable quote domains and numerical failures. Do not return
   infinity or drop a failed required branch from the minimum.
   Compare finite represented yields before display rounding and
   retain the exact set of co-minimizing identities. A fixed-path
   request needs its own path only. A diagnostic ladder may carry
   labelled unavailable yield or failed-row reasons, with no numeric
   complete YTW if a required branch fails. An explicit full YTW
   request surfaces that cause. Optional noncontributing rows cannot
   select the minimum.
   [Monotone yield and returned-price checks](10-quoted-yield.md).

5. **Invert the complete YTW only on matched monotone branches.**
   Freeze the full candidate set, actual cash, quote law and common
   domain while pricing. For a requested finite yield `y*`, require
   each admitted `P_i(y*)` to be finite on its strictly decreasing
   dirty-price functional. Each forward root uses its own admitted
   branch domain on the same elected quote convention. On the
   common invertible price range,
   `y_i(P)≥y*` iff `P≤P_i(y*)`. At least one equality is required
   for `min_i y_i(P)=y*`. Hence the only candidate dirty price is
   `min_i P_i(y*)`. This is conditional on every required branch
   being able to invert the same price. An origin-only branch or
   another branch's independent cash floor can defeat admission
   even if other branches price successfully. Apply the prescribed
   Money and requested quote-output boundaries. On the actual
   represented output, recover dirty using the same conversion
   when needed, require a positive admissible value, and rerun the
   full forward ladder. Check the represented dirty amount against
   the raw minimum within the explicit positive native-currency
   tolerance, require each returned branch yield to reproduce that
   amount under BONDS-10, and check the returned minimum against
   `y*` within the explicit positive yield tolerance. Return the actual
   verified minimizing identities before display rounding. Reject
   invalid ranges, missing branches or nonreproduction. No alternate
   lowest-price fallback. Fixed-workout pricing remains the single
   elected path's own checked price/yield conversion.

6. **Keep spread benchmarks and downstream path selection explicit.**
   A pure yield or YTW request requires no swap or government
   benchmark. If requested, report a row's I-spread using
   BONDS-12's qualified swap rate, exact benchmark horizon and
   source role. A separate fixed-path G-spread follows the same
   benchmark-identity requirement for its sovereign source.
   An available YTW or tie does not silently select a
   benchmark or make the minimum I-spread equivalent to YTW.
   Keep I-spread price inversion on an explicitly identified path,
   with BONDS-12's forward reproduction and residual. Automatic
   price-dependent workout I-spread inversion is outside this
   package. Changing the benchmark across branches can leave no
   consistent price or more than one. Remove the date-only match
   and lowest-price substitution. No G-spread inverse is introduced.
   A consumer requiring a fixed exit must elect its exact identity
   and qualify its own cash, factors, hedge or model basis. A
   co-minimizing set supplies no default choice, and a distinct
   right sharing its date does not match it. Z/DM, I/G, ASW,
   yield risk, no-hazard CS01, carry and WAL retain the accepted
   owner rules even when they can reuse the prepared workout.
   Lifetime returns keep the separate issue investment, ledger,
   timing and complete eligible scenario rules of BONDS-20.
   [Spread benchmarks and fixed-path inverse](12-spread-quotes.md),
   [asset swaps](13-asset-swaps.md), [yield risk](14-yield-risk.md),
   [spread risk](16-spread-credit-risk.md) and
   [carry and repayment life](19-carry-and-life.md).

7. **Preserve the ladder, quote and path basis across consumers.**
   Use one checked Rust preparation and pricing law across
   `ytw`, `bond_workout_ladder`, direct and registry conversions,
   quote sets and eligible fixed-path consumers. Publish the original
   price provenance, valuation and settlement cuts, currency and
   denomination, selected quote law, full stable candidate IDs,
   dates/phases, exercise accrued versus market settlement accrued,
   payable redemption
   and row eligibility, selected/tied identities, and associated
   availability or failure reasons. Keep rate and price quantities
   typed in APIs and metadata. Preserve deterministic row order with
   no effect on tie selection. An absent benchmark or unrequested
   auxiliary measurement leaves independent results available.
   Reuse a prepared result only on the same claim, controls, quote
   and snapshot. Validate complete result shape and wire state,
   reconcile result tables, schemas, Python, WASM, UI and reporting,
   and remove obsolete clean-only or infinity-based representations.
   Keep the cash-equivalent structural calibration target scope
   with BONDS-09. Qualification includes candidate coverage,
   exact cash replacement, root eligibility and residuals, and
   host agreement at the same phases and on the same inputs.
   [Shared metric result basis](../valuations-framework/metric-orchestration.md),
   [cash-equivalent calibration](09-merton-calibration.md) and
   [public metric identity](../valuations-framework/0010-metric-identifiers.md).

## Observed behavior

### Choice and scope

Workout calculations enumerate contractual exits and use deterministic yield discounting; they do not optimize stochastic exercise. Option-bearing forward YTW/workout metrics require an explicit clean-price quote, including lowered return floors. Plain bonds can derive their dirty target from model PV carried to settlement. This separate quotation convention also supplies selected callable duration/spread cashflows.

### Algorithm

Generate call/put window start and end dates plus cashflow dates within each window, restricted to settlement through maturity; unlike tree exercise, ordinary workout enumeration adds no month ends. Calls sort by date then increasing strike. Puts on the same date collapse to the highest strike. Append maturity at the later of contractual maturity and last holder payment.

For each exit keep cashflows up to the exit, including same-settlement cash only when an exercise window contains settlement. Determine outstanding after economic events, adding contractual maturity principal back at maturity. Redemption is strike percentage of outstanding plus accrued, or make-whole-adjusted clean redemption plus accrued; subtract any same-day contractual maturity redemption already retained. Maturity path preserves its actual cashflows and appends zero additional redemption.

Solve each path's Street yield against target dirty price after subtracting immediate settlement cash. With no future cash, yield is negative infinity if immediate recovery is below target, otherwise positive infinity. If immediate cash already meets residual target, yield is positive infinity. Select the strict minimum across **calls, puts and maturity**; tie selection follows enumeration order. A put is thus treated as a candidate adverse exit, not an economically optimal holder exercise strategy. Ladder outputs convert nonfinite yields/spreads to nullable fields and report the selected row.

Inverse YTW computes each candidate path's price at requested yield and returns the minimum. I-spread inversion adds each path's horizon par rate, computes candidate prices, sorts by price, then returns the first candidate whose recomputed worst-yield path has the same horizon. If none is consistent it returns the lowest candidate price; it does not error for inconsistent workout selection.

### Limits, alternatives and evidence

Exercise windows are discretized differently from model exercise; workout ordering can include voluntary put paths and ignores call friction. Make-whole/redemption depend on today's curves rather than future conditional rates. Infinity handling and fallback are implemented quote behavior, not accepted market standards. Optimal call/put game valuation is available in trees/LSMC and should not be conflated with this ladder.

Owning code: [workout algorithms](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/yield_price.rs), [ladder/target](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/workout_ladder.rs), [quoted-workout cache selection](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/spread_price.rs), [YTW metric](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/ytw.rs). Inline tests and [workout tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/workout_ladder.rs) locate amortization, same-day exercise and row invariants; not rerun. [Fixed-income reference](../../../REFERENCES.md#fabozzi-fixed-income-handbook) is general background.

Consumer evidence includes the [quote engine](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/compute.rs),
[ladder result schema](../../../../finstack-quant/valuations/schemas/results/1/bond_workout_ladder.schema.json),
[Python ladder](../../../../finstack-quant-py/finstack_quant/valuations/instruments/_bond_workout.pyi)
and [WASM fixed income](../../../../finstack-quant-wasm/src/api/valuations/typed/fixed_income.rs).

## Independent arithmetic and qualification

- With annual nominal discounting, a call paying `100` at one
  year and maturity paying `100` at two years have prices
  `100/(1+y)` and `100/(1+y)^2`. For YTW `+.05`, the inverse
  candidate is `40000/441` and candidate yields at that price
  are `+.1025` and `+.05`. For YTW `−.05`, the inverse is
  `2000/19` and the call selects the minimum. Dirty `100`
  gives a zero-yield tie with both identities preserved.
- A branch with entitled immediate `105` and a later positive
  payment cannot have a finite yield at a dirty price of `100`.
  Even when another branch yields a candidate inverse price
  of `100`, the full YTW price reproduction fails. Immediate-only
  cash of `105` at price `105` also supplies no identified yield.
- For the same two annual nominal paths, use benchmarks `.04`
  and `.06` for the call and maturity. Requested automatic
  I-spread `−.05` then gives two prices consistent with their
  own YTW branch, `100/.99` and `100/(1.01)^2`. Reversing the
  benchmarks gives `100/1.01` and `100/(.99)^2`, each in the
  other branch's region. The automatic equation has no solution
  in that case. Fixed-path inversion has a different, explicit
  scope.
- If earlier economic amortization of `20` leaves par `80`,
  a qualified clean call of `101` per 100 with payable accrued
  `4` pays dirty `84.80`. Keep an independently payable co-dated
  coupon `3` once and a separately vested later `6` at its own
  date. Remove the replaced ordinary principal. The `20` enters
  the quote strip only if the buyer has that still-payable claim.

Exact rational checks reproduce the YTW inverse and ties,
I-spread overlap and gap, and replacement cash. A 70-digit
calculation checks the branch yields at both consistent I-spread prices.

Qualification includes whole legal date/window coverage, notice
and settlement cuts, same-date rights, accrued and lagged cash,
qualified make-whole and protected calls, no-yield paths, tie
identity, represented Money roundtrips, yield-law branch changes,
benchmarks that fail independently, and Rust/host/table agreement.
The arithmetic establishes the examples' price and selection
properties. Runtime and venue-specific quoting remain unqualified.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
