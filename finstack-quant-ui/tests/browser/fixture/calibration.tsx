import { useState } from "react";
import { createRoot } from "react-dom/client";
import { FinstackQueryProvider } from "./hooks/use-finstack/use-finstack";
import {
  useCalibrate,
  useCalibrationDryRun,
  useCalibrationValidator,
} from "./hooks/use-calibrate/use-calibrate";
import { usePriceInstrument } from "./hooks/use-price-instrument/use-price-instrument";
import { CalibrationForm } from "./components/finstack/components/calibration-form/calibration-form";
import { MarketContextBrowser } from "./components/finstack/components/market-context-browser/market-context-browser";
import { JsonViewer } from "./components/finstack/primitives/json-viewer/json-viewer";
import { serializeHost } from "./lib/finstack/codec.mjs";
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
  Object.assign(window, {
    calibrationProbe: {
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
          {failure.solver_diagnostics === undefined && (
            <p>Solver diagnostics unavailable</p>
          )}
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
