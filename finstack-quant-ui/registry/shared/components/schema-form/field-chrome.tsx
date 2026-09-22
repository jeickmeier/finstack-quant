"use client";
import { createContext, useContext, type ReactNode } from "react";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
  PopoverTitle,
} from "@/components/ui/popover";
import { Button } from "@/components/ui/button";
import {
  initialValue,
  type InstrumentModule,
  type SchemaLocation,
} from "./schema";
import type { SchemaFormApi, FieldFilter } from "./schema-form";

/** Embedded editors can omit an explicit null, which is the same as leaving the field off the wire. */
export const FormChromeContext = createContext({ explicitNull: true });

type RenderProps = {
  module: InstrumentModule;
  form: SchemaFormApi;
  location: SchemaLocation;
  path: string;
  label: string;
  value: unknown;
  required?: boolean;
  layout: "basic" | "full";
  fields?: FieldFilter;
  skipOverride?: boolean;
  sectionless?: boolean;
};

function TermActions({
  label,
  content,
  children,
}: {
  label: string;
  content: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="finstack-editable-term">
      <div className="min-w-0">{content}</div>
      <Popover>
        <PopoverTrigger
          aria-label={`${label} options`}
          render={<Button variant="ghost" size="icon-xs" />}
        >
          ⋯
        </PopoverTrigger>
        <PopoverContent>
          <PopoverTitle>{label}</PopoverTitle>
          {children}
        </PopoverContent>
      </Popover>
    </div>
  );
}

/** Add, omit, and clear controls for nullable values. Returns null when the value is present and non-null. */
export function nullableFieldChrome(
  props: RenderProps & {
    nonNull: SchemaLocation | null;
    renderField(next: RenderProps): ReactNode;
  },
): ReactNode | null {
  const { required = true, value, nonNull, label, form, path, module } = props;
  const { explicitNull } = useContext(FormChromeContext);
  if ((!required && value === undefined) || (nonNull && value == null))
    return (
      <div className="finstack-optional-term flex min-w-0 flex-wrap items-center gap-2 border-b border-border py-1 text-sm">
        <span>{label}</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() =>
            form.setFieldValue(
              path,
              initialValue(module.schema, nonNull ?? props.location),
            )
          }
        >
          Add {label.toLowerCase()}
        </Button>
        {!required && value !== undefined && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => form.setFieldValue(path, undefined)}
          >
            Omit {label.toLowerCase()}
          </Button>
        )}
        {explicitNull && nonNull && value === undefined && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => form.setFieldValue(path, null)}
          >
            Set {label.toLowerCase()} to null
          </Button>
        )}
      </div>
    );
  if (!nonNull) return null;
  return (
    <TermActions
      label={label}
      content={props.renderField({
        module,
        form,
        path,
        label,
        value,
        layout: props.layout,
        fields: props.fields,
        skipOverride: props.skipOverride,
        sectionless: props.sectionless,
        location: nonNull,
      })}
    >
      {!required && (
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => form.setFieldValue(path, undefined)}
        >
          Omit {label.toLowerCase()}
        </Button>
      )}
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => form.setFieldValue(path, null)}
      >
        Clear {label.toLowerCase()}
      </Button>
    </TermActions>
  );
}

/** Omit control for an optional, non-nullable value. The nested field is required so it does not wrap itself again. */
export function optionalFieldChrome(
  props: RenderProps & {
    nonNull: SchemaLocation | null;
    renderField(next: RenderProps): ReactNode;
  },
): ReactNode | null {
  const { required = true, nonNull, label, form, path } = props;
  if (required || nonNull) return null;
  return (
    <TermActions
      label={label}
      content={props.renderField({
        module: props.module,
        form,
        location: props.location,
        path,
        label,
        value: props.value,
        layout: props.layout,
        fields: props.fields,
        skipOverride: props.skipOverride,
        sectionless: props.sectionless,
        required: true,
      })}
    >
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => form.setFieldValue(path, undefined)}
      >
        Omit {label.toLowerCase()}
      </Button>
    </TermActions>
  );
}
