import type { core, models } from "finstack-quant-wasm";
import { formatMoney } from "@/lib/finstack/format/format";
import type { CoreApi, CurveQueryResult } from "../finstack-contract";
import { attempt, type ServiceContext } from "./context";
/** core facade members this slice reads, and the surface evaluator; MarketContext backs `validateMarket` and `curveQuery` through the context cache. */
export type CoreNative = {
  core: Pick<
    typeof core,
    "availableCalendars" | "Currency" | "MarketContext" | "Money"
  >;
  models: { volatility: Pick<typeof models.volatility, "getSurfaceVol"> };
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
    curveQuery(request) {
      return context.result((): CurveQueryResult => {
        const market = context.market(request.marketJson).handle;
        switch (request.kind) {
          case "discount": {
            const curve = market.getDiscount(request.id);
            try {
              return {
                kind: "discount",
                points: request.points.map((t) => ({
                  t,
                  df: attempt(() => curve.df(t)),
                  zeroRate: attempt(() => curve.zeroRate(t)),
                })),
                forwards: request.forwards.map(([t1, t2]) => ({
                  t1,
                  t2,
                  rate: attempt(() => curve.forward(t1, t2)),
                })),
              };
            } finally {
              curve.free();
            }
          }
          case "forward": {
            const curve = market.getForward(request.id);
            try {
              return {
                kind: "forward",
                points: request.points.map((t) => ({
                  t,
                  rate: attempt(() => curve.rate(t)),
                })),
              };
            } finally {
              curve.free();
            }
          }
          case "hazard": {
            const curve = market.getHazard(request.id);
            try {
              return {
                kind: "hazard",
                points: request.points.map((t) => ({
                  t,
                  sp: attempt(() => curve.sp(t)),
                  hazardRate: attempt(() => curve.hazardRate(t)),
                })),
              };
            } finally {
              curve.free();
            }
          }
          case "surface": {
            const surface = market.getSurface(request.id);
            try {
              return {
                kind: "surface",
                points: request.points.map(({ expiry, strike }) => ({
                  expiry,
                  strike,
                  vol: attempt(() =>
                    native.models.volatility.getSurfaceVol(
                      surface,
                      expiry,
                      strike,
                    ),
                  ),
                })),
              };
            } finally {
              surface.free();
            }
          }
          default: {
            const unhandled: never = request;
            throw new RangeError("Unsupported curve query kind", {
              cause: unhandled,
            });
          }
        }
      });
    },
    calendars: () => context.result(() => native.core.availableCalendars()),
  };
}
