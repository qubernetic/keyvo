/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** "1" mounts the app against the in-memory mock (`just app-dev`). */
  readonly VITE_KEYVO_MOCK?: string;
}
