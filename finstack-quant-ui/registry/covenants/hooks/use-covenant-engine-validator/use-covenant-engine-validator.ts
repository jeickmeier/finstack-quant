"use client";
import { useNativeValidator } from "@/hooks/shared/use-finstack/validation";
/** Native canonical CovenantEngine text through the shared worker; superseded revisions reject. */
export function useCovenantEngineValidator() {
  return useNativeValidator("validateCovenantEngine", "Covenant engine");
}
