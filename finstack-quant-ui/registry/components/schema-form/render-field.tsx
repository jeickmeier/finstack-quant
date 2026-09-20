"use client";
import { useState } from "react";
import { Fieldset, buttonClass } from "@/lib/finstack/form";
import { EnumField } from "../../primitives/enum-field/enum-field";
import { discriminator } from "./discriminator";
import {
  resolve,
  propertiesOf,
  nullable,
  initialValue,
  objectValue,
  labelFor,
  type InstrumentModule,
  type SchemaLocation,
} from "./schema";
import type { SchemaFormApi, FieldFilter } from "./schema-form";

/** Bond-subset renderer over generated structure; the module codec owns validation. */
export function RenderField({
  module,
  form,
  location,
  path,
  label,
  value,
  required = true,
  layout,
  fields,
}: {
  module: InstrumentModule;
  form: SchemaFormApi;
  location: SchemaLocation;
  path: string;
  label: string;
  value: unknown;
  required?: boolean;
  layout: "basic" | "full";
  fields?: FieldFilter;
}) {
  const [more, setMore] = useState(false);
  const within = (child: string, parent: string) =>
    child === parent ||
    child.startsWith(`${parent}.`) ||
    child.startsWith(`${parent}[`);
  if (fields?.deny?.some((key) => within(path, key))) return null;
  if (
    fields?.allow &&
    path &&
    !fields.allow.some((key) => within(key, path) || within(path, key))
  )
    return null;
  const resolved = resolve(module.schema, location);
  const { schema, pointer } = resolved;
  const metadata = module.metadata.find((meta) => meta.path === pointer);
  const help = schema.description ?? metadata?.description;
  const info = { label, help };
  const nonNull = nullable(module.schema, resolved);
  if ((!required && value === undefined) || (nonNull && value == null))
    return (
      <div className="space-y-1 text-sm">
        <span>{label}</span>
        <button
          type="button"
          className={buttonClass}
          onClick={() =>
            form.setFieldValue(
              path,
              initialValue(module.schema, nonNull ?? resolved),
            )
          }
        >
          Add {label.toLowerCase()}
        </button>
      </div>
    );
  if (nonNull)
    return (
      <div className="col-span-full space-y-2">
        <RenderField
          {...{ module, form, path, label, value, layout, fields }}
          location={nonNull}
        />
        <button
          type="button"
          className={buttonClass}
          onClick={() => form.setFieldValue(path, null)}
        >
          Clear {label.toLowerCase()}
        </button>
      </div>
    );
  if (Object.hasOwn(schema, "const")) return null;
  if (!required && !nonNull)
    return (
      <div className="col-span-full space-y-2">
        <RenderField
          {...{ module, form, location, path, label, value, layout, fields }}
          required
        />
        <button
          type="button"
          className={buttonClass}
          onClick={() => form.setFieldValue(path, undefined)}
        >
          Omit {label.toLowerCase()}
        </button>
      </div>
    );
  const union = discriminator(module.schema, resolved);
  if (union) {
    const selected = union.selected(value);
    return (
      <Fieldset {...info}>
        <EnumField
          label={`${label} type`}
          value={selected < 0 ? "" : String(selected)}
          options={union.branches.map((branch, index) => ({
            value: String(index),
            label: labelFor(branch.label),
            description: branch.schema.description,
          }))}
          onValueChange={(index) =>
            form.setFieldValue(path, union.switchTo(Number(index), value))
          }
        />
        {selected >= 0 && (
          <RenderField
            {...{ module, form, path, label, value, layout, fields }}
            location={union.branches[selected]}
          />
        )}
      </Fieldset>
    );
  }
  const enumVariants =
    schema.enum ??
    (schema.oneOf ?? schema.anyOf)?.flatMap(
      (node) => node.enum ?? (Object.hasOwn(node, "const") ? [node.const] : []),
    );
  if (
    enumVariants?.length &&
    enumVariants.every((value) => typeof value === "string")
  )
    return (
      <form.AppField name={path}>
        {(field) => (
          <field.SelectField
            {...info}
            options={enumVariants.map((value) => ({
              value: String(value),
              label: String(value),
              description: (schema.oneOf ?? schema.anyOf)?.find(
                (node) => node.enum?.includes(value) || node.const === value,
              )?.description,
            }))}
          />
        )}
      </form.AppField>
    );
  if (schema.type === "object") {
    const properties = propertiesOf(schema);
    const primary = properties.filter(
      ([, node]) => !Object.hasOwn(node, "default"),
    );
    const secondary = properties.filter(([, node]) =>
      Object.hasOwn(node, "default"),
    );
    const render = ([key, node]: (typeof properties)[number]) => (
      <RenderField
        key={key}
        {...{ module, form, layout, fields }}
        location={{ schema: node, pointer: `${pointer}/properties/${key}` }}
        path={path ? `${path}.${key}` : key}
        label={
          path === "instrument" && key === "spec"
            ? "Instrument terms"
            : labelFor(key)
        }
        value={objectValue(value)[key]}
        required={schema.required?.includes(key) ?? false}
      />
    );
    if (!path || (path === "instrument" && schema.properties?.spec))
      return (
        <div className="col-span-full grid gap-3">{properties.map(render)}</div>
      );
    return (
      <Fieldset {...info}>
        <div className="grid gap-3 md:grid-cols-2">
          {primary.map(render)}
          {(layout === "full" || more) && secondary.map(render)}
        </div>
        {layout === "basic" && secondary.length > 0 && (
          <button
            type="button"
            className={buttonClass}
            aria-expanded={more}
            onClick={() => setMore(!more)}
          >
            {more ? "Fewer" : "More"} {label.toLowerCase()} fields
          </button>
        )}
      </Fieldset>
    );
  }
  if (schema.type === "array" && schema.prefixItems)
    return (
      <Fieldset {...info}>
        {schema.prefixItems.map((node, index) => (
          <RenderField
            key={index}
            {...{ module, form, layout, fields }}
            location={{
              schema: node,
              pointer: `${pointer}/prefixItems/${index}`,
            }}
            path={`${path}[${index}]`}
            label={node.title ?? `${label} ${index + 1}`}
            value={Array.isArray(value) ? value[index] : undefined}
          />
        ))}
      </Fieldset>
    );
  if (schema.type === "array")
    return (
      <form.AppField name={path} mode="array">
        {(field) => (
          <field.ArrayField
            label={label}
            create={() =>
              initialValue(module.schema, {
                schema: schema.items ?? {},
                pointer: `${pointer}/items`,
              })
            }
          >
            {(index) => (
              <RenderField
                {...{ module, form, layout, fields }}
                location={{
                  schema: schema.prefixItems?.[index] ?? schema.items ?? {},
                  pointer: `${pointer}/items`,
                }}
                path={`${path}[${index}]`}
                label={`${label} ${index + 1}`}
                value={Array.isArray(value) ? value[index] : undefined}
              />
            )}
          </field.ArrayField>
        )}
      </form.AppField>
    );
  return (
    <form.AppField name={path}>
      {(field) => {
        if (schema.type === "boolean") return <field.BooleanField {...info} />;
        if (schema.format === "date") return <field.DateField {...info} />;
        if (
          schema.type === "string" &&
          (metadata?.source.includes("/decimal") || schema.title === "Decimal")
        )
          return <field.DecimalField {...info} pattern={schema.pattern} />;
        if (["string", "integer", "number"].includes(String(schema.type)))
          return (
            <field.TextField
              {...info}
              inputMode={
                schema.type === "integer"
                  ? "numeric"
                  : schema.type === "number"
                    ? "decimal"
                    : "text"
              }
            />
          );
        return (
          <p className="text-sm text-muted-foreground">
            {label}: this schema construct is outside the bond editor subset.
          </p>
        );
      }}
    </form.AppField>
  );
}
