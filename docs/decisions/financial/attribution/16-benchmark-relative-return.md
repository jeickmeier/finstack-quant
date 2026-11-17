# ATTRIBUTION-016: Single-period grouped benchmark-relative attribution

- Decision status: Accepted 2026-11-16 for checked grouped benchmark-relative returns.
- Implementation status: Observed for the source scope listed below. Runtime, arithmetic, wire and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked one-sided groups, exact weights, zero-net active groups, centered attribution closure and an active difference below side-total projection precision. Production and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked grouped benchmark-relative returns

1. Elect the benchmark and align both sides explicitly. Bind a qualified benchmark identity, the same dated interval and cuts, and compatible capital, return, currency, valuation, cash, fee and FX bases. Use the ordered union of qualified position identities, including benchmark-only positions and applicable cash and financing. A present side requires a finite return and nonzero opening weight. A proven absent side has zero contribution and unavailable observed return. Each side's opening weights must sum to exactly one using admitted Decimal weights or exact market-value ratios before projection. Retire the `1e−9` allowance and automatic normalization. [Accepted return contribution inputs](14-return-contribution-weights.md#accepted-decision-for-checked-single-period-return-contributions) · [Accepted Brinson–Fachler inputs](../portfolio-attribution/05-brinson-fachler.md#accepted-decision-for-checked-single-period-brinsonfachler-attribution).
2. Elect one complete common grouping dimension. Use a declared partition from record 014 and preserve its order and explicit Unclassified membership. An explicitly elected whole-book group gives zero allocation and interaction, with selection equal to active return. Retire the implicit sector, lexical and whole-book fallbacks.
3. Distinguish genuine absence from zero net group weight. Derive the group return for a present side from its exact contribution and exact signed weight. A group with offsetting active positions and zero net weight has an undefined group return even when contribution cancels. Fail the selected comparison with the group and side identified. Raw contribution remains available under record 014. A genuinely portfolio-only group uses whole-benchmark return as the effective comparator and contributes through interaction. A genuinely benchmark-only group uses its benchmark group return on the portfolio side and contributes through allocation. A declared group absent from both sides has zero effects. Publish effective comparators separately from observed returns.
4. Use centered Brinson–Fachler effects. For each group, with `Δw = w_P − w_B`, effective group returns `r_P*` and `r_B*` and whole-benchmark return `R_B`, compute allocation `Δw(r_B* − R_B)`, selection `w_B(r_P* − r_B*)` and interaction `Δw(r_P* − r_B*)`. Raw active position and group contributions are differences of the actual weighted side contributions. The per-group centered total equals the raw group contribution less `Δw × R_B`. Centering cancels over the complete partition. [Brinson and Fachler, 1985](../../../REFERENCES.md#brinson-fachler-1985).
5. Check closure before projecting results. Derive portfolio and benchmark returns and their active difference independently from admitted inputs. Use bounded widened arithmetic and verify that the total raw contribution and the sum of the three centered effects both reconcile to active return before projection. Preserve a representable active difference when displayed side totals coincide. Required range or closure failures produce typed errors. Keep projection rounding separate from economic effects.
6. Publish one checked result across hosts. Retain the benchmark, interval, weight basis, taxonomy, ordered rows, side presence, observed returns, comparators and closure evidence. Python and WASM project the Rust result and follow the accepted restoration rules. The scope is single-period arithmetic attribution. Portfolio performance and Carino linking retain their own eligibility rules. [Accepted report contract](15-detail-and-reporting-contracts.md#accepted-decision-for-checked-detail-identities-report-projections-and-schemas) · [Accepted Carino linking](../portfolio-attribution/17-carino-linking-and-reconciliation.md#accepted-decision-for-checked-carino-linking-and-reconciliation).

An exact three-group example gave `9.0%` portfolio return and `4.4%` benchmark return. The `4.6` percentage points of active return comprised `1.2` allocation, `1.2` selection and `2.2` interaction. A separate example retained a nonzero active difference even though both projected side totals were identical.

## Observed choice and eligible inputs

Benchmark mode requires both benchmark weight and return on every position, or neither anywhere. Both portfolio and benchmark weight sums must equal one within absolute 1e-9. Negative weights are not categorically prohibited, but a group with absolute net weight at most 1e-9 and contribution greater than 1e-9 is rejected because its net-weighted return is undefined. The error recommends separating long/short groups. This applies to input return contributions, not repriced instrument P&L.

## Algorithm and conventions

Compute total benchmark return `Rb=sum(wb_i*rb_i)` and active return `Rp-Rb`. Choose one grouping dimension: `sector` if any position has it, otherwise the lexicographically first supplied dimension; with no dimensions use a single `all` group. Missing selected dimension labels become `unknown`. The chosen dimension is stamped in output; other grouping dimensions remain ordinary contribution reports.

For each group form compensated portfolio/benchmark weights and contributions, and derive group returns by contribution/weight. For a zero benchmark group with zero contribution, substitute total benchmark return and emit a warning. For a zero portfolio group with zero contribution, substitute that group's benchmark return and warn. Then sum:

- Allocation: `(wp_g-wb_g)*(rb_g-Rb)`.
- Selection: `wb_g*(rp_g-rb_g)`.
- Interaction: `(wp_g-wb_g)*(rp_g-rb_g)`.

Residual is active return minus these three sums. Instrument active contribution is `wp_i*rp_i-wb_i*rb_i`, not simply portfolio weight times relative return. All arithmetic rejects nonfinite derived values. This is single-period arithmetic attribution; no geometric linking, FX decomposition or transaction timing is introduced.

## Rationale and alternatives

The observed Brinson-Fachler-style allocation formula centers benchmark group returns on the whole benchmark. Zero-group substitutions affect the economic classification as well as the sum. The [Brinson and Fachler reference](../../../REFERENCES.md#brinson-fachler-1985) supplies the centered formula. Code also cites Bacon without a canonical entry here.

The sampled [return-contribution tests](../../../../finstack-quant/attribution/tests/attribution/return_contribution.rs) contain hand-calculated grouped/factor examples and assert that the illustrated absent groups allocate -4% active return entirely to interaction. These are formula and policy tests. A small residual is algebraic reconciliation and cannot alone establish appropriate group selection or economic adoption.

## Source and evidence

- [return_contribution.rs](../../../../finstack-quant/attribution/src/return_contribution.rs) — owning implementation.

## Follow-up tracking

Runtime reconciliation and validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). Production and host
qualification remain pending.
