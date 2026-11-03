import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import Components from 'unplugin-vue-components/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'
import fs from 'fs'
import path from 'path'

// simple-mind-map 以源码形式引入（../simple-mind-map/src），而那个目录在仓库里没有
// node_modules，裸包名按默认规则解析不到。把引擎用到的第三方包显式指到 web/node_modules。
const ENGINE_DEPS = [
  '@svgdotjs/svg.js',
  'deepmerge',
  'eventemitter3',
  'jszip',
  'katex',
  'mdast-util-from-markdown',
  'pdf-lib',
  'parchment',
  'quill',
  'quill-delta',
  'uuid',
  'xml-js',
  'y-webrtc',
  'yjs'
]

const engineAlias = ENGINE_DEPS.filter(name => {
  const pkgDir = path.resolve(__dirname, 'node_modules', name)
  if (!fs.existsSync(pkgDir)) {
    throw new Error(`缺少引擎依赖 ${name}，请先在 web 下安装`)
  }
  return true
}).map(name => ({
  find: name,
  replacement: path.resolve(__dirname, 'node_modules', name)
}))

/**
 * 产物目录结构沿用 webpack 时代：js/、css/、img/ 平铺在 dist 下，
 * index.html 由 scripts/copy.js 提到仓库根目录并把资源前缀补成 ./dist/。
 */
const buildOutput = {
  entryFileNames: 'js/[name].js',
  chunkFileNames: 'js/[name].js',
  assetFileNames: assetInfo => {
    const name = assetInfo.names?.[0] || assetInfo.name || ''
    if (name.endsWith('.css')) {
      return 'css/[name][extname]'
    }
    // index.html 也算 asset，不处理会被塞进 img/ 目录
    if (name.endsWith('.html')) {
      return '[name][extname]'
    }
    if (/\.(woff2?|ttf|eot|svg|png|jpe?g|gif|webp|ico)$/.test(name)) {
      return 'img/[name][extname]'
    }
    return 'js/[name][extname]'
  }
}

/**
 * element-plus 的组件样式是按需注入的：unplugin-vue-components 在 transform 阶段
 * 才往 .vue 里塞 `element-plus/es/components/x/style/css`，Vite 的依赖扫描器读的是
 * 原始源码，看不到这些引用。结果就是每打开一个还没访问过的页面，就"发现"一批新依赖
 * → 重新预构建 → 整页 reload，开发时的状态会丢。
 *
 * 这里把已安装版本里的组件样式全部列出来，启动时一次性预构建。
 * 不能用 `element-plus/es/components/*\/style/css` 通配：Vite 展开通配时走的是包
 * 的 exports 映射，得到的是带 .mjs 后缀的键，和运行时的无后缀说明符对不上。
 */
const EL_COMPONENTS_DIR = path.resolve(
  __dirname,
  'node_modules/element-plus/es/components'
)
const elementPlusStyleDeps = (() => {
  if (!fs.existsSync(EL_COMPONENTS_DIR)) {
    // 依赖没装好时静默降级，会让「每打开一个新页面就整页刷新」的老问题悄悄回来，
    // 所以这里显式提醒一句，而不是直接返回空数组。
    console.warn(
      '[vite] element-plus is not installed, component styles will not be pre-bundled'
    )
    return []
  }
  return fs
    .readdirSync(EL_COMPONENTS_DIR)
    .filter(name =>
      fs.existsSync(path.join(EL_COMPONENTS_DIR, name, 'style/css.mjs'))
    )
    .map(name => `element-plus/es/components/${name}/style/css`)
})()

export default defineConfig(({ command }) => ({
  // 页面最终被 copy.js 提到仓库根目录，资源留在 dist/ 下
  base: command === 'build' ? './' : '/',
  plugins: [
    vue(),
    // 模板里的 el-* 按需引入：只打包用到的组件和它们各自的样式，
    // 不再整包引入 element-plus（整包 CSS 有 405KB）
    Components({
      dts: false,
      resolvers: [ElementPlusResolver({ importStyle: 'css' })]
    })
  ],
  resolve: {
    alias: [
      ...engineAlias,
      { find: '@', replacement: path.resolve(__dirname, 'src') },
      // sax 只在 Node 下才用 stream，构建期给它一个空模块，避免 "externalized for browser" 提示
      {
        find: /^stream$/,
        replacement: path.resolve(__dirname, 'build/empty-module.js')
      },
      {
        find: 'simple-mind-map/src',
        replacement: path.resolve(__dirname, '../simple-mind-map/src')
      },
      {
        find: 'simple-mind-map/example',
        replacement: path.resolve(__dirname, '../simple-mind-map/example')
      },
      // 包名本身也指向仓库里的引擎。否则核心从 node_modules 的旧副本加载，
      // 这里改过的节点渲染不会进编辑器。
      {
        find: /^simple-mind-map$/,
        replacement: path.resolve(__dirname, '../simple-mind-map/index.js')
      }
    ]
  },
  define: {
    'process.env.NODE_ENV': JSON.stringify(
      command === 'build' ? 'production' : 'development'
    )
  },
  css: {
    preprocessorOptions: {
      less: {
        javascriptEnabled: true
      }
    }
  },
  optimizeDeps: {
    include: [
      // 组件本身是从包根导入的：unplugin-vue-components 生成的 import 就是
      // `element-plus/es`。不预构建它，第一个用到组件的页面照样会触发一次重建。
      'element-plus/es',
      ...elementPlusStyleDeps
    ]
  },
  build: {
    outDir: '../dist',
    emptyOutDir: true,
    assetsDir: 'js',
    sourcemap: false,
    cssCodeSplit: true,
    /**
     * mermaid 自带的构建产物就有 3.3MB，是这里最大的单块，属于已知上限
     * （webpack 时代也是按 3.5MB 设的预算）。把阈值调到实际基线，
     * 免得每次构建都拿一条跟代码无关的提示刷屏。
     */
    chunkSizeWarningLimit: 3500,
    rollupOptions: {
      output: {
        ...buildOutput,
        /**
         * 只把几个「叶子」依赖单独分块：它们不参与 vue / element-plus 那套互相引用，
         * 拆出来不会出现分块初始化顺序问题，但可以让浏览器并行下载、
         * 并且和业务代码分开缓存。
         *
         * katex 不能改成动态加载：quill 的公式 blot 通过 window.katex 同步渲染，
         * 插件构造函数里就要把它挂上，延后加载会让含公式的旧文档渲染失败。
         */
        manualChunks: id => {
          if (id.includes('node_modules/katex')) {
            return 'katex'
          }
          if (id.includes('node_modules/quill')) {
            return 'quill'
          }
          if (id.includes('simple-mind-map-plugin-themes')) {
            return 'themes'
          }
        }
      }
    }
  },
  server: {
    port: 8080,
    proxy: {
      '^/api/v3/': {
        target: 'http://ark.cn-beijing.volces.com',
        changeOrigin: true
      }
    }
  }
}))
