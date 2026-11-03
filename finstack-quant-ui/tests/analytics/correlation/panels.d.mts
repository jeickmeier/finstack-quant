/** A host return panel as the worker's correlationMatrix takes it. */
export interface CorrelationPanel {
  readonly dates: readonly string[];
  readonly values: readonly (readonly number[])[];
  readonly kind: "returns";
  readonly tickerNames: readonly string[];
  readonly frequency: string;
}
/** The valid panel: a rates, a credit, an equity and a gold sleeve. */
export declare const panel: CorrelationPanel;
/** Panels Rust rejects, each with the reason it does. */
export declare const failures: {
  readonly zeroVariance: CorrelationPanel;
  readonly interiorGap: CorrelationPanel;
  readonly noOverlap: CorrelationPanel;
};
