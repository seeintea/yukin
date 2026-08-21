import { invoke } from "@tauri-apps/api/core";

import type { DirectoryReference, FileReference } from "#/protocol/file";

export function fileReferenceSelect(): Promise<FileReference | null> {
  return invoke("file_reference_select");
}

export function directoryReferenceSelect(): Promise<DirectoryReference | null> {
  return invoke("directory_reference_select");
}

export async function directoryReferenceRelease(referenceId: string): Promise<void> {
  await invoke("directory_reference_release", { request: { referenceId } });
}

export async function fileReferenceRelease(referenceId: string): Promise<void> {
  await invoke("file_reference_release", { request: { referenceId } });
}
