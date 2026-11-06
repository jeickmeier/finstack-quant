# ATTRIBUTION-016: Single-period grouped benchmark-relative attribution

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and eligible inputs

Benchmark mode requires both benchmark weight and return on every position, or neither anywhere. Both portfolio and benchmark weight sums must equal one within absolute 1e-9. Negative weights are not categorically prohibited, but a group with absolute net weight at most 1e-9 and contribution greater than 1e-9 is rejected because its net-weighted return is undefined. The error recommends separating long/short groups. This applies to input return contributions, not repriced instrument P&L.

## Algorithm and conventions

Compute total benchmark return `Rb=sum(wb_i*rb_i)` and active return `Rp-Rb`. Choose one grouping dimension: `sector` if any position has it, otherwise the lexicographically first supplied dimension; with no dimensions use a single `all` group. Missing selected dimension labels become `unknown`. The chosen dimension is stamped in output; other grouping dimensions remain ordinary contribution reports.

For each group form compensated portfolio/benchmark weights and contributions, and derive group returns by contribution/weight. For a zero benchmark group with zero contribution, substitute total benchmark return and emit a warning. For a zero portfolio group with zero contribution, substitute that group's benchmark return and warn. Then sum:

- Allocation: `(wp_g-wb_g)*(rb_g-Rb)`.
- Selection: `wb_g*(rp_g-rb_g)`.
- Interaction: `(wp_g-wb_g)*(rp_g-rb_g)`.

Residual is active return minus these three sums. Instrument active contribution is `wp_i*rp_i-wb_i*rb_i`, not simply portfolio weight times relative return. All arithmetic rejects nonfinite derived values. This is single-period arithmetic attribution; no geometric linking, FX decomposition or transaction timing is introduced.

## Rationale and alternatives

The observed Brinson-Fachler-style allocation formula centers benchmark group returns on the whole benchmark. Zero-group substitutions preserve the selected economic classification as well as the sum; they are assumptions requiring agreement, not universal consequences of reconciliation. The repository's [Brinson reference anchor](../../../REFERENCES.md#brinson-fachler-1985) supplies background, but its heading and bibliographic entry do not pin one exact source for these zero-group conventions; code additionally cites Bacon without a canonical entry here. Supported alternative group choices are implicit through available labels, not an explicit group-dimension control; explicit selection or multi-dimensional attribution is unimplemented here.

The sampled [return-contribution tests](../../../../finstack-quant/attribution/tests/attribution/return_contribution.rs) contain hand-calculated grouped/factor examples and assert that the illustrated absent groups allocate -4% active return entirely to interaction. These are formula and policy tests. A small residual is algebraic reconciliation and cannot alone establish appropriate group selection or economic adoption.

## Source and evidence

- [return_contribution.rs](../../../../finstack-quant/attribution/src/return_contribution.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
