import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";

import type {
  ApiFormat,
  ModelProvider,
  ReplaceCredentialRequest,
  UpdateRequest,
} from "#/protocol/model-provider";
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

const updateSchema = z.object({
  providerAlias: z.string().trim().min(1, "请输入别名"),
  apiFormat: z.enum(["openai", "anthropic"]),
  baseUrl: z.url("请输入有效的请求地址"),
});

const credentialSchema = z.object({
  apiKey: z.string().trim().min(1, "请输入密钥"),
});

const apiFormatLabels: Record<ApiFormat, string> = {
  openai: "OpenAI",
  anthropic: "Anthropic",
};

type UpdateValues = z.infer<typeof updateSchema>;
type CredentialValues = z.infer<typeof credentialSchema>;

interface ProviderUpdateFormProps {
  id: string;
  provider: ModelProvider;
  apiFormats: ApiFormat[];
  onSubmit: (request: UpdateRequest) => void;
}

export function ProviderUpdateForm({
  id,
  provider,
  apiFormats,
  onSubmit,
}: ProviderUpdateFormProps) {
  const form = useForm<UpdateValues>({
    resolver: zodResolver(updateSchema),
    defaultValues: {
      providerAlias: provider.providerAlias,
      apiFormat: provider.apiFormat,
      baseUrl: provider.baseUrl,
    },
  });

  return (
    <form
      id={id}
      onSubmit={form.handleSubmit((values) => onSubmit({ id: provider.id, ...values }))}
    >
      <FieldGroup>
        <Controller
          name="providerAlias"
          control={form.control}
          render={({ field, fieldState }) => (
            <Field>
              <FieldLabel htmlFor={`${id}-alias`}>别名</FieldLabel>
              <Input {...field} id={`${id}-alias`} aria-invalid={fieldState.invalid} />
              {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
            </Field>
          )}
        />
        <Controller
          name="apiFormat"
          control={form.control}
          render={({ field, fieldState }) => (
            <Field>
              <FieldLabel htmlFor={`${id}-format`}>兼容 API 格式</FieldLabel>
              <Select
                id={`${id}-format`}
                value={field.value}
                onValueChange={(value) => value && field.onChange(value)}
                items={apiFormats.map((format) => ({
                  label: apiFormatLabels[format],
                  value: format,
                }))}
              >
                <SelectTrigger className="w-full" aria-invalid={fieldState.invalid}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {apiFormats.map((format) => (
                      <SelectItem key={format} value={format}>
                        {apiFormatLabels[format]}
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
          name="baseUrl"
          control={form.control}
          render={({ field, fieldState }) => (
            <Field>
              <FieldLabel htmlFor={`${id}-base-url`}>请求地址</FieldLabel>
              <Input {...field} id={`${id}-base-url`} aria-invalid={fieldState.invalid} />
              {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
            </Field>
          )}
        />
      </FieldGroup>
    </form>
  );
}

interface ProviderCredentialFormProps {
  id: string;
  providerId: string;
  onSubmit: (request: ReplaceCredentialRequest) => void;
}

export function ProviderCredentialForm({ id, providerId, onSubmit }: ProviderCredentialFormProps) {
  const form = useForm<CredentialValues>({
    resolver: zodResolver(credentialSchema),
    defaultValues: { apiKey: "" },
  });

  return (
    <form id={id} onSubmit={form.handleSubmit((values) => onSubmit({ id: providerId, ...values }))}>
      <Controller
        name="apiKey"
        control={form.control}
        render={({ field, fieldState }) => (
          <Field>
            <FieldLabel htmlFor={`${id}-api-key`}>新 API Key</FieldLabel>
            <Input
              {...field}
              id={`${id}-api-key`}
              type="password"
              autoComplete="off"
              aria-invalid={fieldState.invalid}
            />
            {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
          </Field>
        )}
      />
    </form>
  );
}
