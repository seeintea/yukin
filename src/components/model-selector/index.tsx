import { useQuery } from "@tanstack/react-query";
import { BrainIcon, ChevronDownIcon } from "lucide-react";
import { useEffect, useMemo } from "react";

import { modelProviderList, modelProviderPresetList } from "#/api/model-provider";
import type { ModelPreset, ModelProvider } from "#/protocol/model-provider";
import { Button } from "#/shadcn/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "#/shadcn/dropdown-menu";

interface ProviderOption {
  provider: ModelProvider;
  models: ModelPreset[];
}

export interface ModelSelection {
  providerId: string;
  modelId: string;
}

interface ModelSelectorProps {
  value: ModelSelection | null;
  onValueChange: (value: ModelSelection | null) => void;
  disabled?: boolean;
}

function createDefaultSelection(option: ProviderOption | undefined): ModelSelection | null {
  const model = option?.models[0];
  if (!option || !model) {
    return null;
  }

  return {
    providerId: option.provider.id,
    modelId: model.modelId,
  };
}

function normalizeSelection(
  selection: ModelSelection | null,
  options: ProviderOption[],
): ModelSelection | null {
  const option = options.find((item) => item.provider.id === selection?.providerId) ?? options[0];
  const defaultSelection = createDefaultSelection(option);
  if (!option || !defaultSelection) {
    return null;
  }

  const model =
    option.models.find((item) => item.modelId === selection?.modelId) ?? option.models[0];
  return {
    providerId: option.provider.id,
    modelId: model.modelId,
  };
}

function isSameSelection(left: ModelSelection | null, right: ModelSelection | null) {
  return left?.providerId === right?.providerId && left?.modelId === right?.modelId;
}

export function ModelSelector({ value, onValueChange, disabled }: ModelSelectorProps) {
  const providersQuery = useQuery({
    queryKey: ["model-provider", "list"],
    queryFn: modelProviderList,
    staleTime: Infinity,
  });
  const presetsQuery = useQuery({
    queryKey: ["model-provider", "preset-list"],
    queryFn: modelProviderPresetList,
    staleTime: Infinity,
  });

  const options = useMemo(() => {
    const presets = presetsQuery.data ?? [];

    return (providersQuery.data ?? []).flatMap<ProviderOption>((provider) => {
      const preset = presets.find((item) => item.providerKey === provider.providerKey);
      const connection = preset?.connections.find((item) => item.apiFormat === provider.apiFormat);

      return preset && connection && connection.models.length > 0
        ? [{ provider, models: connection.models }]
        : [];
    });
  }, [presetsQuery.data, providersQuery.data]);

  useEffect(() => {
    const normalized = normalizeSelection(value, options);
    if (!isSameSelection(value, normalized)) {
      onValueChange(normalized);
    }
  }, [onValueChange, options, value]);

  const selectedOption = options.find((item) => item.provider.id === value?.providerId);
  const selectedModel = selectedOption?.models.find((item) => item.modelId === value?.modelId);
  const isPending = providersQuery.isPending || presetsQuery.isPending;
  const isError = providersQuery.isError || presetsQuery.isError;

  const handleProviderChange = (providerId: string) => {
    onValueChange(createDefaultSelection(options.find((item) => item.provider.id === providerId)));
  };

  const handleModelChange = (modelId: string) => {
    if (!selectedOption) {
      return;
    }

    const model = selectedOption.models.find((item) => item.modelId === modelId);
    if (!model) {
      return;
    }

    onValueChange({
      providerId: selectedOption.provider.id,
      modelId: model.modelId,
    });
  };

  const placeholder = isPending ? "正在加载模型" : isError ? "模型加载失败" : "没有可用模型";

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={disabled || isPending || isError || !value || !selectedModel}
          />
        }
      >
        <BrainIcon />
        <span>{selectedModel?.displayName ?? placeholder}</span>
        {value && <span className="text-muted-foreground">· 不推理</span>}
        <ChevronDownIcon className="text-muted-foreground" />
      </DropdownMenuTrigger>
      <DropdownMenuContent side="top" align="start" className="min-w-64">
        <DropdownMenuGroup>
          <DropdownMenuLabel>模型配置</DropdownMenuLabel>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />

        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            <span>供应商</span>
            <DropdownMenuShortcut>{selectedOption?.provider.providerAlias}</DropdownMenuShortcut>
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent className="min-w-44">
            <DropdownMenuRadioGroup value={value?.providerId} onValueChange={handleProviderChange}>
              {options.map((option) => (
                <DropdownMenuRadioItem
                  key={option.provider.id}
                  value={option.provider.id}
                  closeOnClick={false}
                >
                  {option.provider.providerAlias}
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
          </DropdownMenuSubContent>
        </DropdownMenuSub>

        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            <span>模型</span>
            <DropdownMenuShortcut>{selectedModel?.displayName}</DropdownMenuShortcut>
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent className="min-w-56">
            <DropdownMenuRadioGroup value={value?.modelId} onValueChange={handleModelChange}>
              {selectedOption?.models.map((model) => (
                <DropdownMenuRadioItem
                  key={model.modelId}
                  value={model.modelId}
                  closeOnClick={false}
                >
                  {model.displayName}
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
          </DropdownMenuSubContent>
        </DropdownMenuSub>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
