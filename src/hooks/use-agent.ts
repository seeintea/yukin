export function useAgent() {}

// type OutputStatus =
//   | "idle" // 尚未请求
//   | "streaming" // 正在接收内容
//   | "complete" // 收到可信的 finish 事件
//   | "incomplete" // 已有部分内容，但随后发生错误
//   | "failed"; // 尚未收到任何内容就发生错误

// interface AgentOutput {
//   status: OutputStatus;
//   content: string;
//   error: string | null;
//   finishReason: DeepSeekFinishReason | null;
// }

// const [agentOutput, setAgentOutput] = useState<AgentOutput>(() => ({
//   status: "idle",
//   content: "",
//   error: null,
//   finishReason: null,
// }));

// async function onSubmit(data: ProviderInput) {
//   if (isRunningRef.current) return;

//   isRunningRef.current = true;
//   setAgentOutput({
//     status: "streaming",
//     content: "",
//     error: null,
//     finishReason: null,
//   });

//   try {
//     for await (const event of streamDeepSeek(data.baseUrl, data.key, [
//       { role: "user", content: "你好，请介绍一下自己" },
//     ])) {
//       switch (event.type) {
//         case "content":
//           setAgentOutput((current) => ({
//             ...current,
//             status: "streaming",
//             content: current.content + event.content,
//           }));
//           break;

//         case "finish":
//           setAgentOutput((current) => ({
//             ...current,
//             status: "complete",
//             finishReason: event.reason,
//           }));
//           break;
//       }
//     }
//   } catch (cause) {
//     if (cause instanceof DeepSeekError) {
//       setAgentOutput((current) => ({
//         ...current,
//         status: current.content.length > 0 ? "incomplete" : "failed",
//         error: `[${cause.code}] ${cause.message}`,
//       }));
//       return;
//     }

//     setAgentOutput((current) => ({
//       ...current,
//       status: current.content.length > 0 ? "incomplete" : "failed",
//       error: "发生未知错误",
//     }));
//   } finally {
//     isRunningRef.current = false;
//   }
// }
