import { useEffect, useRef, useState } from "react";
import { Button, Card, Input, Space, Select, message } from "antd";
import { type DefaultOptionType } from "antd/es/select";
import { db } from "~/database";

export default function SelectedAPIKey() {
  const userInput = useRef<string>("");
  const [modelOptions, setModelOptions] = useState<DefaultOptionType[]>([]);

  const onSubmit = () => {
    if (!userInput.current) {
      message.error("请输入API Key！");
    }
  };

  useEffect(() => {
    db.model_platform.toArray().then((platform) => {
      setModelOptions(
        platform.map((plat) => ({ label: plat.name, value: plat.id }))
      );
    });
  }, []);

  return (
    <Card>
      <Space direction={"vertical"} size={"middle"}>
        <Space>
          <Select
            className={"w-32"}
            popupMatchSelectWidth={false}
            placeholder="模型"
            options={modelOptions}
          />
          <Input className={"w-64!"} placeholder="请输入API Key" />
          <Button onClick={onSubmit}>确定</Button>
        </Space>
        <Button block type="primary">
          加载本地 API Key
        </Button>
      </Space>
    </Card>
  );
}
