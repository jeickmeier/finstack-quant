"use client";
import { useNativeValidator } from "@/hooks/shared/use-finstack/validation";
/** Native canonical validation through the existing shared worker. */
export function useInstrumentValidator() {
  return useNativeValidator("validate", "Instrument");
}
