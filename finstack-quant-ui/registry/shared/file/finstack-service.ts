import type { WorkerApi } from "./finstack-contract";
import { createContext } from "./services/context";
import { createCoreService, type CoreNative } from "./services/core";
import {
  createValuationsService,
  type ValuationsNative,
} from "./services/valuations";
import {
  createCalibrationService,
  type CalibrationNative,
} from "./services/calibration";
import { createModelsService, type ModelsNative } from "./services/models";
import {
  createStatementsService,
  type StatementsNative,
} from "./services/statements";
/** Dependencies are the published facade; injection permits the same service in a real Node worker. */
export function createService(
  native: { initialize(wasmUrl?: string): Promise<unknown> } & CoreNative &
    ValuationsNative &
    CalibrationNative &
    ModelsNative &
    StatementsNative,
): WorkerApi {
  const context = createContext(native);
  return {
    initialize(url) {
      void context.initialize(url);
      return context.result(() => ({
        state: "ready" as const,
        worker: typeof document === "undefined",
      }));
    },
    ...createCoreService(native, context),
    ...createValuationsService(native, context),
    ...createCalibrationService(native, context),
    ...createModelsService(native, context),
    ...createStatementsService(native, context),
  };
}
