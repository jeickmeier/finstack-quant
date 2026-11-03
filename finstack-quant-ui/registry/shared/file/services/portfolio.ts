import type { attribution, portfolio } from "finstack-quant-wasm";
import type { PortfolioApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** portfolio facade members this slice reads; the Portfolio handle comes from the context cache. */
export type PortfolioNative = {
  portfolio: Pick<
    typeof portfolio,
    | "valuePortfolio"
    | "aggregateMetrics"
    | "portfolioMetricsSeries"
    | "parsePortfolioSpecJson"
    | "scenarioPnlBatch"
    | "applyScenarioAndRevalue"
    | "portfolioMargin"
    | "computeFactorSensitivities"
    | "attributePortfolioPnl"
    | "portfolioAttributionExplainText"
    | "portfolioAttributionReconciliationCheck"
    | "portfolioAttributionPctOfTotalByBucket"
  >;
  attribution: Pick<
    typeof attribution,
    | "pnlAttributionExplainText"
    | "pnlAttributionResidualWithinTolerance"
    | "pnlAttributionPctOfTotalByBucket"
  >;
};
/** PortfolioApi over the portfolio facade namespace and the shared handle caches. */
export function createPortfolioService(
  native: PortfolioNative,
  context: ServiceContext,
): PortfolioApi {
  return {
    valuePortfolio(request) {
      return context.result(() => {
        const book = context.portfolio(request.specJson).handle;
        const market = context.market(request.marketJson).handle;
        // Omitted options stay unset so Rust applies its own defaults.
        const valuation = native.portfolio.valuePortfolio(
          book,
          market,
          request.strictRisk ?? undefined,
          request.metrics == null ? undefined : [...request.metrics],
        );
        return {
          positionIds: book.positionIds,
          entityIds: book.entityIds,
          valuation,
          metrics: native.portfolio.aggregateMetrics(valuation, market),
        };
      });
    },
    portfolioMetricsSeries(request) {
      return context.result(() =>
        native.portfolio.portfolioMetricsSeries(request.metrics, request.base),
      );
    },
    validatePortfolio(specJson) {
      return context.result(() => {
        // Schema check and canonical text, then Portfolio.fromSpec through the
        // cache for the book invariants (duplicate ids, unknown entities); the
        // built handle is retained for the valuation that usually follows.
        const canonical = native.portfolio.parsePortfolioSpecJson(specJson);
        return context.portfolio(canonical).json;
      });
    },
    scenarioPnlBatch(request) {
      return context.result(() => {
        const book = context.portfolio(request.specJson).handle;
        const market = context.market(request.marketJson).handle;
        // The scenario texts are joined into one JSON array; none is parsed here.
        const items = native.portfolio.scenarioPnlBatch(
          book,
          `[${request.scenarioJsons.join(",")}]`,
          market,
        );
        return { positionIds: book.positionIds, items };
      });
    },
    applyScenarioAndRevalue(request) {
      return context.result(() => {
        const book = context.portfolio(request.specJson).handle;
        const market = context.market(request.marketJson).handle;
        return {
          positionIds: book.positionIds,
          entityIds: book.entityIds,
          view: native.portfolio.applyScenarioAndRevalue(
            book,
            request.scenarioJson,
            market,
          ),
        };
      });
    },
    portfolioMargin(request) {
      return context.result(() =>
        // Omitted collateral stays unset; Rust treats it as none held.
        native.portfolio.portfolioMargin(
          context.portfolio(request.specJson).handle,
          context.market(request.marketJson).handle,
          request.asOf,
          request.currentImCollateral ?? undefined,
        ),
      );
    },
    attributePortfolioPnl(request) {
      return context.result(() => {
        const api = native.portfolio;
        const single = native.attribution;
        // Omitted config stays unset so Rust applies its default configuration.
        const attribution = api.attributePortfolioPnl(
          context.portfolio(request.specJson).handle,
          context.market(request.marketT0Json).handle,
          context.market(request.marketT1Json).handle,
          request.asOfT0,
          request.asOfT1,
          request.method,
        );
        return {
          attribution,
          explain: api.portfolioAttributionExplainText(attribution),
          reconciliation: api.portfolioAttributionReconciliationCheck(
            attribution,
            request.reconciliationTolerance,
          ),
          buckets: api.portfolioAttributionPctOfTotalByBucket(attribution),
          // finstack-compose: Rust's own text, check and shares per returned position attribution
          positions: Object.entries(attribution.by_position).map(
            ([positionId, position]) => ({
              positionId,
              attribution: position,
              explain: single.pnlAttributionExplainText(position),
              residualWithinTolerance:
                single.pnlAttributionResidualWithinTolerance(position),
              buckets: single.pnlAttributionPctOfTotalByBucket(position),
            }),
          ),
        };
      });
    },
    computeFactorSensitivities(request) {
      return context.result(() =>
        // The factor and bump JSON pass through; an omitted bump config stays unset.
        native.portfolio.computeFactorSensitivities(
          context.portfolio(request.specJson).handle,
          request.factorsJson,
          context.market(request.marketJson).handle,
          request.asOf,
          request.bumpConfigJson ?? undefined,
        ),
      );
    },
  };
}
