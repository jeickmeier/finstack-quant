# PORTFOLIO-ATTRIBUTION-005: Single-period Brinson–Fachler attribution

- ID: PORTFOLIO-ATTRIBUTION-005
- Decision status: Accepted 2026-11-16 for checked single-period Brinson–Fachler attribution.
- Implementation status: Observed for the stated owning source scope. Runtime, caller and host reconciliation with the accepted contract are pending.
- Validation status: The original worked example and independent absence, signed-weight, closure and projection examples were checked. Runtime, host and numerical-range qualification of the accepted contract are pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked single-period Brinson–Fachler attribution

1. Require a complete sector partition for one actual positive-length valuation window in one reporting currency. Declare the common taxonomy and compatible total-return, valuation, FX and fee bases. Require unique, nonblank sector IDs and preserve exact text and row order. Require exact decimal opening weights, signed where applicable, with each side summing exactly to one. Include applicable cash and financing as explicit rows. Reject incomplete weights without normalization.
2. Compute `R_P=sum_i wP_i*rP_i` and `R_B=sum_i wB_i*rB_i`. On the effective sector returns below, use `A_i=(wP_i-wB_i)*(rB_i-R_B)`, `S_i=wB_i*(rP_i-rB_i)` and `I_i=(wP_i-wB_i)*(rP_i-rB_i)`. Their sum is the centered sector attribution effect. Its difference from the raw sector contribution `wP_i*rP_i-wB_i*rB_i` is `-(wP_i-wB_i)*R_B`, which cancels over the complete partition. [Brinson–Fachler original formulation](../../../REFERENCES.md#brinson-fachler-1985). [Separate Brinson–Hood–Beebower reference](../../../REFERENCES.md#brinson-hood-beebower-1986).
3. Require a finite return on each active side and an absent return on each exact-zero-weight side. Exact zero represents genuine absence. For portfolio-only rows use `R_B` as the benchmark comparator, so only interaction is populated. For benchmark-only rows use the sector benchmark return as the effective portfolio return, so only allocation is populated. If both sides are absent all effects are zero. Offsetting positions or trading that produces a return contribution at zero opening net weight requires a qualified upstream sector partition.
4. Evaluate from exact admitted decimal weights and exact binary fractions of the admitted binary64 returns with bounded widened arithmetic. Verify row totals, component totals and total active return `R_P-R_B` before converting each output once to binary64. Derive active return from the underlying totals, retaining it even if the displayed side totals coincide. Reject unrepresentable required outputs. Preserve computed effects without assigning a discrepancy to a component.
5. Permit finite signed sector and aggregate rates in single-period arithmetic attribution, including rates at or below `-1`. [Record 004](04-performance-returns.md#accepted-decision-for-checked-portfolio-performance-returns) owns wealth-return eligibility. [Record 017](17-carino-linking-and-reconciliation.md#accepted-decision-for-checked-carino-linking-and-reconciliation) owns Carino's positive-growth requirement, alignment and cross-period reconciliation. Use the same checked Rust calculation and restoration across Python, WASM and the UI. Retain dates, measurement basis, sector identity and source inputs needed to verify a restored result.

Independent arithmetic reproduced the published example's `-110 bp` active return with `-10 bp` allocation, `-160 bp` selection and `+60 bp` interaction. In the current contract a one-sector portfolio weight of `1.0000005` against benchmark weight `1`, with both returns at `20%`, passes weight admission and produces `0.0000001` active return with zero effects. Exact weight admission rejects it.

## Observed choice and input domain at the audit revision

The function decomposes portfolio active return into benchmark-centered allocation, selection and interaction. Inputs are sector labels and decimal portfolio/benchmark weights and sector returns. Require a nonempty list, all numerical inputs finite, and each side's weight sum within `1e-6` of one. Short weights are permitted; sector labels need not be unique or nonempty. There is no automatic cash bucket or weight normalization.

## Formula and selection

Compute `R_P=sum wP_i*rP_i` and `R_B=sum wB_i*rB_i` using compensated sums. When a sector has exactly zero benchmark weight, replace its benchmark sector return with the whole-benchmark return `R_B`; otherwise use supplied `rB_i`. Then:

- `A_i=(wP_i-wB_i)*(rB_i-R_B)`;
- `S_i=wB_i*(rP_i-rB_i)`;
- `I_i=(wP_i-wB_i)*(rP_i-rB_i)`.

The sector total is their direct sum; overall effects are compensated sums. The reference-centered allocation is Brinson–Fachler rather than the uncentered Brinson–Hood–Beebower expression. The exact-zero-benchmark substitution makes an out-of-benchmark sector's allocation zero and puts its active contribution into interaction. The active-return identity is numerical algebra, not independent evidence that a sector taxonomy or benchmark is suitable.

Single-period inputs can contain returns at or below `-100%`; the [Carino linker](17-carino-linking-and-reconciliation.md) has the stricter logarithmic domain. Labels and input ordering are preserved and matter to linked-period row matching.

## Alternatives and tradeoffs

Two-effect attribution combining selection and interaction, uncentered allocation, geometric attribution and alternate out-of-benchmark conventions are distinct methods. This implementation exposes three effects and a specific zero-benchmark convention without a selection flag. Signed weights preserve algebraic identities but may produce economically unintuitive effects.

## Evidence

[Source and inline attribution/reconciliation tests](../../../../finstack-quant/portfolio/src/brinson.rs) supply formula and algebraic evidence. The bibliography now identifies [Brinson and Fachler's centered method](../../../REFERENCES.md#brinson-fachler-1985), with [Brinson, Hood and Beebower](../../../REFERENCES.md#brinson-hood-beebower-1986) recorded separately. The one-sided conventions belong to the accepted calculation above. No production tests ran for this review.

## Follow-up tracking

Implementation and independent validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution).
