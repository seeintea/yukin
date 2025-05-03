import { Button, Layout, Space } from "antd";
import { PlusOutlined, ReloadOutlined } from "@ant-design/icons";
import Database from "~/components/database";
import type { Route } from "./+types/index";
import ChatContent from "~/components/chat-content";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "chatbox" },
    { name: "description", content: "chatbox playground" },
  ];
}

export default function Index() {
  return (
    <Layout hasSider className={"w-screen h-screen"}>
      <Layout.Sider width={256} theme={"light"} className={"p-4"}>
        <div className={"flex flex-col h-full gap-4"}>
          <Space className={"text-2xl font-bold pb-4"}>
            <img className={"w-12 rounded-full"} src="/logo.png" alt="logo" />
            <span>chatbox</span>
          </Space>
          <Button block icon={<PlusOutlined />}>
            开启新对话
          </Button>
          <div className={"flex-1 overflow-y-auto"}> history chat </div>
          <div className={"border-t py-4 border-stone-300 flex gap-4"}>
            <Button>加载历史数据</Button>
            <Button icon={<ReloadOutlined />} />
          </div>
        </div>
      </Layout.Sider>
      <Layout>
        <Layout.Content className={"p-4"}>
          <ChatContent />
        </Layout.Content>
      </Layout>
      <Database />
    </Layout>
  );
}
