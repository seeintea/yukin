import { Channel, invoke } from "@tauri-apps/api/core";

import type { StreamEvent, StreamRequest } from "#/protocol/model-response";

export function modelResponseStream(
  request: StreamRequest,
  onEvent: (event: StreamEvent) => void,
): Promise<void> {
  const events = new Channel<StreamEvent>(onEvent);

  return invoke("model_response_stream", { request, events });
}
