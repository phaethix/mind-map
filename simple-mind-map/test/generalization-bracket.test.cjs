const assert = require('assert')
const path = require('path')
const Module = require('module')

const webNodeModules = path.join(__dirname, '../../web/node_modules')
const esbuild = require(path.join(webNodeModules, 'esbuild'))

async function loadUtils() {
  const result = await esbuild.build({
    absWorkingDir: path.join(__dirname, '..'),
    entryPoints: ['src/layouts/generalizationBracket.js'],
    bundle: true,
    platform: 'node',
    format: 'cjs',
    write: false,
    nodePaths: [webNodeModules]
  })
  const filename = path.join(__dirname, 'generalization-bracket-bundle.cjs')
  const mod = new Module(filename)
  mod.filename = filename
  mod.paths = Module._nodeModulePaths(path.dirname(filename))
  mod._compile(result.outputFiles[0].text, filename)
  return mod.exports
}

async function main() {
  const { generalizationBracketPath } = await loadUtils()

  // 右侧括号：中间是竖直线，两端才圆角，不再用一条大弧
  const side = generalizationBracketPath(0, 0, 0, 100, {
    horizontal: true,
    sign: 1,
    radius: 6
  })
  assert.strictEqual(
    side,
    'M 0,0 Q 6,0 6,6 L 6,94 Q 6,100 0,100'
  )
  assert.ok(!side.includes('Q 20,'))

  // 太短时圆角收到跨度的一半，不能鼓出括号
  const short = generalizationBracketPath(0, 0, 0, 8, {
    horizontal: true,
    sign: 1,
    radius: 6
  })
  assert.ok(short.includes('L 4,4'))

  // 下方括号：中间是水平直线
  const below = generalizationBracketPath(0, 10, 80, 10, {
    horizontal: false,
    sign: 1,
    radius: 6
  })
  assert.strictEqual(
    below,
    'M 0,10 Q 0,16 6,16 L 74,16 Q 80,16 80,10'
  )

  console.log('generalization bracket ok')
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
