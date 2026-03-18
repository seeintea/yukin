import { type ChildProcess, spawn } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { BrowserWindow, app } from 'electron'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

let mainWindow: BrowserWindow | null = null
let goProcess: ChildProcess | null = null

// 启动 Go 后端
function startGoBackend(): void {
  const isDev = process.env.NODE_ENV === 'development'
  let goBinaryPath: string

  if (isDev) {
    // 开发环境：使用项目目录下的二进制
    goBinaryPath = path.join(__dirname, '../../backend/cmd/server/main.go')
    // 开发时直接 go run，不编译
    goProcess = spawn('go', ['run', goBinaryPath], {
      cwd: path.join(__dirname, '../../backend'),
      stdio: 'inherit',
    })
  } else {
    // 生产环境：使用打包后的二进制
    goBinaryPath = path.join(process.resourcesPath, 'bin/agent')
    goProcess = spawn(goBinaryPath, [], {
      stdio: 'inherit',
    })
  }

  goProcess.on('error', (err: Error) => {
    console.error('Failed to start Go backend:', err)
  })
}

// 停止 Go 后端
function stopGoBackend(): void {
  if (goProcess) {
    goProcess.kill()
    goProcess = null
  }
}

function createWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    webPreferences: {
      preload: path.join(__dirname, '../preload/preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  })

  const isDev = process.env.NODE_ENV === 'development'
  if (isDev) {
    mainWindow.loadURL('http://localhost:5173')
    mainWindow.webContents.openDevTools()
  } else {
    mainWindow.loadFile(path.join(__dirname, '../dist/index.html'))
  }

  mainWindow.on('closed', () => {
    mainWindow = null
  })
}

app.whenReady().then(() => {
  startGoBackend()
  createWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow()
    }
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit()
  }
})

app.on('will-quit', () => {
  stopGoBackend()
})
