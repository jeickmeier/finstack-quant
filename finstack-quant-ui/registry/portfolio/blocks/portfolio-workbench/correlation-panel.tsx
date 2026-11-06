"use client";
import { CorrelationHeatmap } from "@/components/finstack/analytics/components/correlation-heatmap/correlation-heatmap";
import {
  useCorrelationMatrix,
  type CorrelationMatrixRequest,
} from "@/hooks/analytics/use-correlation-matrix/use-correlation-matrix";

interface CorrelationPanelProps {
  /** The host's return or price panel; correlated when the Risk tab opens. */
  panel: CorrelationMatrixRequest;
  density: "compact" | "comfortable";
}

/**
 * Pearson correlation of host-supplied return series: one native
 * correlationMatrix, mounted (and so called) when the Risk tab first opens.
 */
export function CorrelationPanel({ panel, density }: CorrelationPanelProps) {
  const correlation = useCorrelationMatrix(panel);
  return (
    <div className="flex flex-col gap-2">
      <p className="text-xs text-muted-foreground">
        Rust correlates the host&apos;s return series, not the valued book.
      </p>
      <CorrelationHeatmap
        report={correlation.data ?? null}
        loading={correlation.isFetching}
        error={correlation.error?.message}
        density={density}
      />
    </div>
  );
}
