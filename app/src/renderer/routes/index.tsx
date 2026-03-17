import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/')({
  component: Index,
})

function Index() {
  return (
    <div style={{ padding: '10px 0' }}>
      <h2>首页</h2>
      <p>POC 初始化完成</p>
    </div>
  )
}
