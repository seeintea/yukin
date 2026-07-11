import { RefObject, useImperativeHandle, useRef } from "react";
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

import {
  apiRespFormat,
  providerDefaultQueryUrl,
  providerItems,
} from "./variable";

function getProviderDefaultQueryUrl(
  provider: string | null,
  format: string | null,
) {
  if (!provider || !format) return undefined;

  const key = `${provider}_${format}` as keyof typeof providerDefaultQueryUrl;

  return providerDefaultQueryUrl[key];
}

export const formSchema = z.object({
  provider: z.string().min(1, { message: "请选择供应商" }),
  format: z.string().min(1, { message: "请选择兼容 API 格式" }),
  providerAlias: z.string().min(1, { message: "请输入别名" }),
  baseUrl: z.string().min(1, { message: "请输入请求地址" }),
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
  const aliasManuallyEditedRef = useRef(false);
  const baseUrlManuallyEditedRef = useRef(false);

  const form = useForm<z.infer<typeof formSchema>>({
    resolver: zodResolver(formSchema),
    defaultValues: {
      provider: "",
      format: "",
      providerAlias: "",
      baseUrl: "",
      key: "",
    },
  });

  useImperativeHandle(ref, () => ({
    reset: () => {
      aliasManuallyEditedRef.current = false;
      baseUrlManuallyEditedRef.current = false;
      form.reset();
    },
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
                onValueChange={(value) => {
                  aliasManuallyEditedRef.current = false;
                  baseUrlManuallyEditedRef.current = false;
                  const providerLabel = providerItems.find(
                    (item) => item.value === value,
                  )?.label;

                  form.setValue("providerAlias", providerLabel ?? "", {
                    shouldDirty: false,
                    shouldTouch: false,
                    shouldValidate: true,
                  });
                  const defaultQueryUrl = getProviderDefaultQueryUrl(
                    value,
                    form.getValues("format"),
                  );

                  if (defaultQueryUrl) {
                    form.setValue("baseUrl", defaultQueryUrl, {
                      shouldDirty: false,
                      shouldTouch: false,
                      shouldValidate: true,
                    });
                  }
                  field.onChange(value);
                }}
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
          name="format"
          control={form.control}
          render={({ field, fieldState }) => (
            <Field data-invalid={fieldState.invalid}>
              <FieldLabel htmlFor={`${id}-format`}>
                请选择兼容 API 格式
              </FieldLabel>
              <Select
                {...field}
                onValueChange={(value) => {
                  if (!baseUrlManuallyEditedRef.current) {
                    form.setValue(
                      "baseUrl",
                      getProviderDefaultQueryUrl(
                        form.getValues("provider"),
                        value,
                      ) ?? "",
                      {
                        shouldDirty: false,
                        shouldTouch: false,
                        shouldValidate: true,
                      },
                    );
                  }
                  field.onChange(value);
                }}
                id={`${id}-format`}
                aria-invalid={fieldState.invalid}
                items={apiRespFormat}
              >
                <SelectTrigger>
                  <SelectValue placeholder="请选择兼容 API 格式" />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {apiRespFormat.map((item) => (
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
          name="providerAlias"
          control={form.control}
          render={({ field, fieldState }) => (
            <Field data-invalid={fieldState.invalid}>
              <FieldLabel htmlFor={`${id}-provider-alias`}>
                请输入别名
              </FieldLabel>
              <Input
                {...field}
                onChange={(event) => {
                  aliasManuallyEditedRef.current = true;
                  field.onChange(event);
                }}
                id={`${id}-provider-alias`}
                aria-invalid={fieldState.invalid}
                placeholder="请输入别名"
                autoComplete="off"
              />
              {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
            </Field>
          )}
        />

        <Controller
          name="baseUrl"
          control={form.control}
          render={({ field, fieldState }) => (
            <Field data-invalid={fieldState.invalid}>
              <FieldLabel htmlFor={`${id}-base-url`}>请输入请求地址</FieldLabel>
              <Input
                {...field}
                onChange={(event) => {
                  baseUrlManuallyEditedRef.current = true;
                  field.onChange(event);
                }}
                id={`${id}-base-url`}
                aria-invalid={fieldState.invalid}
                placeholder="请输入请求地址"
                autoComplete="off"
              />
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
