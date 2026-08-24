import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  FolderOpenIcon,
  MoreHorizontalIcon,
  PackageOpenIcon,
  PowerIcon,
  Trash2Icon,
} from "lucide-react";
import { useState } from "react";

import {
  importedSkillDelete,
  importedSkillImportArchive,
  importedSkillImportDirectory,
  importedSkillList,
  importedSkillSetEnabled,
} from "#/api/imported-skill";
import type { ImportedSkill } from "#/protocol/imported-skill";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "#/shadcn/alert-dialog";
import { Button } from "#/shadcn/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "#/shadcn/dropdown-menu";
import { Skeleton } from "#/shadcn/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "#/shadcn/table";
import { toast } from "#/shadcn/toast";
import { showErrorToast } from "#/utils/toast";

const listKey = ["imported-skill", "list"] as const;

function upsert(skills: ImportedSkill[], skill: ImportedSkill) {
  return [skill, ...skills.filter((item) => item.id !== skill.id)];
}

export function ImportedSkillSettings() {
  const queryClient = useQueryClient();
  const skillsQuery = useQuery({
    queryKey: listKey,
    queryFn: importedSkillList,
    staleTime: Infinity,
  });
  const [deletingSkill, setDeletingSkill] = useState<ImportedSkill | null>(null);

  const importMutation = useMutation({
    mutationFn: (kind: "directory" | "archive") =>
      kind === "directory" ? importedSkillImportDirectory() : importedSkillImportArchive(),
    onSuccess: (skill) => {
      if (!skill) return;
      queryClient.setQueryData<ImportedSkill[]>(listKey, (skills = []) => upsert(skills, skill));
      toast.add({ title: "Skill 导入成功", type: "success" });
    },
    onError: (error) => showErrorToast("Skill 导入失败", error),
  });
  const enabledMutation = useMutation({
    mutationFn: importedSkillSetEnabled,
    onSuccess: (skill) => {
      queryClient.setQueryData<ImportedSkill[]>(listKey, (skills = []) => upsert(skills, skill));
    },
    onError: (error) => showErrorToast("Skill 状态更新失败", error),
  });
  const deleteMutation = useMutation({
    mutationFn: importedSkillDelete,
    onSuccess: (_, request) => {
      queryClient.setQueryData<ImportedSkill[]>(listKey, (skills = []) =>
        skills.filter((skill) => skill.id !== request.id),
      );
      setDeletingSkill(null);
      toast.add({ title: "Skill 已删除", type: "success" });
    },
    onError: (error) => showErrorToast("Skill 删除失败", error),
  });

  const skills = skillsQuery.data ?? [];
  return (
    <div className="h-full overflow-auto p-8">
      <div className="mx-auto flex max-w-5xl flex-col gap-6">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold">Skills</h1>
            <p className="mt-1 text-sm text-muted-foreground">
              从本地目录或 ZIP 导入 Skill。当前仅管理托管副本，不接入 Agent Runtime。
            </p>
          </div>
          <div className="flex gap-2">
            <Button
              variant="outline"
              disabled={importMutation.isPending}
              onClick={() => importMutation.mutate("archive")}
            >
              <PackageOpenIcon />
              导入 ZIP
            </Button>
            <Button
              disabled={importMutation.isPending}
              onClick={() => importMutation.mutate("directory")}
            >
              <FolderOpenIcon />
              导入目录
            </Button>
          </div>
        </div>

        <div className="overflow-hidden rounded-xl border bg-card">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Skill</TableHead>
                <TableHead>来源</TableHead>
                <TableHead>状态</TableHead>
                <TableHead>内容摘要</TableHead>
                <TableHead className="w-12" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {skillsQuery.isPending ? (
                Array.from({ length: 3 }, (_, index) => (
                  <TableRow key={index}>
                    <TableCell colSpan={5}>
                      <Skeleton className="h-10 w-full" />
                    </TableCell>
                  </TableRow>
                ))
              ) : skillsQuery.isError ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-32 text-center text-muted-foreground">
                    Skill 加载失败
                  </TableCell>
                </TableRow>
              ) : skills.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-32 text-center text-muted-foreground">
                    尚未导入 Skill
                  </TableCell>
                </TableRow>
              ) : (
                skills.map((skill) => (
                  <TableRow key={skill.id}>
                    <TableCell className="max-w-md">
                      <div className="font-medium">{skill.name}</div>
                      <div className="truncate text-xs text-muted-foreground">
                        {skill.description}
                      </div>
                    </TableCell>
                    <TableCell>{skill.sourceKind === "directory" ? "目录" : "ZIP"}</TableCell>
                    <TableCell>{skill.enabled ? "已启用" : "已停用"}</TableCell>
                    <TableCell className="font-mono text-xs text-muted-foreground">
                      {skill.contentDigest.slice(0, 12)}
                    </TableCell>
                    <TableCell>
                      <DropdownMenu>
                        <DropdownMenuTrigger
                          render={
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon-sm"
                              aria-label="操作"
                            />
                          }
                        >
                          <MoreHorizontalIcon />
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem
                            disabled={enabledMutation.isPending}
                            onClick={() =>
                              enabledMutation.mutate({ id: skill.id, enabled: !skill.enabled })
                            }
                          >
                            <PowerIcon />
                            {skill.enabled ? "停用" : "启用"}
                          </DropdownMenuItem>
                          <DropdownMenuSeparator />
                          <DropdownMenuItem
                            variant="destructive"
                            onClick={() => setDeletingSkill(skill)}
                          >
                            <Trash2Icon />
                            删除
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      </div>

      <AlertDialog
        open={deletingSkill !== null}
        onOpenChange={(open) => !open && setDeletingSkill(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>删除 Skill？</AlertDialogTitle>
            <AlertDialogDescription>
              将删除“{deletingSkill?.name}”的记录和应用托管副本，不会影响最初选择的本地目录或 ZIP。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleteMutation.isPending}>取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={deleteMutation.isPending}
              onClick={() => deletingSkill && deleteMutation.mutate({ id: deletingSkill.id })}
            >
              {deleteMutation.isPending ? "正在删除" : "删除"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
