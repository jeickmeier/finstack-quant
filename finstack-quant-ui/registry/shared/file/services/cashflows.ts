import type { cashflows } from "finstack-quant-wasm";
import type {
  CashflowEnvelope,
  CashflowsApi,
  CurrencyLadder,
} from "../finstack-contract";
import type { ServiceContext } from "./context";
/** cashflows facade members this slice reads. */
export type CashflowsNative = {
  cashflows: Pick<typeof cashflows, "calendarYearLadder">;
};
/** CashflowsApi over the cashflows facade namespace. */
export function createCashflowsService(
  native: CashflowsNative,
  context: ServiceContext,
): CashflowsApi {
  return {
    cashflowLadder(envelope) {
      return context.result(() => {
        // Rust requires one currency's units per call, so flows are partitioned
        // by row currency (first appearance); `native_pv` is the row-currency PV.
        const groups = new Map<
          CurrencyLadder["currency"],
          CashflowEnvelope["flows"]
        >();
        for (const flow of envelope.flows) {
          const group = groups.get(flow.currency);
          if (group) group.push(flow);
          else groups.set(flow.currency, [flow]);
        }
        return [...groups].map(([currency, flows]) => ({
          currency,
          rows: native.cashflows.calendarYearLadder(
            flows.map((flow) => flow.date),
            flows.map((flow) => flow.kind),
            flows.map((flow) => flow.amount),
            flows.map((flow) => flow.native_pv),
          ),
        }));
      });
    },
  };
}
