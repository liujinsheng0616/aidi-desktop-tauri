/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

interface ImportMetaEnv {
  readonly VITE_FS_APPID: string
  readonly VITE_FS_REDIRECT_URI: string
  readonly VITE_API_BASE_URL: string
  readonly VITE_BASIC_USERNAME: string
  readonly VITE_BASIC_PASSWORD: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
