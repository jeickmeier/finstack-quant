import type { core, models } from "finstack-quant-wasm";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import type { ModelsApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** models.volatility evaluators and the core surface handles they sample. */
export type ModelsNative = {
  core: Pick<typeof core, "FxDeltaVolSurface" | "VolCube">;
  models: {
    volatility: Pick<
      typeof models.volatility,
      "getFxDeltaVol" | "getCubeVol" | "getCubeNormalVol"
    >;
  };
};
/** ModelsApi over the models facade namespace. */
export function createModelsService(
  native: ModelsNative,
  context: ServiceContext,
): ModelsApi {
  return {
    sampleCube(request) {
      return context.result(() => {
        const evaluate =
          request.convention === "normal"
            ? native.models.volatility.getCubeNormalVol
            : request.convention === "black_lognormal"
              ? native.models.volatility.getCubeVol
              : null;
        if (!evaluate)
          throw new RangeError("Unsupported cube output convention");
        const handle = native.core.VolCube.fromJson(
          serializeHost(request.cube),
        );
        try {
          return request.coordinates.map((point) =>
            evaluate(handle, point.expiry, point.tenor, point.strike),
          );
        } finally {
          handle.free();
        }
      });
    },
    sampleFxDelta(request) {
      return context.result(() => {
        const handle = native.core.FxDeltaVolSurface.fromJson(
          serializeHost(request.surface),
        );
        try {
          return request.coordinates.map((c) =>
            native.models.volatility.getFxDeltaVol(
              handle,
              c.expiry,
              c.strike,
              c.forward,
            ),
          );
        } finally {
          handle.free();
        }
      });
    },
  };
}
