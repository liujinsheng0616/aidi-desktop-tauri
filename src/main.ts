import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";

createApp(App).mount("#app");

import { trackDesktopStartup } from "./services/tracker";
void trackDesktopStartup();
