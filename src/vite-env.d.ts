/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** "true" : force les handlers mockés même dans Tauri (`pnpm tauri:mock`). */
  readonly VITE_MOCK_IPC?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
