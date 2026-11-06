# ATTRIBUTION-014: Single-period return contribution and exposure weighting

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and input domain

The return-contribution API is separate from priced P&L attribution: it consumes caller-supplied dimensionless simple period returns and market values or explicit weights, without fetching prices/FX or inferring period boundaries. `as_of` is a nonempty string, not parsed as a date. At least one position is required; IDs/factor names must be nonempty. Finite returns, market values, weights, exposures and factor returns are required, and derived arithmetic must remain finite. Each position supplies exactly one of weight/market value and the book cannot mix modes. Duplicate position/factor names are not rejected or deduplicated.

## Algorithm and controls

Gross weighting is the default: `w_i=MV_i/sum(abs(MV))`, retaining short signs. NetMarketValue uses `w_i=MV_i/sum(MV)`. Sums use Neumaier compensation. An absolute denominator at most 1e-9 gives all-zero gross weights, but errors for net weighting. Net magnitude below 1% of gross adds a leverage warning. These are absolute input-unit thresholds, not currency-normalized measures.

Explicit weights are used unchanged; weighting mode has no further effect. Without benchmarks a weight sum more than 1e-6 away from one gives a warning and preserves the leveraged/partially invested basis. Contribution is `w_i*r_i`; portfolio return is their compensated sum. Output instrument rows sort by ID. Each grouping dimension sums contributions by supplied key; positions missing a dimension contribute to its `unknown` bucket. An explicit `unknown` group merges with that fallback. Group dimensions/keys are BTree-ordered and each existing dimension covers total return; dimensions absent on every position are not invented.

Caller factor contributions are exposure × factor return, sorted by factor name, with specific return=`portfolio_return-sum(factor_contributions)` only when factors exist. This is an arithmetic overlay; the kernel does not estimate betas, prove factor orthogonality or normalize exposures. JSON validation runs the full calculation before canonical reserialization, so it catches derived overflow as well as shape errors.

## Rationale and alternatives

Gross weighting gives a defined signed basis for flat long/short books; net weighting gives net-capital return and may amplify near-flat positions. Both are implemented alternatives, while external-cashflow/time-weighted linking is outside this kernel. The [return-contribution tests](../../../../finstack-quant/attribution/tests/attribution/return_contribution.rs) were sampled for overflow and hand-calculated grouped/factor output; they also locate gross signs, net-flat rejection, leverage warning and specific-return behavior. They are formula/contract evidence, not market-data validation.

## Source and evidence

- [return_contribution.rs](../../../../finstack-quant/attribution/src/return_contribution.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
