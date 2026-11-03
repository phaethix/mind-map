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

async function main() {
  const { clampOuterFramePadding } = await loadUtils()

  // 兄弟之间空隙充足，留白保持默认值
  assert.deepStrictEqual(
    clampOuterFramePadding(
      rect(100, 50),
      rect(20, 40),
      rect(200, 40),
      10
    ),
    { top: 10, bottom: 10 }
  )

  // 下方只有 14px 空隙，留白收到 7px，边框正好停在中间
  assert.deepStrictEqual(
    clampOuterFramePadding(rect(100, 50), rect(20, 40), rect(164, 40), 10),
    { top: 10, bottom: 7 }
  )

  // 上方同样受限：兄弟底部在 90，空隙 10，留白收到 5
  assert.deepStrictEqual(
    clampOuterFramePadding(rect(100, 50), rect(50, 40), null, 10),
    { top: 5, bottom: 10 }
  )
  assert.deepStrictEqual(
    clampOuterFramePadding(rect(100, 50), rect(85, 8), null, 10),
    { top: 3.5, bottom: 10 }
  )

  // 节点已经贴在一起时不出现负留白
  assert.deepStrictEqual(
    clampOuterFramePadding(rect(100, 50), null, rect(140, 40), 10),
    { top: 10, bottom: 0 }
  )

  // 邻居没有渲染出来（包围盒是 Infinity）时不改留白
  assert.deepStrictEqual(
    clampOuterFramePadding(
      rect(100, 50),
      { left: 0, top: Infinity, width: 0, height: -Infinity },
      null,
      10
    ),
    { top: 10, bottom: 10 }
  )

  // 并排的兄弟不算上下方：组织架构图里三个子节点 top 相同，
  // 中间那个外框不能把上下留白压成 0
  const side = (left, top) => ({ left, top, width: 80, height: 40 })
  assert.deepStrictEqual(
    clampOuterFramePadding(side(200, 100), side(0, 100), side(400, 100), 10),
    { top: 10, bottom: 10 }
  )

  // 自己还没渲染时同样保持默认值
  assert.deepStrictEqual(
    clampOuterFramePadding(
      { left: Infinity, top: Infinity, width: -Infinity, height: -Infinity },
      rect(20, 40),
      rect(200, 40),
      10
    ),
    { top: 10, bottom: 10 }
  )

  console.log('outer frame padding ok')
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
