import * as z from "zod";

export const providerInputSchema = z.object({
  provider: z.string().min(1, { message: "请选择供应商" }),
  format: z.string().min(1, { message: "请选择兼容 API 格式" }),
  providerAlias: z.string().min(1, { message: "请输入别名" }),
  baseUrl: z.string().min(1, { message: "请输入请求地址" }),
  key: z.string().min(1, { message: "请输入密钥" }),
});

export type ProviderInput = z.infer<typeof providerInputSchema>;

export interface ProviderOutput extends ProviderInput {
  id: string;
}
