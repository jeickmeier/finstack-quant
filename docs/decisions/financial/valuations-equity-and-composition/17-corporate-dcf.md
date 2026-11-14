# VALUATIONS-EQUITY-AND-COMPOSITION-17: Corporate DCF terminal value, equity bridge and dilution

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-17`.
- **Decision status:** Accepted 2026-11-14 — complete corporate DCF timing, terminal value, equity bridge, dilution and WACC risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent forecast timing, terminal formulas, WACC, bridge and treasury-stock checks completed. Production valuation, statements consumers and hosts remain to be qualified.

## Accepted decision for checked corporate DCF and dilution

1. **Use one authored valuation date and explicit forecast timing.**
   Keep `discounted_cash_flow` under `discounting`. Preserve the
   requested date, authored effective valuation date and the rule
   that selects it. Use finite signed same-currency FCFF and one
   annual effective decimal WACC `w>−1` on ACT/365.25. Elect
   literal dated receipts or midpoint timing on each authored
   forecast period's actual start and end. Use the exact elapsed
   midpoint on the named clock. Remove the average-spacing shift,
   implicit half-year and time-zero clamp. Give terminal value an
   explicit date at or after the end of the explicit forecast.
   [Accepted valuation dates](../valuations-framework/pricing-lifecycle.md).

2. **Select one terminal formula on a qualified horizon basis.**
   For Gordon, use `C_T(1+g)/(w−g)`. For the H-model, use
   `C_T[(1+g_s)+H(g_h−g_s)]/(w−g_s)`, with finite positive
   `H` equal to half the linear fade duration, and
   `g_h≥g_s`. The latter is the FCFF adaptation of the H-model
   approximation. Both require explicitly qualified normalized
   annual horizon FCFF `C_T` on the forecast basis, finite
   stable growth above −100% and WACC above stable growth.
   Remove inferred annual FCFF and its spacing threshold.
   For exit multiple, require an explicitly matched enterprise
   metric and multiple, both finite nonnegative, at the selected
   terminal horizon. Use checked complete arithmetic and report
   the actual WACC–growth spread for applicable variants without
   a numerical floor. Discount each terminal result at the
   authored terminal date, separately from explicit flow timing.
   [Damodaran on terminal value](https://pages.stern.nyu.edu/~adamodar/New_Home_Page/valquestions/termvalapproaches.htm),
   [Fuller and Hsia's H-model](https://rpc.cfainstitute.org/research/financial-analysts-journal/1984/a-simplified-common-stock-valuation-model).

3. **Bridge enterprise value to common equity once.** Add
   qualified cash and nonoperating assets, subtract debt,
   preferred claims and noncontrolling interests, and retain
   separately identified signed adjustments. Use matched
   valuation date and currency and verify each item's place
   in the FCFF perimeter and capital structure. Report debt
   less cash separately from the full bridge. If elected,
   multiply the qualified positive common-equity pool by
   each `1−discount` for lack of control or marketability.
   Retain the signed negative residual with per-share value
   unavailable.
   [Damodaran on cash and nonoperating assets](https://people.stern.nyu.edu/adamodar/pdfiles/papers/cashval.pdf).

4. **Use the checked treasury-stock active set on its declared basis.**
   With basic shares `B>0`, eligible same-class exercise right
   counts `q_j`, qualified native currency strikes `K_j`
   and adjusted common-equity value `E>0`, solve
   `B p+Σ q_j max(p−K_j,0)=E`. Only strictly in-the-money
   rights enter the active set. Preserve each security and
   strike identity and check complete scaled arithmetic. Report
   per-share value and treasury-stock diluted shares from the
   same raw equity pool. Check the represented result against
   an explicit native-currency residual tolerance. At literal
   zero equity use zero per share and basic shares under this
   convention. Option time value and convertible claim changes
   require their own valuation basis.
   [Damodaran on treasury-stock dilution](https://pages.stern.nyu.edu/~adamodar/pdfiles/papers/esops.pdf).

5. **Reprice the full WACC effect on the prepared raw valuation.**
   Retain signed `dcf::wacc01` of post-bridge, post-discount
   common equity in currency per positive bp. Require an explicit
   centered computation step and checked represented WACC
   endpoints. Reprice WACC in each discount factor and in
   Gordon or H-model capitalization. Hold forecasts, timing,
   growth, terminal multiple or metric, bridge and discounts
   fixed. A failed endpoint fails the stencil. Use the same
   prepared raw basis for enterprise value, equity, per-share
   values and risk. Export explicit FCFF as a scoped projected
   cash view and report terminal value and claims bridge as
   valuation components. Reconcile statement consumers and
   existing Rust, Python, WASM and schema routes.
   [Accepted finite differences](../valuations-framework/finite-difference-rates.md),
   [accepted cashflow export](../valuations-framework/cashflow-export.md).

## Observed behavior

### Choice and rationale

The enterprise DCF discounts explicit cashflows and a caller-selected terminal formula at a single WACC. Equity value then applies a claims bridge and multiplicative valuation discounts. These are observed choices, not approval of a WACC, growth rate, multiple or liquidity discount for a company.

### Scope and input domain

Finite increasing flows strictly after the configured valuation date, WACC in (-50%,100%) as decimal annual rate, consistent instrument currency and optional positive basic shares/dilutive security counts. The market context does not supply WACC or growth. Growth terminal formulas require an annual terminal-flow override when average forecast spacing is below 0.75 years.

### Algorithm

1. Use actual days/365.25 for times. With mid-year convention, explicit cashflow times shift earlier by half average flow spacing (half a year for a single flow), floored at zero; otherwise use full dates.
2. Gordon terminal value is `CFterminal×(1+g)/(WACC−g)`, requiring WACC>g and warning when the spread is below 0.001. Exit-multiple terminal value is metric×multiple. H-model value is `CFterminal×[(1+gstable)+half_life×(ghigh−gstable)]/(WACC−gstable)` with positive half-life, high growth at least stable growth and WACC above stable growth.
3. Discount exit-multiple value at the full terminal horizon. Growth terminal formulas use the ordinary mid-year shift unless an annual terminal-flow override is supplied, in which case the terminal shift is half a year. Enterprise value is explicit-flow PV plus terminal PV.
4. Subtract the explicit equity-bridge net claims, then multiply by each `(1−discount)` factor. Bridge cash/nonoperating assets offset debt/preferred/minority/other liabilities.
5. For dilution, sort security exercise prices and solve the active set `price=(equity_value+exercise_proceeds)/(basic_shares+in_the_money_securities)`. Convert this to treasury-stock diluted shares and require the reconstructed equity residual to be within `1e-10×max(abs(equity_value),1)`; return no diluted result if the solvable domain is absent. Per-share metrics require shares.
6. Wacc01 uses the shared rate-component bump: change both discount factors and the growth terminal denominator at the terminal bump, preserving bridge and discounts.

### Selection, controls and conventions

`DiscountedCashFlow`/`DcfPricer` use the instrument's configured valuation date even if the caller requests another date, and stamp that date on the result. Market discount curves are not dependencies. Cashflow export includes explicit projected flows only; terminal value and the equity bridge are valuation constructs, not exported contractual cashflows.

### Limitations, failures and alternatives

Mid-year spacing is an average-grid approximation; it does not model actual intra-period receipt patterns. Growth terminal values become highly sensitive near WACC=g despite the warning. Exit multiples and annual terminal overrides are supported alternatives; varying discount rates, explicit fade paths and alternative dilution conventions would require another methodology. A returned price does not substantiate economic assumptions.

### Evidence

[corporate valuation reference](../../../REFERENCES.md#koller-valuation) and [DCF reference](../../../REFERENCES.md#rosenbaum-pearl-2020); inline [types.rs](../../../../finstack-quant/valuations/equity/src/dcf_equity/types.rs) includes manual Gordon/H-model, timing, bridge and large-warrant active-set identities; [pricer.rs](../../../../finstack-quant/valuations/equity/src/dcf_equity/pricer.rs) checks configured-date behavior and instrument/pricer agreement. These are formula and implementation-consistency evidence, not company forecast validation.

## Independent numerical checks

- Forecast periods on elapsed days `[0,90]` and `[90,365]`
  with flows 15 and 85 use midpoints 45 and 227.5 on ACT/365.25.
  At WACC .08 the explicit midpoint PV is 95.879998631628.
  Terminal FCFF 120 and growth .02 give Gordon terminal value
  2,040 at day 365 and enterprise value 1,984.868391069042.
  With no bridge or valuation discounts, the full centered
  1 bp WACC stencil gives −3.327952064358 per bp. Changing
  discount factors alone gives −.179628819138. With the same
  timing, H-model half-fade duration 2 years and high growth
  .08 give terminal value 2,280. A 9× exit multiple on 180
  gives terminal value 1,620.
- A separate bridge gives 1,200 prior to elected discounts
  and 864 after .10 and .20 discounts. For 80 basic shares
  and right classes `20@0`, `40@6` and `30@8`, the
  checked exercise identity gives 7.885714285714 per share
  and 109.565217391304 diluted shares, with strike 8 inactive.
  At equity 880, price is exactly 8 and that class remains
  inactive.

Repository pricing, statement reconciliation, economic forecast
qualification and host suites remain unrun for this review.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/dcf_equity/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/equity/src/dcf_equity/pricer.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/dcf_equity/metrics.rs)
