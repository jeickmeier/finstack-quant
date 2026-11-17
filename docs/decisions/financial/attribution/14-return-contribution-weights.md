# ATTRIBUTION-014: Single-period return contribution and exposure weighting

- Decision status: Accepted 2026-11-16 for checked single-period return contributions.
- Implementation status: Observed for the source scope listed below. Runtime, arithmetic, wire and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked tiny gross and exact-zero net values, negative net capital, supplied weights, cancellation, grouped coverage and specific return. Production and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked single-period return contributions

1. Bind one dated return basis. Require a positive-length opening-to-closing interval, one reporting currency, compatible valuation, cash, fee and FX conventions, and a nonempty set of unique position IDs. Distinct positions may reference the same instrument. Validate every supplied simple period return, including zero-weight rows. Apply holding size and direction once through the weight. Finite signed arithmetic returns at or below `−100%` are eligible on a qualified basis. Wealth returns and multiperiod linking retain their own eligibility rules. [Accepted single-period attribution](../portfolio-attribution/05-brinson-fachler.md#accepted-decision-for-checked-single-period-brinsonfachler-attribution).
2. Elect one weight basis for the whole book. Gross weighting divides signed opening Money by the sum of absolute opening values. Net weighting divides by the signed net opening value and reports contribution per signed net capital. An exact-zero selected denominator fails. Negative net capital and tiny nonzero denominators are eligible when required results are representable. Use supplied signed decimal weights as entered and report their actual net and gross sums. Retire the default, fixed denominator cutoff, and fixed leverage and unity warnings. [Accepted Money precision](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).
3. Calculate contributions before rounding weights for output. Define each contribution as `w_i r_i`. For market-value weights compute the total on the common denominator `D` as `Σ_i MV_i r_i / D`. Use bounded widened arithmetic on admitted Decimal amounts and weights and exact admitted binary64 return values. Verify position, group and factor closure before projecting required outputs. Reject required results lost to range and preserve genuine zeros. [Accepted widened attribution arithmetic](../portfolio-attribution/06-continuous-factor-brinson.md#accepted-decision-for-checked-continuous-factor-attribution).
4. Make each elected grouping dimension a complete partition. Require exactly one membership per position and dimension, with an explicit Unclassified membership distinct from the literal label `unknown`. Sum each bucket directly from its position contributions. Retain zero-net-weight groups with nonzero contribution. Each dimension independently reconciles to the same book total. [Record 016](16-benchmark-relative-return.md) owns benchmark-relative group-return and allocation rules.
5. Elect a complete factor view or leave it absent. Require unique ordered factor IDs, opening exposures on the elected weight basis and same-period factor moves in their declared units. Calculate each signed exposure-times-move contribution. Specific return is the independently verified book total less the factor sum. Missing elected factors fail. Specific return is unavailable when the overlay is unelected. Factor and grouping views each explain the same total.
6. Use one checked Rust contract across entry points. Preserve interval, basis, identities, ordering and admitted source inputs with the result. Reject duplicate JSON keys before map conversion and duplicate DataFrame columns before row conversion. JSON validation executes the full calculation, including derived range and closure. Python and WASM consume the same calculation and availability rules. [Accepted numeric wire contract](../core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity).

Exact examples verified that opening values of `(+1e−28, −1e−28)` with returns `(+10%, +6%)` produce `+2%` on the gross basis and have a zero net denominator. A separate grouping example gave `−5` percentage points of contribution for a group with zero net weight.

## Observed choice and input domain

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

Runtime reconciliation and validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). Production and host
qualification remain pending.
