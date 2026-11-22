# VALUATIONS-OTHER-FIXED-INCOME-16: Fixed-income index TRS carry returns and duration risk proxy

- **Decision status:** Accepted 2026-11-14 — complete fixed-income index TRS package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below. Reconciliation to the accepted contract is pending.
- **Validation status:** Live source and primary references inspected. Independent cash, carry, financing, annuity, spread and duration arithmetic checked. Market, margin, runtime and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked fixed-income index total-return swaps

1. **Bind the exact contractual total-return series and cash entitlement.**
   Use one fixed nonnegative cash notional `N`, common currency, explicit
   receive- or pay-return side and one qualified discount source for both
   legs. Bind the administrator, exact total-return series and variant,
   currency, observation dates, publication authority and versions as of the
   information cut. Keep observation, financing reset, accrual and actual
   payment distinct. From positive finite exact observations on the same
   series, compute signed return cash `N × (I_end/I_start−1)` with checked
   arithmetic. The total-return series owns constituent price and income
   effects. An expressly agreed initial level may establish only the first
   observation. A confirmed amount may replace the formula for its exact leg,
   parent, currency and cash date. Require the eligible exact observation when
   due. Missing required history or mismatched series is an error. A live
   quote qualifies as a same-day fixing only under its publication authority.
   [Shared TRS contract](../valuations-framework/trs-legs.md#accepted-decision-for-checked-trs-legs-and-par-spread),
   [market observations and fixings](../core-market/scalar-series-and-fixings.md),
   [iBoxx index methodology](https://www.spglobal.com/spdji/en/documents/methodologies/methodology-iboxx-bond-index-calculus.pdf).

2. **Require explicit carry for the eligible unresolved portion.** Elect a
   finite annual continuously compounded rate `c` on ACT/365F, allowing
   explicit zero and negative values. Remove omitted-rate zero. If both
   endpoints remain future and unresolved on the same exponential path,
   use `expm1(c × days(start,end)/365)` without a live level. With an
   authoritative start level and an eligible unresolved end, require the
   positive same-basis live level `I_V` at the elected cut. Compute
   `I_V × exp(c × days(V,end)/365)/I_start − 1`, including a binding first
   observation already known before its date. Apply checked stable ratio
   and exponential arithmetic. An eligible same-day projected end requires
   the explicit unpublished phase and a qualified live anchor, with zero
   remaining carry. Resolve confirmed cash and exact fixings first and load
   only active scenario inputs. No projected substitute for missing due
   history. Label PV and par using this projection as carry-scenario
   outputs. Market marking requires an independently qualified basis.

3. **Inherit shared financing and the actual payment lifecycle.** Compile
   the admitted parent grid and separate contractual observations, financing
   resets and actual payments once under FRAMEWORK-06. Elect simple-term
   or compounded-overnight financing. Use `N × (H_i+s A_i)` for the
   financing payment, where `H_i` is admitted native floating growth, `s`
   the signed decimal spread and `A_i` the contractual financing accrual
   fraction. Keep signed return and financing cash. Receiver PV is discounted
   return less discounted financing. Reverse it for the payer. Fixed cash
   survives economic expiry through its actual payment and entitlement
   phase. Eligible unpaid same-day cash has factor one. Settled cash leaves
   the residual. Require explicit treatment of unresolved overdue amounts.
   Independent return grids or special settlement terms require separate
   product qualification. [Shared TRS financing and dates](../valuations-framework/trs-legs.md),
   [iBoxx standardized TRS terms](https://www.spglobal.com/spdji/en/documents/methodologies/iBoxx%20Standardized%20TRS%20Trading%20Convention%20Guide%20%28September%202022%29.pdf).

4. **Use the matching prepared cash basis for annuity and par spread.**
   Let `R` be return PV, `F_0` the financing intercept and
   `A=ΣD_i N A_i` over eligible replaceable financing-spread cash. Confirmed
   financing cash stays fixed in `F_0`. A known floating fixing keeps its
   spread weight when the contract amount remains replaceable. Annuity is
   currency per unit decimal spread and is available independently of index
   observations and floating rates. For positive `A`, quote the complete
   side-independent replacement `10,000 × (R−F_0)/A` bp. Preserve signed
   fractional bp. Reprice on the same eligible basis to the explicit positive
   native amount residual tolerance. Zero annuity is a valid result and
   leaves par unavailable. [Shared affine quote and residual rules](../valuations-framework/trs-legs.md).

5. **Identify distinct rate-risk, hedge and margin bases.** Combined and
   bucketed DV01 reprice both legs on the actual discount and active
   financing projection sources. Hold known index observations and the
   independent carry experiment fixed. Preserve exact roles and shock
   coordinates, signed raw currency per positive bp and the selected result
   basis. Fixed unpaid cash can retain discount risk. Retain `duration_dv01`
   as an explicit hedge diagnostic only for a qualified live index exposure:
   `−side × N × D × .0001`, with receive side `+1` and an authored matching
   nonnegative duration `D` in years on the identified index, date and yield
   convention. The diagnostic has no exposure once the index payoff is
   fixed. Constituent CS01 and index delta remain unavailable under this
   profile. Preserve the matching direction and basis in attribution.
   [Shared curve risk](../valuations-framework/finite-difference-rates.md).

   Remove duration-to-SIMM tenor bucketing and automatic carry-PV selection
   for VM. Require a complete authorized sensitivity, classification and
   source basis for the requested regulatory calculation, or the authorized
   mark for the applicable VM agreement. Preserve netting-set membership
   and surface missing qualifying data in margin outputs. A curve-risk
   number computed with direct zero-rate shocks does not certify an ISDA
   market-rate risk input. [ISDA SIMM risk factors and sensitivities](https://www.isda.org/a/2IOgE/ISDA_SIMM_v2.7_PUBLIC.pdf#page=10),
   [portfolio netting and initial margin](../portfolio-execution/16-margin-netting-and-initial-margin.md).

6. **Preserve identity and status through Rust, export and hosts.** Use
   the same checked prepared per-leg and per-parent cash stream for PV,
   metrics and reporting. Retain payment date, signed cash, known or
   projected status, source, observation and phase. Report index-return
   settlement outside coupon and principal measures. Replace the current
   fixed-coupon net schedule. An export without a faithful representation
   of the return payment reports it as unavailable. A horizon roll crossing
   a fixing or cash boundary requires matching resolved or explicitly
   hypothetical state. Keep funding projection candidates separate from
   index observations. Reconcile native/registry results, attribution,
   portfolio and margin, generated schemas and UI, and existing JSON-first
   Python/WASM routes. [Checked valuation export](../valuations-framework/cashflow-export.md),
   [horizon fixing materialization](../cashflows/16-fixing-materialization.md).

## Observed behavior

### Choice and rationale

[FI index TRS pricing](../../../../finstack-quant/valuations/fixed-income/src/fi_trs/pricer.rs) uses continuously compounded index yield for projected total return. Source explicitly selects carry analytics to retain yield sensitivity and explicitly disclaims a full index mark-to-market model. Both native and registered generic pricing use this same kernel; no alternative model selector supplies omitted underlying price risk. That documented intent is not approval for fair-value or margin uses.

### Algorithm and selection

1. [Validate](../../../../finstack-quant/valuations/fixed-income/src/fi_trs/types.rs) finite nonnegative notional, matching underlying currency, schedule/financing inputs and positive finite optional initial level. Read yield from a strict unitless scalar if an identifier exists; configured missing/wrong-type data error. No identifier means intentional zero carry.
2. For fully future return period `[start,end]`, project `exp(y×year_fraction(start,end))−1` on the schedule day count. Yield must already be an annual continuous decimal; the engine does not convert a supplied bond-equivalent yield.
3. For a period in progress, resolve its exact start reset from fixing series, or initial level only at schedule start. Read live positive index level in underlying currency or unitless form, then use `live_level×exp(y×remaining_time)/reset_level−1`. For a completed period whose delayed payment remains relevant, return exact end/start fixing ratio minus one. Missing endpoint fixings error; no interpolation of realized index levels occurs.
4. Shared [TRS engine](../valuations-framework/trs-legs.md) multiplies period returns by trade notional and discounts on the financing discount curve. Financing uses its configured index, fixings, spread and day count through that engine. Receive-total-return PV is return-leg PV minus financing PV; pay side reverses it. Both cashflow legs are aggregated by payment date with the same signs and financing projected-fixing metadata.
5. [ParSpread](../../../../finstack-quant/valuations/fixed-income/src/fi_trs/metrics/par_spread.rs) divides return PV less zero-spread financing PV by notional-weighted discounted financing accrual and reports basis points, always from the receiver's perspective. FinancingAnnuity is the money-times-year denominator; multiplying it by `.0001` gives a one-bp financing-spread PV.

### Risk and margin conventions

[DurationDv01](../../../../finstack-quant/valuations/fixed-income/src/fi_trs/metrics/mod.rs) requires explicit finite unitless index duration in years. It reports `−N×D×1e−4` for receiving total return and positive for paying. This measures intended economic long-bond yield risk; it is **not** the derivative of the carry-only pricing function. There is no fabricated default duration. The resolver checks finiteness but does not impose nonnegative duration.

Shared parallel/bucketed DV01 shocks financing discount/forward curves and reprices the carry model. It therefore captures financing/discount effects, not a marked-to-market underlying bond basket. [SIMM input](../../../../finstack-quant/valuations/fixed-income/src/fi_trs/marginable.rs) emits only the signed duration proxy into one currency IR tenor bucket selected from duration; it does not combine full financing key rates, index spread/credit risk or the index constituents. VM mark uses the carry-only PV.

### Limitations, alternatives and evidence

No constituent cashflows, roll-down, duration/convexity repricing, spread migration/default, or rebalancing dynamics are projected. Using yield as expected total return is a scenario/carry assumption, not a demonstrated risk-neutral price. The risk proxy and PV can have inconsistent yield responses by construction. A constituent-based forward mark, full price-index curve or joint rates/credit model are unimplemented alternatives; observed fixing-based realized returns are supported.

[TRS integration cases](../../../../finstack-quant/valuations/fixed-income/tests/fi_trs/test_fi_index_trs.rs) and colocated `duration_risk_requires_a_duration_input` provide located examples and required-input checks. [ISDA definitions](../../../REFERENCES.md#isda-2006-definitions) and [SIMM](../../../REFERENCES.md#isda-simm) are reference context, not certification of this proxy.  No independent constituent-index price reconciliation was established.

## Independent arithmetic and qualification

- With `N=1,000,000`, index `100→97.5`, term fixing `−.01`, financing
  fraction `.125`, spread `+.0025` and payment factor `.997`, return cash
  is `−25,000` and financing cash is `−937.50`. Receive PV is
  `−23,990.3125`, annuity is `124,625` per unit decimal spread, and
  replacement par is `−1,900` bp. Same-day unpaid net cash is `−24,062.50`.
  A proved closed position has zero residual and annuity, and par is
  unavailable. The known unpaid return needs no index carry input.
- With the same notional, fixed start 100 and live level 97, 29 days of 4%
  annual continuous ACT/365F carry produce projected return cash of
  `−26,912.35648`. A confirmed end level of 97 fixes cash at `−30,000`.
- A five-year authorized duration on the live receive profile produces a
  `−500` currency-per-bp hedge diagnostic on 1,000,000 notional.

Validate actual publication and correction phases, all payment states,
selected financing and residuals, model and hedge provenance, independent
market marks, regulatory margin inputs and host reconciliation during
implementation. Repository pricing and host suites were unrun.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
