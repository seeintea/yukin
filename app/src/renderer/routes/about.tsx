import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/about')({
  component: About,
})

function About() {
  return (
    <div style={{ padding: '10px 0' }}>
      <h2>关于</h2>
      <p>Yukin - Electron + Go 本地 AI 助手</p>
    </div>
  )
}
