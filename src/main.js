import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'

// 很多 Tauri 模板自带了一个 style.css，如果你不需要它原有的样式，可以把它注释掉
// import './style.css'

const app = createApp(App)
const pinia = createPinia()

// 必须先 use(pinia)，然后再 mount('#app')
app.use(pinia)
app.mount('#app')