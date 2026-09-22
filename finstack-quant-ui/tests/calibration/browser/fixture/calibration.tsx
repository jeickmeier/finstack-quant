import { useState, useRef } from "react";
import { createRoot } from "react-dom/client";
import { FinstackQueryProvider } from "./hooks/shared/use-finstack/use-finstack";
import {
  useCalibrate,
  useCalibrationDryRun,
  useCalibrationValidator,
} from "./hooks/calibration/use-calibrate/use-calibrate";
import { usePriceInstrument } from "./hooks/valuations/use-price-instrument/use-price-instrument";
import { CalibrationForm } from "./components/finstack/calibration/components/calibration-form/calibration-form";
import { MarketContextBrowser } from "./components/finstack/core/components/market-context-browser/market-context-browser";
import { JsonViewer } from "./components/finstack/shared/primitives/json-viewer/json-viewer";
import { serializeHost } from "../../../../src/codec.mjs";
import { CalibrationReport } from "./components/finstack/calibration/components/calibration-report/calibration-report";
import {
  CalibrationFitChart,
  residualPanels,
} from "./components/finstack/calibration/components/calibration-fit-chart/calibration-fit-chart";
import { useLinkedSelection } from "./hooks/shared/use-linked-selection/use-linked-selection";
import type { FigureHandle } from "./components/finstack/shared/chart/finstack-chart/finstack-chart";
import fixture from "./fixture.json";
function App() {
  const [request, setRequest] = useState<string | null>(null),
    [validated, setValidated] = useState<string | null>(null),
    [diagnostics, setDiagnostics] = useState<string | null>(null),
    [priceRequest, setPriceRequest] = useState<
      typeof fixture.bond.request | null
    >(null);
  const validate = useCalibrationValidator(),
    result = useCalibrate(request),
    dry = useCalibrationDryRun(diagnostics),
    price = usePriceInstrument(
      priceRequest ?? fixture.bond.request,
      priceRequest !== null,
    );
  const error = result.error as typeof result.error & {
    payload?: Record<string, unknown>;
  };
  const failure = error?.payload
    ? Object.fromEntries(
        Object.entries(error.payload).filter(
          ([, value]) => value !== undefined,
        ),
      )
    : null;
  const link = useLinkedSelection(),
    figure = useRef<FigureHandle>(null),
    activated = useRef<unknown>(null);
  const report = result.data?.result.step_reports["USD-OIS"];
  const panels = report
    ? residualPanels("USD-OIS", report, fixture.input.market_data)
    : [];
  Object.assign(window, {
    calibrationProbe: {
      selectedKey: link.selectedKey,
      get activated() {
        return activated.current;
      },
      export: async () =>
        (
          await figure.current!.exportSvg({
            width: 1000,
            height: 650,
            theme: "light",
          })
        ).text(),
      request,
      result: result.data,
      error: failure,
      diagnostics: dry.data,
      price: price.data,
    },
  });
  return (
    <main className="p-4 space-y-4">
      <h1>Canonical calibration workflow</h1>
      <button disabled={!validated} onClick={() => setDiagnostics(validated)}>
        Run diagnostics
      </button>
      <button onClick={() => setDiagnostics(JSON.stringify(fixture.missing))}>
        Invalid plan diagnostics
      </button>
      <button onClick={() => setRequest(JSON.stringify(fixture.target))}>
        Target failure
      </button>
      <button onClick={() => setRequest(JSON.stringify(fixture.solver))}>
        Solver failure
      </button>
      <button
        disabled={!result.data}
        onClick={() =>
          setPriceRequest({
            ...fixture.bond.request,
            marketJson: serializeHost(result.data!.result.final_market),
          })
        }
      >
        Price returned market
      </button>
      {result.isFetching && <p role="status">Calibrating…</p>}
      {error && <p role="alert">{error.message}</p>}
      {failure && (
        <>
          <JsonViewer
            label="Structured calibration error"
            text={serializeHost(failure)}
          />
          <CalibrationReport stepId="Failed step" error={failure} />
        </>
      )}
      {dry.data && <JsonViewer label="Static diagnostics" text={dry.data} />}
      {dry.error && <p role="alert">{dry.error.message}</p>}
      {result.data && (
        <>
          <JsonViewer
            label="Native calibration result"
            text={serializeHost(result.data)}
          />
          <CalibrationReport stepId="Plan" report={result.data.result.report} />
          {report && (
            <>
              <CalibrationReport stepId="USD-OIS" report={report} link={link} />
              <CalibrationFitChart
                stepId="USD-OIS"
                report={report}
                marketData={fixture.input.market_data}
                link={link}
                width={900}
                height={600}
                title="Native residual figure"
                caption="Signed returned solver values"
                sources={[{ label: "Canonical calibration report" }]}
                figureAnnotations={[
                  { text: "Residuals, not repriced quotes", x: 0.55, y: 0.12 },
                ]}
                figureRefs={{ [panels[0].key]: figure }}
                onSelect={(point) => {
                  if (point) {
                    if (point.datum.report !== report)
                      throw new Error("Stale report callback");
                    activated.current = {
                      key: point.datum.key,
                      residual: point.datum.residual,
                    };
                  }
                }}
                renderTooltipBody={({ points }) => (
                  <span>Transient residual {points[0]?.datum.quoteId}</span>
                )}
              />
            </>
          )}
          <MarketContextBrowser state={result.data.result.final_market} />
        </>
      )}
      {price.data && (
        <JsonViewer
          label="Returned market price"
          text={serializeHost(price.data)}
        />
      )}
      <CalibrationForm
        defaultJson={JSON.stringify(fixture.input)}
        validate={validate}
        onValidated={setValidated}
        onSubmit={(json) => setRequest(json)}
      />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  <FinstackQueryProvider>
    <App />
  </FinstackQueryProvider>,
);
