/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** "tauri" のとき、Tauri のアプリで core/ につながる TauriBackend を使う。それ以外は MockBackend */
  readonly VITE_BACKEND?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
