import type { core } from "finstack-quant-wasm";
import { formatMoney } from "@/lib/finstack/format/format";
import type { CoreApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** core facade members this slice reads; MarketContext backs `validateMarket` through the context cache. */
export type CoreNative = {
  core: Pick<
    typeof core,
    "availableCalendars" | "Currency" | "MarketContext" | "Money"
  >;
};
/** CoreApi over the core facade namespace. */
export function createCoreService(
  native: CoreNative,
  context: ServiceContext,
): CoreApi {
  return {
    formatMoney(request) {
      return context.result(() =>
        formatMoney(request.value, request.rounding, native.core),
      );
    },
    validateMarket(json) {
      return context.result(() => context.market(json).json);
    },
    calendars: () => context.result(() => native.core.availableCalendars()),
  };
}
