import { queryOptions } from "@tanstack/react-query";

import { modelProviderList, modelProviderPresetList } from "#/api/model-provider";

export const modelProviderKeys = {
  all: ["model-provider"] as const,
  list: ["model-provider", "list"] as const,
  presets: ["model-provider", "preset-list"] as const,
};

export const modelProviderListQueryOptions = queryOptions({
  queryKey: modelProviderKeys.list,
  queryFn: modelProviderList,
  staleTime: Infinity,
});

export const modelProviderPresetListQueryOptions = queryOptions({
  queryKey: modelProviderKeys.presets,
  queryFn: modelProviderPresetList,
  staleTime: Infinity,
});
