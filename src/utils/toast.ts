import { toast } from "#/shadcn/toast";

import { getErrorMessage } from "./error";

export function showErrorToast(title: string, error: unknown, fallback?: string) {
  toast.add({
    title,
    description: getErrorMessage(error, fallback),
    type: "error",
    priority: "high",
  });
}
