# Statements reconciliation plan

**Created:** 2026-11-15 at HEAD `b523c5bb5c`.
**Covers:** the 40 statements records accepted on 2026-11-15
([capital 01–09](../decisions/financial/statements-capital/README.md),
[contracts 01–16](../decisions/financial/statements-contracts/README.md),
[evaluation 01–15](../decisions/financial/statements-evaluation/README.md))
and their 45 entries in
[financial/FOLLOW_UPS.md](../decisions/financial/FOLLOW_UPS.md#statements-capital-structure):
41 Reconciliation (K-003 has two), 2 Investigate and 2 Evidence.
**Replaces:** the earlier "Blocker & High-Severity Audit Findings" remediation
draft, which predates the accepted decisions and contradicts them (see
[Superseded draft](#superseded-draft)).

Record shorthand used below: `K-nnn` is STATEMENTS-CAPITAL, `C-nnn` is
STATEMENTS-CONTRACTS, `E-nnn` is STATEMENTS-EVALUATION.

## Ground rules

- **No decisions are reopened.** Each slice implements its record's
  `#accepted-decision-…` section. This plan links to those sections and does
  not restate them. Read the record in full at the start of its slice; the
  summaries here are for ordering only.
- **Anything a record leaves open goes to
  [Owner resolutions](#owner-resolutions).** Nothing is guessed.
- **Removals have no aliases, defaults, or fallbacks** (AGENTS.md).
- **Re-check the baseline per slice.** The records' "audited path" text
  describes source revision `7e82fd03ad`. Before changing code, confirm each
  discrepancy still exists at HEAD with a failing test. Drop any that does not
  reproduce and say so in the commit body.
- **A slice is a full triplet:** Rust, then Python (`src/bindings/statements`,
  `.pyi`), then WASM (`src/api/statements`, `exports/**`), then
  `parity_contract.toml`, then `mise run gen-write`. Schemas, UI fixtures and
  notebooks moved by the slice are part of it.
- **Each slice carries its own qualification tests.** Use independently
  calculated expected values, including the worked example in the record.
  Existing goldens detect change; they do not qualify the contract.
- **The product stays green after every slice.** Where a slice introduces a
  new representation, it migrates every consumer in that slice. No slice
  leaves two representations alive.
- **Gates per slice:** `mise run rust-lint-crate -- finstack-quant-statements`
  and `mise run rust-test-crate -- finstack-quant-statements`, the same pair
  for every other touched crate, `mise run python-test-file` for touched
  bindings, `mise run wasm-test-built`, `mise run gen-write`, then
  `mise run slice-gate`. Run `mise run all` once per phase.
- **One slice, one commit**, self-contained, message
  `<slice id>: <summary> (<record id>)`. The commit carries its register
  edit (see [Closing register entries](#closing-register-entries)).
- **Register text was read from the working tree.** `FOLLOW_UPS.md` has
  uncommitted edits at creation, so the entries this plan cites may differ
  from HEAD.
- **Start from a clean base.** At creation the working tree has 990 modified
  paths, 40 of them statements-related, and `cashflows/src/accrual.rs` and
  much of `core` are modified. Land or park those first, or work in a
  worktree. Stage by hunk in shared register files.

## Ownership against the Statements Analytics plan

[STATEMENTS-ANALYTICS-IMPLEMENTATION-PLAN.md](STATEMENTS-ANALYTICS-IMPLEMENTATION-PLAN.md)
depends on this work. Its Slice 1 lists "Statements result, calendar and check
contracts" as a prerequisite, and its Slices 3, 5 and 8 need "dated capital
inputs".

- **This plan owns everything inside `finstack-quant-statements`:** typed
  results and cell states (A5, A6), the check context and comparison policy
  (E1–E4), and dated capital fields (phase D).
- **The Analytics plan owns `finstack-quant-statements-analytics`** and
  verifies these contracts at its boundary. Its Slice 1 starts after A6 and
  E1; its Slices 3 and 5 start after D9; its Slice 8 after D10.
- If Analytics work starts first and needs a missing Statements behaviour,
  land the owning slice from this plan rather than a local workaround.

## Dependency order

| Slice | Work | Records | Depends on |
| --- | --- | --- | --- |
| A1 | Formula grammar, signatures and AST validation | C-004 | — |
| A2 | Runnable model: timeline, values, readiness | C-001 | A1 |
| A3 | Authoring, imports and availability | C-002 | A2 |
| A4 | Formula units and currency | C-005 | A1, A2 |
| A5 | Cell states, dependency graph and prepared plans | E-001 | A2, A3, A4 |
| A6 | Typed results and exports | E-013 | A5 |
| A7 | Checked scalar formulas and unavailable values | E-002 | A5, A6 |
| B1 | Offsets, growth and reporting windows | E-003 | A7, Core 014 |
| B2 | Historical statistics, cumulative functions and EWM | E-004 | A7 |
| B3 | Metric registration and dependencies | C-006 | A1, A5 |
| B4 | Reported-cost earnings and margins | C-007 | B3 |
| B5 | Period returns on capital | C-009 | B4 |
| C1 | Typed forecast specification and staged validation | C-003 | A2, A4 |
| C2 | Forecast suffix, anchoring and bounds | E-005 | C1, A5 |
| C3 | Deterministic forecasts and fades | E-006 | C2 |
| C4 | Stochastic kernels and versioned streams | E-007 | C2 |
| C5 | Latent one-peer correlation | E-008 | C4 |
| C6 | Trend extrapolation | E-009 | C2 |
| C7 | Seasonal decomposition | E-010 | C2 |
| D1 | Remove Statements bond and swap construction helpers | K-001 | — |
| D2 | Period admission and cashflow classification | K-002 | D1 |
| D3 | Opening principal and contractual accrued interest | K-003 | D2, upstream |
| D4 | Dated principal claims, arrears and reserves | K-004 | D3 |
| D5 | Waterfall allocation and conservation | K-006 | D4 |
| D6 | Prepayments and ECF sweeps | K-007 | D5 |
| D7 | Residual coupon rebuilding | K-005 | D4, upstream |
| D8 | Contractual PIK elections and carried claims | K-008 | D7, upstream |
| D9 | Capital reporting scopes and period-close FX | K-009 | D2–D8, A6 |
| D10 | Capital-state orchestration and period snapshots | E-012 | D5–D9, A5 |
| D11 | Independent capital and waterfall qualification | Evidence entry | D10 |
| B6 | Leverage and coverage | C-008 | B4, B1, D9 |
| C8 | Monte Carlo paths, quantiles and breach probability | E-011 | C4, C5, A6, D10 |
| E1 | Numerical comparison and check-suite policy | C-014 | A6 |
| E2 | Accounting identities | C-010, C-011, C-012 | E1 |
| E3 | Data-quality coverage and validity | C-013 | E1 |
| E4 | Formula checks | C-015 | E1, A7 |
| E5 | Period rollup | E-014 | A6, A7 |
| E6 | Normalization bridges | E-015 | A6, B4 |
| E7 | Wire admission, schemas and errors | C-016 | all of A, C1, D1, D8 |

Phases B, C, D and E are independent of one another once phase A is in,
except where the table says otherwise. B6 and C8 sit in their phases by topic
but run after phase D.

**Split records.** Four records finish in a later slice than the one that
owns them. Rewrite their register entries in place at the owning slice and
delete them only at the finishing slice.

| Record | Owning slice | Remainder | Finishes in |
| --- | --- | --- | --- |
| C-004 | A1 | typed `cs` selectors and scopes (§2) | D9 |
| C-005 | A4 | capital reference units and consolidation (§4) | D9 |
| E-001 | A5 | check suite applied at single-run finalization (§4) | E1 |
| E-011 | C8 | check suites at single-run finalization (§2) | E1, if C8 lands first |

## Upstream prerequisites

Phase D reads contracts that are accepted but not yet reconciled in their
owning crates. Status at creation:

| Upstream record | Status | Needed by |
| --- | --- | --- |
| [Cashflows 05 coupon cash and PIK](../decisions/financial/cashflows/05-coupon-cash-and-pik.md) | reconciliation pending | D7, D8 |
| [Cashflows 08 floating replay](../decisions/financial/cashflows/08-floating-replay.md) | reconciliation pending | D7 |
| [Cashflows 12 schedule lifecycle](../decisions/financial/cashflows/12-schedule-lifecycle.md) | reconciliation pending | D3, D7 |
| [Cashflows 13 accrued interest](../decisions/financial/cashflows/13-accrued-interest.md) | reconciliation pending | D3 |
| [Term-loan cashflows](../decisions/financial/valuations-other-fixed-income/07-term-loan-cashflows.md) | reconciliation pending | D7 |
| [Revolver cashflows](../decisions/financial/valuations-other-fixed-income/10-revolver-cashflows.md) | reconciliation pending | D7 |
| [Bond schedules](../decisions/financial/valuations-bonds/01-schedule-and-holder-cashflows.md) | reconciliation pending | D1 |
| [Interest-rate swaps](../decisions/financial/valuations-rates/03-interest-rate-swaps.md) | reconciliation pending | D1 |
| [Core 012 money precision](../decisions/financial/core-conventions/012-money-decimal-precision.md) | partly reconciled | A6, D5 |
| [Core 014 periods and LTM](../decisions/financial/core-conventions/014-fiscal-periods-ltm.md) | runtime landed 2026-11-12/13; record header not updated | B1 |
| [Core 015 cashflow semantics](../decisions/financial/core-conventions/015-cashflow-semantics.md) | reconciled 2026-11-11 | D2–D4 |

Rule, as in the Analytics plan: verify each prerequisite against the live
owner at the start of the slice. If an accepted upstream behaviour is missing,
land the smallest owner-side change and its tests in the owning crate first,
as its own commit, then the Statements consumer. Do not reimplement an
upstream contract inside Statements. D3, D7 and D8 are the slices most likely
to be blocked; see Owner resolution 1.

## Phase A — Model, units and evaluation foundations

Today a cell is an `f64`: a masked cell is written as `0.0`, unavailable
values travel as `NaN`, and `CellSource` has no unavailable state. Almost
every accepted record says "unavailable with a reason" or "error". Phase A
builds that vocabulary once so later phases use it.

**A5, A6 and A7 are one unit of work in three consecutive commits.** A5
changes what a cell is while the formula kernels (A7) and published results
(A6) still speak `f64`. Between those commits there is exactly one interim
conversion, at the formula-evaluator boundary, mapping the new unavailable
state to and from `NaN`. A7 deletes it. Nothing else may start between A5 and
A7, and the phase gate runs only after A7.

**A6 changes the result type for every consumer.** The dependents are
`finstack-quant-statements-analytics`, `finstack-quant-scenarios`, the Python
and WASM bindings and the UI fixtures. A6 does their mechanical migration so
the workspace builds; it preserves their current semantics. The Analytics
plan then changes Analytics semantics on the new types.

**A1 — Formula grammar, signatures and AST validation.** [C-004]. One
canonical signature table shared by parser, compiler, evaluator and host
docs; exactly one argument for the seven cumulative and full-history
functions; grouping-preserving rendering; the stated precedence for `!` and
`not`; explicit parentheses for nested comparisons; the 64-level and
256-term limits; preflight of caller-built ASTs; rejection of invalid
untaken branches and future reads. The typed `cs` selectors land in D9.
*Validation:* parse → render → parse identity over operator and grouping
cases; surplus-argument rejection at compile time for each of the seven
functions; limit boundaries at and above; a constructed AST that bypasses the
text parser.

**A2 — Runnable model.** [C-001]. One guarded validation path for build and
load; canonical contiguous plan with actuals as a prefix; node kinds and
their permitted sources; finite scalar or single-currency `Money`
observations; absence as the only missing-cell representation; closed
references and cycle rejection for a runnable model, with drafts allowed
during authoring.
*Validation:* the record's Q1–Q3 example; a gap in the plan; a misspelled
reference; a Value node carrying a forecast or formula; a non-finite
observation through every constructor and the wire.

**A3 — Authoring, imports and availability.** [C-002]. Shared identifier
and reserved-name rules; duplicate rejection; named atomic edits; forecast
attachment that promotes to Mixed; plan-validated imports by period label;
identified source snapshots with explicit availability dates and an
inclusive cutoff; explicit full-snapshot mode.
*Validation:* the record's April 25 / May 15 revision replay; unordered and
sparse import columns; an off-plan column; a duplicate observation.

**A4 — Formula units and currency.** [C-005]. Only `Scalar` and
`Monetary(currency)` exist for a runnable model. Reject monetary variance
and apply the operation table in the record. Today the unresolved dimension
has two sources: a node reference with no resolved type, and a `cs` reference
with no known currency. A4 closes the first, relying on A2's closed
references. The second needs the reporting scopes, so the unresolved
dimension is deleted in D9, and until then it can arise only from a `cs`
reference.
*Validation:* one case per row of the operation table; `min(usd, eur)`;
`var(usd)`; money × money; same-currency ratio returning Scalar; a declared
unit that disagrees with inference.

**A5 — Cell states, dependency graph and prepared plans.** [E-001]. Distinct
value, known zero, missing, masked and unavailable-with-reason cells; errors
separate from unavailability; `where` evaluated before source selection;
the positive-integer-literal rule for `lag`/`shift` edges; deterministic
in-period order; explicit snapshot or cutoff; plan reuse only for payload
edits in existing typed slots, with re-prepare on any structural, forecast or
capital-structure change.
*Validation:* a masked cell is not zero downstream; a missing mask value; a
zero and a dynamic lag offset keep the same-period edge; a capital-structure
edit invalidates a prepared plan; precedence of explicit, forecast and
formula sources under a cutoff.

**A6 — Typed results and exports.** [E-013]. Authoritative `Scalar` and exact
currency-tagged `Money` results; representability checks at the
forecast-to-money boundary; exact long table with status, reason and source;
approximate wide table with full period index and units; explicit period
anchor for dated exports; model, snapshot, cutoff and run identity on the
result; one contract across Rust, Python and WASM.
*Validation:* the record's USD 125.01 March export; a value unrepresentable
as `Money`; a missing April cell that stays visible; long-table round trip;
chronological order when insertion order differs.

**A7 — Checked scalar formulas.** [E-002]. Checked arithmetic with errors
for invalid domains, division or remainder by zero and overflow; exact
comparisons with tolerance only when selected; lazy `if`, `and`, `or`;
`is_missing` and `coalesce` over the A5 states; all-operands-available
aggregates; rounding and clamp rules; explicit positive finite annualization
with the stated compound domain.
*Validation:* a truth table over available, unavailable and error operands
for each control; `coalesce` returning an available zero; `1/0`, `ln(-5)`
and overflow as errors; compound annualization at −100% and below.

## Phase B — Formula library and bundled metrics

**B1 — Offsets, growth and reporting windows.** [E-003]. Exact nonnegative
integer offsets; signed `diff` and `pct_change` with an unavailable exact-zero
base; `growth_rate` with an explicit lookback, Actual/365 Fixed years on
daily and weekly plans and slot-count years otherwise; complete additive
windows for `ytd`, `qtd`, `fiscal_ytd` and `ttm`; weekly TTM rejected;
missing history unavailable but invalid geometry an error. Confirm first what
the Core 014 landing already changed in Statements.
*Validation:* independent fixed-365 leap-window cases; fiscal quarter and
year boundaries; an interior daily gap; a fractional and a future offset.

**B2 — Historical statistics, cumulative functions and EWM.** [E-004].
Exact-slot rolling windows with minimum counts; `n−1` sample statistics;
competition ranks; R-7 quantiles; complete-prefix cumulative functions;
exact monetary sums and currency-preserving standard deviation; slot-based
EWM decay with the Boolean correction toggle.
*Validation:* gapped windows at the minimum count; rank ties; an incomplete
prefix; corrected EWM with fewer than two positive-weight observations.

**B3 — Metric registration and dependencies.** [C-006]. Atomic catalogue
registration; shared namespace grammar with `cs` reserved; bindings pinned at
registration; AST-only qualification replacing the textual rewriter;
deterministic closure including qualified and historical references; atomic
import of a metric with its closure; conflicts rejected.
*Validation:* a metric ID that collides with a function name; a qualified
cross-namespace reference in the closure; a later registration that does not
rebind an earlier metric; two imports sharing an unchanged template.

**B4 — Reported-cost earnings and margins.** [C-007]. The six-row formula
table, four margins and two cost ratios, with unavailable-with-reason on
missing, zero or negative revenue. This flips the bundled convention from
ex-D&A OPEX, so every example, notebook, binding doc and UI fixture that pins
the old values moves in this commit.
*Validation:* an independently specified income statement separating included
D&A from the single add-back; a negative expense credit; coarser-period
recomputation from aggregated inputs.

**B5 — Period returns on capital.** [C-009]. Same-period unannualized
returns over average opening and closing balances; shared net income and
EBIT; ROIC on explicit operating tax and operating invested capital;
positive denominators. Needs Owner resolution 4.
*Validation:* hand-computed averages; a nonpositive denominator; a signed
loss; an operating tax credit; a coarser period using its own opening and
closing balances.

**B6 — Leverage and coverage.** [C-008]. Runs after D9. Period-end gross
debt; complete TTM EBITDA; interest coverage on statement interest including
recognized PIK; debt-service coverage on contractual due-date scheduled cash
interest and principal; generic thresholds removed.
*Validation:* hand-summed complete and gapped TTM; zero gross debt; negative
EBITDA with valid denominators; a missed amortization recognized once on its
due date; prepayments, sweeps, PIK and fees excluded from scheduled service.

## Phase C — Forecasts and simulation

**C1 — Typed forecast specification.** [C-003]. One closed typed
specification replacing the free-form parameter map across Rust, Python, WASM
and JSON; explicit method choices; all static checks at build or load, even
when observations override the forecast; exact `u64` seeds as decimal strings
on the wire.
*Validation:* a linear fade carrying `half_life`; an unknown, null and
duplicate field; seed `9007199254740993` round-tripped through each host.

**C2 — Forecast suffix, anchoring and bounds.** [E-005]. One suffix per node
and run; one step per slot including masked and explicitly published slots;
explicit start level or the immediately preceding eligible slot; bounds
applied to published outputs only, with clipping recorded.
*Validation:* phase and recurrence unchanged by an explicit value mid-suffix;
a missing base giving a typed unavailable forecast; clipping that does not
feed the next step.

**C3 — Deterministic forecasts and fades.** [E-006]. Signed growth
recurrence; keyed `Override` carry; linear, log-magnitude geometric and
half-life exponential fades returning the exact target in the final slot.
*Validation:* a rate below −100%; leading slots before the first override
key; a geometric path whose endpoint ratio overflows or underflows while
every level is representable; opposite-sign geometric endpoints rejected.

**C4 — Stochastic kernels and versioned streams.** [E-007]. Signed normal
and mean-reverting steps; unclipped log-space lognormal with a strictly
positive restored level; explicit growth or difference bootstrap; no draws at
zero volatility; shared PCG64 with a versioned stream key.
*Validation:* moments against independent closed forms; a positive subnormal
accepted and a zero result rejected; draw-count invariance at zero
volatility; replay from recorded seed provenance.

**C5 — Latent one-peer correlation.** [E-008]. Peer validation; exact
behaviour at ρ = 0, 1 and −1; the stated interior mix; deterministic peer
ordering with cycle rejection; shocks generated through masked and published
slots; active correlation rejected outside Monte Carlo.
*Validation:* ρ = 0 bit-identical to an uncorrelated node; a three-node
chain; a peer cycle; sample correlation against the coefficient.

**C6 — Trend extrapolation.** [E-009]. Centered and scaled OLS, damped Holt
and the moving-average rule on aligned consecutive history.
*Validation:* OLS against an independent fit at large levels; Holt against a
hand recursion; moving average at `n = W`.

**C7 — Seasonal decomposition.** [E-010]. Centered moving-average trend with
the even-`m` endpoint weights; unsnapped edge slopes; centered additive
effects and normalized multiplicative factors; phase continued across every
slot.
*Validation:* two exact cycles; even and odd `m`; a zero observation in
multiplicative mode; mixed-sign trend rejected.

**C8 — Monte Carlo.** [E-011]. Runs after D10. Path-count and resource
limits; fresh complete paths on the shared ordered engine with the selected
evaluation context; deterministic failure order; one typed sample per path,
metric and period; R-7 quantiles by exact probability key; complete-key path
restoration; typed finite strict-upper breach probability. Needs Owner
resolution 6.
*Validation:* serial and parallel equality; R-7 against an independent
calculation; a duplicate percentile request rejected; breach probability
unavailable without path samples; a floating-rate capital structure on a
supplied market context.

## Phase D — Capital structure

Order follows the debt ledger from construction to reporting. D5, D6 and D8
all change `capital_structure/waterfall/mod.rs` (about 780 source lines
before its tests), so they run in sequence, never in parallel.

**D1 — Remove construction helpers.** [K-001 construction]. Delete
`add_bond`, `add_bond_with_convention`, `add_swap`,
`add_swap_with_conventions` and their Python and WASM counterparts; move
callers to canonical Valuations instruments attached through `add_debt` /
`addDebt`. The opt-in prepayment half of K-001 is recorded as already
enforced; confirm with a test and leave it.
*Validation:* resolved terms, source and schedule survive attachment and
replay; parity ledger and notebooks updated.

**D2 — Period admission and classification.** [K-002]. Validate the period
before reading a schedule; convert once from the producer's declared
perspective; separate funding proceeds, repayment cash and face effects;
classify fees and hedge settlements; reject unclassified effects; report
distress cash separately. Coupon netting and the Interest claim are recorded
as implemented; confirm with tests.
*Validation:* the record's USD 100 / USD 102 March–April example and its
liability-reduction-with-distress example; a custom schedule without a stated
perspective.

**D3 — Opening principal and accrued interest.** [K-003 opening],
[K-003 accrual]. Replace opening-balance scaling with a hard mismatch error;
compute contractual accrued cash interest from the instrument's own earning
terms and full accrual context; keep overdue arrears separate; keep
issue-discount and effective-interest measures out of both. Includes the
Investigate entry on new-funding closes: reproduce early and late settlement
with PIK on newly issued and redrawn debt before changing that branch.
*Validation:* a schedule/ledger mismatch naming instrument, period, currency
and both amounts; accrual on an ISMA-basis bond mid-coupon; an unpaid coupon
moving from accrued to arrears at its due date.

**D4 — Dated principal claims.** [K-004]. Stable dated claims with separate
cash and face; no synthesized same-period claim; economic principal, pending
settlement, arrears and advance reserves reported separately; oldest-due
first within an instrument; exact native-currency reconciliation; atomic
commit.
*Validation:* the record's March 31 / April 15 example; a partial, an early
and a late payment; an advance reserved until its effective date.

**D5 — Waterfall allocation and conservation.** [K-006]. One finite
pre-debt-service pool with transitive same-period independence; required
categories once each; class and seniority rules; exact `Money` quotas or an
authorized payment quantum with largest-remainder distribution; every
shortfall carried; terminal Equity or retained cash; atomic commit. Includes
the Investigate entry on class remainders: reproduce it first; this slice's
allocation rule replaces the code it describes. Needs Owner resolution 2.
*Validation:* the record's five-cents-across-three-claims example and its
\$15 residual example; a negative pool; a debt-free period; a pool that reads
its own payment through an alias; exact conservation across row orders.

**D6 — Prepayments and ECF sweeps.** [K-007]. Separately elected at-par
priorities; explicit facility eligibility; agreement-defined ECF formula,
dates, deductions and credits; fraction in `[0,1]`; negative requests
rejected; requested, paid and unpaid reported.
*Validation:* the record's \$95 / \$35 / 50% example; a mixed term-loan and
revolver allocation; a zero-percentage sweep still requiring its priority.

**D7 — Residual coupon rebuilding.** [K-005]. Rebuild affected coupon
parents through the canonical instrument owner from dated principal;
preserve settled cash, earned interest and locked observations; apply the
cash/PIK split once; rebuild balance-sensitive fees from canonical term-loan
and revolver terms; stage schedule and ledger together. Includes the Evidence
entry on residual coupons.
*Validation:* the record's 10% on \$100 then \$60 example against an
independent schedule; a zero balance followed by a redraw; irregular day
counts; a swept revolver's commitment and usage fees.

**D8 — Contractual PIK elections.** [K-008]. Per-instrument and per-coupon
authorization; finite metric and threshold in matching units; minimum
duration counted in coupon periods; elected rate and split from coupon terms;
capitalization once on the contractual date; arrears carried separately and
never capitalized implicitly; atomic election, ledger and schedule state.
Needs Owner resolution 3.
*Validation:* the record's \$10 coupon, 40% PIK, \$3 arrears example; a
retried period counted once; an election after full paydown; a non-finite
threshold through typed and wire paths.

**D9 — Capital reporting scopes and FX.** [K-009]. Per-instrument native,
native-currency total and optional consolidated scopes; explicit `PeriodEnd`
FX at the inclusive close with pair, rate, date and source retained;
consolidated values unavailable on a missing quote while native values stay;
component reconciliation with known zero distinct from unavailable; exact
decimal exports; chronological accessors; atomic period replacement. Lands
the typed `cs` selectors and their units from C-004 and C-005.
*Validation:* the record's EUR/USD consolidation example; a missing quote; a
conflicting reporting currency; a stale row cleared on replacement; an
unknown `cs` component or instrument rejected at validation.

**D10 — Capital-state orchestration.** [E-012]. Obligations established from
the run context and explicit market inputs; transitive input independence;
the waterfall run exactly once per period, including zero-claim periods;
downstream formulas evaluated once after allocation on the same shocks;
statement cells, provenance, snapshots and debt state committed together;
one ordered engine for ordinary, prepared and Monte Carlo evaluation.
*Validation:* the record's \$20 due / \$12 paid example with the Q2 lag;
a multi-period cash and debt bridge built independently; issuance at a period
boundary; replay equality.

**D11 — Independent capital qualification.** The Evidence entry spanning
K-004, K-006, K-007, K-008 and K-009. Build one independent multi-period debt
ledger, one agreement-style ECF definition and one multi-currency case, and
reconcile D4–D10 against them. This slice adds evidence only; a defect it
finds is fixed under the owning slice's id.

## Phase E — Checks, transforms and wire

**E1 — Comparison and check-suite policy.** [C-014]. Zero default identity
tolerance with the stated allowance rule; verdicts from unfiltered outcomes;
pass, fail, incomplete and unevaluated; declared order and unique IDs;
compatible merges; supplied results accepted only with verified model, plan
and run identity.
*Validation:* a difference exactly at the allowance; an empty suite; a
filter that hides an advisory but not a failure; mismatched provenance.

**E2 — Accounting identities.** [C-010], [C-011], [C-012]. Disjoint
validated account groups; checked exact or compensated sums; signed and
absolute residuals; every period reconciled from an explicit dated opening;
all-or-none cash-flow components; accepted dividend signs with a negative
magnitude rejected; duplicate movement IDs rejected; missing inputs make a
period incomplete rather than skipped.
*Validation:* one hand-built three-statement model exercising all three
identities; large cancelling balances; the first period reconciled; a
partial CFO/CFI/CFF configuration.

**E3 — Data-quality coverage and validity.** [C-013]. Explicit required
scope; incomplete on missing or unavailable required observations; typed zero
valid; raw NaN, infinity or unrepresentable money an error even on optional
cells; unknown and duplicate IDs rejected; advisory sign groups.
*Validation:* empty scope unevaluated; an optional non-finite cell;
overlapping sign groups.

**E4 — Formula checks.** [C-015]. Explicit Predicate or Residual mode and
period scope; full-AST validation; lazy observation consumption; optional
eligibility condition; missing needed observations incomplete; domain errors
and overflow as execution errors even for advisory checks.
*Validation:* `coalesce(missing, backup) > 0` passing; an untaken branch
with a missing input; `sqrt(-1)` as an error; an unavailable eligibility
condition.

**E5 — Period rollup.** [E-014]. Explicit method per rolled node with
automatic `Sum` removed; exact canonical coverage; checked `Sum`, `PeriodEnd`
copying final-cell status, and `Formula` re-evaluated at target cadence;
provenance to the source run.
*Validation:* the record's H1 revenue, profit, cash and margin example,
including the absent-Q2 variant; an unresolved method; a partial period.

**E6 — Normalization bridges.** [E-015]. Ordered bridge to a new,
collision-checked node ID; validated references, units and period keys
before calculation; individual `Reported` and `Progressive` caps; checked
arithmetic on every row; provenance to the source run.
*Validation:* the record's USD 150 and USD 156 cap example; an infinite
intermediate with a finite total; a collision with an existing node.

**E7 — Wire admission, schemas and errors.** [C-016]. One bounded loading
path for all hosts; exact `schema_version` 1; duplicate and unknown fields
rejected; the 64 MiB, depth-96 and 256-diagnostic limits; validation before
canonical `c2` hashing; exact tagged `Money` and decimal-string seeds;
the seven admitted instrument forms; a runnable schema fixture; structured
error codes across hosts. Runs last because it publishes the shapes the other
slices settle.
*Validation:* each limit at and above its bound; a model hashed only after
validation; Rust, Python and WASM loading the same documents to the same
result or the same structured error.

## Owner resolutions

Questions the records leave open. Each blocks only the slice named.

1. **Upstream gating (D3, D7, D8).** Six upstream cashflows and valuations
   records are accepted but not reconciled. Either reconcile those owners
   first, or allow this plan to land the smallest owner-side change per
   slice. The second is faster but spreads cashflows work across Statements
   commits.
2. **Payment quantum (D5).** K-006 allows "an authorized payment quantum"
   when exact quotas are not permitted or representable. State who authorizes
   it and whether the currency minor unit is the default.
3. **Forecast PIK exercise (D8).** K-008 requires "explicitly modeled
   exercise for forecasts" and separately defines a metric-below-threshold
   request. Confirm that the threshold rule is the modeled exercise, and how
   authorization and minimum duration are authored per instrument.
4. **ROIC inputs (B5).** C-009 requires an explicit operating tax and
   operating invested capital. Name the input nodes the bundled metric reads.
5. **Which `cs` components are waterfall outputs (D5, D10).** K-006 and
   E-012 reject a cash pool that depends on same-period outputs of the
   waterfall, and E-012's example is a pool reading its own payment. The
   records do not list the components. State which same-period `cs`
   components count as outputs, in particular recognized interest expense
   and accrued interest. E-012 also says a simultaneous cash-and-debt solve
   "requires its own decision", so whatever is classed as an output cannot
   feed the pool in this plan.
6. **Monte Carlo entry point (C8).** E-011 and E-012 put Monte Carlo on the
   shared engine with the selected evaluation context. Decide whether that is
   one entry point taking the context or a separate market-aware variant.
7. **Non-`PeriodEnd` FX policies on capital (D9).** K-009 requires explicit
   `PeriodEnd`. Confirm that `CashflowDate` is removed from the capital
   specification rather than rejected at validation.

## Capabilities removed

For owner attention before the phase that removes them.

- Statements bond and swap construction helpers (D1).
- Ex-D&A OPEX as the bundled earnings convention (B4).
- Period-end-balance returns and the reported-tax ROIC approximation (B5).
- Generic 3×/1.5×/1.25× threshold prose on bundled ratios (B6).
- Monetary variance and unresolved formula units (A4).
- NaN as a formula result for invalid domains and division by zero (A7).
- Surplus arguments to the seven unary functions (A1).
- Weekly TTM and one-argument `growth_rate` (already removed with Core 014).
- Opening-balance scaling for carried debt (D3).
- Implicit sweep eligibility across all term loans (D6).
- Implicit transfer of interest arrears into PIK (D8).
- Per-flow-date capital FX (D9).
- Cash pools that depend on same-period waterfall outputs, to the extent
  Owner resolution 5 defines them (D5, D10).
- Automatic `Sum` in rollup (E5).
- Overwriting an existing node with normalized values (E6).
- The free-form forecast parameter map (C1).

## Closing register entries

**A fixed issue is removed from the follow-up register in the commit that
fixes it.** A slice is not done, and must not be committed, while its entry
still describes work the commit has completed. Same rule as the Core
conventions plan.

1. **Delete** the entry from `docs/decisions/financial/FOLLOW_UPS.md` in the
   commit that completes its last named action, including the qualification
   it lists. Do not strike it through, mark it done or move it to a
   "resolved" list.
2. **Rewrite in place** when a slice completes only part of an entry, so it
   states only what remains. This applies to the four
   [split records](#dependency-order) and to any slice that lands runtime
   work before its independent qualification.
3. **Update the index.** Recount the section and correct its row in the
   count table at the top of `financial/FOLLOW_UPS.md`. The counts there
   already differ from the sections at creation (15, 16 and 17 listed against
   14, 16 and 15 entries), so recount; do not decrement.
4. **Remove matching cross-references.** Search the central
   `docs/decisions/FOLLOW_UPS.md` and `financial/README.md` for the same
   issue and delete or rewrite those lines in the same commit.
5. **Update the record.** Change its implementation-status line, and its
   validation-status line when the qualification is complete.
6. **Investigate entries** are deleted either by the reproduction and fix or
   by a recorded non-reproduction, with the evidence in the commit body.
7. **Newly found issues** go into the register as new entries in the commit
   that finds them. They are not folded silently into an existing entry.
8. **Check before committing.** The slice's commit body lists each entry
   deleted or rewritten by its bold title. `mise run slice-gate` then runs on
   the tree that includes the register edit.

### Entries closed by each slice

Entries are named by the bold title they carry in the register at creation.
"Rewrite" means the entry stays with only the named remainder.

| Slice | Register entry | Action |
| --- | --- | --- |
| A1 | Canonical Statements formula grammar, signatures and AST validation | Rewrite to the `cs` selector remainder; delete in D9 |
| A2 | Runnable Statements models require a canonical timeline and validated inputs | Delete |
| A3 | Statements authoring, imports and point-in-time availability | Delete |
| A4 | Statements formula units and currency | Rewrite to the capital-reference remainder; delete in D9 |
| A5 | Prepared Statements evaluation and cell precedence | Rewrite to the finalization check-suite remainder; delete in E1 |
| A6 | Authoritative typed results and explicit export boundaries | Delete |
| A7 | Checked Statements formulas and unavailable values | Delete |
| B1 | Exact historical offsets, growth and complete reporting windows | Delete |
| B2 | Canonical historical windows, ranks, cumulative functions and EWM | Delete |
| B3 | Metric registration, pinned dependencies and atomic model import | Delete |
| B4 | Bundled reported-cost earnings and margins | Delete |
| B5 | Bundled period returns on comparable average capital | Delete |
| B6 | Bundled leverage and coverage definitions | Delete |
| C1 | Typed forecasts and staged validation | Delete |
| C2 | Canonical forecast suffix, anchoring and output bounds | Delete |
| C3 | Deterministic forecast levels, keyed carry and fades | Delete |
| C4 | Checked stochastic forecast kernels and replay | Delete |
| C5 | Latent one-peer correlation, ordering and replay | Delete |
| C6 | Aligned trend history, checked fitted levels and extrapolation | Delete |
| C7 | Aligned seasonal decomposition and phase-preserving projection | Delete |
| C8 | Complete Monte Carlo paths, typed samples and exact quantiles | Delete, or rewrite to the check-suite remainder if E1 is not in |
| D1 | Explicit bond and swap construction | Delete |
| D2 | Debt and hedge cashflow classification and period admission | Delete |
| D3 | Opening principal must match between the schedule and debt ledger | Delete |
| D3 | Contractual accrued cash interest must stay separate from arrears | Delete |
| D3 | New-funding closes mix economic and cash-period clocks (Investigate) | Delete |
| D4 | Dated principal claims, settlement arrears and advance reserves | Delete |
| D5 | Exact waterfall cash allocation and conservation | Delete |
| D5 | Class allocation can direct floating remainder to an ineligible final row (Investigate) | Delete |
| D6 | Agreement-defined prepayments and excess-cash-flow sweeps | Delete |
| D7 | Canonical residual coupon rebuilding | Delete |
| D7 | Residual coupons need independent debt-ledger reconciliation (Evidence) | Delete |
| D8 | Contractual PIK elections and carried claims | Delete |
| D9 | Capital reporting, period-close FX and atomic replacement | Delete, with the A1 and A4 remainders |
| D10 | Capital-state orchestration, dependency validation and atomic period snapshots | Delete |
| D11 | Capital and waterfall policy lacks independent contractual reconciliation (Evidence) | Delete |
| E1 | Numerical comparison and check-suite outcomes | Delete, with the A5 remainder |
| E2 | Complete balance-sheet articulation at every model close | Delete |
| E2 | Cash roll-forward and independent component reconciliation | Delete |
| E2 | Complete retained-earnings reconciliation | Delete |
| E3 | Statements data-quality coverage and validity | Delete |
| E4 | Explicit formula-check modes, eligibility and lazy evaluation | Delete |
| E5 | Explicit period rollup, typed accumulation and provenance | Delete |
| E6 | Ordered normalization bridges and separately named adjusted metrics | Delete |
| E7 | Bounded Statements wire admission, schemas and errors | Delete |

When the plan is complete, the three Statements sections of the register are
empty. Remove their headings and index rows in the final commit.

## Superseded draft

The earlier six-slice remediation draft must not be used. It conflicts with
the accepted records on: per-flow-date FX (K-009); a fixed-point waterfall
loop (E-012); cash pools reading `cs.interest_expense` (K-006); threading an
opening-balance scale (K-003); an accrual-basis `cs.interest_expense` (K-009);
adding swap receipts to the pool (K-002); last-slot remainder allocation
(K-006); TTM return numerators (C-009); an identity "unknown" dimension
(C-005); NaN-propagating comparisons (E-002); a calendar-year `growth_rate`
lookback (E-003); default-`Sum` rollup (E-014); and in-place normalized
overwrite (E-015).

## Completion

Verify all 45 register entries against the three decision inventories.
Close each only when its owning code, host surfaces and independent evidence
meet the accepted record. Run one end-to-end three-statement model with a
multi-currency capital structure, a waterfall with a sweep and a PIK
election, a Monte Carlo run and a rollup through Rust, Python and WASM.
Confirm generated files are current and run `mise run all`.

[K-001 construction]: ../decisions/financial/statements-capital/01-instrument-construction.md#accepted-decision-for-explicit-bond-and-swap-construction
[K-002]: ../decisions/financial/statements-capital/02-cashflow-classification.md#accepted-decision-for-period-admission-and-cashflow-classification
[K-003 opening]: ../decisions/financial/statements-capital/03-funding-and-balances.md#accepted-decision-for-opening-principal-reconciliation
[K-003 accrual]: ../decisions/financial/statements-capital/03-funding-and-balances.md#accepted-decision-for-contractual-accrued-cash-interest
[K-004]: ../decisions/financial/statements-capital/04-dated-principal.md#accepted-decision-for-dated-principal-claims
[K-005]: ../decisions/financial/statements-capital/05-residual-coupons.md#accepted-decision-for-residual-coupon-rebuilding
[K-006]: ../decisions/financial/statements-capital/06-priority-allocation.md#accepted-decision-for-waterfall-cash-allocation-and-conservation
[K-007]: ../decisions/financial/statements-capital/07-prepayments-and-ecf.md#accepted-decision-for-agreement-defined-prepayments-and-ecf-sweeps
[K-008]: ../decisions/financial/statements-capital/08-pik-and-carry-state.md#accepted-decision-for-contractual-pik-elections-and-carried-claims
[K-009]: ../decisions/financial/statements-capital/09-reporting-and-fx.md#accepted-decision-for-capital-reporting-and-fx
[C-001]: ../decisions/financial/statements-contracts/01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness
[C-002]: ../decisions/financial/statements-contracts/02-authoring-and-availability.md#accepted-decision-for-authoring-imports-and-observation-availability
[C-003]: ../decisions/financial/statements-contracts/03-forecast-parameter-contract.md#accepted-decision-for-typed-forecasts-and-validation-staging
[C-004]: ../decisions/financial/statements-contracts/04-dsl-syntax-and-lowering.md#accepted-decision-for-formula-grammar-function-dispatch-and-ast-validation
[C-005]: ../decisions/financial/statements-contracts/05-currency-dimensions.md#accepted-decision-for-formula-units-and-currency
[C-006]: ../decisions/financial/statements-contracts/06-registry-and-dependencies.md#accepted-decision-for-metric-registration-and-dependencies
[C-007]: ../decisions/financial/statements-contracts/07-earnings-and-margins.md#accepted-decision-for-reported-cost-earnings-and-margins
[C-008]: ../decisions/financial/statements-contracts/08-leverage-and-coverage.md#accepted-decision-for-bundled-leverage-and-coverage
[C-009]: ../decisions/financial/statements-contracts/09-returns-on-capital.md#accepted-decision-for-bundled-period-returns-on-capital
[C-010]: ../decisions/financial/statements-contracts/10-balance-sheet-articulation.md#accepted-decision-for-balance-sheet-articulation
[C-011]: ../decisions/financial/statements-contracts/11-cash-reconciliation.md#accepted-decision-for-cash-roll-forward-and-component-reconciliation
[C-012]: ../decisions/financial/statements-contracts/12-retained-earnings-reconciliation.md#accepted-decision-for-retained-earnings-reconciliation
[C-013]: ../decisions/financial/statements-contracts/13-data-quality-checks.md#accepted-decision-for-data-quality-coverage-and-validity
[C-014]: ../decisions/financial/statements-contracts/14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites
[C-015]: ../decisions/financial/statements-contracts/15-formula-checks.md#accepted-decision-for-formula-checks-and-coverage
[C-016]: ../decisions/financial/statements-contracts/16-wire-schema-and-errors.md#accepted-decision-for-statements-wire-admission-schemas-and-errors
[E-001]: ../decisions/financial/statements-evaluation/01-graph-precedence-and-prepared-plans.md#accepted-decision-for-dependency-graphs-cell-precedence-and-prepared-evaluation
[E-002]: ../decisions/financial/statements-evaluation/02-scalar-formulas-and-undefined-values.md#accepted-decision-for-scalar-formulas-and-unavailable-values
[E-003]: ../decisions/financial/statements-evaluation/03-temporal-operators-and-period-windows.md#accepted-decision-for-historical-offsets-growth-and-reporting-windows
[E-004]: ../decisions/financial/statements-evaluation/04-historical-statistics-and-ewm.md#accepted-decision-for-historical-statistics-and-ewm
[E-005]: ../decisions/financial/statements-evaluation/05-forecast-segments-and-bounds.md#accepted-decision-for-forecast-segments-anchoring-and-output-bounds
[E-006]: ../decisions/financial/statements-evaluation/06-deterministic-forecasts-and-fades.md#accepted-decision-for-deterministic-forecasts-and-fades
[E-007]: ../decisions/financial/statements-evaluation/07-stochastic-forecast-kernels.md#accepted-decision-for-stochastic-forecast-kernels-and-replay
[E-008]: ../decisions/financial/statements-evaluation/08-correlated-shocks.md#accepted-decision-for-latent-one-peer-correlation
[E-009]: ../decisions/financial/statements-evaluation/09-trend-extrapolation.md#accepted-decision-for-trend-extrapolation
[E-010]: ../decisions/financial/statements-evaluation/10-seasonal-decomposition.md#accepted-decision-for-seasonal-decomposition-and-projection
[E-011]: ../decisions/financial/statements-evaluation/11-monte-carlo-evaluation.md#accepted-decision-for-monte-carlo-paths-quantiles-and-breach-probabilities
[E-012]: ../decisions/financial/statements-evaluation/12-capital-state-orchestration.md#accepted-decision-for-capital-state-orchestration-and-period-snapshots
[E-013]: ../decisions/financial/statements-evaluation/13-result-units-and-exports.md#accepted-decision-for-typed-results-and-exports
[E-014]: ../decisions/financial/statements-evaluation/14-period-rollup.md#accepted-decision-for-period-rollup
[E-015]: ../decisions/financial/statements-evaluation/15-normalization-adjustments.md#accepted-decision-for-normalization-bridges
