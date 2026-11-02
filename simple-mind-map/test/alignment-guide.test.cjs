const assert = require('assert')
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
    entryPoints: ['src/plugins/alignmentGuide/alignmentGuideUtils.js'],
    bundle: true,
    platform: 'node',
    format: 'cjs',
    write: false,
    nodePaths: [webNodeModules]
  })
  const filename = path.join(__dirname, 'alignment-guide-bundle.cjs')
  const mod = new Module(filename)
  mod.filename = filename
  mod.paths = Module._nodeModulePaths(path.dirname(filename))
  mod._compile(result.outputFiles[0].text, filename)
  return mod.exports
}

const box = (left, top, width, height) => ({ left, top, width, height })

async function main() {
  const { computeAlignmentGuide } = await loadUtils()

  // 左边差 3px：吸附到 100，并给出竖直参考线
  const edge = computeAlignmentGuide(
    { x: 103, y: 50, width: 80, height: 30 },
    [box(100, 200, 60, 20)],
    6,
    1
  )
  assert.strictEqual(edge.dx, -3)
  assert.strictEqual(edge.dy, 0)
  assert.strictEqual(edge.guides.length, 1)
  assert.strictEqual(edge.guides[0].axis, 'x')
  assert.strictEqual(edge.guides[0].value, 100)
  // 参考线要同时盖住两个节点
  assert.strictEqual(edge.guides[0].start, 50)
  assert.strictEqual(edge.guides[0].end, 220)

  // 超过阈值不吸附，也不画线
  const far = computeAlignmentGuide(
    { x: 130, y: 50, width: 80, height: 30 },
    [box(400, 500, 60, 20)],
    6,
    1
  )
  assert.deepStrictEqual(far, { dx: 0, dy: 0, guides: [] })

  // 中线对齐：两边中线重合
  const center = computeAlignmentGuide(
    { x: 90, y: 50, width: 80, height: 30 },
    [box(100, 200, 60, 20)],
    6,
    1
  )
  assert.strictEqual(center.dx, 0)
  assert.strictEqual(center.guides[0].value, 130)

  // 两个轴同时吸附
  const both = computeAlignmentGuide(
    { x: 102, y: 198, width: 80, height: 30 },
    [box(100, 200, 60, 20)],
    6,
    1
  )
  assert.strictEqual(both.dx, -2)
  assert.strictEqual(both.dy, 2)
  assert.deepStrictEqual(
    both.guides.map(item => item.axis),
    ['x', 'y']
  )

  // 缩放越大，同样的屏幕阈值对应的画布距离越小
  const scaled = computeAlignmentGuide(
    { x: 105, y: 50, width: 80, height: 30 },
    [box(100, 200, 60, 20)],
    6,
    2
  )
  assert.strictEqual(scaled.dx, 0)
  assert.strictEqual(scaled.guides.length, 0)

  // 一条线上有多个节点时，参考线要一直画到最远的那个
  const multi = computeAlignmentGuide(
    { x: 100, y: 50, width: 80, height: 30 },
    [box(100, 200, 60, 20), box(100, 400, 60, 20)],
    6,
    1
  )
  assert.strictEqual(multi.guides[0].start, 50)
  assert.strictEqual(multi.guides[0].end, 420)

  console.log('alignment guide ok')
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
