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
    entryPoints: ['src/plugins/outerFrame/outerFrameUtils.js'],
    bundle: true,
    platform: 'node',
    format: 'cjs',
    write: false,
    nodePaths: [webNodeModules]
  })
  const filename = path.join(__dirname, 'outer-frame-bundle.cjs')
  const mod = new Module(filename)
  mod.filename = filename
  mod.paths = Module._nodeModulePaths(path.dirname(filename))
  mod._compile(result.outputFiles[0].text, filename)
  return mod.exports
}

const rect = (top, height) => ({ left: 0, top, width: 100, height })
const near = list =>
  list.filter(item => item !== null && item !== undefined)

async function main() {
  const { clampOuterFramePadding, collectNodeRects } = await loadUtils()

  // 兄弟之间空隙充足，留白保持默认值
  assert.deepStrictEqual(
    clampOuterFramePadding(
      rect(100, 50),
      near([rect(20, 40), rect(200, 40)]),
      10,
      0
    ),
    { top: 10, bottom: 10 }
  )

  // 下方只有 14px 空隙，留白收到 7px，边框正好停在中间
  assert.deepStrictEqual(
    clampOuterFramePadding(
      rect(100, 50),
      near([rect(20, 40), rect(164, 40)]),
      10,
      0
    ),
    { top: 10, bottom: 7 }
  )

  // 上方同样受限：兄弟底部在 90，空隙 10，留白收到 5
  assert.deepStrictEqual(
    clampOuterFramePadding(rect(100, 50), near([rect(50, 40)]), 10, 0),
    { top: 5, bottom: 10 }
  )
  assert.deepStrictEqual(
    clampOuterFramePadding(rect(100, 50), near([rect(85, 8)]), 10, 0),
    { top: 3.5, bottom: 10 }
  )

  // 下方节点与外框贴在一起时，下方留白收到 0
  assert.deepStrictEqual(
    clampOuterFramePadding(rect(100, 50), near([rect(150, 40)]), 10, 0),
    { top: 10, bottom: 0 }
  )

  // 邻居没有渲染出来（包围盒是 Infinity）时不改留白
  assert.deepStrictEqual(
    clampOuterFramePadding(
      rect(100, 50),
      near([
        { left: 0, top: Infinity, width: 0, height: -Infinity }
      ]),
      10,
      0
    ),
    { top: 10, bottom: 10 }
  )

  // 其他分支的文字只和外框横向重叠一部分，也要限制上方留白；
  // 只看同级兄弟时，这种跨分支遮挡会被漏掉
  const nearby = { left: 80, top: 60, width: 80, height: 30 }
  assert.deepStrictEqual(
    clampOuterFramePadding(rect(100, 50), near([nearby]), 10, 10),
    { top: 5, bottom: 10 }
  )

  // 外框自己罩住的节点不应该把留白压成 0
  assert.deepStrictEqual(
    clampOuterFramePadding(rect(100, 50), near([rect(110, 20)]), 10, 0),
    { top: 10, bottom: 10 }
  )

  // 并排的兄弟不算上下方：组织架构图里三个子节点 top 相同，
  // 中间那个外框不能把上下留白压成 0
  const side = (left, top) => ({ left, top, width: 80, height: 40 })
  assert.deepStrictEqual(
    clampOuterFramePadding(
      side(200, 100),
      near([side(0, 100), side(400, 100)]),
      10,
      0
    ),
    { top: 10, bottom: 10 }
  )

  // 自己还没渲染时同样保持默认值
  assert.deepStrictEqual(
    clampOuterFramePadding(
      { left: Infinity, top: Infinity, width: -Infinity, height: -Infinity },
      near([rect(20, 40), rect(200, 40)]),
      10,
      0
    ),
    { top: 10, bottom: 10 }
  )

  // 概要节点也要参与碰撞检测
  const root = {
    group: {},
    children: [
      { group: {} },
      {
        group: {},
        _generalizationList: [
          { generalizationNode: { group: {} } },
          { generalizationNode: {} }
        ]
      }
    ]
  }
  assert.deepStrictEqual(collectNodeRects(root, node => node), [
    root,
    root.children[0],
    root.children[1],
    root.children[1]._generalizationList[0].generalizationNode
  ])

  console.log('outer frame padding ok')
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
