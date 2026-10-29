import { parentPort, workerData } from "node:worker_threads";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { expose } from "comlink";
import nodeEndpoint from "comlink/dist/esm/node-adapter.mjs";
import { createService } from "@/workers/finstack-service";
// The published facade, imported at run time so the build never bundles it.
const { default: init, ...facade } = await import(
  pathToFileURL(workerData.facadePath).href
);
let constructed = 0,
  freed = 0,
  calls = 0;
class MarketContext extends facade.core.MarketContext {
  static fromJson(json) {
    const handle = super.fromJson(json);
    constructed++;
    const toJson = handle.toJson.bind(handle);
    handle.toJson = () => {
      if (workerData.failMarketSerialization)
        throw new TypeError("Market serialization failed");
      return toJson();
    };
    const free = handle.free.bind(handle);
    handle.free = () => {
      free();
      freed++;
    };
    return handle;
  }
}
let cubeConstructed = 0,
  cubeFreed = 0,
  cubeJson = "";
class VolCube extends facade.core.VolCube {
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
class FxDeltaVolSurface extends facade.core.FxDeltaVolSurface {
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
let gridConstructed = 0,
  gridFreed = 0;
function materializeFxDeltaSurface(...args) {
  const handle = facade.models.volatility.materializeFxDeltaSurface(...args);
  gridConstructed++;
  const free = handle.free.bind(handle);
  handle.free = () => {
    free();
    gridFreed++;
  };
  return handle;
}
let sliceConstructed = 0,
  sliceFreed = 0;
// The four cube-slice materializers share one constructed/freed count.
const slices = Object.fromEntries(
  [
    "materializeCubeTenorSlice",
    "materializeCubeTenorSliceNormal",
    "materializeCubeExpirySlice",
    "materializeCubeExpirySliceNormal",
  ].map((name) => [
    name,
    (...args) => {
      const handle = facade.models.volatility[name](...args);
      sliceConstructed++;
      const free = handle.free.bind(handle);
      handle.free = () => {
        free();
        sliceFreed++;
      };
      return handle;
    },
  ]),
);
let sabrConstructed = 0,
  sabrFreed = 0;
class SabrParameters extends facade.models.volatility.SabrParameters {
  static fromJson(json) {
    const handle = super.fromJson(json);
    sabrConstructed++;
    const free = handle.free.bind(handle);
    handle.free = () => {
      free();
      sabrFreed++;
    };
    return handle;
  }
}
let smileConstructed = 0,
  smileFreed = 0;
class SabrSmile extends facade.models.volatility.SabrSmile {
  constructor(...args) {
    super(...args);
    smileConstructed++;
    const free = this.free.bind(this);
    this.free = () => {
      free();
      smileFreed++;
    };
  }
}
let moneyConstructed = 0,
  moneyFreed = 0;
class Money extends facade.core.Money {
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
let performanceConstructed = 0,
  performanceFreed = 0;
function countPerformance(handle) {
  performanceConstructed++;
  const free = handle.free.bind(handle);
  handle.free = () => {
    free();
    performanceFreed++;
  };
  return handle;
}
class Performance extends facade.analytics.Performance {
  constructor(...args) {
    super(...args);
    countPerformance(this);
  }
  static fromReturns(...args) {
    return countPerformance(super.fromReturns(...args));
  }
}
let fiscalConstructed = 0,
  fiscalFreed = 0;
class FiscalConfig extends facade.core.FiscalConfig {
  constructor(...args) {
    super(...args);
    fiscalConstructed++;
    const free = this.free.bind(this);
    this.free = () => {
      free();
      fiscalFreed++;
    };
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
    await init({ module_or_path: readFileSync(workerData.wasmPath) });
  },
  ...facade,
  core: {
    ...facade.core,
    FiscalConfig,
    FxDeltaVolSurface,
    MarketContext,
    Money,
    VolCube,
  },
  analytics: { ...facade.analytics, Performance },
  models: {
    ...facade.models,
    volatility: {
      ...facade.models.volatility,
      ...slices,
      materializeFxDeltaSurface,
      SabrParameters,
      SabrSmile,
    },
  },
  valuations: {
    ...facade.valuations,
    instruments: {
      ...facade.valuations.instruments,
      priceInstrumentWithMarket(...args) {
        calls++;
        return facade.valuations.instruments.priceInstrumentWithMarket(...args);
      },
    },
  },
});
expose(
  {
    ...service,
    methods: () => Object.keys(service),
    resources: () => ({
      constructed,
      freed,
      calls,
      fxConstructed,
      fxFreed,
      gridConstructed,
      gridFreed,
      sliceConstructed,
      sliceFreed,
      sabrConstructed,
      sabrFreed,
      smileConstructed,
      smileFreed,
      cubeConstructed,
      cubeFreed,
      cubeJson,
      moneyConstructed,
      moneyFreed,
      performanceConstructed,
      performanceFreed,
      fiscalConstructed,
      fiscalFreed,
    }),
  },
  nodeEndpoint(parentPort),
);
