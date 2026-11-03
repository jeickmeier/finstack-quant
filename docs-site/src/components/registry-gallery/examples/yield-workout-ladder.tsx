"use client";
import type { ExampleProps } from "./props";
import {
  workoutLadderMetrics,
  YieldWorkoutLadder,
  type YieldWorkoutLadderProps,
} from "@/components/finstack/valuations/components/yield-workout-ladder/yield-workout-ladder";
import { useBondWorkoutLadder } from "@/hooks/valuations/use-bond-workout-ladder/use-bond-workout-ladder";
import { useMetricMetadata } from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
const { premium, quoteDateCall } = data.workoutLadder.cases;
// Host display choice: six decimal places of the returned values.
const sixPlaces = (value: number) => value.toFixed(6);
function Live({ density }: ExampleProps) {
  const ladder = useBondWorkoutLadder(premium.request);
  const metadata = useMetricMetadata(workoutLadderMetrics);
  return (
    <YieldWorkoutLadder
      ladder={(ladder.data as YieldWorkoutLadderProps["ladder"]) ?? null}
      metadata={metadata.data}
      loading={ladder.isFetching}
      error={ladder.error?.message}
      formatValue={sixPlaces}
      density={density}
    />
  );
}
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a supplied ladder.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <div className="space-y-6">
        <Live density={density} />
        <YieldWorkoutLadder
          ladder={quoteDateCall.ladder as YieldWorkoutLadderProps["ladder"]}
          metadata={
            data.workoutLadder.metadata as YieldWorkoutLadderProps["metadata"]
          }
          formatValue={sixPlaces}
          density={density}
        />
      </div>
    </WorkerPreview>
  );
}
