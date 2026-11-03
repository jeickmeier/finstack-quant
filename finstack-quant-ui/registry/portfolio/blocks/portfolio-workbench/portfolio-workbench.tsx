"use client";
import { useMemo, useState } from "react";
import type { MoneyValue } from "finstack-quant-wasm";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { SurfaceContext } from "@/components/finstack/shared/primitives/surface/surface";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { PortfolioSummary } from "@/components/finstack/portfolio/components/portfolio-summary/portfolio-summary";
import { PositionsTable } from "@/components/finstack/portfolio/components/positions-table/positions-table";
import { PortfolioMetrics } from "@/components/finstack/portfolio/components/portfolio-metrics/portfolio-metrics";
import { useValuePortfolio } from "@/hooks/portfolio/use-value-portfolio/use-value-portfolio";
import { usePortfolioMetricsSeries } from "@/hooks/portfolio/use-portfolio-metrics-series/use-portfolio-metrics-series";
import { usePortfolioSpecValidator } from "@/hooks/portfolio/use-portfolio-spec-validator/use-portfolio-spec-validator";
import { useMetricMetadata } from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
import { ScenarioPanel } from "./scenario-panel";
import { MarginPanel } from "./margin-panel";
import {
  AttributionPanel,
  type PortfolioAttributionInputs,
} from "./attribution-panel";
export type { PortfolioAttributionInputs } from "./attribution-panel";
import { ReturnsPanel } from "./returns-panel";
import type { PortfolioReturnsInputs } from "@/components/finstack/portfolio/components/portfolio-returns/portfolio-returns";
export type { PortfolioReturnsInputs } from "@/components/finstack/portfolio/components/portfolio-returns/portfolio-returns";
import { BrinsonPanel } from "./brinson-panel";
import type { BrinsonReportInputs } from "@/components/finstack/portfolio/components/brinson-report/brinson-report";
export type { BrinsonReportInputs } from "@/components/finstack/portfolio/components/brinson-report/brinson-report";
import { ExposurePanel } from "./exposure-panel";
import type { ExposureReportInputs } from "@/components/finstack/portfolio/components/exposure-report/exposure-report";
export type { ExposureReportInputs } from "@/components/finstack/portfolio/components/exposure-report/exposure-report";
import { CorrelationPanel } from "./correlation-panel";
import type { CorrelationMatrixRequest } from "@/hooks/analytics/use-correlation-matrix/use-correlation-matrix";
export type { CorrelationMatrixRequest } from "@/hooks/analytics/use-correlation-matrix/use-correlation-matrix";

type Tab =
  | "positions"
  | "risk"
  | "scenarios"
  | "attribution"
  | "returns"
  | "brinson"
  | "margin"
  | "book";

export interface PortfolioWorkbenchProps {
  /** Complete PortfolioSpec JSON supplied by the host (positions with their instrument specs, entities, base currency). Remount for a different book; the Book tab replaces it after native validation. */
  defaultSpecJson: string;
  /** MarketContextState JSON every tab values the book over. */
  marketJson: string;
  /** ISO valuation date of the margin run and the factor sensitivity matrix; not read from the spec. */
  asOf: string;
  /** Risk-metric menu offered to every position; omitted is Rust's standard set (PV plus dv01), empty is PV only. */
  metrics?: readonly string[] | null;
  /** Canonical base metric id charted by bucket on the Risk tab, for example "bucketed_dv01"; omitted charts no series. */
  seriesBase?: string;
  /** The host's exposure choices for the Risk tab: additive metric ids per primitive (omitted is quantity and value only) and FactorDefinition JSON with an optional BumpSizeConfig for the sensitivity matrix (omitted shows none). The book's primitive exposures show on the Risk tab whether or not this is supplied. */
  exposures?: ExposureReportInputs | null;
  /** The host's return (or price) panel for the Risk tab's correlation heatmap: ISO dates, ticker-major values, their kind, ticker names and an optional frequency; the heatmap appears only when supplied. */
  correlation?: CorrelationMatrixRequest | null;
  /** ScenarioSpec JSON texts in batch order, never parsed here; the Scenarios tab appears only when some are supplied. */
  scenarioJsons?: readonly string[];
  /** The closing (T1) market and date and the reconciliation tolerance of the Attribution tab, which attributes the book from marketJson on asOf to them; the tab appears only when supplied. */
  attribution?: PortfolioAttributionInputs | null;
  /** The host's performance data for the Returns tab: sub-period returns and their horizon, one period's values and flows, and dated investor flows; the tab appears only when supplied. */
  returns?: PortfolioReturnsInputs | null;
  /** The host's sector weights and returns per period (and optionally each period's name) for the Brinson tab; the tab appears only when supplied. */
  brinson?: BrinsonReportInputs | null;
  /** IM collateral already held by CSA id, in that CSA's currency; omitted is none held. */
  currentImCollateral?: Readonly<Record<string, MoneyValue>> | null;
  /** Row and text density of the workbench surface; compact by default. Sets SurfaceContext and data-density on the root, while data-theme is inherited from the host. */
  density?: "compact" | "comfortable";
}

/** Value a host-supplied book over a host-supplied market and review its positions, risk, scenarios and margin through native WASM. Wrap in FinstackQueryProvider. */
export function PortfolioWorkbench({
  defaultSpecJson,
  marketJson,
  asOf,
  metrics,
  seriesBase,
  exposures,
  correlation,
  scenarioJsons,
  attribution,
  returns,
  brinson,
  currentImCollateral,
  density = "compact",
}: PortfolioWorkbenchProps) {
  const { status } = useFinstack();
  const validate = usePortfolioSpecValidator();
  const [specJson, setSpecJson] = useState(defaultSpecJson);
  const [draft, setDraft] = useState(defaultSpecJson);
  const [bookError, setBookError] = useState<string | null>(null);
  const [applying, setApplying] = useState(false);
  const [tab, setTab] = useState<Tab>("positions");
  const report = useValuePortfolio({ specJson, marketJson, metrics });
  const valuation = report.data ?? null;
  const series = usePortfolioMetricsSeries(
    valuation && seriesBase
      ? { metrics: valuation.metrics, base: seriesBase }
      : null,
  );
  // The returned measure ids of every position, for Rust's labels and units.
  const measureIds = useMemo(
    () => [
      ...new Set(
        Object.values(valuation?.valuation.position_values ?? {}).flatMap(
          (position) => Object.keys(position.valuation_result?.measures ?? {}),
        ),
      ),
    ],
    [valuation],
  );
  const metadata = useMetricMetadata(measureIds, valuation !== null);
  const hasScenarios = Boolean(scenarioJsons?.length);
  const tabs: { id: Tab; label: string }[] = [
    { id: "positions", label: "Positions" },
    { id: "risk", label: "Risk" },
    ...(hasScenarios ? [{ id: "scenarios" as const, label: "Scenarios" }] : []),
    ...(attribution
      ? [{ id: "attribution" as const, label: "Attribution" }]
      : []),
    ...(returns ? [{ id: "returns" as const, label: "Returns" }] : []),
    ...(brinson ? [{ id: "brinson" as const, label: "Brinson" }] : []),
    { id: "margin", label: "Margin" },
    { id: "book", label: "Book" },
  ];
  const apply = async () => {
    setApplying(true);
    setBookError(null);
    try {
      const canonical = await validate(draft);
      setSpecJson(canonical);
      setDraft(canonical);
    } catch (error) {
      setBookError(error instanceof Error ? error.message : String(error));
    } finally {
      setApplying(false);
    }
  };
  const valuationError = report.error?.message;
  return (
    <SurfaceContext.Provider value={{ density }}>
      <section
        aria-label="Portfolio workbench"
        data-density={density}
        className="space-y-5 rounded-xl border border-border bg-background p-4 font-sans text-foreground md:p-6"
      >
        <header className="border-b border-border pb-4">
          <p className="text-xs font-semibold uppercase tracking-widest text-muted-foreground">
            Portfolio · positions, risk, attribution, returns, Brinson and
            margin
          </p>
          <p className="mt-1 text-sm text-muted-foreground">
            Book valued natively over the supplied market; margin as of {asOf}.
          </p>
        </header>
        <Tabs value={tab} onValueChange={(value) => setTab(value as Tab)}>
          <TabsList
            variant="line"
            aria-label="Portfolio workspace"
            className="finstack-workbench__tabs"
          >
            {tabs.map((item) => (
              <TabsTrigger key={item.id} value={item.id}>
                {item.label}
              </TabsTrigger>
            ))}
          </TabsList>
          <TabsContent value="positions">
            <div className="space-y-6">
              <PortfolioSummary
                report={valuation}
                loading={report.isFetching}
                error={valuationError}
                density={density}
              />
              {valuation && (
                <PositionsTable
                  report={valuation}
                  metadata={metadata.data}
                  density={density}
                />
              )}
            </div>
          </TabsContent>
          <TabsContent value="risk">
            <div className="space-y-6">
              <PortfolioMetrics
                report={valuation}
                series={
                  seriesBase && series.data
                    ? { base: seriesBase, entries: series.data }
                    : null
                }
                metadata={metadata.data}
                loading={report.isFetching}
                error={valuationError ?? series.error?.message}
                density={density}
              />
              <ExposurePanel
                specJson={specJson}
                marketJson={marketJson}
                asOf={asOf}
                inputs={exposures}
                density={density}
              />
              {correlation && (
                <CorrelationPanel panel={correlation} density={density} />
              )}
            </div>
          </TabsContent>
          {hasScenarios && (
            <TabsContent value="scenarios">
              <ScenarioPanel
                specJson={specJson}
                marketJson={marketJson}
                scenarioJsons={scenarioJsons!}
                density={density}
              />
            </TabsContent>
          )}
          {attribution && (
            <TabsContent value="attribution">
              <AttributionPanel
                specJson={specJson}
                marketJson={marketJson}
                asOf={asOf}
                {...attribution}
                density={density}
              />
            </TabsContent>
          )}
          {returns && (
            <TabsContent value="returns">
              <ReturnsPanel inputs={returns} density={density} />
            </TabsContent>
          )}
          {brinson && (
            <TabsContent value="brinson">
              <BrinsonPanel inputs={brinson} density={density} />
            </TabsContent>
          )}
          <TabsContent value="margin">
            <MarginPanel
              specJson={specJson}
              marketJson={marketJson}
              asOf={asOf}
              currentImCollateral={currentImCollateral}
              density={density}
            />
          </TabsContent>
          <TabsContent value="book">
            <div className="space-y-3 rounded-lg border border-border bg-card p-4">
              <h3 className="font-semibold">Replace the book</h3>
              <p className="text-sm text-muted-foreground">
                Paste a complete PortfolioSpec JSON. It is validated natively
                (schema, duplicate ids, unknown entities) before every tab
                values it.
              </p>
              <FieldFrame
                label="PortfolioSpec JSON"
                error={bookError ?? undefined}
              >
                {(control) => (
                  <Textarea
                    {...control}
                    className="min-h-48 font-mono"
                    value={draft}
                    onChange={(event) => setDraft(event.target.value)}
                    spellCheck={false}
                  />
                )}
              </FieldFrame>
              <Button
                type="button"
                disabled={status !== "ready" || applying}
                onClick={() => void apply()}
              >
                Validate & value
              </Button>
            </div>
          </TabsContent>
        </Tabs>
      </section>
    </SurfaceContext.Provider>
  );
}
