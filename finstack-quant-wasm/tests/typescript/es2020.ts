import { core, type WasmOwned } from '../../index.js';

const numberValues: number[] = [0, 1, 1, 0.99];
const typedValues = new Float64Array(numberValues);

const discountFromArray = new core.DiscountCurve('USD-OIS', '2025-01-01', numberValues);
const discountFromTyped = new core.DiscountCurve('USD-OIS-TYPED', '2025-01-01', typedValues);

const forwardFromArray = new core.ForwardCurve(
  'USD-SOFR-3M',
  0.25,
  '2025-01-01',
  [0, 0.04, 1, 0.045],
  undefined,
  undefined,
  undefined,
  [0, 0.25, 1]
);
const forwardFromTyped = new core.ForwardCurve(
  'USD-SOFR-3M-TYPED',
  0.25,
  '2025-01-01',
  new Float64Array([0, 0.04, 1, 0.045]),
  undefined,
  undefined,
  undefined,
  new Float64Array([0, 0.25, 1])
);

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

const projectionGrid: Float64Array | null = forwardFromTyped.projectionGrid;
const expiries: Float64Array = fxVolFromTyped.expiries;
const pillarVols: Float64Array = fxVolFromArrays.pillarVols(0);

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
