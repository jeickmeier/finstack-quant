import {
  analytics,
  core,
  features,
  models,
  portfolio,
  scenarios,
  type AttributionFactor,
  type HorizonResult,
  type MaterializationPhases,
  type MaterializationReport,
  type WasmOwned,
  type ValuationResult,
} from '../../index.js';

const numberValues: number[] = [0, 1, 1, 0.99];
const typedValues = new Float64Array(numberValues);
declare const materializationReport: MaterializationReport;
const materializationPhases: MaterializationPhases = materializationReport.phase_nanos;
const materializationParseNanos: number = materializationPhases.parse;
const materializationTimingAvailable: boolean = materializationReport.timing_available;
const materializationDependencies: number = materializationReport.dependencies;

const discountFromArray = new core.DiscountCurve({
  id: 'USD-OIS',
  baseDate: '2025-01-01',
  knots: numberValues,
});
const discountFromTyped = new core.DiscountCurve({
  id: 'USD-OIS-TYPED',
  baseDate: '2025-01-01',
  knots: typedValues,
});

const forwardFromArray = new core.ForwardCurve({
  id: 'USD-SOFR-3M',
  tenor: 0.25,
  baseDate: '2025-01-01',
  knots: [0, 0.04, 1, 0.045],
  projectionGrid: [0, 0.25, 1],
});
const forwardFromTyped = new core.ForwardCurve({
  id: 'USD-SOFR-3M-TYPED',
  tenor: 0.25,
  baseDate: '2025-01-01',
  knots: new Float64Array([0, 0.04, 1, 0.045]),
  projectionGrid: new Float64Array([0, 0.25, 1]),
});

const cubeFromArrays = new core.VolCube(
  'USD-SWAPTION',
  [1],
  [5],
  [0.02, 0.5, 0, 0.3, Number.NaN],
  [0.04]
);
const cubeFromTyped = new core.VolCube(
  'USD-SWAPTION-TYPED',
  new Float64Array([1]),
  new Float64Array([5]),
  new Float64Array([0.02, 0.5, 0, 0.3, Number.NaN]),
  new Float64Array([0.04])
);

const fxVolFromArrays = new core.FxDeltaVolSurface(
  'EURUSD',
  [0.25, 1],
  [0.08, 0.09],
  [0.01, 0.012],
  [0.005, 0.006],
  [0.02, 0.022],
  [0.008, 0.009]
);
const fxVolFromTyped = new core.FxDeltaVolSurface(
  'EURUSD-TYPED',
  new Float64Array([0.25, 1]),
  new Float64Array([0.08, 0.09]),
  new Float64Array([0.01, 0.012]),
  new Float64Array([0.005, 0.006]),
  new Float64Array([0.02, 0.022]),
  new Float64Array([0.008, 0.009])
);

const projectionGrid: Float64Array | undefined = forwardFromTyped.projectionGrid;
const expiries: Float64Array = fxVolFromTyped.expiries;
const pillarVols: Float64Array = models.volatility.getFxDeltaPillarVols(fxVolFromArrays, 0);
const factorReturns = analytics.constrainedLeastSquares([1], 1, [0.01], [1]);
const factorAttribution: Record<string, unknown> = portfolio.factorBrinsonAttribution(
  '{}',
  factorReturns
);

const owned: WasmOwned[] = [
  discountFromArray,
  discountFromTyped,
  forwardFromArray,
  forwardFromTyped,
  cubeFromArrays,
  cubeFromTyped,
  fxVolFromArrays,
  fxVolFromTyped,
];
owned.forEach((value) => value.free());

void projectionGrid;
void expiries;
void pillarVols;
void factorAttribution;
void materializationParseNanos;
void materializationTimingAvailable;
void materializationDependencies;

features.riskScaledWeights([1, -1], ['d', 'd'], [0.1, 0.1]);
features.rankToWeights([1, 2], ['d', 'd']);
// @ts-expect-error This helper does not accept transform parameters.
features.riskScaledWeights([1, -1], ['d', 'd'], [0.1, 0.1], {});
// @ts-expect-error This helper does not accept transform parameters.
features.rankToWeights([1, 2], ['d', 'd'], {});

// Native output retains generated nested shapes and stricter emitted-field presence.
declare const valuation: ValuationResult;
const roundingMode: import('../../types/valuation-result').ValuationResult['meta']['rounding']['mode'] =
  valuation.meta.rounding.mode;
const covenantReports: NonNullable<
  import('../../types/valuation-result').ValuationResult['covenants']
> | null = valuation.covenants;
const explanation: import('../../types/valuation-result').ExplanationTrace | undefined =
  valuation.explanation;
// @ts-expect-error Native absence is omitted, never null.
valuation.explanation = null;
// @ts-expect-error Native covenant field is always emitted.
valuation.covenants = undefined;
// @ts-expect-error Metadata is a complete canonical structure.
valuation.meta = {};
void [roundingMode, covenantReports, explanation];

const scenarioSpec = scenarios.buildScenarioSpec('metadata', []);
const scenarioName: string | null | undefined = scenarioSpec.name;
scenarioSpec.name = null;
scenarioSpec.description = null;
const horizon: HorizonResult = scenarios.computeHorizonReturn('{}', '{}', '2025-01-15', '{}');
const horizonReturn: number | null = horizon.total_return;
const horizonAnnualized: number | null = horizon.annualized_return;
const horizonInitialAmount: string = horizon.initial_value.amount;
const horizonDays: number | null = horizon.horizon_days;
const horizonFactor: AttributionFactor = 'carry';
const carryContribution: number | null = horizon.factor_contributions[horizonFactor];
const horizonOperations: number = horizon.scenario_report.operations_applied;
// @ts-expect-error Undefined compounded returns must be handled by the caller.
const requiredAnnualized: number = horizon.annualized_return;
// @ts-expect-error Factor keys use canonical Rust attribution names.
void horizon.factor_contributions.rates;
void [
  scenarioName,
  horizonReturn,
  horizonAnnualized,
  horizonInitialAmount,
  horizonDays,
  carryContribution,
  horizonOperations,
  requiredAnnualized,
];
