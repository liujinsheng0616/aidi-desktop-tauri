import { createApp } from 'vue'
import OptimizerPanel from './components/OptimizerPanel.vue'
import './style.css'

createApp(OptimizerPanel).mount('#app')

import { initTracker } from "./services/tracker"
initTracker()
