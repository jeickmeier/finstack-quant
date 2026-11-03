"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import {
  RecoveryWaterfall,
  type SeniorityTerms,
  type WaterfallTerms,
  type WorkoutTerms,
} from "@/components/finstack/models/components/recovery-waterfall/recovery-waterfall";
import {
  useAllocateRecovery,
  useSeniorityCalibrationIds,
  useSeniorityRecoveryStats,
  useWorkoutLgd,
} from "@/hooks/models/use-recovery-waterfall/use-recovery-waterfall";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
const defaultWaterfall = data.recovery.waterfall.shortfall
  .request as WaterfallTerms;
const defaultWorkout = data.recovery.workout.secured.request as WorkoutTerms;
const defaultStats = data.recovery.stats.seniorUnsecured
  .request as SeniorityTerms;
function Card({ density }: ExampleProps) {
  // Starts from the fixture's terms; each form replaces its request.
  const [estate, setEstate] = useState(defaultWaterfall);
  const [terms, setTerms] = useState(defaultWorkout);
  const [seniority, setSeniority] = useState(defaultStats);
  const waterfall = useAllocateRecovery(estate);
  const workout = useWorkoutLgd(terms);
  const stats = useSeniorityRecoveryStats(seniority);
  const ids = useSeniorityCalibrationIds();
  return (
    <RecoveryWaterfall
      defaultWaterfall={defaultWaterfall}
      defaultWorkout={defaultWorkout}
      defaultSeniority={defaultStats.seniority}
      onWaterfallSubmit={setEstate}
      onWorkoutSubmit={setTerms}
      onStatsSubmit={setSeniority}
      waterfall={{
        result: waterfall.data ?? null,
        loading: waterfall.isFetching,
        error: waterfall.error?.message,
      }}
      workout={{
        result: workout.data ?? null,
        loading: workout.isFetching,
        error: workout.error?.message,
      }}
      stats={{
        result: stats.data ?? null,
        loading: stats.isFetching,
        error: stats.error?.message,
      }}
      calibrationIds={ids.data}
      density={density}
    />
  );
}
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with supplied results.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Card density={density} />
    </WorkerPreview>
  );
}
