const assert = require('assert')
const fs = require('fs')
const path = require('path')

const root = path.join(__dirname, '../../..')
const indexHtml = fs.readFileSync(path.join(root, 'web/index.html'), 'utf8')
const viteConfig = fs.readFileSync(
  path.join(root, 'web/vite.config.mjs'),
  'utf8'
)
const repoNode = fs.readFileSync(
  path.join(root, 'simple-mind-map/src/core/render/node/nodeCreateContents.js'),
  'utf8'
)
const publishedNode = fs.readFileSync(
  path.join(
    root,
    'web/node_modules/simple-mind-map/src/core/render/node/nodeCreateContents.js'
  ),
  'utf8'
)

const saveBranch = indexHtml.slice(
  indexHtml.indexOf("action === 'save'"),
  indexHtml.indexOf("action === 'reveal'")
)
assert.ok(
  saveBranch.includes("'$emit', 'saveMindMap'") ||
    saveBranch.includes("$emit('saveMindMap')"),
  saveBranch
)
assert.ok(!saveBranch.includes('save_mind_map'), saveBranch)

const saveAsBranch = indexHtml.slice(
  indexHtml.indexOf("action === 'save-as'"),
  indexHtml.indexOf("action === 'open-pending'")
)
assert.ok(saveAsBranch.includes('__desktopDocumentData'), saveAsBranch)
assert.ok(!saveAsBranch.includes('mindMapData'), saveAsBranch)

assert.ok(indexHtml.includes('take_pending_open'))
const boot = indexHtml.slice(indexHtml.indexOf('window.onload'))
assert.ok(boot.includes('take_pending_open'), boot)

assert.ok(/find:\s*\/\^simple-mind-map\$\//.test(viteConfig), viteConfig)
assert.ok(viteConfig.includes('simple-mind-map/index.js'))
assert.ok(repoNode.includes("node.attr('text-anchor', 'start')"))
assert.ok(publishedNode.includes("center: 'middle'"))

console.log('desktop shell ok')
