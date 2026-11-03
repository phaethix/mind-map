// 概要括号：两端圆角，中间直线。sign 为 1 时向右或向下凸，为 -1 时向左或向上凸。
export const generalizationBracketPath = (
  x1,
  y1,
  x2,
  y2,
  { horizontal = true, sign = 1, radius = 6 } = {}
) => {
  const span = horizontal ? Math.abs(y2 - y1) : Math.abs(x2 - x1)
  const r = Math.max(Math.min(radius, span / 2), 0)
  if (r < 0.5) {
    return `M ${x1},${y1} L ${x2},${y2}`
  }
  const bulge = sign < 0 ? -r : r
  if (horizontal) {
    const sy = y2 >= y1 ? 1 : -1
    return [
      `M ${x1},${y1}`,
      `Q ${x1 + bulge},${y1} ${x1 + bulge},${y1 + sy * r}`,
      `L ${x1 + bulge},${y2 - sy * r}`,
      `Q ${x1 + bulge},${y2} ${x2},${y2}`
    ].join(' ')
  }
  const sx = x2 >= x1 ? 1 : -1
  return [
    `M ${x1},${y1}`,
    `Q ${x1},${y1 + bulge} ${x1 + sx * r},${y1 + bulge}`,
    `L ${x2 - sx * r},${y1 + bulge}`,
    `Q ${x2},${y1 + bulge} ${x2},${y2}`
  ].join(' ')
}
