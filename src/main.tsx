import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './app/App'
import 'prosemirror-view/style/prosemirror.css'
import './index.css'

const root = document.getElementById('root')
if (!root) throw new Error('#root がありません')

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
