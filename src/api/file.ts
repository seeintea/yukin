import { invoke } from "@tauri-apps/api/core";

import type { FileReference } from "#/protocol/file";

export function fileReferenceSelect(): Promise<FileReference | null> {
  return invoke("file_reference_select");
}

export async function fileReferenceRelease(referenceId: string): Promise<void> {
  await invoke("file_reference_release", { request: { referenceId } });
}
