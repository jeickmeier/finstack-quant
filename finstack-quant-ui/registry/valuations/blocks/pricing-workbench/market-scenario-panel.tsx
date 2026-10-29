"use client";
import { useEffect, useState } from "react";
import type { MetricMetadata, ValuationResult } from "finstack-quant-wasm";
import { Button } from "@/components/ui/button";
import {
  MarketScenarioBuilder,
  type MarketScenarioDraft,
} from "@/components/finstack/scenarios/components/market-scenario-builder/market-scenario-builder";
import { MarketScenarioReport } from "@/components/finstack/scenarios/components/market-scenario-report/market-scenario-report";
import { HorizonReturnReport } from "@/components/finstack/scenarios/components/horizon-return-report/horizon-return-report";
import {
  EnumField,
  type EnumOption,
} from "@/components/finstack/shared/primitives/enum-field/enum-field";
import {
  useMarketScenario,
  type MarketScenarioRequest,
} from "@/hooks/scenarios/use-market-scenario/use-market-scenario";
import { useApplyMarketScenario } from "@/hooks/scenarios/use-apply-market-scenario/use-apply-market-scenario";
import {
  useHorizonReturn,
  type HorizonReturnRequest,
} from "@/hooks/scenarios/use-horizon-return/use-horizon-return";
import {
  useCalendars,
  useMetricMetadata,
  useMoneyFormat,
  usePriceInstrument,
  type PriceRequest,
} from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import { returnedRounding } from "@/components/finstack/valuations/primitives/stamp-badge/stamp-badge";
import { resolveModel } from "@/workers/finstack-contract";

/** Horizon attribution methods offered here; blank leaves Rust's default. A waterfall needs a factor order, which this tab does not edit. */
export type HorizonMethodChoice = "" | "parallel" | "metrics_based" | "taylor";
const horizonMethods: EnumOption[] = [
  { value: "", label: "Default", description: "Omitted; Rust's default" },
  { value: "parallel", label: "parallel" },
  { value: "metrics_based", label: "metrics_based" },
  { value: "taylor", label: "taylor" },
];
/** The AttributionMethod wire value of a choice; Rust rejects a bare "taylor", so its config object is sent empty for Rust to fill. */
function horizonMethod(
  choice: HorizonMethodChoice,
): HorizonReturnRequest["method"] {
  if (choice === "") return null;
  return choice === "taylor" ? { taylor: {} } : choice;
}

/** An empty market scenario: no shifts, no roll, every option left to Rust. */
export const emptyMarketScenario: MarketScenarioDraft = {
  id: "market-scenario",
  shifts: [],
  roll: null,
  hazardBumpMode: "",
};

/** The edited draft as a worker request; blank options are omitted and a blank size stays non-numeric for Rust to reject. */
function scenarioRequest(draft: MarketScenarioDraft): MarketScenarioRequest {
  return {
    id: draft.id,
    shifts: draft.shifts.map((shift) => ({
      curveKind: shift.curveKind,
      curveIds: [shift.curveId],
      bp: shift.bp.trim() === "" ? Number.NaN : Number(shift.bp),
    })),
    roll: draft.roll && {
      period: draft.roll.period,
      rollMode: draft.roll.rollMode || null,
    },
    hazardBumpMode: draft.hazardBumpMode || null,
  };
}

/**
 * Market scenario state for one workbench: the edited draft, its Rust-built
 * spec, the applied spec and, for the completed base request, the native
 * application and the shocked valuation priced with the same model and
 * metrics at the returned market and valuation date. No P&L is computed.
 */
export function useMarketScenarioSession(
  initial: MarketScenarioDraft | undefined,
  base: PriceRequest | null,
) {
  const [draft, setDraft] = useState(() => initial ?? emptyMarketScenario);
  const [applied, setApplied] = useState<string | null>(null);
  // Loading another example drops the completed base; its scenario no longer applies.
  useEffect(() => {
    if (base === null) setApplied(null);
  }, [base]);
  const spec = useMarketScenario(scenarioRequest(draft));
  const application = useApplyMarketScenario(
    applied && base
      ? {
          scenarioJson: applied,
          marketJson: base.marketJson,
          asOf: base.asOf,
          instrumentJson: base.instrumentJson,
        }
      : null,
  );
  const shockedRequest: PriceRequest | null =
    base && application.data
      ? {
          ...base,
          instrumentJson: application.data.instrumentJson,
          marketJson: application.data.marketJson,
          asOf: application.data.asOf,
        }
      : null;
  const [horizonChoice, setHorizonChoice] = useState<HorizonMethodChoice>("");
  const [calendarId, setCalendarId] = useState("");
  const horizon = useHorizonReturn(
    applied && base
      ? {
          instrumentJson: base.instrumentJson,
          marketJson: base.marketJson,
          asOf: base.asOf,
          scenarioJson: applied,
          method: horizonMethod(horizonChoice),
          calendarId: calendarId || null,
        }
      : null,
  );
  const shocked = usePriceInstrument(
    shockedRequest ?? base ?? emptyRequest,
    shockedRequest !== null,
  );
  const result: ValuationResult | undefined =
    shockedRequest !== null ? shocked.data : undefined;
  const metadata = useMetricMetadata(
    Object.keys(result?.measures ?? {}),
    result !== undefined,
  );
  const moneyFormat = useMoneyFormat(
    result
      ? { value: result.value, rounding: returnedRounding(result.meta) }
      : null,
  );
  return {
    draft,
    setDraft,
    spec,
    applied,
    apply: () => setApplied(spec.data ?? null),
    clear: () => setApplied(null),
    application,
    shocked,
    horizon,
    horizonChoice,
    setHorizonChoice,
    calendarId,
    setCalendarId,
    /** Comparison props for MeasuresGrid and DimensionalMeasures; empty until a scenario is applied. */
    comparison:
      applied === null || base === null
        ? noComparison
        : ({
            compareTo: result ?? null,
            comparisonMetadata: metadata.data,
            comparisonFormattedValue: moneyFormat.data,
            comparisonModel: base ? resolveModel(base.model) : undefined,
          } satisfies MarketScenarioComparison),
    error:
      application.error?.message ??
      shocked.error?.message ??
      metadata.error?.message ??
      moneyFormat.error?.message,
  };
}

/** The comparison props MeasuresGrid and DimensionalMeasures already accept. */
export interface MarketScenarioComparison {
  compareTo: ValuationResult | null | undefined;
  comparisonMetadata: readonly MetricMetadata[] | undefined;
  comparisonFormattedValue: string | undefined;
  comparisonModel: string | undefined;
}
/** No scenario applied: the grids show no comparison column. */
export const noComparison: MarketScenarioComparison = {
  compareTo: undefined,
  comparisonMetadata: undefined,
  comparisonFormattedValue: undefined,
  comparisonModel: undefined,
};

/** Never priced: usePriceInstrument needs a request even while disabled. */
const emptyRequest: PriceRequest = {
  instrumentJson: "",
  marketJson: "",
  asOf: "",
};

export type MarketScenarioSession = ReturnType<typeof useMarketScenarioSession>;

/** Inputs tab: the scenario editor and the apply/clear actions. */
export function MarketScenarioInputs({
  session,
  curveIds,
  priced,
  density,
}: {
  session: MarketScenarioSession;
  curveIds: readonly string[];
  /** Whether a completed base valuation exists to apply the scenario to. */
  priced: boolean;
  density?: "compact" | "comfortable";
}) {
  const { spec } = session;
  return (
    <div className="flex flex-col gap-3">
      <MarketScenarioBuilder
        value={session.draft}
        onValueChange={session.setDraft}
        curveIds={curveIds}
        specJson={spec.data}
        loading={spec.isFetching}
        error={spec.error?.message}
        density={density}
      />
      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          size="sm"
          disabled={!priced || !spec.data || spec.data === session.applied}
          onClick={session.apply}
        >
          Apply scenario
        </Button>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={session.applied === null}
          onClick={session.clear}
        >
          Clear scenario
        </Button>
        <span className="text-xs text-muted-foreground">
          {!priced
            ? "Price the instrument to apply a scenario."
            : session.applied === null
              ? "No scenario applied."
              : session.applied === spec.data
                ? "Applied to the last priced request."
                : "Edited since applied."}
        </span>
      </div>
    </div>
  );
}

/** Results tab: the native application report beside the shocked valuation context. */
export function MarketScenarioResults({
  session,
  density,
}: {
  session: MarketScenarioSession;
  density?: "compact" | "comfortable";
}) {
  const { application, shocked } = session;
  if (session.applied === null)
    return (
      <p role="status" className="text-muted-foreground">
        Apply a market scenario to price the shocked valuation.
      </p>
    );
  return (
    <div className="flex flex-col gap-3">
      <p className="text-xs text-muted-foreground">
        The comparison valuation is the instrument repriced with the same model
        and metrics on the shocked market
        {application.data ? (
          <>
            {" "}
            at <span className="font-mono">{application.data.asOf}</span>
          </>
        ) : null}
        .
      </p>
      {session.error && !application.error && (
        <p role="alert" className="text-sm text-error">
          {session.error}
        </p>
      )}
      {shocked.isFetching && (
        <p role="status" className="text-sm text-muted-foreground">
          Pricing shocked valuation…
        </p>
      )}
      <MarketScenarioReport
        report={application.data?.report}
        loading={application.isFetching}
        error={application.error?.message}
        density={density}
      />
      <HorizonReturnSection session={session} density={density} />
    </div>
  );
}

/** Rust's horizon total return of the last priced request under the applied scenario. */
function HorizonReturnSection({
  session,
  density,
}: {
  session: MarketScenarioSession;
  density?: "compact" | "comfortable";
}) {
  const calendars = useCalendars();
  const { horizon } = session;
  return (
    <section aria-label="Horizon return" className="flex flex-col gap-3">
      <h3 className="text-sm font-semibold">Horizon return</h3>
      <p className="text-xs text-muted-foreground">
        The applied scenario's time roll sets the horizon; without one the
        return is instantaneous and has no annualized value.
      </p>
      <div className="flex flex-wrap items-end gap-4">
        <EnumField
          label="Attribution method"
          value={session.horizonChoice}
          onValueChange={(choice) =>
            session.setHorizonChoice(choice as HorizonMethodChoice)
          }
          options={horizonMethods}
          layout="radio"
        />
        <EnumField
          label="Roll calendar"
          help="Holiday calendar for business-day roll targets."
          value={session.calendarId}
          onValueChange={session.setCalendarId}
          options={[
            {
              value: "",
              label: "Default",
              description: "Omitted; Rust's weekends-only calendar",
            },
            ...(calendars.data ?? []).map((id) => ({ value: id, label: id })),
          ]}
          layout="select"
          disabled={!calendars.data}
        />
      </div>
      <HorizonReturnReport
        report={horizon.data?.report}
        explain={horizon.data?.explain}
        loading={horizon.isFetching}
        error={horizon.error?.message ?? calendars.error?.message}
        density={density}
      />
    </section>
  );
}
