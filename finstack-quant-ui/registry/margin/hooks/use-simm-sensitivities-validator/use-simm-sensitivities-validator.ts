"use client";
import { useNativeValidator } from "@/hooks/shared/use-finstack/validation";
/** Native canonical SimmSensitivitiesJson through the shared worker; superseded revisions reject. */
export function useSimmSensitivitiesValidator() {
  return useNativeValidator("validateSimmSensitivities", "SIMM sensitivities");
}
