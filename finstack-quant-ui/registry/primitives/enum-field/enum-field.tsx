"use client";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectGroup,
  SelectLabel,
  SelectItem,
} from "@/components/ui/select";
import { FieldFrame, type FieldInfo } from "../field-frame/field-frame";
export interface EnumOption {
  value: string;
  label: string;
  description?: string;
  group?: string;
  disabled?: boolean;
}
export interface EnumFieldProps extends FieldInfo {
  value: string;
  onValueChange: (value: string) => void;
  options: readonly EnumOption[];
  disabled?: boolean;
  layout?: "select" | "radio";
}
/** Canonical supplied choices composed from unmodified shadcn controls. */
export function EnumField({
  value,
  onValueChange,
  options,
  disabled,
  layout = options.length <= 4 ? "radio" : "select",
  ...field
}: EnumFieldProps) {
  const groups = Map.groupBy(options, (option) => option.group ?? "");
  return (
    <FieldFrame {...field}>
      {(control) =>
        layout === "radio" ? (
          <RadioGroup
            {...control}
            value={value}
            onValueChange={(next) => onValueChange(next)}
            disabled={disabled}
          >
            {options.map((option, index) => (
              <div key={option.value} className="flex items-center gap-2">
                <RadioGroupItem
                  id={`${control.id}-${index}`}
                  value={option.value}
                  disabled={option.disabled}
                  aria-invalid={control["aria-invalid"]}
                />
                <Label
                  htmlFor={`${control.id}-${index}`}
                  title={option.description}
                >
                  {option.label}
                </Label>
              </div>
            ))}
          </RadioGroup>
        ) : (
          <Select
            value={value || null}
            onValueChange={(next) => onValueChange(next ?? "")}
            disabled={disabled}
            items={options}
          >
            <SelectTrigger {...control}>
              <SelectValue placeholder="Select…" />
            </SelectTrigger>
            <SelectContent>
              {[...groups].map(([group, items]) => (
                <SelectGroup key={group}>
                  {group && <SelectLabel>{group}</SelectLabel>}
                  {items.map((option) => (
                    <SelectItem
                      key={option.value}
                      value={option.value}
                      disabled={option.disabled}
                      title={option.description}
                    >
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectGroup>
              ))}
            </SelectContent>
          </Select>
        )
      }
    </FieldFrame>
  );
}
