"use client";
import { ScenarioPnlTable } from "@/components/finstack/portfolio/components/scenario-pnl-table/scenario-pnl-table";
import { useScenarioPnlBatch } from "@/hooks/portfolio/use-scenario-pnl-batch/use-scenario-pnl-batch";

interface ScenarioPanelProps {
  /** Canonical PortfolioSpec JSON the workbench values. */
  specJson: string;
  /** MarketContextState JSON of the unshocked market. */
  marketJson: string;
  /** Host-supplied ScenarioSpec JSON texts in batch order; never parsed here. */
  scenarioJsons: readonly string[];
  density: "compact" | "comfortable";
}
/**
 * The book under the host's scenarios: one native portfolio.scenarioPnlBatch,
 * mounted (and so called) when the Scenarios tab first opens.
 */
export function ScenarioPanel({
  specJson,
  marketJson,
  scenarioJsons,
  density,
}: ScenarioPanelProps) {
  const batch = useScenarioPnlBatch({ specJson, marketJson, scenarioJsons });
  return (
    <ScenarioPnlTable
      report={batch.data}
      loading={batch.isFetching}
      error={batch.error?.message}
      density={density}
    />
  );
}
