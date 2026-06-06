import { MemoryRouter, Navigate, Route, Routes } from "react-router";
import { AppShell } from "@/components/layout/AppShell";
import { ChatPage } from "@/pages/ChatPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { Toaster } from "@/components/ui/sonner";

function App() {
  return (
    <MemoryRouter initialEntries={["/chat/new"]}>
      <AppShell>
        <Routes>
          <Route path="/" element={<Navigate to="/chat/new" replace />} />
          <Route path="/chat/:sessionId" element={<ChatPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/chat/new" replace />} />
        </Routes>
      </AppShell>
      <Toaster />
    </MemoryRouter>
  );
}

export default App;