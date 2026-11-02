// 对齐辅助线：拖动节点时把它和其它节点的左/中/右、上/中/下做比较，
// 差值小于阈值就吸附过去，并返回需要画的参考线

// 一个盒子在某个轴上的三个参考位置
const threeEdges = (start, size) => {
  return [start, start + size / 2, start + size]
}

// 同距离时优先对齐边缘，中线对齐没那么直观
const priorityOf = index => (index === 1 ? 1 : 0)

const pickBest = candidates => {
  let best = null
  candidates.forEach(item => {
    if (
      !best ||
      Math.abs(item.delta) < Math.abs(best.delta) ||
      (Math.abs(item.delta) === Math.abs(best.delta) &&
        item.priority < best.priority)
    ) {
      best = item
    }
  })
  return best
}

// 参考线要盖住被拖拽节点和所有对齐到同一条线的节点
const buildGuide = (axis, value, draggedBox, matched) => {
  let start =
    axis === 'x' ? draggedBox.top : draggedBox.left
  let end =
    axis === 'x'
      ? draggedBox.top + draggedBox.height
      : draggedBox.left + draggedBox.width
  matched.forEach(item => {
    const box = item.node
    if (!box) return
    const boxStart = axis === 'x' ? box.top : box.left
    const boxEnd =
      axis === 'x' ? box.top + box.height : box.left + box.width
    if (Number.isFinite(boxStart) && boxStart < start) {
      start = boxStart
    }
    if (Number.isFinite(boxEnd) && boxEnd > end) {
      end = boxEnd
    }
  })
  return { axis, value, start, end }
}

/**
 * @param {{x:number,y:number,width:number,height:number}} rect 被拖拽节点的目标包围盒（画布坐标）
 * @param {Array} nodeList 其它节点，需要 left/top/width/height
 * @param {number} threshold 吸附阈值，屏幕像素
 * @param {number} scale 当前画布缩放，用来把阈值换算到画布坐标
 * @returns {{dx:number,dy:number,guides:Array}} 吸附位移与参考线
 */
export const computeAlignmentGuide = (
  rect,
  nodeList,
  threshold = 6,
  scale = 1
) => {
  const limit = threshold / (scale > 0 ? scale : 1)
  const draggedX = threeEdges(rect.x, rect.width)
  const draggedY = threeEdges(rect.y, rect.height)
  const xCandidates = []
  const yCandidates = []
  nodeList.forEach(node => {
    if (!node || !Number.isFinite(node.left) || !Number.isFinite(node.top)) {
      return
    }
    const edgesX = threeEdges(node.left, node.width)
    const edgesY = threeEdges(node.top, node.height)
    edgesX.forEach((value, index) => {
      draggedX.forEach(draggedValue => {
        const delta = value - draggedValue
        if (Math.abs(delta) <= limit) {
          xCandidates.push({
            delta,
            value,
            priority: priorityOf(index),
            node
          })
        }
      })
    })
    edgesY.forEach((value, index) => {
      draggedY.forEach(draggedValue => {
        const delta = value - draggedValue
        if (Math.abs(delta) <= limit) {
          yCandidates.push({
            delta,
            value,
            priority: priorityOf(index),
            node
          })
        }
      })
    })
  })
  const bestX = pickBest(xCandidates)
  const bestY = pickBest(yCandidates)
  const dx = bestX ? bestX.delta : 0
  const dy = bestY ? bestY.delta : 0
  const snapped = {
    left: rect.x + dx,
    top: rect.y + dy,
    width: rect.width,
    height: rect.height
  }
  const guides = []
  if (bestX) {
    const matched = xCandidates.filter(
      item => Math.abs(item.value - bestX.value) < 0.5
    )
    guides.push(buildGuide('x', bestX.value, snapped, matched))
  }
  if (bestY) {
    const matched = yCandidates.filter(
      item => Math.abs(item.value - bestY.value) < 0.5
    )
    guides.push(buildGuide('y', bestY.value, snapped, matched))
  }
  return { dx, dy, guides }
}
