<template>
  <Teleport to="body">
    <!-- 常驻挂载、只切换透明度：显示时不用等一帧，也不会从 (0,0) 弹过去 -->
    <div
      ref="bar"
      class="hoverToolbar"
      :class="{ isDark: isDark, isVisible: visible }"
      :style="{ left: left + 'px', top: top + 'px' }"
      @mouseenter="cancelHide"
      @mouseleave="scheduleHide"
      @mousedown.stop.prevent
    >
      <template v-if="kind === 'node'">
        <button
          type="button"
          class="tbBtn tbText"
          :title="$t('hoverToolbar.bold')"
          @click="toggleFontWeight"
        >
          B
        </button>
        <button
          type="button"
          class="tbBtn"
          :title="$t('hoverToolbar.smaller')"
          @click="changeFontSize(-1)"
        >
          A<span class="tbSub">-</span>
        </button>
        <button
          type="button"
          class="tbBtn"
          :title="$t('hoverToolbar.larger')"
          @click="changeFontSize(1)"
        >
          A<span class="tbSub">+</span>
        </button>
        <el-popover placement="bottom" trigger="click" :width="232">
          <template #reference>
            <button
              type="button"
              class="tbBtn tbColor"
              :title="$t('hoverToolbar.color')"
            >
              <span
                class="tbColorBlock"
                :style="{ backgroundColor: currentColor }"
              ></span>
              A
            </button>
          </template>
          <Color :color="currentColor" @change="onColorChange"></Color>
        </el-popover>
        <span class="tbDivider"></span>
        <button
          v-if="canDelete"
          type="button"
          class="tbBtn tbIcon tbDanger"
          :title="$t('hoverToolbar.delete')"
          @click="removeNode"
        >
          ×
        </button>
      </template>
    </div>
    <!-- 节点右侧的「+」，和飞书一样直接加子节点 -->
    <button
      ref="addButton"
      type="button"
      class="hoverAdd"
      :class="{ isDark: isDark, isVisible: visible && kind === 'node' }"
      :style="{ left: addLeft + 'px', top: addTop + 'px' }"
      :title="$t('hoverToolbar.addChild')"
      @mouseenter="cancelHide"
      @mouseleave="scheduleHide"
      @click="addChild"
    >
      <span class="hoverAddIcon">+</span>
    </button>
  </Teleport>
</template>

<script>
import { mapState } from 'vuex'
import Color from './Color.vue'
import { canDeleteHoveredNode } from '@/utils/hoverNode'

// 画布悬停工具条：鼠标停在节点上就在它上方浮出一排操作，不必再去右侧面板
export default {
  name: 'CanvasHoverToolbar',
  components: {
    Color
  },
  data() {
    return {
      visible: false,
      kind: '',
      node: null,
      left: 0,
      top: 0,
      addLeft: 0,
      addTop: 0,
      hideTimer: null,
      mindMap: null
    }
  },
  computed: {
    ...mapState({
      isDark: state => state.localConfig.isDark
    }),

    currentColor() {
      return (this.node && this.node.getStyle('color')) || '#1d1d1f'
    },

    canDelete() {
      return canDeleteHoveredNode(this.node)
    }
  },
  created() {
    this.bindEvent('mindMapReady', this.onMindMapReady)
    this.bindEvent('node_mouseenter', this.onNodeMouseenter)
    this.bindEvent('node_mouseleave', this.scheduleHide)
    this.bindEvent('node_tree_render_end', this.reposition)
    this.bindEvent('draw_click', this.hideNow)
    this.bindEvent('scale', this.reposition)
    this.bindEvent('translate', this.reposition)
  },
  beforeUnmount() {
    Object.keys(this.safeHandlers || {}).forEach(name => {
      this.$bus.$off(name, this.safeHandlers[name])
    })
    this.safeHandlers = {}
    clearTimeout(this.hideTimer)
  },
  methods: {
    // 浮层的回调绝不能让异常冒到事件总线上：同一个事件上还挂着编辑器的
    // loading、工具栏的选中状态等监听者，一旦被打断整个界面就"半死"
    bindEvent(name, handler) {
      const safe = (...args) => {
        try {
          handler(...args)
        } catch (error) {
          console.error(error)
        }
      }
      this.safeHandlers = this.safeHandlers || {}
      this.safeHandlers[name] = safe
      this.$bus.$on(name, safe)
    },

    onMindMapReady(mindMap) {
      this.mindMap = mindMap
    },

    onNodeMouseenter(node) {
      if (!node || node.isGeneralization) return
      this.cancelHide()
      this.kind = 'node'
      this.node = node
      // 先等这一帧把按钮渲染出来再量宽度，否则用的是上一个状态的气泡尺寸，
      // 位置会整体偏掉半个宽度
      this.$nextTick(() => {
        if (!this.node) return
        this.position()
        this.visible = true
      })
    },

    // 画布缩放、平移、节点重新布局时跟着走，而不是直接消失
    reposition() {
      if (!this.visible) return
      if (!this.node || !this.node.group) {
        this.hideNow()
        return
      }
      this.position()
    },

    // 直接读节点的屏幕矩形：rbox() 是画布坐标，还要自己套 transform，
    // 一旦算错就整块飘走；getBoundingClientRect 天然就是屏幕坐标
    position() {
      const node = this.node
      const bar = this.$refs.bar
      if (!node || !bar || !node.group || !node.group.node) return
      const box = node.group.node.getBoundingClientRect()
      const barRect = bar.getBoundingClientRect()
      const gap = 10
      let barTop = box.y - barRect.height - gap
      if (barTop < 8) {
        barTop = box.y + box.height + gap
      }
      let barLeft = box.x + box.width / 2 - barRect.width / 2
      const maxLeft = window.innerWidth - barRect.width - 8
      barLeft = Math.min(Math.max(barLeft, 8), Math.max(maxLeft, 8))
      this.left = barLeft
      this.top = barTop
      // 「+」贴在节点右侧垂直居中
      this.addLeft = box.x + box.width + 8
      this.addTop = box.y + box.height / 2 - 11
    },

    cancelHide() {
      clearTimeout(this.hideTimer)
      this.hideTimer = null
    },

    // 留一点缓冲，鼠标从节点挪到工具条上时不会先消失
    scheduleHide() {
      this.cancelHide()
      this.hideTimer = setTimeout(this.hideNow, 120)
    },

    hideNow() {
      this.cancelHide()
      this.visible = false
      this.node = null
      this.kind = ''
    },

    changeFontSize(step) {
      const node = this.node
      if (!node) return
      const size = Number(node.getStyle('fontSize')) || 16
      const next = Math.min(Math.max(size + step, 10), 40)
      node.setStyle('fontSize', next)
      this.$nextTick(this.position)
    },

    toggleFontWeight() {
      const node = this.node
      if (!node) return
      const weight = node.getStyle('fontWeight')
      node.setStyle('fontWeight', weight === 'bold' ? 'normal' : 'bold')
      this.$nextTick(this.position)
    },

    onColorChange(color) {
      if (!this.node) return
      this.node.setStyle('color', color)
      this.$nextTick(this.position)
    },

    addChild() {
      const node = this.node
      if (!node || !this.mindMap) return
      this.mindMap.execCommand('INSERT_CHILD_NODE', false, [node])
      this.$nextTick(this.position)
    },

    removeNode() {
      const node = this.node
      if (!canDeleteHoveredNode(node) || !this.mindMap) return
      this.hideNow()
      this.mindMap.execCommand('REMOVE_NODE', [node])
    }
  }
}
</script>

<style lang="less" scoped>
.hoverToolbar {
  position: fixed;
  z-index: 2000;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 4px 6px;
  border-radius: 10px;
  background: #fff;
  box-shadow:
    0 6px 20px rgba(0, 0, 0, 0.12),
    0 0 0 1px rgba(0, 0, 0, 0.04);
  color: #1d1d1f;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.12s ease;

  &.isVisible {
    opacity: 1;
    pointer-events: auto;
  }

  &.isDark {
    background: #2c2c2e;
    color: #f5f5f7;
    box-shadow:
      0 6px 20px rgba(0, 0, 0, 0.5),
      0 0 0 1px hsla(0, 0%, 100%, 0.08);
  }
}

.tbBtn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 1px;
  min-width: 26px;
  height: 26px;
  padding: 0 6px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font-size: 13px;
  line-height: 1;
  cursor: pointer;
  transition: background 0.15s;

  &:hover {
    background: rgba(0, 0, 0, 0.06);
  }
}

.isDark .tbBtn:hover {
  background: hsla(0, 0%, 100%, 0.1);
}

.tbText {
  font-weight: 700;
}

.tbSub {
  font-size: 10px;
  transform: translateY(2px);
}

.tbColor {
  min-width: 34px;
}

.tbColorBlock {
  width: 8px;
  height: 8px;
  border-radius: 3px;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.12);
}

.tbIcon {
  font-size: 15px;
}

.tbDanger:hover {
  color: #f56c6c;
}

.tbDivider {
  width: 1px;
  height: 16px;
  margin: 0 4px;
  background: rgba(0, 0, 0, 0.08);
}

.isDark .tbDivider {
  background: hsla(0, 0%, 100%, 0.12);
}

// 节点右侧的「+」：白底圆形，和飞书一致
.hoverAdd {
  position: fixed;
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 50%;
  background: #fff;
  color: #4e5969;
  cursor: pointer;
  opacity: 0;
  pointer-events: none;
  transition:
    opacity 0.12s ease,
    background 0.15s;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.12);

  &.isVisible {
    opacity: 1;
    pointer-events: auto;
  }

  &:hover {
    background: #f2f3f5;
  }

  &.isDark {
    background: #2c2c2e;
    border-color: hsla(0, 0%, 100%, 0.14);
    color: #d0d3d9;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.5);
  }

  &.isDark:hover {
    background: #3a3a3c;
  }
}

.hoverAddIcon {
  font-size: 14px;
  line-height: 1;
  transform: translateY(-1px);
}
</style>
