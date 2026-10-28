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
  core: { ...facade.core, FxDeltaVolSurface, MarketContext, Money, VolCube },
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
      cubeConstructed,
      cubeFreed,
      cubeJson,
      moneyConstructed,
      moneyFreed,
    }),
  },
  nodeEndpoint(parentPort),
);
