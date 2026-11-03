const assert = require('assert')
const path = require('path')
const Module = require('module')

const webNodeModules = path.join(__dirname, '../../web/node_modules')
const esbuild = require(path.join(webNodeModules, 'esbuild'))

async function loadUtils() {
  const result = await esbuild.build({
    absWorkingDir: path.join(__dirname, '..'),
    entryPoints: ['src/utils/sanitizeRichText.js'],
    bundle: true,
    platform: 'node',
    format: 'cjs',
    write: false,
    nodePaths: [webNodeModules]
  })
  const filename = path.join(__dirname, 'sanitize-rich-text-bundle.cjs')
  const mod = new Module(filename)
  mod.filename = filename
  mod.paths = Module._nodeModulePaths(path.dirname(filename))
  mod._compile(result.outputFiles[0].text, filename)
  return mod.exports
}

async function main() {
  const { sanitizeRichText } = await loadUtils()
  assert.strictEqual(sanitizeRichText('<p><strong>ok</strong></p>'), '<p><strong>ok</strong></p>')
  assert.strictEqual(sanitizeRichText(null), '')

  const attack =
    '<img src=x onerror="window.__TAURI_INTERNALS__.invoke(\'delete_file\',{path:\'/tmp\'})">'
  const cleaned = sanitizeRichText(attack)
  assert.ok(!/onerror/i.test(cleaned), cleaned)
  assert.ok(!cleaned.includes('__TAURI_INTERNALS__'), cleaned)

  assert.ok(!/onerror/i.test(sanitizeRichText('<img src=x onerror=alert(1)>')))
  assert.strictEqual(
    sanitizeRichText('<p>a</p><script>alert(1)</script><p>b</p>'),
    '<p>a</p><p>b</p>'
  )
  assert.ok(
    !/javascript\s*:/i.test(
      sanitizeRichText('<a href="javascript:alert(1)">x</a>')
    )
  )
  assert.ok(sanitizeRichText('<a href="https://example.com">x</a>').includes('https://example.com'))
  assert.ok(
    sanitizeRichText('<img src="data:image/png;base64,aaa">').includes(
      'data:image/png;base64,aaa'
    )
  )

  // 正文里的 JavaScript: 和 onerror= 不是属性，不能删
  assert.strictEqual(
    sanitizeRichText('<p>JavaScript: the good parts</p>'),
    '<p>JavaScript: the good parts</p>'
  )
  assert.strictEqual(
    sanitizeRichText('<p>see onerror=resume now</p>'),
    '<p>see onerror=resume now</p>'
  )
  const encoded = sanitizeRichText('<a href="jav&#x61;script:alert(1)">x</a>')
  assert.ok(encoded.includes('href=""'), encoded)
  assert.ok(!encoded.includes('alert(1)'), encoded)

  console.log('sanitize rich text ok')
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
