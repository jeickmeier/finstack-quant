"use client";
import {
  BrinsonReport,
  type BrinsonReportInputs,
} from "@/components/finstack/portfolio/components/brinson-report/brinson-report";
import { useBrinsonLinked } from "@/hooks/portfolio/use-brinson-linked/use-brinson-linked";

interface BrinsonPanelProps {
  /** The host's attribution periods and their names; linked when the tab opens. */
  inputs: BrinsonReportInputs;
  density: "compact" | "comfortable";
}

/**
 * Multi-period Brinson-Fachler attribution of host-supplied sector data: one
 * native brinsonLinked, mounted (and so called) when the Brinson tab first
 * opens.
 */
export function BrinsonPanel({ inputs, density }: BrinsonPanelProps) {
  const linked = useBrinsonLinked(inputs.periods);
  return (
    <div className="flex flex-col gap-4">
      <p className="text-xs text-muted-foreground">
        Rust attributes the host&apos;s sector weights and returns, not the
        valued book.
      </p>
      <BrinsonReport
        report={linked.data ?? null}
        periodLabels={inputs.periodLabels}
        loading={linked.isFetching}
        error={linked.error?.message}
        density={density}
      />
    </div>
  );
}
