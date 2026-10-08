import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './app/App'
import { CaptureWindow } from './capture/CaptureWindow'
import { isCaptureWindow } from './lib/platform'
import 'prosemirror-view/style/prosemirror.css'
import './index.css'

const root = document.getElementById('root')
if (!root) throw new Error('#root がありません')
// クイックキャプチャのウィンドウは背景を透かし、角の丸いパネルだけを見せる
if (isCaptureWindow()) document.documentElement.dataset.window = 'capture'

createRoot(root).render(
  <StrictMode>
    {isCaptureWindow() ? <CaptureWindow /> : <App />}
  </StrictMode>,
)
