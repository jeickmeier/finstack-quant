import { parentPort, workerData } from "node:worker_threads";
import { createRequire } from "node:module";
import { expose } from "comlink";
import nodeEndpoint from "comlink/dist/esm/node-adapter.mjs";
import { createService } from "@/workers/finstack-service";
const wasm = createRequire(import.meta.url)(workerData.packagePath);
let constructed = 0,
  freed = 0,
  calls = 0;
class Market extends wasm.Market {
  constructor(json) {
    super(json);
    constructed++;
  }
  toJson() {
    if (workerData.failMarketSerialization)
      throw new TypeError("Market serialization failed");
    return super.toJson();
  }
  free() {
    super.free();
    freed++;
  }
}
let cubeConstructed = 0,
  cubeFreed = 0,
  cubeJson = "";
class VolCube extends wasm.VolCube {
  static fromJson(json) {
    const handle = super.fromJson(json);
    cubeConstructed++;
    cubeJson = json;
    const free = handle.free.bind(handle);
    handle.free = () => {
      free();
      cubeFreed++;
    };
    return handle;
  }
}
let fxConstructed = 0,
  fxFreed = 0;
class FxDeltaVolSurface extends wasm.FxDeltaVolSurface {
  static fromJson(json) {
    const handle = super.fromJson(json);
    fxConstructed++;
    const free = handle.free.bind(handle);
    handle.free = () => {
      free();
      fxFreed++;
    };
    return handle;
  }
}
let moneyConstructed = 0,
  moneyFreed = 0;
class Money extends wasm.Money {
  static fromDecimalStr(amount, currency) {
    const handle = super.fromDecimalStr(amount, currency);
    moneyConstructed++;
    const free = handle.free.bind(handle);
    handle.free = () => {
      free();
      moneyFreed++;
    };
    return handle;
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
  core: { ...wasm, FxDeltaVolSurface, Money, VolCube },
  models: {
    volatility: {
      getCubeVol: wasm.getCubeVol,
      getCubeNormalVol: wasm.getCubeNormalVol,
      getFxDeltaVol: wasm.getFxDeltaVol,
    },
  },
  calibration: {
    calibrate: wasm.calibrate,
    dryRun: wasm.dryRun,
    validateCalibrationJson: wasm.validateCalibrationJson,
  },
  statements: wasm,
  statements_analytics: wasm,
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
      cubeJson,
      moneyConstructed,
      moneyFreed,
    }),
  },
  nodeEndpoint(parentPort),
);
