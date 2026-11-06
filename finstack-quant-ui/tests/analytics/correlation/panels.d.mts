import type { analytics as analyticsWire } from "finstack-quant-wasm/types";
/** A host return panel as the worker's correlationMatrix takes it. */
export type CorrelationPanel = analyticsWire.PerformancePanel;
/** The valid panel: a rates, a credit, an equity and a gold sleeve. */
export declare const panel: CorrelationPanel;
/** Panels Rust rejects, each with the reason it does. */
export declare const failures: {
  readonly zeroVariance: CorrelationPanel;
  readonly interiorGap: CorrelationPanel;
  readonly noOverlap: CorrelationPanel;
};
