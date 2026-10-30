<template>
  <div class="desktopHome" :class="{ isDark: isDark }">
    <aside class="sidebar">
      <div class="brand">Mind</div>
      <button
        type="button"
        class="navItem"
        :class="{
          active: view === 'folder' && !currentPath,
          dragOver: dragOverPath === rootPath
        }"
        :data-drop-path="rootPath"
        @click="goAll"
      >
        <span class="icon iconfont iconwenjian"></span>
        {{ $t('home.all') }}
      </button>
      <button
        type="button"
        class="navItem"
        :class="{ active: view === 'recent' }"
        @click="showRecent"
      >
        <span class="icon iconfont iconshijian"></span>
        {{ $t('home.recent') }}
      </button>

      <div class="sectionTitle">{{ $t('home.folders') }}</div>
      <div class="folderNav">
        <button
          v-for="folder in rootFolders"
          :key="folder.path"
          type="button"
          class="navItem"
          :class="{
            active: activeFolder === folder.path,
            dragOver: dragOverPath === folder.path
          }"
          :data-drop-path="folder.path"
          @click="openFolder(folder)"
        >
          <span class="icon iconfont iconwenjianjia"></span>
          <span class="label">{{ folder.name }}</span>
        </button>
        <p class="hint" v-if="!rootFolders.length">
          {{ $t('home.noFolders') }}
        </p>
      </div>

      <div class="spacer"></div>
      <button type="button" class="navItem subtle" @click="openGithub">
        <span class="icon iconfont icongithub"></span>
        <span class="label">{{ githubLabel }}</span>
      </button>
      <button type="button" class="navItem subtle" @click="reveal">
        <span class="icon iconfont icondakai"></span>
        {{ $t('home.reveal') }}
      </button>
    </aside>

    <main class="main">
      <header class="topbar">
        <div class="crumbs" v-if="view === 'folder' && !query">
          <template
            v-for="(crumb, index) in listing.breadcrumb"
            :key="crumb.path"
          >
            <span class="sep" v-if="index">/</span>
            <button
              type="button"
              class="crumb"
              :class="{
                current: index === listing.breadcrumb.length - 1,
                dragOver: dragOverPath === crumb.path
              }"
              :data-drop-path="crumb.path"
              @click="openPath(crumb.path)"
            >
              {{ index === 0 ? $t('home.all') : crumb.name }}
            </button>
          </template>
        </div>
        <div class="crumbs" v-else>
          <span class="crumb current">{{
            query ? $t('home.search') : $t('home.recent')
          }}</span>
        </div>

        <div class="actions">
          <label class="search">
            <span class="icon iconfont iconsousuo"></span>
            <input
              v-model="query"
              :placeholder="$t('home.search')"
              @input="onSearchInput"
            />
            <button
              v-if="query"
              type="button"
              class="clear"
              @click="clearSearch"
            >
              ×
            </button>
          </label>
          <span v-if="version" class="appVersion">v{{ version }}</span>
          <button
            v-if="!isMac()"
            type="button"
            class="secondary"
            @click="$bus.$emit('checkUpdate')"
          >
            {{ $t('updater.check') }}
          </button>
          <button type="button" class="secondary" @click="openExternal">
            {{ $t('home.open') }}
          </button>
          <button type="button" class="primary" @click="createFile">
            {{ $t('home.new') }}
          </button>
          <button type="button" class="secondary" @click="createFolder">
            {{ $t('home.newFolder') }}
          </button>
        </div>
      </header>

      <section class="content">
        <div class="grid" v-if="!loading">
          <article
            v-for="folder in visibleFolders"
            :key="folder.path"
            class="card folderCard"
            :class="{ dragOver: dragOverPath === folder.path }"
            :data-drop-path="folder.path"
            @click="openFolder(folder)"
          >
            <span class="folderGlyph"></span>
            <div class="folderHead">
              <input
                v-if="renamingPath === folder.path"
                ref="renameInput"
                v-model="renameValue"
                class="renameInput"
                @click.stop
                @keydown.enter.stop.prevent="commitRename(folder)"
                @keydown.esc.stop.prevent="cancelRename"
                @blur="commitRename(folder)"
              />
              <template v-else>
                <div class="folderName" :title="folder.name">
                  {{ folder.name }}
                </div>
                <el-dropdown
                  class="folderMenu"
                  trigger="click"
                  @command="command => onFolderCommand(command, folder)"
                >
                  <button
                    type="button"
                    class="more folderMore"
                    @click.stop
                    aria-label="menu"
                  >
                    <span></span><span></span><span></span>
                  </button>
                  <template #dropdown>
                    <el-dropdown-menu>
                      <el-dropdown-item command="rename">{{
                        $t('home.rename')
                      }}</el-dropdown-item>
                      <el-dropdown-item command="delete">{{
                        $t('home.delete')
                      }}</el-dropdown-item>
                    </el-dropdown-menu>
                  </template>
                </el-dropdown>
              </template>
            </div>
          </article>

          <article
            v-for="file in visibleFiles"
            :key="file.path"
            class="card fileCard"
            :class="{ dragging: draggingPath === file.path }"
            @mousedown="onPointerDown($event, file)"
            @dragstart.prevent
            @click="openFile(file)"
          >
            <div class="thumb">
              <img
                v-if="file.thumbnail"
                :src="file.thumbnail"
                alt=""
                draggable="false"
              />
              <span v-else class="placeholder iconfont iconwenjian"></span>
              <el-dropdown
                class="menu"
                trigger="click"
                @command="command => onCommand(command, file)"
              >
                <button
                  type="button"
                  class="more"
                  @click.stop
                  aria-label="menu"
                >
                  <span></span><span></span><span></span>
                </button>
                <template #dropdown>
                  <el-dropdown-menu>
                    <el-dropdown-item command="rename">{{
                      $t('home.rename')
                    }}</el-dropdown-item>
                    <el-dropdown-item command="move">{{
                      $t('home.moveTo')
                    }}</el-dropdown-item>
                    <el-dropdown-item command="delete">{{
                      $t('home.delete')
                    }}</el-dropdown-item>
                  </el-dropdown-menu>
                </template>
              </el-dropdown>
            </div>
            <div class="meta">
              <input
                v-if="renamingPath === file.path"
                ref="renameInput"
                v-model="renameValue"
                class="renameInput"
                @click.stop
                @keydown.enter.stop.prevent="commitRename(file)"
                @keydown.esc.stop.prevent="cancelRename"
                @blur="commitRename(file)"
              />
              <template v-else>
                <div class="nameRow">
                  <div class="name" :title="file.name">{{ file.name }}</div>
                  <button
                    type="button"
                    class="nameEdit"
                    :title="$t('home.rename')"
                    @click.stop="beginRename(file, 'file')"
                    @mousedown.stop
                  >
                    <span class="iconfont iconbianji"></span>
                  </button>
                </div>
                <div class="time">{{ formatTime(file.modifiedMs) }}</div>
              </template>
            </div>
          </article>
        </div>

        <div class="empty" v-if="!loading && isEmpty">
          <span
            class="iconfont"
            :class="query ? 'iconsousuo' : 'iconwenjian'"
          ></span>
          <p>{{ query ? $t('home.noResults') : $t('home.emptyFolder') }}</p>
        </div>
      </section>
    </main>

    <el-dialog
      v-model="moveVisible"
      :title="$t('home.moveTo')"
      width="440px"
      append-to-body
    >
      <div class="moveCrumbs">
        <template
          v-for="(crumb, index) in moveListing.breadcrumb"
          :key="crumb.path"
        >
          <span class="sep" v-if="index">/</span>
          <button type="button" class="crumb" @click="loadMoveDir(crumb.path)">
            {{ index === 0 ? $t('home.all') : crumb.name }}
          </button>
        </template>
      </div>
      <div class="moveList">
        <button
          v-for="folder in moveListing.folders"
          :key="folder.path"
          type="button"
          class="moveItem"
          @click="loadMoveDir(folder.path)"
        >
          <span class="icon iconfont iconwenjianjia"></span>
          {{ folder.name }}
        </button>
        <p class="hint" v-if="!moveListing.folders.length">
          {{ $t('home.noFolders') }}
        </p>
      </div>
      <template #footer>
        <el-button @click="moveVisible = false">
          {{ $t('home.cancel') }}
        </el-button>
        <el-button type="primary" :disabled="!canMoveHere" @click="confirmMove">
          {{ $t('home.moveHere') }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script>
import { mapState } from 'vuex'
import { isMac, invoke } from '@/utils/desktop'

// 桌面端首页：直接以磁盘目录为结构，不维护额外索引
export default {
  data() {
    return {
      view: 'folder',
      listing: { path: '', breadcrumb: [], folders: [], files: [] },
      rootFolders: [],
      recentFiles: [],
      query: '',
      searchResults: [],
      loading: false,
      moveVisible: false,
      movingFile: null,
      movePath: null,
      moveListing: { path: '', breadcrumb: [], folders: [] },
      searchTimer: null,
      rootPath: '',
      version: '',
      githubStatus: null,
      draggingPath: '',
      dragOverPath: '',
      pointerDrag: null,
      suppressClick: false,
      ghost: null,
      renamingPath: '',
      renameValue: '',
      renamingKind: '',
      renaming: false
    }
  },
  computed: {
    ...mapState({
      isDark: state => state.localConfig.isDark
    }),

    currentPath() {
      const crumbs = this.listing.breadcrumb || []
      return crumbs.length > 1 ? this.listing.path : ''
    },

    activeFolder() {
      return this.view === 'folder' ? this.currentPath : ''
    },

    visibleFolders() {
      if (this.query || this.view !== 'folder') {
        return []
      }
      return this.listing.folders || []
    },

    visibleFiles() {
      if (this.query) {
        return this.searchResults
      }
      return this.view === 'folder'
        ? this.listing.files || []
        : this.recentFiles
    },

    isEmpty() {
      return !this.visibleFolders.length && !this.visibleFiles.length
    },

    githubLabel() {
      const status = this.githubStatus
      if (status && status.signedIn && status.login) {
        return this.$t('github.statusOn', { login: status.login })
      }
      return this.$t('github.statusOff')
    },

    canMoveHere() {
      // movePath 为空说明目录没加载成功，此时不能落点，否则会被当成根目录
      return Boolean(
        this.movingFile &&
        this.movePath &&
        this.movePath !== this.dirOf(this.movingFile.path) &&
        this.movePath !== this.movingFile.path
      )
    }
  },
  created() {
    this.reload()
    this.loadGithub()
    this.$bus.$on('githubSyncChanged', this.loadGithub)
    // macOS 的版本号和更新入口都在应用菜单里，界面不再重复
    if (!isMac()) {
      invoke('desktop_version')
        .then(version => {
          this.version = version
        })
        .catch(error => {
          console.log(error)
        })
    }
  },
  mounted() {
    // WKWebView 不支持页面内的 HTML5 拖拽，统一改成指针事件实现
    window.addEventListener('mousemove', this.onPointerMove)
    window.addEventListener('mouseup', this.onPointerUp)
    window.addEventListener('blur', this.cancelPointerDrag)
  },
  beforeUnmount() {
    clearTimeout(this.searchTimer)
    this.$bus.$off('githubSyncChanged', this.loadGithub)
    window.removeEventListener('mousemove', this.onPointerMove)
    window.removeEventListener('mouseup', this.onPointerUp)
    window.removeEventListener('blur', this.cancelPointerDrag)
    this.cancelPointerDrag()
  },
  methods: {
    isMac,

    dirOf(path) {
      const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
      return index > 0 ? path.slice(0, index) : ''
    },

    formatTime(value) {
      if (!value) {
        return ''
      }
      return new Date(value).toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
      })
    },

    async reload() {
      // loadDir 会把视图切回目录，先记住当前是不是「最近打开」
      const recent = this.view === 'recent'
      await Promise.all([
        this.loadDir(this.listing.path || null),
        this.loadRoot()
      ])
      if (recent) {
        this.view = 'recent'
        await this.loadRecent()
      }
    },

    async loadDir(path) {
      this.loading = true
      try {
        const listing = await invoke('library_list', { dir: path || null })
        this.listing = listing
        this.view = 'folder'
      } catch (error) {
        this.notify(error)
      } finally {
        this.loading = false
      }
    },

    async loadRoot() {
      try {
        const listing = await invoke('library_list', { dir: null })
        this.rootFolders = listing.folders || []
        this.rootPath = listing.path
      } catch (error) {
        console.log(error)
        this.rootFolders = []
      }
    },

    async loadRecent() {
      try {
        this.recentFiles = (await invoke('library_recent')) || []
      } catch (error) {
        console.log(error)
        this.recentFiles = []
      }
    },

    openPath(path) {
      this.query = ''
      this.loadDir(path)
    },

    goAll() {
      this.query = ''
      this.loadDir(null)
    },

    openFolder(folder) {
      this.query = ''
      this.loadDir(folder.path)
    },

    async showRecent() {
      this.query = ''
      this.view = 'recent'
      await this.loadRecent()
    },

    async openFile(file) {
      if (this.suppressClick) {
        this.suppressClick = false
        return
      }
      try {
        const session = await invoke('open_path', { path: file.path })
        if (window.__desktopShow) {
          window.__desktopShow(session)
        }
        this.$emit('open')
      } catch (error) {
        this.notify(error)
      }
    },

    async createFile() {
      try {
        const dir = this.view === 'folder' ? this.listing.path : null
        const session = await invoke('library_create_file', {
          dir: dir || null
        })
        if (window.__desktopShow) {
          window.__desktopShow(session)
        }
        this.$emit('open')
      } catch (error) {
        this.notify(error)
      }
    },

    // 打开文档目录之外的文件：走系统文件选择器
    async openExternal() {
      try {
        const session = await invoke('open_file')
        if (!session) {
          return
        }
        if (window.__desktopShow) {
          window.__desktopShow(session)
        }
        this.$emit('open')
      } catch (error) {
        this.notify(error)
      }
    },

    async createFolder() {
      let name = ''
      try {
        const result = await this.$prompt(
          this.$t('home.name'),
          this.$t('home.newFolder'),
          {
            confirmButtonText: this.$t('home.confirm'),
            cancelButtonText: this.$t('home.cancel')
          }
        )
        name = String(result.value || '').trim()
      } catch (error) {
        return
      }
      if (!name) {
        return
      }
      try {
        await invoke('library_create_folder', {
          dir: this.view === 'folder' ? this.listing.path : null,
          name
        })
        await Promise.all([
          this.loadDir(this.listing.path || null),
          this.loadRoot()
        ])
      } catch (error) {
        this.notify(error)
      }
    },

    onCommand(command, file) {
      if (command === 'rename') {
        this.beginRename(file, 'file')
      } else if (command === 'move') {
        this.openMove(file)
      } else if (command === 'delete') {
        this.removeEntry(file, 'file')
      }
    },

    onFolderCommand(command, folder) {
      if (command === 'rename') {
        this.beginRename(folder, 'folder')
      } else if (command === 'delete') {
        this.removeEntry(folder, 'folder')
      }
    },

    // 文件和文件夹共用一套就地改名状态，kind 只决定调用哪个命令
    beginRename(item, kind) {
      this.renamingPath = item.path
      this.renameValue = item.name
      this.renamingKind = kind
      this.$nextTick(() => {
        const refs = this.$refs.renameInput
        const input = Array.isArray(refs) ? refs[0] : refs
        if (input) {
          input.focus()
          input.select()
        }
      })
    },

    cancelRename() {
      this.renamingPath = ''
      this.renameValue = ''
      this.renamingKind = ''
    },

    async commitRename(item) {
      if (this.renaming || !this.renamingPath) {
        return
      }
      const name = this.renameValue.trim()
      if (!name || name === item.name) {
        this.cancelRename()
        return
      }
      this.renaming = true
      try {
        if (this.renamingKind === 'folder') {
          await invoke('library_rename_folder', { path: item.path, name })
        } else {
          await invoke('rename_file', { path: item.path, name })
        }
        this.cancelRename()
        await this.reload()
      } catch (error) {
        this.cancelRename()
        this.notify(error)
      } finally {
        this.renaming = false
      }
    },

    async removeEntry(item, kind) {
      const tip =
        kind === 'folder'
          ? this.$t('home.deleteFolderTip')
          : this.$t('home.deleteTip')
      try {
        await this.$confirm(tip, this.$t('home.delete'), {
          confirmButtonText: this.$t('home.confirm'),
          cancelButtonText: this.$t('home.cancel'),
          type: 'warning'
        })
      } catch (error) {
        return
      }
      try {
        if (kind === 'folder') {
          await invoke('library_delete_folder', { path: item.path })
        } else {
          await invoke('delete_file', { path: item.path })
        }
        await this.reload()
      } catch (error) {
        this.notify(error)
      }
    },

    onPointerDown(event, file) {
      // 菜单、改名输入框和铅笔按钮都不应该触发拖拽
      if (
        event.button !== 0 ||
        event.target.closest('.more, .renameInput, .nameEdit')
      ) {
        return
      }
      this.pointerDrag = {
        file,
        x: event.clientX,
        y: event.clientY,
        active: false
      }
    },

    onPointerMove(event) {
      const drag = this.pointerDrag
      if (!drag) {
        return
      }
      if (!drag.active) {
        // 移动超过阈值才算拖拽，普通点击仍然能打开文件
        if (Math.hypot(event.clientX - drag.x, event.clientY - drag.y) < 4) {
          return
        }
        drag.active = true
        this.draggingPath = drag.file.path
        this.suppressClick = true
        document.body.classList.add('fileDragging')
        this.createGhost(drag.file)
      }
      event.preventDefault()
      this.moveGhost(event)
      const target = document.elementFromPoint(event.clientX, event.clientY)
      const zone = target && target.closest('[data-drop-path]')
      this.dragOverPath = zone ? zone.getAttribute('data-drop-path') : ''
    },

    async onPointerUp() {
      const drag = this.pointerDrag
      this.pointerDrag = null
      if (!drag || !drag.active) {
        return
      }
      const target = this.dragOverPath
      this.draggingPath = ''
      this.dragOverPath = ''
      document.body.classList.remove('fileDragging')
      this.removeGhost()
      // 拖拽结束后浏览器还会补一个 click，这里吞掉，避免误打开文件
      window.setTimeout(() => {
        this.suppressClick = false
      }, 0)
      if (!target || this.dirOf(drag.file.path) === target) {
        return
      }
      try {
        await invoke('library_move', { path: drag.file.path, dir: target })
        await this.reload()
      } catch (error) {
        this.notify(error)
      }
    },

    cancelPointerDrag() {
      this.pointerDrag = null
      this.draggingPath = ''
      this.dragOverPath = ''
      this.suppressClick = false
      document.body.classList.remove('fileDragging')
      this.removeGhost()
    },

    createGhost(file) {
      const ghost = document.createElement('div')
      ghost.className = 'fileDragGhost'
      ghost.textContent = file.name
      document.body.appendChild(ghost)
      this.ghost = ghost
    },

    moveGhost(event) {
      if (!this.ghost) {
        return
      }
      this.ghost.style.left = `${event.clientX}px`
      this.ghost.style.top = `${event.clientY}px`
    },

    removeGhost() {
      if (this.ghost && this.ghost.parentNode) {
        this.ghost.parentNode.removeChild(this.ghost)
      }
      this.ghost = null
    },

    async openMove(file) {
      this.movingFile = file
      this.moveVisible = true
      await this.loadMoveDir(this.dirOf(file.path))
    },

    async loadMoveDir(path) {
      try {
        const listing = await invoke('library_list', { dir: path || null })
        this.moveListing = listing
        this.movePath = listing.path
      } catch (error) {
        this.notify(error)
      }
    },

    async confirmMove() {
      if (!this.movingFile || !this.movePath) {
        return
      }
      try {
        await invoke('library_move', {
          path: this.movingFile.path,
          dir: this.movePath || null
        })
        this.moveVisible = false
        await this.reload()
      } catch (error) {
        this.notify(error)
      }
    },

    onSearchInput() {
      clearTimeout(this.searchTimer)
      this.searchTimer = setTimeout(() => {
        this.runSearch()
      }, 250)
    },

    async runSearch() {
      const query = this.query.trim()
      if (!query) {
        this.searchResults = []
        return
      }
      try {
        this.searchResults = (await invoke('library_search', { query })) || []
      } catch (error) {
        console.log(error)
        this.searchResults = []
      }
    },

    clearSearch() {
      this.query = ''
      this.searchResults = []
    },

    reveal() {
      invoke('reveal_documents').catch(error => this.notify(error))
    },

    openGithub() {
      this.$bus.$emit('showGithubSync', 'github-sign-in')
    },

    async loadGithub() {
      try {
        this.githubStatus = await invoke('github_status', { path: '' })
      } catch (error) {
        console.log(error)
        this.githubStatus = null
      }
    },

    notify(error) {
      this.$message.error(
        typeof error === 'string' ? error : this.$t('home.actionFailed')
      )
    }
  }
}
</script>

<style lang="less" scoped src="./desktop-home.less"></style>
