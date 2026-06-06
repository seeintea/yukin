import { useParams } from "react-router";

export function ChatPage() {
  const { sessionId } = useParams<{ sessionId: string }>();

  return (
    <div className="flex flex-1 flex-col items-center justify-center text-muted-foreground">
      {sessionId && sessionId !== "new" ? (
        <p>Chat session: {sessionId}</p>
      ) : (
        <div className="space-y-2 text-center">
          <p className="text-lg font-medium">Start a new conversation</p>
          <p className="text-sm">Ask something, or pick a session from the sidebar.</p>
        </div>
      )}
    </div>
  );
}