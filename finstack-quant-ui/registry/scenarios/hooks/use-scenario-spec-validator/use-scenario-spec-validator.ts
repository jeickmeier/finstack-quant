"use client";
import { useNativeValidator } from "@/hooks/shared/use-finstack/validation";
/** Native canonical ScenarioSpec text through the shared worker; superseded revisions reject. */
export function useScenarioSpecValidator() {
  return useNativeValidator("validateScenarioSpec", "Scenario");
}
