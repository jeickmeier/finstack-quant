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
  /** Render this field with the generated renderer while retaining overrides for descendants. */
  renderDefault(): ReactNode;
}) => ReactNode | undefined;
export const FieldRendererContext = createContext<FieldRenderer | undefined>(
  undefined,
);
