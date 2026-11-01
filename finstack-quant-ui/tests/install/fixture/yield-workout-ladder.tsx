"use client";
import {
  YieldWorkoutLadder,
  type YieldWorkoutLadderProps,
} from "@/components/finstack/valuations/components/yield-workout-ladder/yield-workout-ladder";

import data from "./data.json";
// The ladder is the supplied native fixture: the item installs without a worker.
const { cases, metadata } = data.workoutLadder;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone yield workout ladder.</p>
      <YieldWorkoutLadder
        ladder={cases.premium.ladder as YieldWorkoutLadderProps["ladder"]}
        metadata={metadata as YieldWorkoutLadderProps["metadata"]}
      />
    </>
  );
}
