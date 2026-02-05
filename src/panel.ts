import { createApp } from 'vue'
import './style.css'

createApp({
  template: `<div class="panel-container">
    <iframe src="https://aidi.yadea.com.cn/aigc/?lk_jump_to_browser=true"></iframe>
  </div>`,
}).mount('#app')
