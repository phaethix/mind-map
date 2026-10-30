<template>
  <div class="toolbarContainer" :class="{ isDark: isDark }">
    <div class="toolbar" ref="toolbarRef">
      <!-- 导出 -->
      <div class="toolbarBlock">
        <div class="toolbarBtn" @click="$bus.$emit('showFileHome')">
          <span class="icon iconfont iconzhuye"></span>
          <span class="text">{{ $t('home.back') }}</span>
        </div>
        <div class="toolbarBtn" @click="openDirectory" v-if="!isMobile">
          <span class="icon iconfont icondakai"></span>
          <span class="text">{{ $t('toolbar.directory') }}</span>
        </div>
        <el-tooltip
          effect="dark"
          :content="$t('toolbar.newFileTip')"
          placement="bottom"
          v-if="!isMobile"
        >
          <div class="toolbarBtn" @click="createNewLocalFile">
            <span class="icon iconfont iconxinjian"></span>
            <span class="text">{{ $t('toolbar.newFile') }}</span>
          </div>
        </el-tooltip>
        <el-tooltip
          effect="dark"
          :content="$t('toolbar.openFileTip')"
          placement="bottom"
          v-if="!isMobile"
        >
          <div class="toolbarBtn" @click="openLocalFile">
            <span class="icon iconfont iconwenjian1"></span>
            <span class="text">{{ $t('toolbar.openFile') }}</span>
          </div>
        </el-tooltip>
        <el-tooltip
          effect="dark"
          :content="$t('toolbar.saveTip')"
          placement="bottom"
        >
          <div class="toolbarBtn" @click="saveCurrentFile">
            <span class="icon saveIcon">
              <svg
                viewBox="0 0 16 16"
                width="16"
                height="16"
                aria-hidden="true"
              >
                <path
                  d="M3.2 2.2h7.4l3 3v8.6a1 1 0 0 1-1 1H3.2a1 1 0 0 1-1-1V3.2a1 1 0 0 1 1-1z"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.3"
                  stroke-linejoin="round"
                />
                <path
                  d="M5 2.2v3.6h6V2.2M5 13.8V9.2h6v4.6"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.3"
                  stroke-linejoin="round"
                />
              </svg>
            </span>
            <span class="text">{{ $t('toolbar.save') }}</span>
          </div>
        </el-tooltip>
        <div class="toolbarBtn" @click="saveLocalFile" v-if="!isMobile">
          <span class="icon iconfont iconlingcunwei"></span>
          <span class="text">{{ $t('toolbar.saveAs') }}</span>
        </div>
        <div class="toolbarBtn" @click="$bus.$emit('showImport')">
          <span class="icon iconfont icondaoru"></span>
          <span class="text">{{ $t('toolbar.import') }}</span>
        </div>
        <div class="toolbarBtn" @click="$bus.$emit('showExport')">
          <span class="icon iconfont iconexport"></span>
          <span class="text">{{ $t('toolbar.export') }}</span>
        </div>
        <div
          class="toolbarBtn"
          v-if="isDesktopApp"
          @click="$bus.$emit('showGithubSync', 'github-push')"
        >
          <span class="icon iconfont icongithub"></span>
          <span class="text">{{ $t('github.short') }}</span>
        </div>
        <div class="toolbarBtn" @click="$bus.$emit('showMermaid')">
          <span class="icon mermaidIcon">
            <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
              <circle
                cx="3"
                cy="8"
                r="2"
                fill="none"
                stroke="currentColor"
                stroke-width="1.4"
              />
              <circle
                cx="12"
                cy="4"
                r="1.7"
                fill="none"
                stroke="currentColor"
                stroke-width="1.4"
              />
              <circle
                cx="12"
                cy="12"
                r="1.7"
                fill="none"
                stroke="currentColor"
                stroke-width="1.4"
              />
              <path
                d="M5 7.2 L10.2 4.6 M5 8.8 L10.2 11.4"
                fill="none"
                stroke="currentColor"
                stroke-width="1.4"
              />
            </svg>
          </span>
          <span class="text">{{ $t('toolbar.mermaid') }}</span>
        </div>
        <!-- 本地文件树 -->
        <div
          class="fileTreeBox"
          v-if="fileTreeVisible"
          :class="{ expand: fileTreeExpand }"
        >
          <div class="fileTreeToolbar">
            <div class="fileTreeName">
              {{ rootDirName ? '/' + rootDirName : '' }}
            </div>
            <div class="fileTreeActionList">
              <div
                class="btn"
                :class="[
                  fileTreeExpand ? 'el-icon-arrow-up' : 'el-icon-arrow-down'
                ]"
                @click="fileTreeExpand = !fileTreeExpand"
              ></div>
              <div
                class="btn el-icon-close"
                @click="fileTreeVisible = false"
              ></div>
            </div>
          </div>
          <div class="fileTreeWrap">
            <el-tree
              :props="fileTreeProps"
              :load="loadFileTreeNode"
              :expand-on-click-node="false"
              node-key="id"
              lazy
            >
              <template #default="{ node, data }">
                <span class="customTreeNode">
                  <div class="treeNodeInfo">
                    <span
                      class="treeNodeIcon iconfont"
                      :class="[
                        data.type === 'file' ? 'iconwenjian' : 'icondakai'
                      ]"
                    ></span>
                    <span class="treeNodeName">{{ node.label }}</span>
                  </div>
                  <div class="treeNodeBtnList" v-if="data.type === 'file'">
                    <el-button
                      type="text"
                      size="mini"
                      v-if="data.enableEdit"
                      @click="editLocalFile(data)"
                      >编辑</el-button
                    >
                    <el-button
                      type="text"
                      size="mini"
                      v-else
                      @click="importLocalFile(data)"
                      >导入</el-button
                    >
                  </div>
                </span>
              </template>
            </el-tree>
          </div>
        </div>
      </div>
      <!-- 节点操作 -->
      <div class="toolbarBlock">
        <ToolbarNodeBtnList :list="horizontalList"></ToolbarNodeBtnList>
        <!-- 更多 -->
        <el-popover
          v-model="popoverShow"
          placement="bottom-end"
          width="120"
          trigger="hover"
          v-if="showMoreBtn"
          :style="{ marginLeft: horizontalList.length > 0 ? '20px' : 0 }"
        >
          <ToolbarNodeBtnList
            dir="v"
            :list="verticalList"
            @click="popoverShow = false"
          ></ToolbarNodeBtnList>
          <template #reference>
            <div class="toolbarBtn">
              <span class="icon iconfont icongongshi"></span>
              <span class="text">{{ $t('toolbar.more') }}</span>
            </div>
          </template>
        </el-popover>
      </div>
    </div>
    <NodeImage></NodeImage>
    <NodeHyperlink></NodeHyperlink>
    <NodeIcon></NodeIcon>
    <NodeNote></NodeNote>
    <NodeTag></NodeTag>
    <Export></Export>
    <Import ref="ImportRef"></Import>
  </div>
</template>

<script>
import NodeImage from './NodeImage.vue'
import NodeHyperlink from './NodeHyperlink.vue'
import NodeIcon from './NodeIcon.vue'
import NodeNote from './NodeNote.vue'
import NodeTag from './NodeTag.vue'
import Export from './Export.vue'
import Import from './Import.vue'
import { mapState } from 'vuex'
import { ElNotification } from 'element-plus'
import exampleData from 'simple-mind-map/example/exampleData'
import { getData } from '../../../api'
import ToolbarNodeBtnList from './ToolbarNodeBtnList.vue'
import { browserFileLibrary } from '@/utils/fileLibrary'
import { isDesktop, invoke } from '@/utils/desktop'
import { throttle, isMobile } from 'simple-mind-map/src/utils/index'

// 工具栏
let fileHandle = null
const defaultBtnList = [
  'back',
  'forward',
  'painter',
  'siblingNode',
  'childNode',
  'deleteNode',
  'image',
  'icon',
  'link',
  'note',
  'tag',
  'summary',
  'associativeLine',
  'formula',
  // 'attachment',
  'outerFrame',
  'annotation',
  'ai'
]

export default {
  components: {
    NodeImage,
    NodeHyperlink,
    NodeIcon,
    NodeNote,
    NodeTag,
    Export,
    Import,
    ToolbarNodeBtnList
  },
  data() {
    return {
      isMobile: isMobile(),
      isDesktopApp: isDesktop(),
      horizontalList: [],
      verticalList: [],
      showMoreBtn: true,
      popoverShow: false,
      fileTreeProps: {
        label: 'name',
        children: 'children',
        isLeaf: 'leaf'
      },
      fileTreeVisible: false,
      rootDirName: '',
      fileTreeExpand: true,
      waitingWriteToLocalFile: false
    }
  },
  computed: {
    ...mapState({
      isDark: state => state.localConfig.isDark,
      isHandleLocalFile: state => state.isHandleLocalFile,
      openNodeRichText: state => state.localConfig.openNodeRichText,
      enableAi: state => state.localConfig.enableAi
    }),

    btnLit() {
      let res = [...defaultBtnList]
      if (!this.openNodeRichText) {
        res = res.filter(item => {
          return item !== 'formula'
        })
      }
      if (!this.enableAi) {
        res = res.filter(item => {
          return item !== 'ai'
        })
      }
      return res
    }
  },
  watch: {
    isHandleLocalFile(val) {
      if (!val) {
        ElNotification.closeAll()
      }
    },
    btnLit: {
      deep: true,
      handler() {
        this.computeToolbarShow()
      }
    }
  },
  created() {
    this.$bus.$on('write_local_file', this.onWriteLocalFile)
    // 桌面端 File → Rename… 菜单项
    this.$bus.$on('renameFile', this.renameFile)
  },
  mounted() {
    this.computeToolbarShow()
    this.computeToolbarShowThrottle = throttle(this.computeToolbarShow, 300)
    window.addEventListener('resize', this.computeToolbarShowThrottle)
    this.$bus.$on('lang_change', this.computeToolbarShowThrottle)
    window.addEventListener('beforeunload', this.onUnload)
    this.$bus.$on('node_note_dblclick', this.onNodeNoteDblclick)
  },
  beforeUnmount() {
    this.$bus.$off('write_local_file', this.onWriteLocalFile)
    this.$bus.$off('renameFile', this.renameFile)
    window.removeEventListener('resize', this.computeToolbarShowThrottle)
    this.$bus.$off('lang_change', this.computeToolbarShowThrottle)
    window.removeEventListener('beforeunload', this.onUnload)
    this.$bus.$off('node_note_dblclick', this.onNodeNoteDblclick)
  },
  methods: {
    // 当前文档名：桌面端来自会话，网页端来自文件库
    currentFileName() {
      if (isDesktop()) {
        const session = window.__desktopSession
        return (session && session.fileName) || ''
      }
      const library = browserFileLibrary()
      const id = library.currentId()
      const item = id ? library.list().find(file => file.id === id) : null
      return item ? item.name : ''
    },

    // 桌面端 File → Rename… 菜单项：在编辑页直接改名，不用退回首页
    async renameFile() {
      const current = this.currentFileName()
      if (!current) {
        return
      }
      let value = ''
      try {
        const result = await this.$prompt(
          this.$t('home.name'),
          this.$t('home.rename'),
          {
            inputValue: current,
            confirmButtonText: this.$t('home.confirm'),
            cancelButtonText: this.$t('home.cancel')
          }
        )
        value = String(result.value || '').trim()
      } catch (error) {
        return
      }
      if (!value || value === current) {
        return
      }
      try {
        if (isDesktop()) {
          const session = window.__desktopSession || {}
          const card = await invoke('rename_file', {
            path: session.path,
            name: value
          })
          session.path = card.path
          session.fileName = card.name
          document.title = card.name
        } else {
          const library = browserFileLibrary()
          const id = library.currentId()
          if (!id) {
            return
          }
          library.rename(id, value)
        }
        this.$message.success(this.$t('toolbar.renameSuccess'))
      } catch (error) {
        console.log(error)
        this.$message.error(
          typeof error === 'string' ? error : this.$t('toolbar.renameFailed')
        )
      }
    },

    // 计算工具按钮如何显示
    computeToolbarShow() {
      if (!this.$refs.toolbarRef) return
      const windowWidth = window.innerWidth - 40
      const all = [...this.btnLit]
      let index = 1
      const loopCheck = () => {
        if (index > all.length) return done()
        this.horizontalList = all.slice(0, index)
        this.$nextTick(() => {
          const width = this.$refs.toolbarRef.getBoundingClientRect().width
          if (width < windowWidth) {
            index++
            loopCheck()
          } else if (index > 0 && width > windowWidth) {
            index--
            this.horizontalList = all.slice(0, index)
            done()
          }
        })
      }
      const done = () => {
        this.verticalList = all.slice(index)
        this.showMoreBtn = this.verticalList.length > 0
      }
      loopCheck()
    },

    // 监听本地文件读写
    onWriteLocalFile(content) {
      clearTimeout(this.timer)
      if (fileHandle && this.isHandleLocalFile) {
        this.waitingWriteToLocalFile = true
      }
      this.timer = setTimeout(() => {
        this.writeLocalFile(content)
      }, 1000)
    },

    onUnload(e) {
      if (this.waitingWriteToLocalFile) {
        const msg = '存在未保存的数据'
        e.returnValue = msg
        return msg
      }
    },

    // 加载本地文件树
    async loadFileTreeNode(node, resolve) {
      try {
        let dirHandle
        if (node.level === 0) {
          dirHandle = await window.showDirectoryPicker()
          this.rootDirName = dirHandle.name
        } else {
          dirHandle = node.data.handle
        }
        const dirList = []
        const fileList = []
        for await (const [key, value] of dirHandle.entries()) {
          const isFile = value.kind === 'file'
          if (isFile && !/\.(smm|xmind|md|json)$/.test(value.name)) {
            continue
          }
          const enableEdit = isFile && /\.smm$/.test(value.name)
          const data = {
            id: key,
            name: value.name,
            type: value.kind,
            handle: value,
            leaf: isFile,
            enableEdit
          }
          if (isFile) {
            fileList.push(data)
          } else {
            dirList.push(data)
          }
        }
        resolve([...dirList, ...fileList])
      } catch (error) {
        console.log(error)
        this.fileTreeVisible = false
        resolve([])
        if (error.toString().includes('aborted')) {
          return
        }
        this.$message.warning(this.$t('toolbar.notSupportTip'))
      }
    },

    runDesktopAction(action) {
      if (
        window.__TAURI_INTERNALS__ &&
        typeof window.__desktopOnMenu === 'function'
      ) {
        window.__desktopOnMenu(action)
        return true
      }
      return false
    },

    // 扫描本地文件夹
    openDirectory() {
      if (this.runDesktopAction('reveal')) {
        return
      }
      this.fileTreeVisible = false
      this.fileTreeExpand = true
      this.rootDirName = ''
      this.$nextTick(() => {
        this.fileTreeVisible = true
      })
    },

    // 编辑指定文件
    editLocalFile(data) {
      if (data.handle) {
        fileHandle = data.handle
        this.readFile()
      }
    },

    // 导入指定文件
    async importLocalFile(data) {
      try {
        const file = await data.handle.getFile()
        this.$refs.ImportRef.onChange({
          raw: file,
          name: file.name
        })
        this.$refs.ImportRef.confirm()
      } catch (error) {
        console.log(error)
      }
    },

    // 打开本地文件
    async openLocalFile() {
      if (this.runDesktopAction('open')) {
        return
      }
      try {
        let [_fileHandle] = await window.showOpenFilePicker({
          types: [
            {
              description: '',
              accept: {
                'application/json': ['.smm']
              }
            }
          ],
          excludeAcceptAllOption: true,
          multiple: false
        })
        if (!_fileHandle) {
          return
        }
        fileHandle = _fileHandle
        if (fileHandle.kind === 'directory') {
          this.$message.warning(this.$t('toolbar.selectFileTip'))
          return
        }
        this.readFile()
      } catch (error) {
        console.log(error)
        if (error.toString().includes('aborted')) {
          return
        }
        this.$message.warning(this.$t('toolbar.notSupportTip'))
      }
    },

    // 读取本地文件
    async readFile() {
      let file = await fileHandle.getFile()
      let fileReader = new FileReader()
      fileReader.onload = async () => {
        this.$store.commit('setIsHandleLocalFile', true)
        this.setData(fileReader.result)
        ElNotification.closeAll()
        Notification({
          title: this.$t('toolbar.tip'),
          message: `${this.$t('toolbar.editingLocalFileTipFront')}${
            file.name
          }${this.$t('toolbar.editingLocalFileTipEnd')}`,
          duration: 0,
          showClose: true
        })
      }
      fileReader.readAsText(file)
    },

    // 渲染读取的数据
    setData(str) {
      try {
        let data = JSON.parse(str)
        if (typeof data !== 'object') {
          throw new Error(this.$t('toolbar.fileContentError'))
        }
        if (data.root) {
          this.isFullDataFile = true
        } else {
          this.isFullDataFile = false
          data = {
            ...exampleData,
            root: data
          }
        }
        this.$bus.$emit('setData', data)
      } catch (error) {
        console.log(error)
        this.$message.error(this.$t('toolbar.fileOpenFailed'))
      }
    },

    // 写入本地文件
    async writeLocalFile(content) {
      if (!fileHandle || !this.isHandleLocalFile) {
        this.waitingWriteToLocalFile = false
        return
      }
      if (!this.isFullDataFile) {
        content = content.root
      }
      let string = JSON.stringify(content)
      const writable = await fileHandle.createWritable()
      await writable.write(string)
      await writable.close()
      this.waitingWriteToLocalFile = false
    },

    // 创建本地文件
    async createNewLocalFile() {
      if (this.runDesktopAction('new')) {
        return
      }
      await this.createLocalFile(exampleData)
    },

    // 保存当前文件（由 Edit 页统一处理浏览器文件库 / 桌面 App / 本地文件句柄）
    saveCurrentFile() {
      this.$bus.$emit('saveMindMap')
    },

    // 另存为
    async saveLocalFile() {
      if (this.runDesktopAction('save-as')) {
        return
      }
      let data = getData()
      await this.createLocalFile(data)
    },

    // 创建本地文件
    async createLocalFile(content) {
      try {
        let _fileHandle = await window.showSaveFilePicker({
          types: [
            {
              description: '',
              accept: { 'application/json': ['.smm'] }
            }
          ],
          suggestedName: this.$t('toolbar.defaultFileName')
        })
        if (!_fileHandle) {
          return
        }
        const loading = this.$loading({
          lock: true,
          text: this.$t('toolbar.creatingTip'),
          spinner: 'el-icon-loading',
          background: 'rgba(0, 0, 0, 0.7)'
        })
        fileHandle = _fileHandle
        this.$store.commit('setIsHandleLocalFile', true)
        this.isFullDataFile = true
        await this.writeLocalFile(content)
        await this.readFile()
        loading.close()
      } catch (error) {
        console.log(error)
        if (error.toString().includes('aborted')) {
          return
        }
        this.$message.warning(this.$t('toolbar.notSupportTip'))
      }
    },

    onNodeNoteDblclick(node, e) {
      e.stopPropagation()
      this.$bus.$emit('showNodeNote', node)
    }
  }
}
</script>

<style lang="less" scoped>
.toolbarContainer {
  // 用 flex 居中，不用 left:50% + translateX(-50%)：
  // 固定定位元素一旦带 transform 就会被提升为合成层，宽度为奇数时 -50% 会落在半个像素上，
  // WebKit（桌面端 WKWebView）会按半像素合成，整条工具栏的文字就发虚。
  position: fixed;
  left: 0;
  right: 0;
  top: 20px;
  z-index: 2;
  display: flex;
  justify-content: center;
  pointer-events: none;

  &.isDark {
    .toolbar {
      color: hsla(0, 0%, 100%, 0.9);
      .toolbarBlock {
        background-color: #262a2e;
        border-color: hsla(0, 0%, 100%, 0.08);
        box-shadow: 0 2px 16px 0 rgba(0, 0, 0, 0.45);

        .fileTreeBox {
          background-color: #262a2e;
          box-shadow: 0 2px 16px 0 rgba(0, 0, 0, 0.45);

          .fileTreeToolbar {
            border-bottom-color: hsla(0, 0%, 100%, 0.1);
          }

          :deep(.el-tree) {
            background-color: #262a2e;

            &.el-tree--highlight-current {
              .el-tree-node.is-current > .el-tree-node__content {
                background-color: hsla(0, 0%, 100%, 0.05) !important;
              }
            }

            .el-tree-node:focus > .el-tree-node__content {
              background-color: hsla(0, 0%, 100%, 0.05) !important;
            }

            .el-tree-node__content:hover,
            .el-upload-list__item:hover {
              background-color: hsla(0, 0%, 100%, 0.02) !important;
            }
          }

          .fileTreeWrap {
            .customTreeNode {
              .treeNodeInfo {
                color: #fff;
              }

              .treeNodeBtnList {
                .el-button {
                  padding: 7px 5px;
                }
              }
            }
          }
        }
      }

      .toolbarBtn {
        .icon {
          background: transparent;
          border-color: transparent;
        }

        &:hover {
          &:not(.disabled) {
            .icon {
              background: hsla(0, 0%, 100%, 0.05);
            }
          }
        }

        &.disabled {
          color: #54595f;
        }
      }
    }
  }
  .toolbar {
    position: relative;
    width: max-content;
    flex-shrink: 0;
    display: flex;
    pointer-events: auto;
    font-size: 12px;
    // 用系统字体栈：-apple-system 在 macOS 上会走 SF Pro（小字号更清晰），中文自动回落到苹方
    font-family:
      -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Helvetica Neue', Arial,
      sans-serif;
    font-weight: 400;
    color: rgba(26, 26, 26, 0.9);

    .toolbarBlock {
      display: flex;
      background-color: #fff;
      padding: 10px 20px;
      border-radius: 6px;
      box-shadow: 0 2px 16px 0 rgba(0, 0, 0, 0.06);
      border: 1px solid rgba(0, 0, 0, 0.06);
      margin-right: 20px;
      flex-shrink: 0;
      position: relative;

      &:last-of-type {
        margin-right: 0;
      }

      .fileTreeBox {
        position: absolute;
        left: 0;
        top: 68px;
        width: 100%;
        height: 30px;
        background-color: #fff;
        padding: 12px 5px;
        padding-top: 0;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        border-radius: 5px;
        min-width: 200px;
        box-shadow: 0 2px 16px 0 rgba(0, 0, 0, 0.06);

        &.expand {
          height: 300px;

          .fileTreeWrap {
            visibility: visible;
          }
        }

        .fileTreeToolbar {
          width: 100%;
          height: 30px;
          flex-shrink: 0;
          display: flex;
          align-items: center;
          justify-content: space-between;
          border-bottom: 1px solid #e9e9e9;
          margin-bottom: 12px;
          padding-left: 12px;

          .fileTreeName {
          }

          .fileTreeActionList {
            .btn {
              font-size: 18px;
              margin-left: 12px;
              cursor: pointer;
            }
          }
        }

        .fileTreeWrap {
          width: 100%;
          height: 100%;
          overflow: auto;
          visibility: hidden;

          .customTreeNode {
            flex: 1;
            display: flex;
            align-items: center;
            justify-content: space-between;
            font-size: 13px;
            padding-right: 5px;

            .treeNodeInfo {
              display: flex;
              align-items: center;

              .treeNodeIcon {
                margin-right: 5px;
                opacity: 0.7;
              }

              .treeNodeName {
                max-width: 200px;
                overflow: hidden;
                text-overflow: ellipsis;
                white-space: nowrap;
              }
            }

            .treeNodeBtnList {
              display: flex;
              align-items: center;
            }
          }
        }
      }
    }

    .toolbarBtn {
      display: flex;
      justify-content: center;
      flex-direction: column;
      cursor: pointer;
      margin-right: 20px;

      &:last-of-type {
        margin-right: 0;
      }

      &:hover {
        &:not(.disabled) {
          .icon {
            background: #f5f5f5;
          }
        }
      }

      &.active {
        .icon {
          background: #f5f5f5;
        }
      }

      &.disabled {
        color: #bcbcbc;
        cursor: not-allowed;
        pointer-events: none;
      }

      .icon {
        display: flex;
        height: 26px;
        background: #fff;
        border-radius: 4px;
        border: 1px solid #e9e9e9;
        justify-content: center;
        flex-direction: column;
        text-align: center;
        padding: 0 5px;
      }

      .mermaidIcon {
        align-items: center;
        padding: 0;

        svg {
          display: block;
        }
      }

      .saveIcon {
        align-items: center;
        padding: 0;

        svg {
          display: block;
        }
      }

      .text {
        margin-top: 3px;
      }
    }
  }
}
</style>
