# MODELS-NUMERICAL-002: Escrowed cash dividends and lattice exercise ordering

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Source and caller paths inspected. Independent exact-date, claim-value, deterministic and finite-CRR checks were executed. Production, external pricing and risk qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked escrowed dividends and lattice exercise

1. **Select the escrowed stock model explicitly.** Use the checked
   CRR or strike-centered Leisen–Reimer (LR) lattice for plain vanilla
   calls and puts under a deterministic cash reserve. Select the
   carry, discount, volatility and exercise inputs at the financial
   owner. The existence of a cash schedule does not select a model
   or change the meaning of a rate. With no nonzero modeled cash
   events and the same eligibility, recover the ordinary lattice.
   [MODELS-NUMERICAL-001](equity-binomial-construction.md#accepted-decision-for-checked-crr-and-leisenreimer-lattices)

2. **Admit actual dated cash events at the owner.** Resolve the
   configured schedule, security/share class, share and quote basis,
   currency, source snapshot and information cutoff. Validate the
   original ordered, identified events, including exact gross cash
   per share and both ex and pay dates. The owner qualifies amounts
   and dates as deterministic model assumptions from information
   available at valuation. Preserve the distinction between
   announced and forecast amounts. Reject incompatible units and
   unsupported entitlement, due-bill or corporate-action regimes.
   [CORE-MARKET-13](../core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates)

3. **Value each fixed payment at its actual ex boundary.** For a
   supported cash event with ex date e and pay date p, use
   `J=D*P(e,p)`, where P is the owner's qualified deterministic
   same-currency relative discount factor for the claim. Require
   positive finite factors, permit factors above one, and use
   exact one on the same date after validation. Propagate missing
   or invalid discount data. Positive cash paid before ex requires
   different entitlement economics and is unsupported by this
   adapter. This ex-claim value is the model jump. The source cash
   remains payable on its actual date.
   [Dated relative discount factors](../core-conventions/016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting)

4. **Prepare one jump per qualified ex phase.** Value payments
   separately before combining simultaneous ex claims. Retain each
   contributing event ID and pay date. Combine only the compatible
   claims at one qualified transfer boundary, with no intervening
   exercise or other rights inferred from source storage order.
   Keep validated exact-zero rows in source provenance. They create
   no jump or required cash node by themselves. Preserve a positive
   admitted claim through the checked numerical conversion, or
   report failure.

5. **Use an explicit underlying reserve horizon.** The owner
   declares the model-year reserve horizon H, H ≥ T, and the
   complete modeled ex-event set through H under the selected
   cutoff and carry assumptions. H belongs to the selected
   underlying model and is independent of an individual option's
   expiry. Include claims with ex after T and through H in the
   deterministic reserve at earlier times and at T. They require
   no option rollback node after T. A pay date may extend beyond
   T or H when its ex claim is qualified. Across expiries under the
   same stock model, reuse the same dated reserve horizon, event
   set and underlying carry/volatility assumptions at that origin.
   [Escrow references](../../../REFERENCES.md#checked-escrowed-dividends-and-lattice-exercise)

6. **Declare residual continuous carry and the volatility basis.**
   With the record 001 flat inputs, use b=r−q for the stock and
   escrowed component. q is the signed continuous yield remaining
   after the selected cash distributions. The owner qualifies this
   carry through the reserve horizon on the declared clock. Price
   diffusion with the annual lognormal volatility of X, the risky
   component. Between cash events the model has
   `dS=b*S*dt+sigma*X*dW`. The full stock's proportional volatility
   is sigma*X/S. A full-spot market volatility requires an explicitly
   qualified input adapter. The ex-to-pay claim discount, reserve
   carry and option rollback discount serve distinct purposes.

7. **Require exact placement of live model events and rights.**
   On the requested homogeneous lattice, use the canonical label
   for every in-lattice event time. Require exact equality for
   numerical times. The date adapter may prove exact placement
   on its declared clock before float conversion. For example,
   under ACT/365F, `N*elapsed_days=j*expiry_days` proves node j.
   Use that node's canonical label consistently in reserve and
   rollback. Require exact placement of live cash jumps and
   explicit exercise opportunities inside the option horizon.
   [Exact time-grid ownership](../core-numerics/time-grids.md)

8. **Reject incompatible grids or ambiguous chronology.** Retain
   actual date and phase order before numerical conversion. Reject
   an unrepresentable required time, off-grid event, failed clock
   conversion or distinct required boundaries collapsed to one
   coordinate without a qualified same-node phase ordering. A
   required positive interval between rights or events cannot be
   represented as an instantaneous phase change.
   Report the offending event or opportunity and selected clock
   and grid. Do not snap, clamp, silently omit, reorder or deduplicate
   opportunities, insert hidden steps or substitute another lattice.
   An event halfway to expiry cannot align with any odd LR count.

9. **Require an explicit origin and terminal phase.** The owner
   supplies the spot phase, exercise eligibility and terminal payoff
   phase. At a pending root ex event, a qualified cum quote includes
   the current claim in the opening reserve. A qualified ex quote
   excludes the crossed jump, even when cash has yet to be paid.
   Earlier events and receivables stay with the financial owner.
   At a coincident expiry/ex boundary the terminal payoff uses the
   specified cum or ex phase. A cum exercise opportunity before an
   ex terminal payoff requires its own live right. Exercise and
   settlement cutoffs are product inputs.

10. **Evolve one risky component and reconstruct phase-specific spot.**
    At time t use
    `R_ex(t)=sum_{e>t} J_e*exp(-b*(e-t))` and
    `R_cum(t)=R_ex(t)+J_t`, with the sum restricted to the selected
    live reserve events through H and zero J away from a jump.
    Start from `X0=S0-R_origin(0)`, which must be strictly positive
    for a live escrowed lattice. Evolve X with the selected checked
    lattice and reconstruct S=X+R at each phase. X is unchanged
    across the instantaneous ex boundary. Before rounding, the
    difference in full spot between its two phases is exactly J.

11. **Center stochastic LR on the terminal risky-component strike.**
    For selected terminal reserve R_terminal use
    `K_X=K-R_terminal` to construct LR. This follows from terminal
    intrinsic on X+R_terminal. Require positive K_X for stochastic
    LR, including when the reserve contains ex events after expiry.
    Reject unsupported LR geometry without switching methods.
    CRR uses its checked strike-independent transition. All exercise
    comparisons use the original K and full-spot payoff, including
    at phases with a different reserve.

12. **Roll back only explicitly eligible phases.** Start at an
    interior cash node with discounted continuation on the ex side.
    Compare with ex intrinsic only if ex exercise is eligible.
    Carry that value unchanged through the instantaneous jump
    when stepping backward to a live cum phase. Compare with cum
    intrinsic only if cum exercise is eligible. Take the maximum of both
    intrinsic values and continuation only when both rights exist.
    Waiting through the jump gives a plain option holder no extra
    cash. The product qualifies deliverable and settlement
    entitlement before choosing this payoff.

13. **Use one terminal payoff and a prepared exercise set.** Set
    terminal value to intrinsic at the selected phase with no
    surviving continuation. Process any qualified earlier same-node
    right before an ex terminal payoff in backward order. European
    has the terminal payoff only. Bermudan uses canonical increasing,
    unique eligible (node, phase) opportunities, with cum before ex
    in forward order. American tests the contract's eligible grid
    nodes and phases, including the root only when exercisable.
    At ordinary nodes there is a single phase. Eligibility and
    historical exercise remain separate from model expiry.

14. **Preserve the exact deterministic and expiry boundaries.**
    For zero volatility and positive T use one path
    `X(t)=X0*exp(b*t)` with the same reserve, payoff phases and
    exercise logic. LR still requires an odd configured count.
    The stochastic effective-strike constraint only applies when
    building its transition. At T=0 require one owner-qualified
    terminal-phase spot and no unresolved same-time transition
    or earlier exercise right. Validate applicable inputs and
    return literal intrinsic under record 001.

15. **Use one checked cash-capable rollback.** Support the same
    prepared dividend and exercise contract for numerical European,
    Bermudan and American pricing. Prepare reserves from the
    ordered jumps and post-expiry reserve in one backward sweep
    or an equivalent linear pass. Check input shape, counts and
    resources, including event preparation and eligible comparisons
    in the requested lattice work. Use stable complete products,
    sums and differences through reserve, intrinsic and rollback.
    Preserve an intrinsic difference that would be lost by
    rounding reconstructed spot before subtracting the strike.
    Error on an exhausted risky component or unresolved required
    arithmetic. No cash, spot or premium floor, or volatility
    substitution. Keep generic discounting, OAS and callbacks
    with [record 003](tree-discounting-and-induction.md).

16. **Reconcile actual product, risk and host paths.** The early
    equity option path prepares raw spot and the selected escrow
    once, with the same inputs for pricing and risk. Remove
    the separate European reserve precheck, ex-only cash vectors
    and schedule-driven yield suppression. Equity and commodity
    propagate dated eligibility and clock failures. Commodity
    retains its own continuous carry and supplies no synthetic
    cash. Reprepare from authoritative absolute dates for valuation
    rolls and scenario/risk experiments with the chosen clock,
    phase and model. Refuse an inadmissible bumped grid or depleted
    spot. Owners define observed exercise, settlement, entitlement,
    volatility conversion, theta, Dividend01 and analytic European
    dispatch. Update existing Rust product inputs, schemas and
    Python/WASM surfaces together. The prepared lattice remains
    Rust-only.
    [Equity lifecycle](../valuations-equity-and-composition/06-option-lifecycle.md),
    [equity pricing and tree risk](../valuations-equity-and-composition/07-black-tree-and-direct-greeks.md),
    [equity selected risk](../valuations-equity-and-composition/08-selected-risk-and-implied-vol.md)

17. **Qualify model error and grid error independently.** This
    is a displaced stock process with fixed assumed cash and a
    finite exercise grid. It supplies neither a stochastic dividend
    model nor full-spot lognormal dynamics. A common reserve
    horizon preserves the underlying model specification across
    maturities. Strike-centered finite trees still have different
    approximations across strikes and expiries. Require
    independent finite-tree enumeration, a same-model European
    comparison and independent early-exercise evidence. Qualify
    cash size/timing, carry and volatility regimes, phase and
    expiry behavior, tails, dated product/host paths and risk
    convergence with declared price and sensitivity tolerances.
    [Escrow references](../../../REFERENCES.md#checked-escrowed-dividends-and-lattice-exercise)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| ACT/365F with expiry after three days, required event after one day and N=3 | Exact date arithmetic gives node 1 even though the separately rounded `1/365` differs from `(3/365)*(1/3)`. Half-expiry aligns at N=4 and cannot align on any odd N. |
| Cash 10, ex=.5, pay=.75, r=.05, residual q=.02 and flat claim discount | Ex claim ≈ 9.875778004939 after claim discount. Opening reserve ≈ 9.728746825535 after reserve carry. |
| Two separate payments at one ex date, cash 3 and 2, discount factors at ex .98 and at pay .95 and .91 | Aggregate ex claim 467/98 ≈ 4.765306122449, with both payment identities retained. |
| CRR, S=K=100, T=1, N=2, r=.05, q=.02, sigma=.2, claim 8 at .5 | European call/put ≈ 5.67366622620/10.50158467566. Cum-only Bermudan ≈ 7.12941962904/10.50158467566. Ex-only Bermudan ≈ 5.67366622620/11.28826329909. |
| Zero carry and volatility, spot 100, K=95, claim 10 exactly at expiry | Cum terminal call is 5, ex terminal call is 0. With ex terminal and a separately eligible cum exercise right, value is 5. |
| Zero carry and volatility, S=100, K=95, ex claim 10 strictly after expiry but within H | X starts at 90 and terminal reserve remains 10. Full terminal spot is 100 and call is 5. The stochastic LR reference strike would be 85. |
| One- and two-year American calls, CRR h=.5, S=K=100, sigma=.2, r=q=0, claim 99.99 at 1.5 | Truncating each reserve at its expiry gives ≈ 7.059306221467 and 0.001057136973. With the common two-year reserve horizon the same grids give ≈ 0.000705930622 and 0.001057136973, preserving exercise dominance in this fixture. |

These are arithmetic and finite-lattice checks. Dated market-data adapters,
continuous-time approximation error, production pricing and risk remain
qualification work.

## Observed implementation and evidence

### Choice and input domain

The [binomial option valuator](../../../../finstack-quant/models/src/trees/binomial_tree.rs) evolves an escrowed component to retain recombination with known cash dividends. Dividends are positive finite cash amounts in spot units at times in (0,T]; continuous yield remains active. Empty schedules delegate to the ordinary stock tree. This is a model approximation for the stochastic stock component, not merely a cashflow bookkeeping identity.

### Algorithm, controls and conventions

Map each dividend time to round(N·t/T), clamped to [1,N]. Reserve PV is ΣD exp(−(r−q)t) using original times, and X0=S0−PV must be positive finite. Construct CRR or LR using X0 (LR remains strike centered), then evolve X with the same r−q carry and input volatility. At a node add reserves whose mapped dividend steps are later for ex-dividend spot, or later/equal for cum-dividend spot. Reserve discount time uses max(original time−node time,0).

At maturity pay intrinsic on the ex-dividend spot. At eligible interior nodes return max(discounted continuation, intrinsic(ex spot), intrinsic(cum spot)). American selects steps 0..N−1. Bermudan delegates date mapping to core `map_exercise_dates_to_steps`, sorts and deduplicates the result. The engine does not insert exact dates or substeps. `price_european` has no cash-dividend argument.

### Limitations and alternatives

Exercise immediately before and after mapped cash events is represented by a maximum on one node; actual cash times still enter the reserve. The approximation combines original dividend times in reserves with mapped times in exercise and applies volatility to the escrow component. Invalid dividend times/amounts and a reserve exhausting spot return errors. There is no spot floor fallback. An event-aligned non-recombining tree or jump-interpolated PDE would be alternative implementations, not enabled variants. Date validation inside the external core mapper belongs to that owning slice; callers should not infer the exact-date contract of the Hull–White tree.

### Evidence

Inline `cash_dividend_tree_preserves_continuous_carry_in_deterministic_limit` checks the zero-volatility carry identity; `vanishing_cash_dividend_converges_to_continuous_yield_price` checks the small-cash limit; `cash_dividend_at_maturity_is_ex_dividend_at_expiry` checks expiry ordering. `test_bermudan_between_european_and_american` is an inequality check, not an independent American benchmark. [Hull textbook](../../../REFERENCES.md#hull-options-futures) is general background; it does not independently validate this specific escrow/exercise convention.

The equity early-exercise path first runs the European adjusted-spot
check, then refetches raw spot and suppresses q when future cash is
present. Both equity and commodity Bermudan preparation can discard
failed date conversions. Equity theta shifts numerical times and floors
rolled spot after subtracting cash. Equity Dividend01 currently changes
cash inputs and suppresses continuous yield in its repricer.
[Equity market inputs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/inputs.rs),
[equity tree and theta](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/black.rs),
[equity dividend risk](../../../../finstack-quant/valuations/equity/src/equity_option/metrics/dividend_risk.rs),
[commodity option](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/types.rs)

## Relationship to the accepted cash-event adapter boundary

[CORE-MARKET-13's complete accepted package](../core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates) retains full cash-event
identity, ex/payment dates and exact share/currency basis at the financial
source. This numerical tree's single-time reserve/jump inputs require a
qualified adapter for payment-delay claim value, same-date aggregation and
exercise phase, or explicit refusal. The source package was accepted on
**2026-11-08**. The numerical contract and its adapter requirements were
accepted on **2026-11-11**. Source and pricing reconciliation and qualification
remain open.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
