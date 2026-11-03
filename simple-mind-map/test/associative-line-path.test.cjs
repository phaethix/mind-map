const assert = require('assert')
const fs = require('fs')
const path = require('path')
const Module = require('module')

const webNodeModules = path.join(__dirname, '../../web/node_modules')
const esbuild = require(path.join(webNodeModules, 'esbuild'))

global.document = {
  documentElement: {},
  createElement() {
    return {
      set innerHTML(html) {
        this._html = String(html)
      },
      get textContent() {
        return (this._html || '').replace(/<[^>]+>/g, '')
      }
    }
  }
}

async function loadUtils() {
  const result = await esbuild.build({
    absWorkingDir: path.join(__dirname, '..'),
    entryPoints: ['src/plugins/associativeLine/associativeLineUtils.js'],
    bundle: true,
    platform: 'node',
    format: 'cjs',
    write: false,
    nodePaths: [webNodeModules]
  })
  const filename = path.join(__dirname, 'associative-line-bundle.cjs')
  const mod = new Module(filename)
  mod.filename = filename
  mod.paths = Module._nodeModulePaths(path.dirname(filename))
  mod._compile(result.outputFiles[0].text, filename)
  return mod.exports
}

async function main() {
  const {
    getStraightLinePath,
    getStraightLineMid,
    getStraightHandles,
    getStraightAxis,
    getLineCornerRadius,
    isFreeEndPoint,
    previewAssociativeLinePath
  } = await loadUtils()

  // 端点被拖到任意位置后不再跟着节点走：只认 x/y，没有 dir
  assert.strictEqual(isFreeEndPoint({ x: 10, y: 20 }), true)
  assert.strictEqual(isFreeEndPoint({ dir: 'right', range: 0 }), false)
  assert.strictEqual(isFreeEndPoint(null), false)

  // 拐角半径：直角折线为 0，其余 12
  assert.strictEqual(getLineCornerRadius('sharp'), 0)
  assert.strictEqual(getLineCornerRadius('elbow'), 12)
  assert.strictEqual(getLineCornerRadius('curve'), 12)

  // 保存的偏移只有一个轴，主轴翻转后仍按原轴还原
  assert.strictEqual(
    getStraightAxis([
      { x: 10, y: 0 },
      { x: -10, y: 0 }
    ]),
    'x'
  )
  assert.strictEqual(
    getStraightAxis([
      { x: 0, y: 5 },
      { x: 0, y: -5 }
    ]),
    'y'
  )
  assert.strictEqual(
    getStraightAxis([
      { x: 0, y: 0 },
      { x: 0, y: 0 }
    ]),
    ''
  )

  // 折弯段的默认位置取两点中间，两个控制点摆在同一段线上
  const start = { x: 0, y: 0 }
  const end = { x: 200, y: 100 }
  assert.strictEqual(getStraightLineMid(start, end), 100)
  const handles = getStraightHandles(start, end, 100)
  assert.strictEqual(handles.length, 2)
  assert.ok(handles.every(item => item.x === 100))
  assert.ok(handles[0].y < handles[1].y)

  // 拖动控制点后中点为两个控制点的平均位置
  assert.strictEqual(
    getStraightLineMid(start, end, { x: 60, y: 30 }, { x: 80, y: 70 }),
    70
  )
  // 纵向为主时看 y
  const tallStart = { x: 0, y: 0 }
  const tallEnd = { x: 40, y: 300 }
  assert.strictEqual(getStraightLineMid(tallStart, tallEnd), 150)
  assert.strictEqual(
    getStraightLineMid(tallStart, tallEnd, { x: 0, y: 220 }, { x: 40, y: 220 }),
    220
  )
  const tallHandles = getStraightHandles(tallStart, tallEnd, 220)
  assert.ok(tallHandles.every(item => item.y === 220))

  // 指定中点后竖向段跟着移动
  const shifted = getStraightLinePath(start, end, 12, 160)
  assert.ok(shifted.includes('L148,0'))

  // 拖过端点时夹回两端之间，路径不会折返
  const clamped = getStraightLinePath(start, end, 12, 400)
  assert.ok(clamped.includes('L188,0'))

  // 显式指定纵向时，即使横向跨度更大也走竖直-水平-竖直
  const forcedVertical = getStraightLinePath(start, end, 12, 40, 'y')
  assert.ok(forcedVertical.startsWith('M0,0 L0,'))

  // 横向为主：水平-竖直-水平，两个拐角各一个二次贝塞尔
  const wide = getStraightLinePath({ x: 0, y: 0 }, { x: 200, y: 100 }, 12)
  assert.strictEqual(wide.split('Q').length - 1, 2)
  assert.ok(wide.startsWith('M0,0'))
  assert.ok(wide.endsWith('L200,100'))
  // 末段与终点等高，箭头才能水平指进节点
  const wideTail = wide.split('L').pop()
  assert.strictEqual(wideTail, '200,100')

  // 纵向为主：竖直-水平-竖直，中间那段在 y=150 上
  const tall = getStraightLinePath({ x: 0, y: 0 }, { x: 40, y: 300 }, 12)
  assert.strictEqual(tall.split('Q').length - 1, 2)
  assert.ok(tall.endsWith('L40,300'))
  const horizontal = tall
    .split(' ')
    .filter(item => item.startsWith('L'))
    .find(item => {
      const [, y] = item.slice(1).split(',')
      return Math.abs(Number(y) - 150) < 0.001
    })
  assert.ok(horizontal, 'middle horizontal segment is missing')

  // 几乎共线时退化成一条直线，不出现细碎折角
  assert.strictEqual(
    getStraightLinePath({ x: 0, y: 0 }, { x: 200, y: 0 }),
    'M0,0 L200,0'
  )
  assert.strictEqual(
    getStraightLinePath({ x: 0, y: 0 }, { x: 0, y: 200 }),
    'M0,0 L200,0'.replace('L200,0', 'L0,200')
  )

  // radius 传 0 走直角折线：拐角处控制点与端点重合
  const sharp = getStraightLinePath({ x: 0, y: 0 }, { x: 200, y: 100 }, 0)
  assert.ok(sharp.includes('L100,0'))
  assert.ok(sharp.includes('Q100,0 100,0'))

  // 圆角半径不会超过可用空间的一半
  const tight = getStraightLinePath({ x: 0, y: 0 }, { x: 10, y: 6 }, 12)
  const radius = Number(tight.match(/L-?([\d.]+),0/)[1])
  assert.ok(radius <= 5)

  // 反向（终点在起点左上方）也要闭合到终点
  const back = getStraightLinePath({ x: 100, y: 100 }, { x: 0, y: 0 }, 12)
  assert.ok(back.startsWith('M100,100'))
  assert.ok(back.endsWith('L0,0'))

  // 拖动手柄时的预览必须跟真正渲染用同一套形状，不能一律画成圆角折线
  assert.strictEqual(
    previewAssociativeLinePath('line', start, end, null, null),
    'M0,0 L200,100'
  )
  assert.strictEqual(
    previewAssociativeLinePath('sharp', start, end, null, null),
    getStraightLinePath(start, end, 0)
  )
  assert.strictEqual(
    previewAssociativeLinePath('elbow', start, end, null, null),
    getStraightLinePath(start, end, 12)
  )
  assert.notStrictEqual(
    previewAssociativeLinePath('sharp', start, end, null, null),
    previewAssociativeLinePath('elbow', start, end, null, null)
  )
  assert.ok(
    previewAssociativeLinePath('curve', start, end, { x: 100, y: 0 }, { x: 100, y: 100 }).includes(
      'C'
    )
  )

  const controls = fs.readFileSync(
    path.join(
      __dirname,
      '../src/plugins/associativeLine/associativeLineControls.js'
    ),
    'utf8'
  )
  const hide = controls.slice(
    controls.indexOf('function hideControls'),
    controls.indexOf('function showControls')
  )
  assert.ok(hide.includes('startPointHandle'), hide)
  assert.ok(hide.includes('endPointHandle'), hide)
  assert.ok(!/if\s*\(\s*!this\.controlLine1\s*\)\s*return/.test(hide), hide)

  console.log('associative line path ok')
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
