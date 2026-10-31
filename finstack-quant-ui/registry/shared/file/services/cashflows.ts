import type { valuations } from "finstack-quant-wasm";
import type { CashflowsApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** valuations facade members this slice reads. */
export type CashflowsNative = {
  valuations: {
    instruments: Pick<
      typeof valuations.instruments,
      "calendarYearLadderByCurrency"
    >;
  };
};
/** CashflowsApi over the valuations facade namespace. */
export function createCashflowsService(
  native: CashflowsNative,
  context: ServiceContext,
): CashflowsApi {
  return {
    cashflowLadder(envelope) {
      return context.result(() =>
        native.valuations.instruments.calendarYearLadderByCurrency(envelope),
      );
    },
  };
}
