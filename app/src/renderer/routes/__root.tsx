import { createRootRoute, Link, Outlet } from '@tanstack/react-router'
import { TanStackRouterDevtools } from '@tanstack/react-router-devtools'

export const Route = createRootRoute({
  component: () => (
    <>
      <div style={{ padding: '20px' }}>
        <h1>Yukin</h1>
        <p>Electron + Go Local AI Agent</p>
        <nav style={{ marginTop: '20px', marginBottom: '20px' }}>
          <Link to="/" style={{ marginRight: '10px' }}>
            首页
          </Link>
          <Link to="/about">
            关于
          </Link>
        </nav>
        <hr />
        <Outlet />
      </div>
      <TanStackRouterDevtools />
    </>
  ),
})
