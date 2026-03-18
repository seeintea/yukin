import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { getApiUrl } from '../config/api'

export const Route = createFileRoute('/')({
  component: Index,
})

function Index() {
  const [healthStatus, setHealthStatus] = useState<string>('Checking...')
  const [apiUrl, setApiUrl] = useState<string>('')

  useEffect(() => {
    const url = getApiUrl('/health')
    setApiUrl(url)

    fetch(url)
      .then((res) => res.json())
      .then((data) => setHealthStatus(`Backend status: ${data.status}`))
      .catch((err) => setHealthStatus(`Backend status: offline (${err.message})`))
  }, [])

  return (
    <div style={{ padding: '10px 0' }}>
      <h2>首页</h2>
      <p>POC 初始化完成</p>
      <p>API: {apiUrl}</p>
      <p>{healthStatus}</p>
    </div>
  )
}
