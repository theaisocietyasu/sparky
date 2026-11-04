/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Public https origin of the engine the web chat calls. Empty turns the chat off. */
  readonly VITE_ENGINE_URL?: string;
}
