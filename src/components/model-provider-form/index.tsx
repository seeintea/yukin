import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery } from "@tanstack/react-query";
import { useImperativeHandle, useRef } from "react";
import type { Ref } from "react";
import { Controller, useForm, useWatch } from "react-hook-form";
import { z } from "zod";

import { modelProviderPresetList } from "#/api/model-provider";
import type { ApiFormat, CreateRequest, ModelProviderPreset } from "#/protocol/model-provider";
import { Field, FieldError, FieldGroup, FieldLabel } from "#/shadcn/field";
import { Input } from "#/shadcn/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/shadcn/select";

const providerInputSchema = z.object({
  providerKey: z.string().trim().min(1, "请选择供应商"),
  apiFormat: z.enum(["openai", "anthropic"], {
    error: "请选择兼容 API 格式",
  }),
  providerAlias: z.string().trim().min(1, "请输入别名"),
  baseUrl: z.url("请输入有效的请求地址"),
  apiKey: z.string().trim().min(1, "请输入密钥"),
});

type ProviderFormValues = z.infer<typeof providerInputSchema>;

const apiFormatLabels: Record<ApiFormat, string> = {
  openai: "OpenAI",
  anthropic: "Anthropic",
};

function findPreset(presets: ModelProviderPreset[], providerKey: string) {
  return presets.find((preset) => preset.providerKey === providerKey);
}

export interface ModelProviderFormRef {
  reset: () => void;
}

interface ModelProviderFormProps {
  id: string;
  ref?: Ref<ModelProviderFormRef>;
  onSubmit: (data: CreateRequest) => void;
}

export function ModelProviderForm(props: ModelProviderFormProps) {
  const { id, onSubmit, ref } = props;
  const baseUrlManuallyEditedRef = useRef(false);
  const presetsQuery = useQuery({
    queryKey: ["model-provider", "preset-list"],
    queryFn: modelProviderPresetList,
    staleTime: Infinity,
  });
  const presets = presetsQuery.data ?? [];

  const form = useForm<ProviderFormValues>({
    resolver: zodResolver(providerInputSchema),
    defaultValues: {
      providerKey: "",
      providerAlias: "",
      baseUrl: "",
      apiKey: "",
    },
  });

  const selectedProviderKey = useWatch({ control: form.control, name: "providerKey" });
  const selectedPreset = findPreset(presets, selectedProviderKey);
  const providerItems = presets.map((preset) => ({
    label: preset.displayName,
    value: preset.providerKey,
  }));
  const apiFormatItems =
    selectedPreset?.connections.map((connection) => ({
      label: apiFormatLabels[connection.apiFormat],
      value: connection.apiFormat,
    })) ?? [];

  useImperativeHandle(ref, () => ({
    reset: () => {
      baseUrlManuallyEditedRef.current = false;
      form.reset();
    },
  }));

  return (
    <form id={id} onSubmit={form.handleSubmit(onSubmit)}>
      <FieldGroup>
        <Controller
          name="providerKey"
          control={form.control}
          render={({ field, fieldState }) => (
            <Field>
              <FieldLabel htmlFor={`${id}-provider`}>请选择供应商</FieldLabel>
              <Select
                value={field.value || null}
                onValueChange={(value) => {
                  const providerKey = value ?? "";
                  const preset = findPreset(presets, providerKey);
                  const connection = preset?.connections[0];

                  baseUrlManuallyEditedRef.current = false;
                  field.onChange(providerKey);
                  form.setValue("providerAlias", preset?.displayName ?? "", {
                    shouldDirty: false,
                    shouldTouch: false,
                    shouldValidate: true,
                  });

                  if (connection) {
                    form.setValue("apiFormat", connection.apiFormat, {
                      shouldDirty: false,
                      shouldTouch: false,
                      shouldValidate: true,
                    });
                    form.setValue("baseUrl", connection.baseUrl, {
                      shouldDirty: false,
                      shouldTouch: false,
                      shouldValidate: true,
                    });
                  } else {
                    form.resetField("apiFormat");
                    form.setValue("baseUrl", "");
                  }
                }}
                id={`${id}-provider`}
                aria-invalid={fieldState.invalid}
                items={providerItems}
                disabled={presetsQuery.isPending || presetsQuery.isError}
              >
                <SelectTrigger className="w-full">
                  <SelectValue
                    placeholder={presetsQuery.isPending ? "正在加载供应商" : "请选择供应商"}
                  />
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
              {presetsQuery.isError && <FieldError>加载供应商预设失败</FieldError>}
            </Field>
          )}
        />

        <Controller
          name="apiFormat"
          control={form.control}
          render={({ field, fieldState }) => (
            <Field>
              <FieldLabel htmlFor={`${id}-format`}>请选择兼容 API 格式</FieldLabel>
              <Select
                value={field.value ?? null}
                onValueChange={(value) => {
                  if (!value) {
                    return;
                  }

                  if (!baseUrlManuallyEditedRef.current) {
                    const connection = selectedPreset?.connections.find(
                      (item) => item.apiFormat === value,
                    );
                    form.setValue("baseUrl", connection?.baseUrl ?? "", {
                      shouldDirty: false,
                      shouldTouch: false,
                      shouldValidate: true,
                    });
                  }

                  field.onChange(value);
                }}
                id={`${id}-format`}
                aria-invalid={fieldState.invalid}
                items={apiFormatItems}
                disabled={!selectedPreset}
              >
                <SelectTrigger className="w-full">
                  <SelectValue placeholder="请选择兼容 API 格式" />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {apiFormatItems.map((item) => (
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
            <Field>
              <FieldLabel htmlFor={`${id}-provider-alias`}>请输入别名</FieldLabel>
              <Input
                {...field}
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
            <Field>
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
          name="apiKey"
          control={form.control}
          render={({ field, fieldState }) => (
            <Field>
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
