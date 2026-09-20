"use client";
import type { CalibrationReport as NativeReport } from "finstack-quant-wasm";
import { useEffect, useMemo, useRef, useState } from "react";
import { CalibrationForm } from "@/components/finstack/components/calibration-form/calibration-form";
import { calibrationModule } from "@/components/finstack/components/calibration-form/calibration";
import { CalibrationReport } from "@/components/finstack/components/calibration-report/calibration-report";
import {
  CalibrationFitChart,
  type CalibrationFitChartProps,
} from "@/components/finstack/components/calibration-fit-chart/calibration-fit-chart";
import { JsonViewer } from "@/components/finstack/primitives/json-viewer/json-viewer";
import {
  useCalibrate,
  useCalibrationDryRun,
  useCalibrationValidator,
} from "@/hooks/use-calibrate/use-calibrate";
import { useMarketValidator } from "@/hooks/use-market-validator/use-market-validator";
import { useLinkedSelection } from "@/hooks/use-linked-selection/use-linked-selection";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import type { CalibrationWire } from "@/lib/finstack/generated/types/calibration";
/** Workbench orchestration only. The generated form, native hooks and report/figure components own their contracts. */
export function CalibrationPanel({
  defaultJson,
  onMarket,
  chartOptions,
}: {
  defaultJson?: string;
  onMarket(json: string): void;
  chartOptions?: Omit<
    CalibrationFitChartProps,
    "stepId" | "report" | "marketData" | "link"
  >;
}) {
  const [draft, setDraft] = useState(defaultJson ?? ""),
    [document, setDocument] = useState(defaultJson),
    [revision, setRevision] = useState(0);
  const [input, setInput] = useState<string | null>(null),
    [validated, setValidated] = useState<string | null>(null),
    [request, setRequest] = useState<string | null>(null);
  const [selection, setSelection] = useState("plan"),
    [applying, setApplying] = useState(false),
    [applyError, setApplyError] = useState<string | null>(null);
  const validate = useCalibrationValidator(),
    validateMarket = useMarketValidator(),
    dry = useCalibrationDryRun(input),
    solve = useCalibrate(request),
    link = useLinkedSelection();
  const abort = useRef<AbortController | null>(null);
  useEffect(
    () => () => {
      abort.current?.abort();
    },
    [],
  );
  const active = useRef({ validated, request });
  active.current = { validated, request };
  const marketData = useMemo(
    () =>
      request
        ? (calibrationModule.codec.parse(request) as CalibrationWire)
            .market_data
        : undefined,
    [request],
  );
  const result = solve.data?.result,
    stepId = selection.startsWith("step/") ? selection.slice(5) : null;
  const report = (stepId ? result?.step_reports[stepId] : result?.report) as
    NativeReport | undefined;
  const failure = solve.error as typeof solve.error & {
    payload?: { message: string; [key: string]: unknown };
  };
  async function apply() {
    if (!result || validated !== request || !request) return;
    abort.current?.abort();
    const controller = new AbortController();
    abort.current = controller;
    const submitted = request;
    setApplying(true);
    setApplyError(null);
    try {
      const json = await validateMarket(
        serializeHost(result.final_market),
        controller.signal,
      );
      if (
        !controller.signal.aborted &&
        active.current.validated === submitted &&
        active.current.request === submitted
      )
        onMarket(json);
    } catch (error) {
      if (!controller.signal.aborted)
        setApplyError(error instanceof Error ? error.message : String(error));
    } finally {
      if (abort.current === controller) setApplying(false);
    }
  }
  return (
    <section aria-label="Calibration workflow" className="space-y-4">
      <details>
        <summary>Import calibration envelope</summary>
        <label className="block">
          Calibration envelope JSON
          <textarea
            aria-label="Calibration envelope JSON"
            className="block w-full border border-border bg-background p-2 font-mono"
            rows={6}
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
          />
        </label>
        <button
          type="button"
          onClick={() => {
            abort.current?.abort();
            setDocument(draft);
            setRevision((v) => v + 1);
            setInput(null);
            setValidated(null);
            setRequest(null);
            setSelection("plan");
            setApplyError(null);
          }}
        >
          Load envelope
        </button>
      </details>
      {!document ? (
        <p>Load a complete canonical calibration envelope to begin.</p>
      ) : (
        <CalibrationForm
          key={revision}
          defaultJson={document}
          validate={validate}
          onInputJson={setInput}
          onValidated={setValidated}
          onSubmit={(json) => {
            setRequest(json);
            setSelection("plan");
          }}
        />
      )}
      {dry.isFetching && (
        <p role="status">Checking calibration dependencies…</p>
      )}
      {dry.data && <JsonViewer label="Static diagnostics" text={dry.data} />}
      {dry.error && <p role="alert">{dry.error.message}</p>}
      {solve.isFetching && <p role="status">Calibrating…</p>}
      {failure && (
        <CalibrationReport
          stepId="Failed step"
          error={failure.payload ?? { message: failure.message }}
        />
      )}
      {result && (
        <>
          <p>Reports reflect the last submitted envelope.</p>
          <label>
            Calibration report
            <select
              aria-label="Calibration report"
              value={selection}
              onChange={(event) => {
                setSelection(event.target.value);
                link.select(null);
              }}
              className="ml-2 border border-border bg-background"
            >
              <option value="plan">Plan</option>
              {Object.keys(result.step_reports).map((id) => (
                <option key={id} value={`step/${id}`}>
                  {id}
                </option>
              ))}
            </select>
          </label>
          <CalibrationReport
            stepId={stepId ?? "Plan"}
            report={report}
            link={link}
          />
          {stepId && report && (
            <CalibrationFitChart
              {...chartOptions}
              stepId={stepId}
              report={report}
              marketData={marketData}
              link={link}
            />
          )}
          <JsonViewer
            label="Native calibration result"
            text={serializeHost(solve.data)}
          />
          <button
            type="button"
            disabled={applying || validated !== request || !validated}
            onClick={() => void apply()}
          >
            Use calibrated market
          </button>
        </>
      )}
      {applying && <p role="status">Validating calibrated market…</p>}
      {applyError && <p role="alert">{applyError}</p>}
    </section>
  );
}
