import * as z from "zod";
import { Controller, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import {
  FieldGroup,
  Field,
  FieldLabel,
  FieldError,
} from "#/components/ui/field";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectGroup,
  SelectItem,
} from "#/components/ui/select";
import { Input } from "#/components/ui/input";

import { providerItems } from "./variable";
import { RefObject, useImperativeHandle } from "react";

export const formSchema = z.object({
  provider: z.string().min(1, { message: "请选择供应商" }),
  key: z.string().min(1, { message: "请输入密钥" }),
});

export interface ProviderFormRef {
  reset: () => void;
}

interface ProviderFormProps {
  id: string;
  ref?: RefObject<ProviderFormRef>;
  onSubmit: (data: z.infer<typeof formSchema>) => void;
}

export function ProviderForm(props: ProviderFormProps) {
  const { id, onSubmit, ref } = props;

  const form = useForm<z.infer<typeof formSchema>>({
    resolver: zodResolver(formSchema),
    defaultValues: {
      provider: "",
      key: "",
    },
  });

  useImperativeHandle(ref, () => ({
    reset: () => form.reset(),
  }));

  return (
    <form id={id} onSubmit={form.handleSubmit(onSubmit)}>
      <FieldGroup>
        <Controller
          name="provider"
          control={form.control}
          render={({ field, fieldState }) => (
            <Field data-invalid={fieldState.invalid}>
              <FieldLabel htmlFor={`${id}-provider`}>请选择供应商</FieldLabel>
              <Select
                {...field}
                onValueChange={field.onChange}
                id={`${id}-provider`}
                aria-invalid={fieldState.invalid}
                items={providerItems}
              >
                <SelectTrigger>
                  <SelectValue placeholder="请选择供应商" />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {providerItems.map((item) => (
                      <SelectItem key={item.value} value={item.value}>
                        {item.label}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
              {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
            </Field>
          )}
        />

        <Controller
          name="key"
          control={form.control}
          render={({ field, fieldState }) => (
            <Field data-invalid={fieldState.invalid}>
              <FieldLabel htmlFor={`${id}-key`}>请输入密钥</FieldLabel>
              <Input
                {...field}
                id={`${id}-key`}
                aria-invalid={fieldState.invalid}
                type="password"
                placeholder="请输入密钥"
                autoComplete="off"
              />
              {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
            </Field>
          )}
        />
      </FieldGroup>
    </form>
  );
}
