import type { ProviderOptions } from "../types";
import { DEEP_SEEK_MODELS } from "./variable";

export type DeepSeekModel = (typeof DEEP_SEEK_MODELS)[number];

export interface DeepSeekProviderOptions extends ProviderOptions<DeepSeekModel> {
  thinking: boolean;
  reasoningEffort: "high" | "max";
}
