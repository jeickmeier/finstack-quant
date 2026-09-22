"use client";
import type { CalibrationReport as NativeReport } from "finstack-quant-wasm";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import type { LinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
/** Returned report fields in native units. Raw JSON preserves all metadata, diagnostics and severity information. */
export function CalibrationReport({
  stepId,
  report,
  error,
  link,
}: {
  stepId: string;
  report?: NativeReport | null;
  error?: Readonly<{
    message: string;
    solver_diagnostics?: unknown;
    [field: string]: unknown;
  }> | null;
  link?: LinkedSelection;
}) {
  const diagnostics = report?.diagnostics;
  return (
    <section
      aria-label={`${stepId} calibration report`}
      className="space-y-3 font-sans text-sm text-foreground"
    >
      <h2>{stepId} calibration report</h2>
      {error && (
        <>
          <p role="alert">{error.message}</p>
          <JsonViewer
            label={`${stepId} structured failure`}
            text={serializeHost(
              Object.fromEntries(
                Object.entries(error).filter(
                  ([, value]) => value !== undefined,
                ),
              ),
            )}
          />
          {error.solver_diagnostics === undefined ||
          error.solver_diagnostics === null ? (
            <p>Solver diagnostics unavailable</p>
          ) : (
            <JsonViewer
              label={`${stepId} solver diagnostics`}
              text={serializeHost(error.solver_diagnostics)}
            />
          )}
        </>
      )}
      {report ? (
        <>
          <FinstackTable
            caption={`${stepId} report summary`}
            data={[
              "success",
              "iterations",
              "objective_value",
              "max_residual",
              "rmse",
              "max_residual_ratio",
              "rmse_ratio",
              "validation_passed",
              "validation_error",
              "convergence_reason",
              "worst_quote_id",
              "worst_quote_residual",
              "success_tolerance",
            ].map((field) => ({
              field,
              value: report[field as keyof NativeReport],
            }))}
            getRowId={(r) => r.field}
            columns={[
              {
                id: "field",
                header: "Native field",
                accessorFn: (r) => r.field,
              },
              {
                id: "value",
                header: "Returned value",
                accessorFn: (r) =>
                  r.value == null ? "Unavailable" : String(r.value),
              },
            ]}
          />
          <p>
            Raw residual fields retain their solver units. Plan ratios remain
            separate native fields.
          </p>
          <FinstackTable
            caption={`${stepId} returned residuals`}
            data={Object.entries(report.residuals).map(
              ([quoteId, residual]) => ({ quoteId, residual }),
            )}
            getRowId={(r) => r.quoteId}
            link={link}
            getRowKey={(r) => JSON.stringify([stepId, r.quoteId])}
            columns={[
              {
                id: "quote",
                header: "Original quote identity",
                accessorFn: (r) => r.quoteId,
              },
              {
                id: "residual",
                header:
                  report.metadata.residual_units ??
                  "Residual (solver units; quote convention unavailable)",
                accessorFn: (r) => r.residual,
              },
            ]}
          />
          {diagnostics ? (
            <>
              <p>
                Target and fitted fields describe the native solver
                representation. A zero target with fitted equal to residual is
                not an observed-versus-repriced market quote.
              </p>
              <FinstackTable
                caption={`${stepId} native per-quote diagnostics`}
                data={diagnostics.per_quote.map((quality, index) => ({
                  quality,
                  index,
                }))}
                getRowId={(r) => String(r.index)}
                columns={[
                  {
                    id: "quote",
                    header: "Quote label",
                    accessorFn: (r) => r.quality.quote_label,
                  },
                  {
                    id: "target",
                    header: "Target (solver units)",
                    accessorFn: (r) => r.quality.target_value,
                  },
                  {
                    id: "fitted",
                    header: "Fitted (solver units)",
                    accessorFn: (r) => r.quality.fitted_value,
                  },
                  {
                    id: "residual",
                    header: "Residual (solver units)",
                    accessorFn: (r) => r.quality.residual,
                  },
                  {
                    id: "sensitivity",
                    header: "Sensitivity (native units)",
                    accessorFn: (r) => r.quality.sensitivity,
                  },
                ]}
              />
              <JsonViewer
                label={`${stepId} complete diagnostics`}
                text={serializeHost(diagnostics)}
              />
            </>
          ) : (
            <p>Diagnostics unavailable</p>
          )}
          <JsonViewer
            label={`${stepId} complete report`}
            text={serializeHost(report)}
          />
        </>
      ) : (
        !error && <p>Calibration report unavailable</p>
      )}
    </section>
  );
}
