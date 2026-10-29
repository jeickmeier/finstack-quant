import type { margin } from "finstack-quant-wasm";
import type { MarginApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** margin facade members this slice reads; every handle is per call. */
export type MarginNative = {
  margin: Pick<typeof margin, "SimmSensitivities" | "SimmCalculator">;
};
/** MarginApi over the margin facade namespace. */
export function createMarginService(
  native: MarginNative,
  context: ServiceContext,
): MarginApi {
  return {
    validateSimmSensitivities(json) {
      return context.result(() => {
        const sensitivities = native.margin.SimmSensitivities.fromJson(json);
        try {
          sensitivities.validate();
          return sensitivities.toJson();
        } finally {
          sensitivities.free();
        }
      });
    },
    simmIm(request) {
      return context.result(() => {
        const sensitivities = native.margin.SimmSensitivities.fromJson(
          request.sensitivitiesJson,
        );
        try {
          // Omitted options stay unset so Rust applies its own defaults.
          const calculator = new native.margin.SimmCalculator(
            request.version ?? undefined,
            request.mporDays ?? undefined,
          );
          try {
            // The set's own base currency; Rust rejects anything but USD.
            return calculator.calculateFromSensitivities(
              sensitivities,
              sensitivities.baseCurrency,
              request.asOf,
            );
          } finally {
            calculator.free();
          }
        } finally {
          sensitivities.free();
        }
      });
    },
  };
}
