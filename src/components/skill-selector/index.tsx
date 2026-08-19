import { useQuery } from "@tanstack/react-query";
import { ChevronDownIcon, SparklesIcon } from "lucide-react";

import { agentSkillList } from "#/api/skill";
import { Button } from "#/shadcn/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "#/shadcn/dropdown-menu";

const generalSkill = "__general__";

interface SkillSelectorProps {
  value: string | null;
  onValueChange: (value: string | null) => void;
  disabled?: boolean;
}

export function SkillSelector({ value, onValueChange, disabled }: SkillSelectorProps) {
  const skillsQuery = useQuery({
    queryKey: ["agent-skill", "list"],
    queryFn: agentSkillList,
    staleTime: Infinity,
  });
  const selected = skillsQuery.data?.find((skill) => skill.id === value);
  const label = skillsQuery.isPending
    ? "正在加载 Skill"
    : skillsQuery.isError
      ? "Skill 加载失败"
      : (selected?.title ?? "通用");

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={disabled || skillsQuery.isPending || skillsQuery.isError}
          />
        }
      >
        <SparklesIcon />
        <span>{label}</span>
        <ChevronDownIcon className="text-muted-foreground" />
      </DropdownMenuTrigger>
      <DropdownMenuContent side="top" align="start" className="min-w-64">
        <DropdownMenuRadioGroup
          value={value ?? generalSkill}
          onValueChange={(nextValue) =>
            onValueChange(nextValue === generalSkill ? null : nextValue)
          }
        >
          <DropdownMenuRadioItem value={generalSkill}>
            <div>
              <div>通用</div>
              <div className="text-xs text-muted-foreground">不限制当前可用工具</div>
            </div>
          </DropdownMenuRadioItem>
          {skillsQuery.data?.map((skill) => (
            <DropdownMenuRadioItem key={skill.id} value={skill.id}>
              <div>
                <div>{skill.title}</div>
                <div className="text-xs text-muted-foreground">{skill.description}</div>
              </div>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
