"use client";
import {
  ExposureReport,
  type ExposureReportInputs,
} from "@/components/finstack/portfolio/components/exposure-report/exposure-report";
import {
  useFactorSensitivities,
  usePrimitiveExposures,
} from "@/hooks/portfolio/use-exposure-report/use-exposure-report";

interface ExposurePanelProps {
  specJson: string;
  marketJson: string;
  /** ISO date of the sensitivity matrix, the workbench asOf. */
  asOf: string;
  /** The host's metric ids and factor definitions; omitted is none of either. */
  inputs?: ExposureReportInputs | null;
  density: "compact" | "comfortable";
}

/**
 * The book's primitive exposures over the workbench market (one native
 * primitiveExposures) and, when the host supplies factor definitions, its
 * sensitivity matrix (one native computeFactorSensitivities); mounted, and so
 * called, when the Risk tab opens.
 */
export function ExposurePanel({
  specJson,
  marketJson,
  asOf,
  inputs,
  density,
}: ExposurePanelProps) {
  const exposures = usePrimitiveExposures({
    specJson,
    marketJson,
    // Omitted metric ids request quantity and value only, as Rust documents [].
    metrics: inputs?.metrics ?? [],
  });
  const factorsJson = inputs?.factorsJson;
  const sensitivities = useFactorSensitivities(
    factorsJson
      ? {
          specJson,
          marketJson,
          factorsJson,
          asOf,
          bumpConfigJson: inputs?.bumpConfigJson,
        }
      : null,
  );
  return (
    <ExposureReport
      report={exposures.data ?? null}
      loading={exposures.isFetching}
      error={exposures.error?.message}
      sensitivities={factorsJson ? (sensitivities.data ?? null) : undefined}
      sensitivitiesLoading={factorsJson ? sensitivities.isFetching : false}
      sensitivitiesError={
        factorsJson ? sensitivities.error?.message : undefined
      }
      density={density}
    />
  );
}
