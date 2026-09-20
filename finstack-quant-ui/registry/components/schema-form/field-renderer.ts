import { createContext, type ReactNode } from "react";
import type { SchemaFormApi } from "./schema-form";
import type { SchemaLocation } from "./schema";
/** Optional presentation override; undefined delegates to the generated renderer. */
export type FieldRenderer = (field: {
  form: SchemaFormApi;
  location: SchemaLocation;
  path: string;
  label: string;
  value: unknown;
}) => ReactNode | undefined;
export const FieldRendererContext = createContext<FieldRenderer | undefined>(
  undefined,
);
