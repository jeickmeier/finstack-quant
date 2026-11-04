"use client";
import { RecoveryWaterfall } from "@/components/finstack/models/components/recovery-waterfall/recovery-waterfall";
import { SeniorityRecoveryCard } from "@/components/finstack/models/components/recovery-waterfall/seniority-recovery-card";
import { WorkoutLgdCard } from "@/components/finstack/models/components/recovery-waterfall/workout-lgd-card";
import type {
  RecoveryWaterfallRequest,
  SeniorityRecoveryRequest,
  SeniorityRecoveryStats,
  WorkoutLgdRequest,
} from "@/workers/finstack-contract";
import type { models } from "finstack-quant-wasm/types";

import data from "./data.json";
// The results are the supplied native fixture: the item installs without a worker.
const { waterfall, workout, stats, calibrationIds } = data.recovery;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone recovery waterfall.</p>
      <RecoveryWaterfall
        defaultTerms={waterfall.shortfall.request as RecoveryWaterfallRequest}
        onSubmit={() => {}}
        state={{
          result: waterfall.shortfall.result
            .value as models.RecoveryWaterfallResult,
        }}
      />
      <WorkoutLgdCard
        defaultTerms={workout.secured.request as WorkoutLgdRequest}
        onSubmit={() => {}}
        state={{
          result: workout.secured.result.value as models.WorkoutLgdResult,
        }}
      />
      <SeniorityRecoveryCard
        defaultSeniority={
          (stats.seniorUnsecured.request as SeniorityRecoveryRequest).seniority
        }
        calibrationIds={calibrationIds}
        onSubmit={() => {}}
        state={{
          result: stats.seniorUnsecured.result.value as SeniorityRecoveryStats,
        }}
      />
    </>
  );
}
