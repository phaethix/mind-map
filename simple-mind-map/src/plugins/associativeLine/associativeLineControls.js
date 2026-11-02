import { bfsWalk } from '../../utils'
import {
  getAssociativeLineTargetIndex,
  computeCubicBezierPathPoints,
  joinCubicBezierPath,
  getNodePoint,
  getDefaultControlPointOffsets,
  getStraightLinePath,
  getStraightLineMid,
  getStraightHandles,
  getLineCornerRadius
} from './associativeLineUtils'

// 创建控制点、连线节点
function createControlNodes(node, toNode) {
  let { associativeLineActiveColor } = this.getStyleConfig(node, toNode)
  // 连线
  this.controlLine1 = this.associativeLineDraw
    .line()
    .stroke({ color: associativeLineActiveColor, width: 2 })
  this.controlLine2 = this.associativeLineDraw
    .line()
    .stroke({ color: associativeLineActiveColor, width: 2 })
  // 控制点
  this.controlPoint1 = this.createOneControlNode('controlPoint1', node, toNode)
  this.controlPoint2 = this.createOneControlNode('controlPoint2', node, toNode)
}

// 创建两端的手柄：拖它可以把箭头放到任意位置
function createEndPointHandles(node, toNode) {
  const { associativeLineActiveColor } = this.getStyleConfig(node, toNode)
  const make = key => {
    return this.associativeLineDraw
      .circle(this.controlPointDiameter)
      .stroke({ color: associativeLineActiveColor, width: 2 })
      .fill({ color: '#fff' })
      .click(e => {
        e.stopPropagation()
      })
      .mousedown(e => {
        this.onControlPointMousedown(e, key)
      })
  }
  this.startPointHandle = make('startPointHandle')
  this.endPointHandle = make('endPointHandle')
}

// 端点手柄跟着端点走，任何形状都会显示
function renderEndPointHandles(startPoint, endPoint, node, toNode) {
  if (!this.mindMap.opt.enableAdjustAssociativeLinePoints) return
  if (!this.startPointHandle) {
    this.createEndPointHandles(node, toNode)
  }
  const radius = this.controlPointDiameter / 2
  this.startPointHandle
    .x(startPoint.x - radius)
    .y(startPoint.y - radius)
  this.endPointHandle.x(endPoint.x - radius).y(endPoint.y - radius)
}

// 命中测试：端点落到哪个节点上
function findNodeByPoint(mindMap, x, y) {
  let hit = null
  bfsWalk(mindMap.renderer.root, node => {
    if (hit || !node || node.isGeneralization) return
    if (
      x >= node.left &&
      x <= node.left + node.width &&
      y >= node.top &&
      y <= node.top + node.height
    ) {
      hit = node
    }
  })
  return hit
}

// 创建控制点
function createOneControlNode(pointKey, node, toNode) {
  let { associativeLineActiveColor } = this.getStyleConfig(node, toNode)
  return this.associativeLineDraw
    .circle(this.controlPointDiameter)
    .stroke({ color: associativeLineActiveColor })
    .fill({ color: '#fff' })
    .click(e => {
      e.stopPropagation()
    })
    .mousedown(e => {
      this.onControlPointMousedown(e, pointKey)
    })
}

// 控制点的鼠标按下事件
function onControlPointMousedown(e, pointKey) {
  e.stopPropagation()
  e.preventDefault()
  this.isControlPointMousedown = true
  this.mousedownControlPointKey = pointKey
}

// 控制点的鼠标移动事件
function onControlPointMousemove(e) {
  if (
    !this.isControlPointMousedown ||
    !this.mousedownControlPointKey ||
    !this[this.mousedownControlPointKey]
  )
    return
  e.stopPropagation()
  e.preventDefault()
  let radius = this.controlPointDiameter / 2
  // 转换鼠标当前的位置
  let { x, y } = this.getTransformedEventPos(e)
  this.controlPointMousemoveState.pos = {
    x,
    y
  }
  // 更新当前拖拽的控制点的位置
  this[this.mousedownControlPointKey].x(x - radius).y(y - radius)
  let [, , , node, toNode] = this.activeLine
  let targetIndex = getAssociativeLineTargetIndex(node, toNode)
  let { associativeLinePoint, associativeLineTargetControlOffsets } =
    node.getData()
  associativeLinePoint = associativeLinePoint || []
  const nodePos = this.getNodePos(node)
  const toNodePos = this.getNodePos(toNode)
  let [startPoint, endPoint] = this.updateAllLinesPos(
    node,
    toNode,
    associativeLinePoint[targetIndex]
  )
  this.controlPointMousemoveState.startPoint = startPoint
  this.controlPointMousemoveState.endPoint = endPoint
  this.controlPointMousemoveState.targetIndex = targetIndex
  let offsets = []
  if (!associativeLineTargetControlOffsets) {
    // 兼容0.4.5版本，没有associativeLineTargetControlOffsets的情况
    offsets = getDefaultControlPointOffsets(startPoint, endPoint)
  } else {
    offsets = associativeLineTargetControlOffsets[targetIndex]
  }
  let point1 = null
  let point2 = null
  const { x: clientX, y: clientY } = this.mindMap.toPos(e.clientX, e.clientY)
  const _e = {
    clientX,
    clientY
  }
  // 拖动端点手柄：箭头可以放到任意位置，松手时若压在节点上会吸附到最近的边
  if (
    this.mousedownControlPointKey === 'startPointHandle' ||
    this.mousedownControlPointKey === 'endPointHandle'
  ) {
    const isStart = this.mousedownControlPointKey === 'startPointHandle'
    this[this.mousedownControlPointKey].x(x - radius).y(y - radius)
    const start = isStart ? { x, y } : startPoint
    const end = isStart ? endPoint : { x, y }
    const points = computeCubicBezierPathPoints(start.x, start.y, end.x, end.y)
    this.controlPointMousemoveState.startPoint = start
    this.controlPointMousemoveState.endPoint = end
    this.updataAassociativeLine(start, end, points[0], points[1], this.activeLine)
    return
  }
  // 直线模式只平移折弯段，端点继续贴着节点；两个控制点一起跟着走
  if (this.getLineShape(node, toNode) !== 'curve') {
    const mid = getStraightLineMid(startPoint, endPoint, { x, y }, { x, y })
    const handles = getStraightHandles(startPoint, endPoint, mid)
    this.controlPoint1.x(handles[0].x - radius).y(handles[0].y - radius)
    this.controlPoint2.x(handles[1].x - radius).y(handles[1].y - radius)
    this.controlLine1.plot(startPoint.x, startPoint.y, handles[0].x, handles[0].y)
    this.controlLine2.plot(endPoint.x, endPoint.y, handles[1].x, handles[1].y)
    this.updataAassociativeLine(
      startPoint,
      endPoint,
      handles[0],
      handles[1],
      this.activeLine
    )
    return
  }
  // 拖拽的是控制点1
  if (this.mousedownControlPointKey === 'controlPoint1') {
    startPoint = getNodePoint(nodePos, '', 0, _e)
    point1 = {
      x,
      y
    }
    point2 = {
      x: endPoint.x + offsets[1].x,
      y: endPoint.y + offsets[1].y
    }
    if (startPoint) {
      // 保存更新后的坐标
      this.controlPointMousemoveState.startPoint = startPoint
      // 更新控制点1的连线
      this.controlLine1.plot(startPoint.x, startPoint.y, point1.x, point1.y)
    }
  } else {
    // 拖拽的是控制点2
    endPoint = getNodePoint(toNodePos, '', 0, _e)
    point1 = {
      x: startPoint.x + offsets[0].x,
      y: startPoint.y + offsets[0].y
    }
    point2 = {
      x,
      y
    }
    if (endPoint) {
      // 保存更新后结束节点的坐标
      this.controlPointMousemoveState.endPoint = endPoint
      // 更新控制点2的连线
      this.controlLine2.plot(endPoint.x, endPoint.y, point2.x, point2.y)
    }
  }
  this.updataAassociativeLine(
    startPoint,
    endPoint,
    point1,
    point2,
    this.activeLine
  )
}

function updataAassociativeLine(
  startPoint,
  endPoint,
  point1,
  point2,
  activeLine
) {
  const [path, clickPath, text, node] = activeLine
  // 更新关联线
  const pathStr =
    this.getLineShape(node) !== 'curve'
      ? getStraightLinePath(
          startPoint,
          endPoint,
          12,
          getStraightLineMid(startPoint, endPoint, point1, point2)
        )
      : joinCubicBezierPath(startPoint, endPoint, point1, point2)
  path.plot(pathStr)
  clickPath.plot(pathStr)
  this.updateTextPos(path, text)
  this.updateTextEditBoxPos(text)
}

// 控制点的鼠标松开事件
function onControlPointMouseup(e) {
  if (!this.isControlPointMousedown) return
  e.stopPropagation()
  e.preventDefault()
  let { pos, startPoint, endPoint, targetIndex } =
    this.controlPointMousemoveState
  // 只是点了一下控制点、没有拖动：保持原有调整不变
  if (!pos) {
    this.resetControlPoint()
    return
  }
  let [, , , node] = this.activeLine
  let offsetList = []
  let { associativeLinePoint, associativeLineTargetControlOffsets } =
    node.getData()
  if (!associativeLinePoint) {
    associativeLinePoint = []
  }
  associativeLinePoint[targetIndex] = associativeLinePoint[targetIndex] || {
    startPoint,
    endPoint
  }
  if (!associativeLineTargetControlOffsets) {
    // 兼容0.4.5版本，没有associativeLineTargetControlOffsets的情况
    offsetList[targetIndex] = getDefaultControlPointOffsets(
      startPoint,
      endPoint
    )
  } else {
    offsetList = associativeLineTargetControlOffsets
  }
  // 端点手柄：存成自由坐标，落在节点上就贴到最近的边
  if (
    this.mousedownControlPointKey === 'startPointHandle' ||
    this.mousedownControlPointKey === 'endPointHandle'
  ) {
    const isStart = this.mousedownControlPointKey === 'startPointHandle'
    const point = { x: pos.x, y: pos.y }
    const hit = findNodeByPoint(this.mindMap, point.x, point.y)
    if (hit) {
      const centerX = hit.left + hit.width / 2
      const centerY = hit.top + hit.height / 2
      const horizontal =
        Math.abs(point.x - centerX) / (hit.width || 1) >
        Math.abs(point.y - centerY) / (hit.height || 1)
      if (horizontal) {
        point.x = point.x < centerX ? hit.left : hit.left + hit.width
      } else {
        point.y = point.y < centerY ? hit.top : hit.top + hit.height
      }
    }
    const [, , , sourceNode, targetNode] = this.activeLine
    const index = getAssociativeLineTargetIndex(sourceNode, targetNode)
    const list = (sourceNode.getData('associativeLinePoint') || []).slice()
    list[index] = list[index] || {}
    if (isStart) {
      list[index].startPoint = point
    } else {
      list[index].endPoint = point
    }
    this.mindMap.execCommand('SET_NODE_DATA', sourceNode, {
      associativeLinePoint: list
    })
    this.isNotRenderAllLines = true
    setTimeout(() => {
      this.resetControlPoint()
    }, 0)
    return
  }
  let offset1 = null
  let offset2 = null
  if (this.mousedownControlPointKey === 'controlPoint1') {
    // 更新控制点1数据
    offset1 = {
      x: pos.x - startPoint.x,
      y: pos.y - startPoint.y
    }
    offset2 = offsetList[targetIndex][1]
    associativeLinePoint[targetIndex].startPoint = startPoint
  } else {
    // 更新控制点2数据
    offset1 = offsetList[targetIndex][0]
    offset2 = {
      x: pos.x - endPoint.x,
      y: pos.y - endPoint.y
    }
    associativeLinePoint[targetIndex].endPoint = endPoint
  }
  // 直线模式：两个控制点在同一段折弯线上，按中点统一保存，下次渲染才回得来
  if (this.getLineShape(node) !== 'curve') {
    const mid = getStraightLineMid(startPoint, endPoint, pos, pos)
    const horizontal =
      Math.abs(endPoint.x - startPoint.x) >=
      Math.abs(endPoint.y - startPoint.y)
    offset1 = horizontal
      ? { x: mid - startPoint.x, y: 0 }
      : { x: 0, y: mid - startPoint.y }
    offset2 = horizontal
      ? { x: mid - endPoint.x, y: 0 }
      : { x: 0, y: mid - endPoint.y }
  }
  offsetList[targetIndex] = [offset1, offset2]
  this.mindMap.execCommand('SET_NODE_DATA', node, {
    associativeLineTargetControlOffsets: offsetList,
    associativeLinePoint
  })
  this.isNotRenderAllLines = true
  // 这里要加个setTimeout0是因为draw_click事件比mouseup事件触发的晚，所以重置isControlPointMousedown需要等draw_click事件触发完以后
  setTimeout(() => {
    this.resetControlPoint()
  }, 0)
}

// 复位控制点移动
function resetControlPoint() {
  this.isControlPointMousedown = false
  this.mousedownControlPointKey = ''
  this.controlPointMousemoveState = {
    pos: null,
    startPoint: null,
    endPoint: null,
    targetIndex: ''
  }
}

// 渲染控制点
function renderControls(startPoint, endPoint, point1, point2, node, toNode) {
  if (!this.mindMap.opt.enableAdjustAssociativeLinePoints) return
  if (!this.controlLine1) {
    this.createControlNodes(node, toNode)
  }
  let radius = this.controlPointDiameter / 2
  // 控制点和起终点的连线
  this.controlLine1.plot(startPoint.x, startPoint.y, point1.x, point1.y)
  this.controlLine2.plot(endPoint.x, endPoint.y, point2.x, point2.y)
  // 控制点
  this.controlPoint1.x(point1.x - radius).y(point1.y - radius)
  this.controlPoint2.x(point2.x - radius).y(point2.y - radius)
}

// 删除控制点
function removeControls() {
  if (!this.controlLine1) return
  ;[
    this.controlLine1,
    this.controlLine2,
    this.controlPoint1,
    this.controlPoint2,
    this.startPointHandle,
    this.endPointHandle
  ].forEach(item => {
    if (item) item.remove()
  })
  this.controlLine1 = null
  this.controlLine2 = null
  this.controlPoint1 = null
  this.controlPoint2 = null
  this.startPointHandle = null
  this.endPointHandle = null
}

// 隐藏控制点
function hideControls() {
  if (!this.controlLine1) return
  ;[
    this.controlLine1,
    this.controlLine2,
    this.controlPoint1,
    this.controlPoint2
  ].forEach(item => {
    item.hide()
  })
}

// 显示控制点
function showControls() {
  if (!this.controlLine1) return
  ;[
    this.controlLine1,
    this.controlLine2,
    this.controlPoint1,
    this.controlPoint2
  ].forEach(item => {
    item.show()
  })
}

export default {
  createControlNodes,
  createEndPointHandles,
  renderEndPointHandles,
  createOneControlNode,
  onControlPointMousedown,
  onControlPointMousemove,
  onControlPointMouseup,
  resetControlPoint,
  renderControls,
  removeControls,
  hideControls,
  showControls,
  updataAassociativeLine
}
