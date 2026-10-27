// Vue 2 时代用 `new Vue()` 当事件总线，组件里到处是 this.$bus.$on/$emit。
// Vue 3 没有这个用法了，这里保持同样的接口。
//
// 注意：不能用 mitt —— 它的 emit 只转发第一个参数，而项目里的调用是多参数的
// （例如 node_active(node, activeNodeList)、back_forward(index, length)），
// 用 mitt 会导致 args[1] 永远是 undefined，监听器展开时直接抛错。
const handlers = new Map()

const bus = {
  $on(type, handler) {
    if (!handlers.has(type)) {
      handlers.set(type, new Set())
    }
    handlers.get(type).add(handler)
  },
  $off(type, handler) {
    const set = handlers.get(type)
    if (!set) {
      return
    }
    if (handler) {
      set.delete(handler)
    } else {
      handlers.delete(type)
    }
  },
  $emit(type, ...args) {
    const set = handlers.get(type)
    if (!set) {
      return
    }
    // 复制一份再遍历，允许回调里增删监听；单个回调出错不影响其他监听者
    ;[...set].forEach(handler => {
      try {
        handler(...args)
      } catch (error) {
        console.error(error)
      }
    })
  },
  all: handlers
}

bus.on = bus.$on
bus.off = bus.$off
bus.emit = bus.$emit

export default bus
