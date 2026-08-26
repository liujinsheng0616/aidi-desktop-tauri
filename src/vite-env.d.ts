/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

declare module "lucide-vue-next" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<any, any, any>;
  export default component;
  export const Palette: DefineComponent<any, any, any>;
  export const Sun: DefineComponent<any, any, any>;
  export const Moon: DefineComponent<any, any, any>;
  export const Monitor: DefineComponent<any, any, any>;
  export const Check: DefineComponent<any, any, any>;
  export const ChevronDown: DefineComponent<any, any, any>;
  export const X: DefineComponent<any, any, any>;
  export const Search: DefineComponent<any, any, any>;
  export const Send: DefineComponent<any, any, any>;
  export const ArrowRight: DefineComponent<any, any, any>;
  export const Cpu: DefineComponent<any, any, any>;
  export const HardDrive: DefineComponent<any, any, any>;
  export const MemoryStick: DefineComponent<any, any, any>;
  export const Activity: DefineComponent<any, any, any>;
  export const Trash2: DefineComponent<any, any, any>;
  export const Settings: DefineComponent<any, any, any>;
  export const RefreshCw: DefineComponent<any, any, any>;
  export const Power: DefineComponent<any, any, any>;
  export const Zap: DefineComponent<any, any, any>;
  export const Info: DefineComponent<any, any, any>;
  export const AlertTriangle: DefineComponent<any, any, any>;
  export const CheckCircle: DefineComponent<any, any, any>;
  export const XCircle: DefineComponent<any, any, any>;
  export const Loader: DefineComponent<any, any, any>;
  export const ChevronRight: DefineComponent<any, any, any>;
  export const ChevronUp: DefineComponent<any, any, any>;
  export const Minus: DefineComponent<any, any, any>;
  export const Plus: DefineComponent<any, any, any>;
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
