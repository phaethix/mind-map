const assert = require('assert')
const esbuild = require('esbuild')
const path = require('path')
const vm = require('vm')

function load(file) {
  const bundled = esbuild.buildSync({
    entryPoints: [path.join(__dirname, file)],
    bundle: true,
    format: 'cjs',
    write: false,
    platform: 'node'
  })
  const sandbox = { module: { exports: {} }, exports: {}, require }
  sandbox.exports = sandbox.module.exports
  vm.runInNewContext(bundled.outputFiles[0].text, sandbox)
  return sandbox.module.exports
}

const { isQuotaExceededError } = load('storageError.js')
const { shouldResetView } = load('documentView.js')
const { canDeleteHoveredNode } = load('hoverNode.js')

assert.strictEqual(isQuotaExceededError({ name: 'QuotaExceededError' }), true)
assert.strictEqual(
  isQuotaExceededError({ name: 'NS_ERROR_DOM_QUOTA_REACHED' }),
  true
)
assert.strictEqual(isQuotaExceededError({ code: 22 }), true)
assert.strictEqual(isQuotaExceededError(new Error('stringify failed')), false)
assert.strictEqual(isQuotaExceededError(null), false)
assert.strictEqual(isQuotaExceededError('exceeded'), false)

assert.strictEqual(shouldResetView({ root: {}, view: { scale: 2 } }), false)
assert.strictEqual(shouldResetView({ root: {}, view: null }), true)
assert.strictEqual(
  shouldResetView({ data: { text: '根' }, children: [] }),
  false
)
assert.strictEqual(shouldResetView(null), false)

assert.strictEqual(canDeleteHoveredNode({ isRoot: true }), false)
assert.strictEqual(canDeleteHoveredNode({ isGeneralization: true }), false)
assert.strictEqual(canDeleteHoveredNode({ isRoot: false }), true)
assert.strictEqual(canDeleteHoveredNode(null), false)

console.log('editor guards ok')
