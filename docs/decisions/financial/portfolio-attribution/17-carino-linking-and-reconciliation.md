# PORTFOLIO-ATTRIBUTION-017: Carino linking and supplied-attribution validation

- ID: PORTFOLIO-ATTRIBUTION-017
- Decision status: Accepted 2026-11-16 for checked Carino linking and reconciliation.
- Implementation status: Observed for the stated owning source scope. Runtime, restoration and host reconciliation with the accepted contract are pending.
- Validation status: Independent high-precision checks covered ordinary linking, offsetting effects at equal horizon growth, tiny compounded active return, near-loss secants, reconciliation and intermediate range. Production and host qualification remain pending. The publisher lists the original paper for purchase.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked Carino linking and reconciliation

1. Require a nonempty, chronological sequence of contiguous positive-length actual valuation intervals. Portfolio and benchmark cover the same window in every pair. Bind the portfolio and benchmark series identities, attribution method, reporting currency and compatible measurement, valuation, cash, fee and FX conventions. Reject gaps, overlaps and duplicates. Period lengths and holdings may change. Link over the actual horizon without annualization or elapsed-day effect weighting. [Accepted return clock](04-performance-returns.md#accepted-decision-for-checked-portfolio-performance-returns).
2. Link verified single-period results from their underlying returns and effects before display projection. Admit live checked results or independently restore them against retained historical inputs. Preserve the method's component definitions. Brinson and Campisi require the same ordered unique sector IDs and taxonomy, including any empty rows declared by that axis. Grid attribution contributes its three validated effect totals and retains each verified period's detail on its own grid. Single-period owners retain weight and group admission. [Brinson–Fachler](05-brinson-fachler.md#accepted-decision-for-checked-single-period-brinsonfachler-attribution) · [Campisi](07-fixed-income-campisi.md#accepted-decision-for-checked-fixed-income-attribution) · [Grid attribution](15-hierarchical-grid-attribution.md#accepted-decision-for-checked-duration-cell-and-sector-attribution).
3. Require strictly positive underlying growth on both sides of every period. Reject an actual return of `−100%` or below, including in the last interval. Use checked log accumulation. Compute compounded active return directly as the underlying growth difference `G_P − G_B` with range-safe arithmetic. Retain positive growth and an independently computed active return even if displayed cumulative side returns round to `−100%` or otherwise coincide.
4. For positive growth factors `a` and `b`, use `k = (log(a) − log(b))/(a − b)` and the exact equal-growth limit `1/a`. Evaluate nearby positive growth factors with stable relative-gap arithmetic and remove the fixed `1e−12` shortcut. Multiply each period effect by `k_t/K`, using the horizon coefficient `K`. Use range-safe combined scaling and ordered accumulation, allowing an intermediate multiplier outside binary64 range. Preserve offsetting nonzero effects at exactly zero compounded active return.
5. Verify each period's detail and totals, then linked rows and component sums against independent compounded active return before projecting. Replace fixed gross-scaled reconciliation gates with bounds tied to the calculation and result rounding. Return typed errors for unresolved calculations and unrepresentable required results. Preserve the computed effects without assigning a balancing adjustment.
6. Publish both compounded returns, independent compounded active return, ordered effects and dated source and method basis through one bounded Rust result and restoration contract. Check restored reports against the underlying period evidence and regenerated results. A report without that evidence remains inspectable. Python, WASM and UI views consume the Rust result.

Independent checks confirmed offsetting linked effects of `+ln 2` and `−ln 2` over an exactly flat active horizon. In a positive-growth near-loss case, both side returns displayed as `−100%` while compounded active return was `−2⁻¹⁰⁶`.

## Observed choice and domain

A shared Carino kernel links additive single-period effects to compounded active return. Brinson and Campisi provide detailed sector rows; grid attribution supplies three aggregate rows/effects. This method smooths timing interaction across effects, so a linked effect is not a direct sum of monetary P&L. Period ordering is the supplied sequence; the API does not enforce contiguous dated periods.

## Algorithm and numerical controls

1. Validate caller-supplied period returns/effects as finite and reconcile each period's effect sums to its declared totals and active return before linking. Brinson/Campisi require identical sector labels and ordering; grid linking validates top-level totals only and permits changing grids.
2. Compound portfolio and benchmark growth by direct products, giving `R_P=product(1+rP_t)-1` and `R_B=product(1+rB_t)-1`. Empty periods error. Every individual/horizon return must be greater than `-1` when its coefficient is evaluated.
3. Define `k(p,b)=[log(1+p)-log(1+b)]/(p-b)`. The implementation uses `log1p((p-b)/(1+b))/(p-b)` for stability. If `|p-b|<1e-12`, use the continuous limit `1/(1+p)`.
4. Let horizon coefficient be `K=k(R_P,R_B)`. Link each effect/row with compensated sum `E_linked=sum_t [k(rP_t,rB_t)/K]*E_t`.
5. Supplied effect reconciliation uses a scaled-L1 tolerance `1e-10*max(1,sum absolute effects)` with an overflow-aware scale accumulator. The active-return identity uses `1e-12*max(1,|R_P|,|R_B|,|active|)` and rejects nonfinite expected values/residuals. These tolerances qualify arithmetic consistency, not economic attribution accuracy.
6. The shared weight check requires finite total within `1e-6` of one. Group-return users additionally reject `|net|<=1e-6*gross` for positive-gross signed groups; this is separate from linking's logarithmic domain.

## Alternatives, failure behavior and tradeoffs

Arithmetic summation does not reconcile to compounded active return; geometric attribution and other linking algorithms distribute interaction differently. Carino provides an additive linked identity with a continuous equal-return limit. Direct products can overflow/underflow over extreme or long horizons, and the coefficient's near-equality threshold is absolute rather than relative. The kernel checks equal row count; public adapters impose the stronger named-row checks where described.

A valid single-period Brinson or Campisi report can fail linking if its total return is at or below `-100%`. Effect magnitudes may be large and offset; tolerances scale to gross effect magnitude rather than a near-zero net amount. No generic correction rewrites inconsistent caller-supplied reports to reconcile them.

## Evidence

[Shared kernel, validation helpers and inline tests](../../../../finstack-quant/portfolio/src/brinson.rs), [Campisi adapter](../../../../finstack-quant/portfolio/src/fi_attribution.rs), [grid adapter](../../../../finstack-quant/portfolio/src/grid_attribution.rs) are owning sources. [Carino](../../../REFERENCES.md#carino-1999) is the repository formulation reference. Formula/identity tests were inspected/located, not executed; no independent multi-period engine comparison is claimed.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
