"use client";
import { useNativeValidator } from "../use-finstack/validation";
/** Native canonical validation through the existing shared worker. */
export function useMarketValidator() {
  return useNativeValidator("validateMarket", "Market");
}
