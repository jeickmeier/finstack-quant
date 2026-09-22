import type { CalibrationReport } from "finstack-quant-wasm";
import type { CalibrationWire } from "@/lib/finstack/generated/types/calibration";
export const residualKey = (stepId: string, quoteId: string) =>
  JSON.stringify([stepId, quoteId]);
export interface ResidualPoint {
  readonly key: string;
  readonly stepId: string;
  readonly quoteId: string;
  readonly residual: number;
  readonly report: CalibrationReport;
  readonly quote?: NonNullable<CalibrationWire["market_data"]>[number];
}
/** Preserve signed returned residuals. Group only from supplied canonical quote discriminators. */
export function residualPanels(
  stepId: string,
  report: CalibrationReport,
  marketData: CalibrationWire["market_data"] = [],
) {
  const quotes = new Map<
    string,
    NonNullable<CalibrationWire["market_data"]>[number]
  >();
  for (const quote of marketData) {
    if (!("id" in quote) || typeof quote.id !== "string") continue;
    if (quotes.has(quote.id))
      throw new TypeError("Duplicate calibration market-data identity");
    quotes.set(quote.id, quote);
  }
  const groups = new Map<string, ResidualPoint[]>();
  for (const [quoteId, residual] of Object.entries(report.residuals)) {
    if (!Number.isFinite(residual))
      throw new TypeError(`Non-finite residual: ${stepId}/${quoteId}`);
    const quote = quotes.get(quoteId);
    // Unmatched identities receive independent panels; never combine potentially unlike solver scales.
    const group = quote
      ? `${quote.kind}${"type" in quote && typeof quote.type === "string" ? ` / ${quote.type}` : ""}`
      : `Unclassified quote: ${quoteId}`;
    const rows = groups.get(group) ?? [];
    rows.push({
      key: residualKey(stepId, quoteId),
      stepId,
      quoteId,
      residual,
      report,
      quote,
    });
    groups.set(group, rows);
  }
  const units =
    typeof report.metadata.residual_units === "string"
      ? report.metadata.residual_units
      : "Solver units (quote convention unavailable)";
  return [...groups].map(([group, points]) => ({
    key: JSON.stringify([stepId, group]),
    stepId,
    group,
    units,
    points,
  }));
}
