<template>
  <div
    class="container"
    :class="{ isDark: isDark, activeSidebar: activeSidebar }"
  >
    <FileHome
      v-if="show && screen === 'home' && !isDesktop()"
      @open="enterEditor"
    ></FileHome>
    <DesktopHome
      v-else-if="show && screen === 'home'"
      @open="enterEditor"
    ></DesktopHome>
    <template v-if="show && screen === 'edit'">
      <Toolbar v-if="!isZenMode"></Toolbar>
      <Edit></Edit>
    </template>
    <Updater></Updater>
    <GithubSync></GithubSync>
  </div>
</template>

<script>
import Toolbar from './components/Toolbar.vue'
import Edit from './components/Edit.vue'
import FileHome from './components/FileHome.vue'
import DesktopHome from './components/DesktopHome.vue'
import Updater from './components/Updater.vue'
import GithubSync from './components/GithubSync.vue'
import { isDesktop } from '@/utils/desktop'
import { mapState, mapMutations } from 'vuex'
import { getData, getLocalConfig } from '@/api'
import { browserFileLibrary, seedDemoDocument } from '@/utils/fileLibrary'
import {
  isMermaidStyleOn,
  mermaidSourceFor,
  renderMermaidSvg
} from '@/utils/mermaidStyle'
import {
  mindMapPng,
  shrinkThumbnail,
  svgMarkupToThumbnail
} from '@/utils/thumbnail'

const STYLE_COMMANDS = new Set([
  'SET_NODE_STYLE',
  'SET_NODE_STYLES',
  'SET_NODE_SHAPE'
])
const MERMAID_KEYS = [
  'mermaidStyle',
  'mermaidSource',
  'layoutBeforeMermaid',
  'themeBeforeMermaid'
]
// 保存文档时除节点树外需要原样保留的字段
const PASS_THROUGH_KEYS = ['demo', ...MERMAID_KEYS]

export default {
  components: {
    Toolbar,
    Edit,
    FileHome,
    DesktopHome,
    Updater,
    GithubSync
  },
  data() {
    return {
      show: false,
      screen: 'home',
      mindMap: null,
      previewMindMap: null,
      previewTimer: null,
      previewToken: 0,
      restoreThemeConfig: null,
      restoreExec: null
    }
  },
  computed: {
    ...mapState({
      isZenMode: state => state.localConfig.isZenMode,
      isDark: state => state.localConfig.isDark,
      activeSidebar: state => state.activeSidebar
    })
  },
  watch: {
    isDark() {
      this.setBodyDark()
    }
  },
  async created() {
    this.screen = 'home'
    this.$bus.$on('mindMapReady', this.onMindMapReady)
    this.$bus.$on('showFileHome', this.leaveEditor)
    this.$bus.$on('enterEditor', this.enterEditor)
    this.$bus.$on('saveMindMap', this.saveCurrent)
    window.addEventListener('keydown', this.onKeydownSave, true)
    // 推送到 Gist 时用编辑器里的最新数据，而不是磁盘上可能过期的那一份
    window.__desktopDocumentData = () => {
      if (this.screen === 'edit' && this.mindMap) {
        return this.documentData()
      }
      if (
        this.screen === 'edit' &&
        window.__desktopSession &&
        window.__desktopSession.mindMapData
      ) {
        return window.__desktopSession.mindMapData
      }
      return null
    }
    this.initLocalConfig()
    this.openDemoOnFirstVisit()
    const loading = this.$loading({
      lock: true,
      text: this.$t('other.loading')
    })
    this.show = true
    loading.close()
    this.setBodyDark()
  },
  beforeUnmount() {
    clearTimeout(this.previewTimer)
    this.unbindPreview()
    this.$bus.$off('mindMapReady', this.onMindMapReady)
    this.$bus.$off('showFileHome', this.leaveEditor)
    this.$bus.$off('enterEditor', this.enterEditor)
    this.$bus.$off('saveMindMap', this.saveCurrent)
    window.removeEventListener('keydown', this.onKeydownSave, true)
    if (window.__desktopDocumentData) {
      delete window.__desktopDocumentData
    }
  },
  methods: {
    ...mapMutations(['setLocalConfig']),
    isDesktop,

    enterEditor() {
      this.screen = 'edit'
    },

    // 首次打开网页时直接进入内置示例导图
    openDemoOnFirstVisit() {
      if (
        window.takeOverApp ||
        typeof window.__TAURI_INTERNALS__ !== 'undefined' ||
        this.$route.query.fileURL
      ) {
        return
      }
      try {
        if (seedDemoDocument(localStorage)) {
          this.screen = 'edit'
        }
      } catch (error) {
        console.log(error)
      }
    },

    // Cmd / Ctrl + S 保存当前文件
    onKeydownSave(e) {
      if (!(e.metaKey || e.ctrlKey) || e.altKey) {
        return
      }
      if (String(e.key).toLowerCase() !== 's') {
        return
      }
      if (this.screen !== 'edit') {
        return
      }
      e.preventDefault()
      this.saveCurrent()
    },

    // 保存当前文件：桌面 App 写回原文件，浏览器写回文件库
    async saveCurrent() {
      if (!this.mindMap || this.screen !== 'edit') {
        return false
      }
      const data = this.documentData()

      // 1. 桌面 App
      if (window.__TAURI_INTERNALS__ && window.__desktopSession) {
        if (!window.__desktopSession.path) {
          this.$message.info(this.$t('toolbar.saveNeedPath'))
          return false
        }
        try {
          window.__desktopSession.mindMapData = data
          await window.__TAURI_INTERNALS__.invoke('save_mind_map', { data })
          this.$message.success(this.$t('toolbar.saved'))
          return true
        } catch (error) {
          console.log(error)
          this.$message.error(this.$t('toolbar.saveFailed'))
          return false
        }
      }

      // 2. 通过文件系统访问 API 打开的本地 .smm 文件
      if (this.$store.state.isHandleLocalFile) {
        this.$bus.$emit('write_local_file', data)
        this.$message.success(this.$t('toolbar.saved'))
        return true
      }

      // 3. 浏览器文件库
      const library = browserFileLibrary()
      const id = library.currentId()
      if (!id) {
        this.$message.info(this.$t('toolbar.saveNeedPath'))
        return false
      }
      try {
        library.save(id, data)
        this.$message.success(this.$t('toolbar.saved'))
        return true
      } catch (error) {
        console.log(error)
        this.$message.error(this.$t('toolbar.saveFailed'))
        return false
      }
    },

    onMindMapReady(mindMap) {
      this.mindMap = mindMap
      this.bindPreview(mindMap)
    },

    bindPreview(mindMap) {
      this.unbindPreview()
      this.previewMindMap = mindMap
      this.onThemeChange = () => this.schedulePreview()
      this.onLayoutChange = () => this.schedulePreview()
      mindMap.on('view_theme_change', this.onThemeChange)
      mindMap.on('layout_change', this.onLayoutChange)
      this.$bus.$on('mermaidStyleChanged', this.schedulePreview)
      this.$bus.$on('mermaidPreviewReady', this.schedulePreview)
      this.$bus.$on('mapStyleChanged', this.schedulePreview)
      this.$bus.$on('painter_end', this.schedulePreview)
      const rawConfig = mindMap.setThemeConfig.bind(mindMap)
      mindMap.setThemeConfig = (...args) => {
        const result = rawConfig(...args)
        this.schedulePreview()
        return result
      }
      this.restoreThemeConfig = () => {
        mindMap.setThemeConfig = rawConfig
      }
      const rawExec = mindMap.execCommand.bind(mindMap)
      mindMap.execCommand = (...args) => {
        const result = rawExec(...args)
        if (STYLE_COMMANDS.has(args[0])) {
          this.schedulePreview()
        }
        return result
      }
      this.restoreExec = () => {
        mindMap.execCommand = rawExec
      }
    },

    unbindPreview() {
      clearTimeout(this.previewTimer)
      this.previewTimer = null
      const mindMap = this.previewMindMap
      if (this.restoreThemeConfig) {
        this.restoreThemeConfig()
        this.restoreThemeConfig = null
      }
      if (this.restoreExec) {
        this.restoreExec()
        this.restoreExec = null
      }
      if (mindMap && this.onThemeChange) {
        mindMap.off('view_theme_change', this.onThemeChange)
        mindMap.off('layout_change', this.onLayoutChange)
      }
      this.previewMindMap = null
      this.$bus.$off('mermaidStyleChanged', this.schedulePreview)
      this.$bus.$off('mermaidPreviewReady', this.schedulePreview)
      this.$bus.$off('mapStyleChanged', this.schedulePreview)
      this.$bus.$off('painter_end', this.schedulePreview)
    },

    schedulePreview() {
      if (this.screen !== 'edit' || !this.mindMap) {
        return
      }
      clearTimeout(this.previewTimer)
      this.previewTimer = setTimeout(() => {
        this.previewTimer = null
        this.storePreview()
      }, 500)
    },

    async makeThumbnail() {
      if (!this.mindMap) {
        return ''
      }
      const token = ++this.previewToken
      const mermaid = isMermaidStyleOn()
      try {
        let source = ''
        if (mermaid) {
          const markup = await renderMermaidSvg(mermaidSourceFor(this.mindMap))
          source = await svgMarkupToThumbnail(markup)
        } else {
          const png = await mindMapPng(this.mindMap)
          if (typeof png === 'string' && png.indexOf('data:') === 0) {
            source = await shrinkThumbnail(png)
          }
        }
        if (token !== this.previewToken || isMermaidStyleOn() !== mermaid) {
          return ''
        }
        return source
      } catch (error) {
        console.log(error)
        return ''
      }
    },

    async storePreview() {
      const image = await this.makeThumbnail()
      if (!image || this.screen !== 'edit') {
        return
      }
      try {
        await this.storeThumbnail(image)
      } catch (error) {
        console.log(error)
      }
    },

    documentData() {
      const data = this.mindMap.getData(true)
      const stored = getData() || {}
      PASS_THROUGH_KEYS.forEach(key => {
        if (Object.prototype.hasOwnProperty.call(stored, key)) {
          data[key] = stored[key]
        }
      })
      return data
    },

    async storeThumbnail(image) {
      if (
        window.__TAURI_INTERNALS__ &&
        window.__desktopSession &&
        window.__desktopSession.path
      ) {
        await window.__TAURI_INTERNALS__.invoke('save_thumbnail', {
          path: window.__desktopSession.path,
          image
        })
        return
      }
      const library = browserFileLibrary()
      const id = library.currentId()
      if (id) {
        library.saveThumbnail(id, image)
      }
    },

    async leaveEditor() {
      clearTimeout(this.previewTimer)
      this.previewTimer = null
      if (window.__TAURI_INTERNALS__ && window.__desktopSession) {
        if (this.mindMap && window.__desktopSession.path) {
          const data = this.documentData()
          window.__desktopSession.mindMapData = data
          const invoke = window.__TAURI_INTERNALS__.invoke
          try {
            await invoke('save_mind_map', { data })
          } catch (error) {
            console.log(error)
          }
          try {
            const image = await this.makeThumbnail()
            if (image) {
              await invoke('save_thumbnail', {
                path: window.__desktopSession.path,
                image
              })
            }
          } catch (error) {
            console.log(error)
          }
        }
        this.unbindPreview()
        this.mindMap = null
        this.screen = 'home'
        return
      }
      const library = browserFileLibrary()
      const id = library.currentId()
      if (id && this.mindMap) {
        const data = this.documentData()
        let thumbnail = ''
        try {
          thumbnail = await this.makeThumbnail()
        } catch (error) {
          thumbnail = ''
        }
        try {
          if (thumbnail) {
            library.save(id, data, thumbnail)
          } else {
            library.save(id, data)
          }
        } catch (error) {
          try {
            library.save(id, data)
          } catch (again) {
            console.log(again)
          }
        }
      }
      this.unbindPreview()
      this.mindMap = null
      this.screen = 'home'
    },

    initLocalConfig() {
      let config = getLocalConfig()
      if (config) {
        this.setLocalConfig({
          ...this.$store.state.localConfig,
          ...config
        })
      }
    },

    setBodyDark() {
      // body.isDark 管项目自己的样式，html.dark 让 Element Plus 切到它的暗色变量
      document.body.classList.toggle('isDark', this.isDark)
      document.documentElement.classList.toggle('dark', this.isDark)
    }
  }
}
</script>

<style lang="less">
/* 深色主题统一在 src/style/dark.less 中维护，这里只保留容器本身 */
.container {
}
</style>
