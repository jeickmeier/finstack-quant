import { parentPort, workerData } from "node:worker_threads";
import { createRequire } from "node:module";
import { expose } from "comlink";
import nodeEndpoint from "comlink/dist/esm/node-adapter.mjs";
import { createService } from "../../registry/workers/finstack-service";
const wasm = createRequire(import.meta.url)(workerData.packagePath);
let constructed = 0,
  freed = 0,
  calls = 0;
class Market extends wasm.Market {
  constructor(json) {
    super(json);
    constructed++;
  }
  free() {
    super.free();
    freed++;
  }
}
let cubeConstructed = 0,
  cubeFreed = 0,
  cubeParams = [];
class VolCube extends wasm.VolCube {
  constructor(...args) {
    super(...args);
    cubeConstructed++;
    cubeParams = [...args[3]];
  }
  free() {
    super.free();
    cubeFreed++;
  }
}
let fxConstructed = 0,
  fxFreed = 0;
class FxDeltaVolSurface extends wasm.FxDeltaVolSurface {
  constructor(...args) {
    super(...args);
    fxConstructed++;
  }
  free() {
    super.free();
    fxFreed++;
  }
}
const service = createService({
  initialize: async () => {
    if (workerData.fail)
      throw Object.assign(new TypeError("Native initialization failed"), {
        kind: "init",
        report: { wasm: "missing" },
        cause: new Error("fixture cause"),
      });
  },
  core: { ...wasm, FxDeltaVolSurface, VolCube },
  models: {
    volatility: {
      getCubeVol: wasm.getCubeVol,
      getCubeNormalVol: wasm.getCubeNormalVol,
      getFxDeltaVol: wasm.getFxDeltaVol,
    },
  },
  valuations: {
    Market,
    validateValuationResultJson: wasm.validateValuationResultJson,
    instruments: {
      ...wasm,
      priceInstrumentWithMarket(...args) {
        calls++;
        return wasm.priceInstrumentWithMarket(...args);
      },
    },
  },
});
expose(
  {
    ...service,
    resources: () => ({
      constructed,
      freed,
      calls,
      fxConstructed,
      fxFreed,
      cubeConstructed,
      cubeFreed,
      cubeParams,
    }),
  },
  nodeEndpoint(parentPort),
);
