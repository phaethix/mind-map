<template>
  <el-dialog
    class="githubSyncDialog"
    :title="$t('github.title')"
    v-model="visible"
    width="560px"
    append-to-body
  >
    <div class="githubSync" :class="{ isDark: isDark }">
      <p class="privacy">{{ $t('github.privacy') }}</p>
      <p class="status">{{ statusText }}</p>
      <p class="hint" v-if="linkText">{{ linkText }}</p>

      <div class="device" v-if="waiting && userCode">
        <div class="userCodeLabel">{{ $t('github.userCodeLabel') }}</div>
        <code class="userCode">{{ userCode }}</code>
        <p class="hint uri">{{ verificationUri }}</p>
        <p class="hint" v-if="remaining">
          {{ $t('github.expires', { time: remaining }) }}
        </p>
        <p class="hint">{{ $t('github.waiting') }}</p>
        <div class="row">
          <el-button size="small" @click="copyCode">{{
            $t('github.copy')
          }}</el-button>
          <el-button size="small" @click="openUrl(verificationUri)">{{
            $t('github.openGithub')
          }}</el-button>
          <el-button size="small" @click="cancelSignIn">{{
            $t('github.cancel')
          }}</el-button>
        </div>
      </div>

      <div class="row" v-if="status.configured && !waiting">
        <el-button
          v-if="!status.signedIn"
          type="primary"
          :disabled="busy"
          @click="startSignIn"
          >{{ $t('github.signIn') }}</el-button
        >
        <el-button v-else :disabled="busy" @click="signOut">{{
          $t('github.signOut')
        }}</el-button>
      </div>

      <template v-if="status.signedIn">
        <p class="hint">{{ $t('github.pushHint') }}</p>
        <div class="row">
          <el-button type="primary" :disabled="busy" @click="push('json')">{{
            $t('github.pushJson')
          }}</el-button>
          <el-button :disabled="busy" @click="push('markdown')">{{
            $t('github.pushMarkdown')
          }}</el-button>
          <el-button
            v-if="status.linked"
            :disabled="busy"
            @click="pull(null)"
            >{{ $t('github.pullLinked') }}</el-button
          >
          <el-button
            v-if="status.htmlUrl"
            :disabled="busy"
            @click="openUrl(status.htmlUrl)"
            >{{ $t('github.openGist') }}</el-button
          >
        </div>
        <div class="row">
          <el-button :disabled="busy || listing" @click="loadGists">{{
            showList ? $t('github.refresh') : $t('github.choose')
          }}</el-button>
        </div>
        <div class="gistList" v-if="showList">
          <p class="hint" v-if="!gists.length && !listing">
            {{ $t('github.empty') }}
          </p>
          <button
            v-for="gist in gists"
            :key="gist.id"
            type="button"
            class="gistRow"
            :disabled="busy"
            @click="pull(gist.id)"
          >
            <span class="gistName">{{
              gist.description || gist.fileName || gist.id
            }}</span>
            <span class="gistMeta">{{ gist.updatedAt }}</span>
          </button>
          <p class="hint" v-if="gists.length >= 100">
            {{ $t('github.latestOnly') }}
          </p>
        </div>
      </template>
    </div>
  </el-dialog>
</template>

<script>
import { mapState } from 'vuex'
import { isDesktop, invoke } from '@/utils/desktop'

// 桌面端才有 GitHub 同步。网页版不会挂上这些命令。
export default {
  data() {
    return {
      visible: false,
      busy: false,
      listing: false,
      waiting: false,
      showList: false,
      userCode: '',
      verificationUri: '',
      expiresAt: 0,
      remaining: '',
      pollTimer: null,
      expiryTimer: null,
      gists: [],
      status: emptyStatus()
    }
  },
  computed: {
    ...mapState({
      isDark: state => state.localConfig.isDark
    }),
    statusText() {
      if (!this.status.configured) {
        return this.$t('github.notConfigured')
      }
      if (this.status.signedIn && this.status.login) {
        return this.$t('github.signedIn', { login: this.status.login })
      }
      if (this.status.signedIn) {
        return this.$t('github.signedInUnknown')
      }
      return this.$t('github.signedOut')
    },
    linkText() {
      if (!this.currentPayload().path) {
        return ''
      }
      return this.status.linked
        ? this.$t('github.linked')
        : this.$t('github.unlinked')
    }
  },
  created() {
    this.$bus.$on('showGithubSync', this.open)
  },
  beforeUnmount() {
    this.$bus.$off('showGithubSync', this.open)
    this.stopPollTimer()
    this.stopExpiry()
  },
  methods: {
    isDesktop,
    async open(action) {
      if (!this.isDesktop()) {
        this.$message.info(this.$t('github.desktopOnly'))
        return
      }
      this.visible = true
      await this.refresh()
      if (action === 'github-pull' && this.status.signedIn) {
        this.loadGists()
      }
      if (
        action === 'github-sign-in' &&
        this.status.configured &&
        !this.status.signedIn &&
        !this.waiting
      ) {
        this.startSignIn()
      }
    },
    currentPayload() {
      let data = null
      if (typeof window.__desktopDocumentData === 'function') {
        data = window.__desktopDocumentData()
      }
      const path =
        (window.__desktopSession && window.__desktopSession.path) || ''
      return { path, data }
    },
    async refresh() {
      const payload = this.currentPayload()
      try {
        this.status =
          (await invoke('github_status', { path: payload.path || '' })) ||
          emptyStatus()
      } catch (error) {
        console.log(error)
      }
    },
    async startSignIn() {
      this.stopPollTimer()
      this.busy = true
      try {
        const started = await invoke('github_auth_start')
        this.userCode = started.userCode
        this.verificationUri = started.verificationUri
        this.expiresAt = Date.now() + (started.expiresIn || 900) * 1000
        this.waiting = true
        this.armExpiry()
        this.schedulePoll((started.intervalSecs || 5) * 1000)
      } catch (error) {
        this.$message.error(this.presentError(error))
      } finally {
        this.busy = false
      }
    },
    schedulePoll(delay) {
      this.stopPollTimer()
      this.pollTimer = setTimeout(() => {
        this.poll()
      }, delay)
    },
    async poll() {
      if (!this.waiting) {
        return
      }
      try {
        const result = await invoke('github_auth_poll')
        if (!this.waiting) {
          return
        }
        if (result.status === 'success') {
          this.finishWait()
          await this.refresh()
          this.$message.success(
            result.login
              ? this.$t('github.signedIn', { login: result.login })
              : this.$t('github.signedInUnknown')
          )
          this.$bus.$emit('githubSyncChanged')
          return
        }
        if (result.status === 'cancelled') {
          this.finishWait()
          return
        }
        this.schedulePoll((result.intervalSecs || 5) * 1000)
      } catch (error) {
        this.finishWait()
        this.$message.error(this.presentError(error))
      }
    },
    async cancelSignIn() {
      this.finishWait()
      try {
        await invoke('github_auth_cancel')
      } catch (error) {
        console.log(error)
      }
    },
    finishWait() {
      this.waiting = false
      this.userCode = ''
      this.stopPollTimer()
      this.stopExpiry()
    },
    stopPollTimer() {
      if (this.pollTimer) {
        clearTimeout(this.pollTimer)
        this.pollTimer = null
      }
    },
    armExpiry() {
      this.stopExpiry()
      this.tickExpiry()
      this.expiryTimer = setInterval(() => this.tickExpiry(), 1000)
    },
    stopExpiry() {
      if (this.expiryTimer) {
        clearInterval(this.expiryTimer)
        this.expiryTimer = null
      }
    },
    tickExpiry() {
      const left = this.expiresAt - Date.now()
      const total = Math.max(0, Math.ceil(left / 1000))
      const minutes = Math.floor(total / 60)
      const seconds = total % 60
      this.remaining = `${minutes}:${String(seconds).padStart(2, '0')}`
    },
    async copyCode() {
      try {
        await navigator.clipboard.writeText(this.userCode)
        this.$message.success(this.$t('github.copied'))
      } catch (error) {
        this.$message.info(this.userCode)
      }
    },
    openUrl(url) {
      if (!url) {
        return
      }
      invoke('github_open_url', { url }).catch(error => {
        this.$message.error(this.presentError(error))
      })
    },
    async signOut() {
      this.busy = true
      try {
        await this.cancelSignIn()
        await invoke('github_sign_out')
        this.gists = []
        this.showList = false
        await this.refresh()
        this.$message.success(this.$t('github.signedOutDone'))
        this.$bus.$emit('githubSyncChanged')
      } catch (error) {
        this.$message.error(this.presentError(error))
      } finally {
        this.busy = false
      }
    },
    async push(format) {
      if (!this.status.signedIn) {
        this.$message.info(this.$t('github.needSignIn'))
        return
      }
      const payload = this.currentPayload()
      if (!payload.path) {
        this.$message.info(this.$t('github.needDocument'))
        return
      }
      try {
        await this.$confirm(
          this.$t('github.pushBody'),
          this.$t('github.pushTitle'),
          {
            type: 'warning',
            confirmButtonText: this.$t('github.pushConfirm'),
            cancelButtonText: this.$t('github.cancel')
          }
        )
      } catch (error) {
        return
      }
      this.busy = true
      try {
        const result = await invoke('github_push', {
          path: payload.path,
          data: payload.data,
          format
        })
        const key = result.recreated
          ? 'github.recreated'
          : result.created
            ? 'github.created'
            : 'github.updated'
        this.$message.success(this.$t(key))
        this.$bus.$emit('githubSyncChanged')
        await this.refresh()
      } catch (error) {
        this.$message.error(this.presentError(error))
      } finally {
        this.busy = false
      }
    },
    pull(gistId) {
      if (!this.status.signedIn) {
        this.$message.info(this.$t('github.needSignIn'))
        return
      }
      const payload = this.currentPayload()
      if (!gistId && !this.status.linked) {
        this.$message.info(this.$t('github.noLink'))
        return
      }
      this.pullWith(gistId || null, false, payload.path || '')
    },
    async pullWith(gistId, overwrite, path) {
      this.busy = true
      let result
      try {
        result = await invoke('github_pull', {
          path: path || '',
          gistId,
          overwrite
        })
      } catch (error) {
        this.busy = false
        this.$message.error(this.presentError(error))
        return
      }
      if (result.needsConfirm) {
        this.busy = false
        try {
          await this.$confirm(
            this.$t('github.overwriteBody', {
              name: result.fileName || result.path
            }),
            this.$t('github.overwriteTitle'),
            {
              type: 'warning',
              confirmButtonText: this.$t('github.overwriteConfirm'),
              cancelButtonText: this.$t('github.cancel')
            }
          )
        } catch (error) {
          return
        }
        return this.pullWith(gistId, true, path)
      }
      this.busy = false
      if (result.session && window.__desktopShow) {
        window.__desktopShow(result.session)
      }
      this.$message.success(this.$t('github.pulled'))
      this.$bus.$emit('githubSyncChanged')
      this.visible = false
      await this.refresh()
    },
    async loadGists() {
      if (!this.status.signedIn) {
        this.$message.info(this.$t('github.needSignIn'))
        return
      }
      this.listing = true
      this.showList = true
      try {
        this.gists = (await invoke('github_list_gists')) || []
      } catch (error) {
        this.$message.error(this.presentError(error))
      } finally {
        this.listing = false
      }
    },
    presentError(error) {
      const text = errorText(error)
      const size = text.match(/（当前 ([^）]+)）/)
      if (text.includes('超过 Gist')) {
        return this.$t('github.tooLarge', {
          size: size ? size[1] : '1MB+'
        })
      }
      return text || this.$t('github.failed')
    }
  }
}

function emptyStatus() {
  return {
    configured: false,
    signedIn: false,
    login: '',
    linked: false,
    gistId: '',
    htmlUrl: '',
    format: '',
    fileName: ''
  }
}

function errorText(error) {
  if (!error) {
    return ''
  }
  if (typeof error === 'string') {
    return error
  }
  if (typeof error.message === 'string') {
    return error.message
  }
  return ''
}
</script>

<style lang="less" scoped>
.githubSync {
  display: flex;
  flex-direction: column;
  gap: 12px;
  color: inherit;
}

.privacy,
.status,
.hint {
  margin: 0;
  line-height: 1.5;
}

.privacy,
.status {
  font-size: 13px;
}

.hint,
.gistMeta,
.uri {
  font-size: 12px;
  opacity: 0.75;
}

.userCodeLabel {
  font-size: 13px;
  text-align: center;
}

.userCode {
  display: block;
  margin: 8px 0;
  font-size: 28px;
  letter-spacing: 0.18em;
  text-align: center;
  font-weight: 650;
}

.row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.gistList {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 240px;
  overflow: auto;
}

.gistRow {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  padding: 8px 10px;
  border: 1px solid var(--el-border-color);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.gistName {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
