const assert = require('assert')
const path = require('path')
const Module = require('module')

const webNodeModules = path.join(__dirname, '../../web/node_modules')
const esbuild = require(path.join(webNodeModules, 'esbuild'))

async function loadUtils() {
  const result = await esbuild.build({
    absWorkingDir: path.join(__dirname, '..'),
    entryPoints: ['src/utils/cjkLatinSpacing.js'],
    bundle: true,
    platform: 'node',
    format: 'cjs',
    write: false,
    nodePaths: [webNodeModules]
  })
  const filename = path.join(__dirname, 'cjk-latin-spacing-bundle.cjs')
  const mod = new Module(filename)
  mod.filename = filename
  mod.paths = Module._nodeModulePaths(path.dirname(filename))
  mod._compile(result.outputFiles[0].text, filename)
  return mod.exports
}

const gap = '\u2009'

async function main() {
  const { spaceCjkLatin, spaceCjkLatinHtml } = await loadUtils()

  assert.strictEqual(spaceCjkLatin('中文API'), `中文${gap}API`)
  assert.strictEqual(spaceCjkLatin('API核心'), `API${gap}核心`)
  assert.strictEqual(spaceCjkLatin('第1章'), `第${gap}1${gap}章`)
  assert.strictEqual(spaceCjkLatin('增长20%'), `增长${gap}20%`)
  assert.strictEqual(spaceCjkLatin('Hello'), 'Hello')
  assert.strictEqual(spaceCjkLatin('中文'), '中文')
  assert.strictEqual(spaceCjkLatin('中文 API'), '中文 API')
  assert.strictEqual(spaceCjkLatin(''), '')
  assert.strictEqual(spaceCjkLatin(null), null)

  assert.strictEqual(
    spaceCjkLatinHtml('<p>中文<strong>API</strong>核心</p>'),
    `<p>中文<strong>${gap}API</strong>${gap}核心</p>`
  )
  assert.strictEqual(
    spaceCjkLatinHtml('<p>中文</p><p>API</p>'),
    '<p>中文</p><p>API</p>'
  )
  assert.strictEqual(spaceCjkLatinHtml('<p>中文<br>API</p>'), '<p>中文<br>API</p>')

  console.log('cjk latin spacing ok')
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
