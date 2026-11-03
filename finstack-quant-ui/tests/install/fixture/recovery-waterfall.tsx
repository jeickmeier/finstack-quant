"use client";
import {
  RecoveryWaterfall,
  type SeniorityStats,
  type SeniorityTerms,
  type WaterfallTerms,
  type WorkoutTerms,
} from "@/components/finstack/models/components/recovery-waterfall/recovery-waterfall";
import type { models } from "finstack-quant-wasm/types";

import data from "./data.json";
// The results are the supplied native fixture: the item installs without a worker.
const { waterfall, workout, stats, calibrationIds } = data.recovery;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone recovery waterfall.</p>
      <RecoveryWaterfall
        defaultWaterfall={waterfall.shortfall.request as WaterfallTerms}
        defaultWorkout={workout.secured.request as WorkoutTerms}
        defaultSeniority={
          (stats.seniorUnsecured.request as SeniorityTerms).seniority
        }
        onWaterfallSubmit={() => {}}
        onWorkoutSubmit={() => {}}
        onStatsSubmit={() => {}}
        waterfall={{
          result: waterfall.shortfall.result
            .value as models.RecoveryWaterfallResult,
        }}
        workout={{
          result: workout.secured.result.value as models.WorkoutLgdResult,
        }}
        stats={{
          result: stats.seniorUnsecured.result.value as SeniorityStats,
        }}
        calibrationIds={calibrationIds}
      />
    </>
  );
}
