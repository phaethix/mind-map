import { getRectRelativePosition } from '../../utils/index'

// 获取目标节点在起始节点的目标数组中的索引
export const getAssociativeLineTargetIndex = (node, toNode) => {
  return node.getData('associativeLineTargets').findIndex(item => {
    return item === toNode.getData('uid')
  })
}

// 计算贝塞尔曲线的控制点
export const computeCubicBezierPathPoints = (x1, y1, x2, y2) => {
  const min = 5
  let cx1 = x1 + (x2 - x1) / 2
  let cy1 = y1
  let cx2 = cx1
  let cy2 = y2
  if (Math.abs(x1 - x2) <= min) {
    // 两端基本在同一竖线上：往右鼓出一个弧，否则曲线退化成一条贴着节点的竖线
    const bow = Math.max(32, Math.abs(y2 - y1) * 0.28)
    cx1 = x1 + bow
    cx2 = x2 + bow
  }
  if (Math.abs(y1 - y2) <= min) {
    cx1 = x1
    cy1 = y1 - (x2 - x1) / 2
    cx2 = x2
    cy2 = cy1
  }
  return [
    {
      x: cx1,
      y: cy1
    },
    {
      x: cx2,
      y: cy2
    }
  ]
}

// 拼接贝塞尔曲线路径
export const joinCubicBezierPath = (startPoint, endPoint, point1, point2) => {
  return `M ${startPoint.x},${startPoint.y} C ${point1.x},${point1.y} ${point2.x},${point2.y} ${endPoint.x},${endPoint.y}`
}

//  获取节点的位置信息
const getNodeRect = node => {
  let { left, top, width, height } = node
  return {
    right: left + width,
    bottom: top + height,
    left,
    top,
    width,
    height
  }
}

// 三次贝塞尔曲线
export const cubicBezierPath = (x1, y1, x2, y2) => {
  let points = computeCubicBezierPathPoints(x1, y1, x2, y2)
  return joinCubicBezierPath(
    { x: x1, y: y1 },
    { x: x2, y: y2 },
    points[0],
    points[1]
  )
}

export const calcPoint = (node, e) => {
  const { left, top, translateLeft, translateTop, width, height } = node
  const clientX = e.clientX
  const clientY = e.clientY
  // 中心点的坐标
  const centerX = translateLeft + width / 2
  const centerY = translateTop + height / 2
  const translateCenterX = left + width / 2
  const translateCenterY = top + height / 2
  const theta = Math.atan(height / width)
  // 矩形左上角坐标
  const deltaX = clientX - centerX
  const deltaY = centerY - clientY
  // 方向值
  const direction = Math.atan2(deltaY, deltaX)
  // 默认坐标
  let x = left + width
  let y = top + height
  if (direction < theta && direction >= -theta) {
    // 右边
    // 正切值 = 对边/邻边，对边 = 正切值*邻边
    const range = direction * (width / 2)
    if (direction < theta && direction >= 0) {
      // 中心点上边
      y = translateCenterY - range
    } else if (direction >= -theta && direction < 0) {
      //  中心点下方
      y = translateCenterY - range
    }
    return {
      x,
      y,
      dir: 'right',
      range
    }
  } else if (direction >= theta && direction < Math.PI - theta) {
    // 上边
    y = top
    let range = 0
    if (direction < Math.PI / 2 - theta && direction >= theta) {
      // 正切值 = 对边/邻边，邻边 = 对边/正切值
      const side = height / 2 / direction
      range = -side
      // 中心点右侧
      x = translateCenterX + side
    } else if (
      direction >= Math.PI / 2 - theta &&
      direction < Math.PI - theta
    ) {
      // 中心点左侧
      const tanValue = (centerX - clientX) / (centerY - clientY)
      const side = (height / 2) * tanValue
      range = side
      x = translateCenterX - side
    }
    return {
      x,
      y,
      dir: 'top',
      range
    }
  } else if (direction < -theta && direction >= theta - Math.PI) {
    // 下边
    let range = 0
    if (direction >= theta - Math.PI / 2 && direction < -theta) {
      // 中心点右侧
      // 正切值 = 对边/邻边，邻边 = 对边/正切值
      const side = height / 2 / direction
      range = side
      x = translateCenterX - side
    } else if (
      direction < theta - Math.PI / 2 &&
      direction >= theta - Math.PI
    ) {
      // 中心点左侧
      const tanValue = (centerX - clientX) / (centerY - clientY)
      const side = (height / 2) * tanValue
      range = -side
      x = translateCenterX + side
    }
    return {
      x,
      y,
      dir: 'bottom',
      range
    }
  }
  // 左边
  x = left
  const tanValue = (centerY - clientY) / (centerX - clientX)
  const range = tanValue * (width / 2)
  if (direction >= -Math.PI && direction < theta - Math.PI) {
    // 中心点右侧
    y = translateCenterY - range
  } else if (direction < Math.PI && direction >= Math.PI - theta) {
    //  中心点左侧
    y = translateCenterY - range
  }
  return {
    x,
    y,
    dir: 'left',
    range
  }
}
// 获取节点的连接点
export const getNodePoint = (node, dir = 'right', range = 0, e = null) => {
  let { left, top, width, height } = node
  if (e) {
    return calcPoint(node, e)
  }
  switch (dir) {
    case 'left':
      return {
        x: left,
        y: top + height / 2 - range,
        dir
      }
    case 'right':
      return {
        x: left + width,
        y: top + height / 2 - range,
        dir
      }
    case 'top':
      return {
        x: left + width / 2 - range,
        y: top,
        dir
      }
    case 'bottom':
      return {
        x: left + width / 2 - range,
        y: top + height,
        dir
      }
    default:
      break
  }
}

// 根据两个节点的位置计算节点的连接点
export const computeNodePoints = (fromNode, toNode) => {
  const fromRect = getNodeRect(fromNode)
  const toRect = getNodeRect(toNode)
  let fromDir = ''
  let toDir = ''
  const dir = getRectRelativePosition(
    {
      x: fromRect.left,
      y: fromRect.top,
      width: fromRect.width,
      height: fromRect.height
    },
    {
      x: toRect.left,
      y: toRect.top,
      width: toRect.width,
      height: toRect.height
    }
  )
  // 起始矩形在结束矩形的什么方向
  switch (dir) {
    case 'left-top':
      fromDir = 'right'
      toDir = 'top'
      break
    case 'right-top':
      fromDir = 'left'
      toDir = 'top'
      break
    case 'right-bottom':
      fromDir = 'left'
      toDir = 'bottom'
      break
    case 'left-bottom':
      fromDir = 'right'
      toDir = 'bottom'
      break
    case 'left':
      fromDir = 'right'
      toDir = 'left'
      break
    case 'right':
      fromDir = 'left'
      toDir = 'right'
      break
    case 'top':
      fromDir = 'right'
      toDir = 'right'
      break
    case 'bottom':
      fromDir = 'left'
      toDir = 'left'
      break
    case 'overlap':
      fromDir = 'right'
      toDir = 'right'
      break
    default:
      break
  }
  return [getNodePoint(fromNode, fromDir), getNodePoint(toNode, toDir)]
}

// 折线的拐角圆角半径，直角折线传 0
export const getLineCornerRadius = shape => {
  return shape === 'sharp' ? 0 : 12
}

// 折弯段的方向：保存的偏移里只留一个轴，靠它判断横竖，避免主轴翻转后丢调整
export const getStraightAxis = offsets => {
  if (!offsets || offsets.length !== 2) return ''
  const x = Math.abs(offsets[0].x) + Math.abs(offsets[1].x)
  const y = Math.abs(offsets[0].y) + Math.abs(offsets[1].y)
  if (x < 0.5 && y < 0.5) return ''
  return x >= y ? 'x' : 'y'
}

// 直线模式下折弯段所在的坐标：横向为主看 x，纵向为主看 y。
// 两个控制点的坐标取平均，拖哪个控制点都能调，默认落在两点中间。
export const getStraightLineMid = (
  startPoint,
  endPoint,
  point1,
  point2,
  axis
) => {
  const horizontal = axis
    ? axis === 'x'
    : Math.abs(endPoint.x - startPoint.x) >=
      Math.abs(endPoint.y - startPoint.y)
  if (point1 && point2) {
    return horizontal
      ? (point1.x + point2.x) / 2
      : (point1.y + point2.y) / 2
  }
  return horizontal
    ? (startPoint.x + endPoint.x) / 2
    : (startPoint.y + endPoint.y) / 2
}

// 两个控制点摆在同一段折弯线上，拖动其中任意一个就整段平移
export const getStraightHandles = (startPoint, endPoint, mid, axis) => {
  const horizontal = axis
    ? axis === 'x'
    : Math.abs(endPoint.x - startPoint.x) >=
      Math.abs(endPoint.y - startPoint.y)
  if (horizontal) {
    const y1 = startPoint.y
    const y2 = endPoint.y
    return [
      { x: mid, y: y1 + (y2 - y1) / 3 },
      { x: mid, y: y1 + ((y2 - y1) * 2) / 3 }
    ]
  }
  const x1 = startPoint.x
  const x2 = endPoint.x
  return [
    { x: x1 + (x2 - x1) / 3, y: mid },
    { x: x1 + ((x2 - x1) * 2) / 3, y: mid }
  ]
}

// 直线 + 圆角：先水平走到折弯处，再竖直，再水平进入终点，
// 两个拐角用二次贝塞尔画成圆弧，radius 传 0 就是直角折线。
// 末段是直线，箭头的自动朝向才稳。
export const getStraightLinePath = (
  startPoint,
  endPoint,
  radius = 12,
  mid,
  axis
) => {
  const x1 = startPoint.x
  const y1 = startPoint.y
  const x2 = endPoint.x
  const y2 = endPoint.y
  const dx = x2 - x1
  const dy = y2 - y1
  const adx = Math.abs(dx)
  const ady = Math.abs(dy)
  // 几乎水平或几乎竖直时就画一条直线，免得出现细碎的折角
  if (adx < 2 || ady < 2) {
    return `M${x1},${y1} L${x2},${y2}`
  }
  const sx = dx > 0 ? 1 : -1
  const sy = dy > 0 ? 1 : -1
  const r = Math.max(Math.min(radius, adx / 2, ady / 2), 0)
  // 折弯段夹在两端之间，避免拖过头导致路径折返、箭头反向
  const clampToSpan = (value, a, b) => {
    const low = Math.min(a, b)
    const high = Math.max(a, b)
    return Math.min(Math.max(value, low), high)
  }
  const horizontal = axis ? axis === 'x' : adx >= ady
  if (horizontal) {
    // 水平-竖直-水平，竖直段落在 mid 上
    const midX = clampToSpan(typeof mid === 'number' ? mid : x1 + dx / 2, x1, x2)
    return [
      `M${x1},${y1}`,
      `L${midX - sx * r},${y1}`,
      `Q${midX},${y1} ${midX},${y1 + sy * r}`,
      `L${midX},${y2 - sy * r}`,
      `Q${midX},${y2} ${midX + sx * r},${y2}`,
      `L${x2},${y2}`
    ].join(' ')
  }
  // 竖直-水平-竖直，水平段落在 mid 上
  const midY = clampToSpan(
    typeof mid === 'number' ? mid : y1 + dy / 2,
    y1,
    y2
  )
  return [
    `M${x1},${y1}`,
    `L${x1},${midY - sy * r}`,
    `Q${x1},${midY} ${x1 + sx * r},${midY}`,
    `L${x2 - sx * r},${midY}`,
    `Q${x2},${midY} ${x2},${midY + sy * r}`,
    `L${x2},${y2}`
  ].join(' ')
}

// 端点是否被手动拖到了任意位置（没有 dir 就是自由坐标）
export const isFreeEndPoint = point => {
  return Boolean(
    point &&
      typeof point.x === 'number' &&
      typeof point.y === 'number' &&
      !point.dir
  )
}

// 获取节点的关联线路径
export const getNodeLinePath = (
  startPoint,
  endPoint,
  node,
  toNode,
  shape = 'curve'
) => {
  // 直线：两点直接连，没有可调的折弯段
  if (shape === 'line') {
    return {
      path: `M${startPoint.x},${startPoint.y} L${endPoint.x},${endPoint.y}`,
      controlPoints: []
    }
  }
  // 折线（圆角或直角）：控制点摆在同一段折弯线上，拖动就能整段平移。
  // 老数据里的 straight 等同圆角折线。
  if (shape !== 'curve') {
    const targetIndex = getAssociativeLineTargetIndex(node, toNode)
    const saved = (node.getData('associativeLineTargetControlOffsets') || [])[
      targetIndex
    ]
    let point1 = null
    let point2 = null
    if (saved && saved.length === 2) {
      point1 = { x: startPoint.x + saved[0].x, y: startPoint.y + saved[0].y }
      point2 = { x: endPoint.x + saved[1].x, y: endPoint.y + saved[1].y }
    }
    const axis = getStraightAxis(saved)
    const mid = getStraightLineMid(
      startPoint,
      endPoint,
      point1,
      point2,
      axis
    )
    return {
      path: getStraightLinePath(
        startPoint,
        endPoint,
        getLineCornerRadius(shape),
        mid,
        axis
      ),
      controlPoints: getStraightHandles(startPoint, endPoint, mid, axis)
    }
  }
  let targetIndex = getAssociativeLineTargetIndex(node, toNode)
  // 控制点
  let controlPoints = []
  let associativeLineTargetControlOffsets = node.getData(
    'associativeLineTargetControlOffsets'
  )
  if (
    associativeLineTargetControlOffsets &&
    associativeLineTargetControlOffsets[targetIndex]
  ) {
    // 节点保存了控制点差值
    let offsets = associativeLineTargetControlOffsets[targetIndex]
    controlPoints = [
      {
        x: startPoint.x + offsets[0].x,
        y: startPoint.y + offsets[0].y
      },
      {
        x: endPoint.x + offsets[1].x,
        y: endPoint.y + offsets[1].y
      }
    ]
  } else {
    // 没有保存控制点则生成默认的
    controlPoints = computeCubicBezierPathPoints(
      startPoint.x,
      startPoint.y,
      endPoint.x,
      endPoint.y
    )
  }
  // 根据控制点拼接贝塞尔曲线路径
  return {
    path: joinCubicBezierPath(
      startPoint,
      endPoint,
      controlPoints[0],
      controlPoints[1]
    ),
    controlPoints
  }
}

// 获取默认的控制点差值
export const getDefaultControlPointOffsets = (startPoint, endPoint) => {
  let controlPoints = computeCubicBezierPathPoints(
    startPoint.x,
    startPoint.y,
    endPoint.x,
    endPoint.y
  )
  return [
    {
      x: controlPoints[0].x - startPoint.x,
      y: controlPoints[0].y - startPoint.y
    },
    {
      x: controlPoints[1].x - endPoint.x,
      y: controlPoints[1].y - endPoint.y
    }
  ]
}
