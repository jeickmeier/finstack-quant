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
let volSurfaceConstructed = 0,
  volSurfaceFreed = 0;
class VolSurface extends facade.core.VolSurface {
  static fromJson(json) {
    const handle = super.fromJson(json);
    volSurfaceConstructed++;
    const free = handle.free.bind(handle);
    handle.free = () => {
      free();
      volSurfaceFreed++;
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
// The two cube-slice materializers share one constructed/freed count.
const slices = Object.fromEntries(
  ["materializeCubeTenorSlice", "materializeCubeExpirySlice"].map((name) => [
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
let portfolioConstructed = 0,
  portfolioFreed = 0;
class Portfolio extends facade.portfolio.Portfolio {
  static fromSpec(...args) {
    const handle = super.fromSpec(...args);
    portfolioConstructed++;
    const free = handle.free.bind(handle);
    handle.free = () => {
      free();
      portfolioFreed++;
    };
    return handle;
  }
}
// Rating-migration handles: matrices (fromLabeled and project), generators
// and the fresh RatingScale every TransitionMatrix#scale read returns.
const migration = facade.models.credit.migration;
const migrationCounts = {
  matrixConstructed: 0,
  matrixFreed: 0,
  generatorConstructed: 0,
  generatorFreed: 0,
  scaleConstructed: 0,
  scaleFreed: 0,
};
function countMigration(kind, handle) {
  migrationCounts[`${kind}Constructed`]++;
  const free = handle.free.bind(handle);
  handle.free = () => {
    free();
    migrationCounts[`${kind}Freed`]++;
  };
  return handle;
}
const scaleGetter = Object.getOwnPropertyDescriptor(
  migration.TransitionMatrix.prototype,
  "scale",
).get;
function countMatrix(handle) {
  countMigration("matrix", handle);
  Object.defineProperty(handle, "scale", {
    get: () => countMigration("scale", scaleGetter.call(handle)),
  });
  return handle;
}
class TransitionMatrix extends migration.TransitionMatrix {
  static fromLabeled(...args) {
    return countMatrix(super.fromLabeled(...args));
  }
}
class GeneratorMatrix extends migration.GeneratorMatrix {
  static fromTransitionMatrix(...args) {
    return countMigration("generator", super.fromTransitionMatrix(...args));
  }
}
const project = (...args) => countMatrix(migration.project(...args));
// Merton handles (fromJson and fromEquity) and the default master scale.
const mertonCounts = {
  mertonConstructed: 0,
  mertonFreed: 0,
  masterScaleConstructed: 0,
  masterScaleFreed: 0,
};
function countHandle(kind, handle) {
  mertonCounts[`${kind}Constructed`]++;
  const free = handle.free.bind(handle);
  handle.free = () => {
    free();
    mertonCounts[`${kind}Freed`]++;
  };
  return handle;
}
class MertonModel extends facade.models.credit.MertonModel {
  static fromJson(...args) {
    return countHandle("merton", super.fromJson(...args));
  }
  static fromEquity(...args) {
    return countHandle("merton", super.fromEquity(...args));
  }
}
class MasterScale extends facade.models.credit.pd.MasterScale {
  static defaultCalibration() {
    return countHandle("masterScale", super.defaultCalibration());
  }
}
let simmSensitivitiesConstructed = 0,
  simmSensitivitiesFreed = 0,
  simmCalculatorConstructed = 0,
  simmCalculatorFreed = 0,
  simmCalculatorArgs = [];
class SimmSensitivities extends facade.margin.SimmSensitivities {
  static fromJson(json) {
    const handle = super.fromJson(json);
    simmSensitivitiesConstructed++;
    const free = handle.free.bind(handle);
    handle.free = () => {
      free();
      simmSensitivitiesFreed++;
    };
    return handle;
  }
}
class SimmCalculator extends facade.margin.SimmCalculator {
  constructor(...args) {
    super(...args);
    simmCalculatorConstructed++;
    simmCalculatorArgs = args;
    const free = this.free.bind(this);
    this.free = () => {
      free();
      simmCalculatorFreed++;
    };
  }
}
let vmCalculatorConstructed = 0,
  vmCalculatorFreed = 0;
class VmCalculator extends facade.margin.VmCalculator {
  constructor(...args) {
    // Counted only once Rust accepts the CSA, like every other handle here.
    super(...args);
    vmCalculatorConstructed++;
    const free = this.free.bind(this);
    this.free = () => {
      free();
      vmCalculatorFreed++;
    };
  }
}
let covenantEngineConstructed = 0,
  covenantEngineFreed = 0;
class CovenantEngine extends facade.covenants.CovenantEngine {
  static fromJson(json) {
    const handle = super.fromJson(json);
    covenantEngineConstructed++;
    const free = handle.free.bind(handle);
    handle.free = () => {
      free();
      covenantEngineFreed++;
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
  core: {
    ...facade.core,
    FiscalConfig,
    FxDeltaVolSurface,
    MarketContext,
    Money,
    VolCube,
    VolSurface,
  },
  analytics: { ...facade.analytics, Performance },
  covenants: { ...facade.covenants, CovenantEngine },
  portfolio: { ...facade.portfolio, Portfolio },
  margin: { ...facade.margin, SimmSensitivities, SimmCalculator, VmCalculator },
  models: {
    ...facade.models,
    volatility: {
      ...facade.models.volatility,
      ...slices,
      materializeFxDeltaSurface,
      SabrParameters,
      SabrSmile,
    },
    credit: {
      ...facade.models.credit,
      MertonModel,
      pd: { ...facade.models.credit.pd, MasterScale },
      migration: { ...migration, TransitionMatrix, GeneratorMatrix, project },
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
      volSurfaceConstructed,
      volSurfaceFreed,
      moneyConstructed,
      moneyFreed,
      performanceConstructed,
      performanceFreed,
      fiscalConstructed,
      fiscalFreed,
      portfolioConstructed,
      portfolioFreed,
      ...migrationCounts,
      ...mertonCounts,
      covenantEngineConstructed,
      covenantEngineFreed,
      simmSensitivitiesConstructed,
      simmSensitivitiesFreed,
      simmCalculatorConstructed,
      simmCalculatorFreed,
      // Whether each optional calculator argument was passed at all.
      simmCalculatorArgs: simmCalculatorArgs.map((arg) => arg === undefined),
      vmCalculatorConstructed,
      vmCalculatorFreed,
    }),
  },
  nodeEndpoint(parentPort),
);
