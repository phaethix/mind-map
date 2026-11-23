import { getTopAncestorsFomNodeList, walk } from '../../utils'

const isFiniteRect = rect => {
  return (
    rect &&
    Number.isFinite(rect.top) &&
    Number.isFinite(rect.height) &&
    Number.isFinite(rect.left) &&
    Number.isFinite(rect.width)
  )
}

// 外框的上下留白不能越过附近的节点：节点之间的空隙有限，
// 留白超过空隙的一半就会压到隔壁的文字上，所以按空隙的一半封顶。
export const clampOuterFramePadding = (rect, rectList, paddingY, paddingX) => {
  const res = {
    top: paddingY,
    bottom: paddingY
  }
  const horizontalPadding = Number.isFinite(paddingX) ? paddingX : 0
  if (isFiniteRect(rect)) {
    // 留白后外框会横向扩张，扩张范围内有节点时也要参与上下避让
    const frameLeft = rect.left - horizontalPadding
    const frameRight = rect.left + rect.width + horizontalPadding
    ;(rectList || []).forEach(nearbyRect => {
      if (!isFiniteRect(nearbyRect)) return
      const nearbyRight = nearbyRect.left + nearbyRect.width
      const nearbyBottom = nearbyRect.top + nearbyRect.height
      const rectBottom = rect.top + rect.height
      // 外框横向扩张后仍碰不到的节点，或者完全被外框罩住的节点，不参与避让
      if (
        nearbyRight <= frameLeft ||
        nearbyRect.left >= frameRight ||
        (nearbyRect.left >= rect.left &&
          nearbyRight <= rect.left + rect.width &&
          nearbyRect.top >= rect.top &&
          nearbyBottom <= rectBottom)
      ) {
        return
      }
      if (nearbyBottom <= rect.top) {
        const gap = rect.top - nearbyBottom
        res.top = Math.min(res.top, Math.max(gap / 2, 0))
      } else if (nearbyRect.top >= rectBottom) {
        const gap = nearbyRect.top - rectBottom
        res.bottom = Math.min(res.bottom, Math.max(gap / 2, 0))
      }
    })
  }
  return res
}

// 收集整棵树里的可见节点包围框，外框不只会被同级兄弟压到，
// 其他分支在画布上相邻的节点也可能紧贴在上下方。
export const collectNodeRects = (root, getRect) => {
  const rectList = []
  walk(root, null, node => {
    if (node.group) {
      rectList.push(getRect(node))
    }
    ;(node._generalizationList || []).forEach(item => {
      if (item.generalizationNode && item.generalizationNode.group) {
        rectList.push(getRect(item.generalizationNode))
      }
    })
  })
  return rectList
}

// 解析要添加外框的节点实例列表
export const parseAddNodeList = list => {
  // 找出顶层节点
  list = getTopAncestorsFomNodeList(list)
  const cache = {}
  const uidToParent = {}
  // 找出列表中节点在兄弟节点中的索引，并和父节点关联起来
  list.forEach(node => {
    const parent = node.parent
    if (parent) {
      const pUid = parent.uid
      uidToParent[pUid] = parent
      const index = node.getIndexInBrothers()
      const data = {
        node,
        index
      }
      if (cache[pUid]) {
        if (
          !cache[pUid].find(item => {
            return item.index === data.index
          })
        ) {
          cache[pUid].push(data)
        }
      } else {
        cache[pUid] = [data]
      }
    }
  })
  const res = []
  Object.keys(cache).forEach(uid => {
    const indexList = cache[uid]
    const parentNode = uidToParent[uid]
    if (indexList.length > 1) {
      // 多个节点
      const rangeList = indexList
        .map(item => {
          return item.index
        })
        .sort((a, b) => {
          return a - b
        })
      const minIndex = rangeList[0]
      const maxIndex = rangeList[rangeList.length - 1]
      let curStart = -1
      let curEnd = -1
      for (let i = minIndex; i <= maxIndex; i++) {
        // 连续索引
        if (rangeList.includes(i)) {
          if (curStart === -1) {
            curStart = i
          }
          curEnd = i
        } else {
          // 连续断开
          if (curStart !== -1 && curEnd !== -1) {
            res.push({
              node: parentNode,
              range: [curStart, curEnd]
            })
          }
          curStart = -1
          curEnd = -1
        }
      }
      // 不要忘了最后一段索引
      if (curStart !== -1 && curEnd !== -1) {
        res.push({
          node: parentNode,
          range: [curStart, curEnd]
        })
      }
    } else {
      // 单个节点
      res.push({
        node: parentNode,
        range: [indexList[0].index, indexList[0].index]
      })
    }
  })
  return res
}

// 解析获取节点的子节点生成的外框列表
export const getNodeOuterFrameList = node => {
  const children = node.children
  if (!children || children.length <= 0) return
  const res = []
  const map = {}
  children.forEach((item, index) => {
    const outerFrameData = item.getData('outerFrame')
    if (!outerFrameData) return
    const groupId = outerFrameData.groupId
    if (groupId) {
      if (!map[groupId]) {
        map[groupId] = []
      }
      map[groupId].push({
        node: item,
        index
      })
    } else {
      res.push({
        nodeList: [item],
        range: [index, index]
      })
    }
  })
  Object.keys(map).forEach(id => {
    const list = map[id]
    res.push({
      nodeList: list.map(item => {
        return item.node
      }),
      range: [list[0].index, list[list.length - 1].index]
    })
  })
  return res
}
