import type { margin } from "finstack-quant-wasm";
import type { MarginApi, VmReport } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** margin facade members this slice reads; every handle is per call. */
export type MarginNative = {
  margin: Pick<
    typeof margin,
    | "SimmSensitivities"
    | "SimmCalculator"
    | "VmCalculator"
    | "csaSpecUsdRegulatory"
    | "csaSpecEurRegulatory"
    | "csaSpecRegulatory"
    | "csaSpecValidate"
    | "vmResultRequiresCall"
    | "vmResultNetMargin"
  >;
};
/** MarginApi over the margin facade namespace. */
export function createMarginService(
  native: MarginNative,
  context: ServiceContext,
): MarginApi {
  /** Run `use` on a VmCalculator bound to `csaJson`, freeing it afterwards. */
  function withCalculator<T>(
    csaJson: string,
    use: (calculator: InstanceType<typeof native.margin.VmCalculator>) => T,
  ): T {
    const calculator = new native.margin.VmCalculator(csaJson);
    try {
      return use(calculator);
    } finally {
      calculator.free();
    }
  }
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
    csaPreset(request) {
      return context.result(() => {
        switch (request.kind) {
          case "usd":
            return native.margin.csaSpecUsdRegulatory();
          case "eur":
            return native.margin.csaSpecEurRegulatory();
          case "regulatory":
            return native.margin.csaSpecRegulatory(
              request.currency,
              request.id,
              request.collateralCurve,
            );
        }
      });
    },
    validateCsa(json) {
      return context.result(() => {
        native.margin.csaSpecValidate(json);
        // The calculator's CSA is the Rust-serialized canonical form.
        return withCalculator(json, (calculator) =>
          JSON.stringify(calculator.csa),
        );
      });
    },
    vm(request) {
      return context.result(() =>
        withCalculator(request.csaJson, (calculator): VmReport => {
          // The CSA's own base currency; Rust rejects any other.
          const result = calculator.calculate(
            request.exposure,
            request.postedCollateral,
            calculator.csa.base_currency,
            request.asOf,
          );
          return {
            result,
            requiresCall: native.margin.vmResultRequiresCall(result),
            netMargin: native.margin.vmResultNetMargin(result),
          };
        }),
      );
    },
    marginCalls(request) {
      return context.result(() =>
        withCalculator(request.csaJson, (calculator) =>
          calculator.generateMarginCalls(
            request.exposures,
            request.initialCollateral,
          ),
        ),
      );
    },
    marginCallDates(request) {
      return context.result(() =>
        withCalculator(request.csaJson, (calculator) =>
          calculator.marginCallDates(request.start, request.end),
        ),
      );
    },
  };
}
