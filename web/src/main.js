import { createApp } from 'vue'
import {
  ElLoading,
  ElMessage,
  ElMessageBox,
  ElNotification
} from 'element-plus'
// 组件样式由 unplugin-vue-components 按需注入，这里只补“服务式”组件和指令用到的样式
import 'element-plus/es/components/message/style/css'
import 'element-plus/es/components/message-box/style/css'
import 'element-plus/es/components/notification/style/css'
import 'element-plus/es/components/loading/style/css'
// 深色模式的变量集，和整包 CSS 无关，单独引
import 'element-plus/theme-chalk/dark/css-vars.css'
import '@/assets/icon-font/iconfont.css'
import 'viewerjs/dist/viewer.css'
// 全局深色主题，必须在 element-plus / 第三方样式之后引入
import '@/style/dark.less'
// 基础字体与弹窗样式，同样需要覆盖 element-plus 的默认值
import '@/style/ui.less'
import VueViewer from 'v-viewer'
import App from './App.vue'
import router from './router'
import store from './store'
import i18n from './i18n'
import bus from '@/utils/bus'
import { globals } from '@/utils/global'
import { getLang } from '@/api'

// 服务式组件（this.$message / $confirm / $notify / $loading）挂到 globalProperties，
// 保留 Vue 2 时代的调用写法，避免几百处调用点跟着改。
const services = {
  $loading: ElLoading.service,
  $message: ElMessage,
  $msgbox: ElMessageBox,
  $alert: ElMessageBox.alert,
  $confirm: ElMessageBox.confirm,
  $prompt: ElMessageBox.prompt,
  $notify: ElNotification
}

// 打包后的桌面端没有控制台。未捕获异常、以及事件总线内部被 try/catch 吞掉的
// 错误，如果只写进 console，用户和我都看不到，界面会以「能看不能用」的方式坏掉。
// 这里把第一条错误弹出来（只弹一次、可关闭），当作现场证据。
let reportedFirstError = false
const reportFirstError = error => {
  if (reportedFirstError) return
  const text = String((error && (error.message || error.stack)) || error || '')
  if (!text) return
  reportedFirstError = true
  ElMessage({
    type: 'error',
    message: text.split('\n')[0].slice(0, 200),
    duration: 0,
    showClose: true
  })
}
window.addEventListener('error', event =>
  reportFirstError(event.error || event.message)
)
window.addEventListener('unhandledrejection', event =>
  reportFirstError(event.reason)
)
const rawConsoleError = console.error
console.error = (...args) => {
  reportFirstError(args.find(item => item instanceof Error) || args[0])
  rawConsoleError.apply(console, args)
}

const initApp = () => {
  const app = createApp(App)

  // 组件由 vite 插件按需注册，这里只需补上 v-loading 指令
  app.directive('loading', ElLoading.directive)
  app.use(router)
  app.use(store)
  app.use(i18n)
  app.use(VueViewer)

  app.config.globalProperties.$bus = bus
  app.config.globalProperties.getCurrentData = (...args) =>
    globals.getCurrentData(...args)
  Object.keys(services).forEach(name => {
    app.config.globalProperties[name] = services[name]
  })

  i18n.global.locale.value = getLang()
  app.mount('#app')
  window.$app = app
  return app
}

// 是否处于接管应用模式
if (window.takeOverApp) {
  window.initApp = initApp
  window.$bus = bus
} else {
  initApp()
}
