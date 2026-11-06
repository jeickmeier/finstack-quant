"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import type {
  RecoveryWaterfallRequest,
  SeniorityRecoveryRequest,
} from "@/workers/finstack-contract";
import { RecoveryWaterfall } from "@/components/finstack/models/components/recovery-waterfall/recovery-waterfall";
import { SeniorityRecoveryCard } from "@/components/finstack/models/components/recovery-waterfall/seniority-recovery-card";
import { WorkoutLgdCard } from "@/components/finstack/models/components/recovery-waterfall/workout-lgd-card";
import {
  useAllocateRecovery,
  useSeniorityCalibrationIds,
  useSeniorityRecoveryStats,
  useWorkoutLgd,
} from "@/hooks/models/use-recovery-waterfall/use-recovery-waterfall";
import data from "../data.json";
import { workoutLgdRequest } from "../fixtures";
import { WorkerPreview } from "../worker-preview";
const defaultWaterfall = data.recovery.waterfall.shortfall
  .request as RecoveryWaterfallRequest;
const defaultWorkout = workoutLgdRequest;
const defaultStats = data.recovery.stats.seniorUnsecured
  .request as SeniorityRecoveryRequest;
function Cards({ density }: ExampleProps) {
  // Starts from the fixture's terms; each form replaces its request.
  const [estate, setEstate] = useState(defaultWaterfall);
  const [terms, setTerms] = useState(defaultWorkout);
  const [seniority, setSeniority] = useState(defaultStats);
  const waterfall = useAllocateRecovery(estate);
  const workout = useWorkoutLgd(terms);
  const stats = useSeniorityRecoveryStats(seniority);
  const ids = useSeniorityCalibrationIds();
  return (
    <div className="space-y-8">
      <RecoveryWaterfall
        defaultTerms={defaultWaterfall}
        onSubmit={setEstate}
        state={{
          result: waterfall.data,
          loading: waterfall.isFetching,
          error: waterfall.error?.message,
        }}
        density={density}
      />
      <WorkoutLgdCard
        defaultTerms={defaultWorkout}
        onSubmit={setTerms}
        state={{
          result: workout.data,
          loading: workout.isFetching,
          error: workout.error?.message,
        }}
        density={density}
      />
      <SeniorityRecoveryCard
        defaultSeniority={defaultStats.seniority}
        calibrationIds={ids.data}
        onSubmit={setSeniority}
        state={{
          result: stats.data,
          loading: stats.isFetching,
          error: stats.error?.message,
        }}
        density={density}
      />
    </div>
  );
}
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with supplied results.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Cards density={density} />
    </WorkerPreview>
  );
}
