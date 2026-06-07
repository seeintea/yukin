import { MemoryRouter, Navigate, Route, Routes as ReactRoutes } from "react-router";
import { Layout } from "@/components/layout";
import { ChatScreen } from "@/features/chat";
import { SettingsScreen } from "@/features/settings";
import { Toaster } from "@/components/ui/sonner";

export function Routes() {
  return (
    <MemoryRouter initialEntries={["/chat/new"]}>
      <Layout>
        <ReactRoutes>
          <Route path="/" element={<Navigate to="/chat/new" replace />} />
          <Route path="/chat/:sessionId" element={<ChatScreen />} />
          <Route path="/settings" element={<SettingsScreen />} />
          <Route path="*" element={<Navigate to="/chat/new" replace />} />
        </ReactRoutes>
      </Layout>
      <Toaster />
    </MemoryRouter>
  );
}