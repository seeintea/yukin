import { Welcome, Sender } from "@ant-design/x";
import SelectedAPIKey from "./selected-api-key";

export default function ChatContent() {
  return (
    <main className={"w-[1280px] h-full m-auto flex flex-col gap-4"}>
      <div className={"flex-1 overflow-y-auto flex flex-col gap-4"}>
        <Welcome
          icon={
            <div className={"w-14 h-14 rounded-2xl overflow-hidden"}>
              <img src="/logo.png" alt="logo" />
            </div>
          }
          title="chatbox"
          description="chat box with react-router-spa"
        />
        <div className={"flex-1 flex items-center justify-center"}>
          <SelectedAPIKey />
        </div>
      </div>
      <Sender />
      <p className={"text-xs text-center"}>内容由 AI 生成，仅供参考</p>
    </main>
  );
}
