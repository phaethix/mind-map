const assert = require('assert')
const esbuild = require('esbuild')
const path = require('path')
const vm = require('vm')

const bundled = esbuild.buildSync({
  entryPoints: [path.join(__dirname, 'githubStatus.js')],
  bundle: true,
  format: 'cjs',
  write: false,
  platform: 'node'
})
const sandbox = { module: { exports: {} }, exports: {}, require }
sandbox.exports = sandbox.module.exports
vm.runInNewContext(bundled.outputFiles[0].text, sandbox)
const { githubSidebarLabel } = sandbox.module.exports

function assertLabel(status, key, login) {
  const result = githubSidebarLabel(status)
  assert.strictEqual(result.key, key)
  if (login === undefined) {
    assert.strictEqual(result.params, undefined)
    return
  }
  assert.strictEqual(result.params.login, login)
}

assertLabel(null, 'github.statusOff')
assertLabel(undefined, 'github.statusOff')
assertLabel({}, 'github.statusOff')
assertLabel({ signedIn: false, login: 'octocat' }, 'github.statusOff')

assertLabel({ signedIn: true, login: 'octocat' }, 'github.statusOn', 'octocat')

// 有访问令牌但用户名还没写上时，侧栏不能显示「未登录」。
assertLabel({ signedIn: true, login: '' }, 'github.signedInUnknown')
assertLabel({ signedIn: true }, 'github.signedInUnknown')
assertLabel({ signedIn: true, login: null }, 'github.signedInUnknown')
