import exampleData from 'simple-mind-map/example/exampleData'
import { simpleDeepClone } from 'simple-mind-map/src/utils/index'
import vuexStore from '@/store'
import { browserFileLibrary } from '@/utils/fileLibrary'
import { globals } from '@/utils/global'
import { isQuotaExceededError } from '@/utils/storageError'

const SIMPLE_MIND_MAP_DATA = 'SIMPLE_MIND_MAP_DATA'
const SIMPLE_MIND_MAP_CONFIG = 'SIMPLE_MIND_MAP_CONFIG'
const SIMPLE_MIND_MAP_LANG = 'SIMPLE_MIND_MAP_LANG'
const SIMPLE_MIND_MAP_LOCAL_CONFIG = 'SIMPLE_MIND_MAP_LOCAL_CONFIG'
const SIMPLE_MIND_MAP_THEME_MIGRATED = 'SIMPLE_MIND_MAP_THEME_MIGRATED'
const DEFAULT_THEME = 'classic7'

let mindMapData = null

const withDefaultTheme = data => {
  const next = simpleDeepClone(data)
  const config = (next.theme && next.theme.config) || {}
  next.theme = {
    ...(next.theme || {}),
    template: DEFAULT_THEME,
    config
  }
  return next
}

// 已经打开过网页的浏览器会一直用本地缓存。旧默认主题只迁移一次。
const migrateStoredTheme = data => {
  if (localStorage.getItem(SIMPLE_MIND_MAP_THEME_MIGRATED) === DEFAULT_THEME) {
    return data
  }
  const theme = data && data.theme
  const config = (theme && theme.config) || {}
  const usesOldDefault =
    theme &&
    Object.keys(config).length === 0 &&
    (theme.template === 'classic4' || theme.template === 'default')
  if (usesOldDefault) {
    data.theme = {
      ...theme,
      template: DEFAULT_THEME,
      config
    }
    localStorage.setItem(SIMPLE_MIND_MAP_DATA, JSON.stringify(data))
  }
  localStorage.setItem(SIMPLE_MIND_MAP_THEME_MIGRATED, DEFAULT_THEME)
  return data
}

// 获取缓存的思维导图数据
export const getData = () => {
  // 接管模式
  if (window.takeOverApp) {
    mindMapData = window.takeOverAppMethods.getMindMapData()
    return mindMapData
  }
  // 操作本地文件模式
  if (vuexStore.state.isHandleLocalFile) {
    return globals.getCurrentData()
  }
  let store = localStorage.getItem(SIMPLE_MIND_MAP_DATA)
  if (store === null) {
    try {
      localStorage.setItem(SIMPLE_MIND_MAP_THEME_MIGRATED, DEFAULT_THEME)
    } catch (error) {
      console.log(error)
    }
    return withDefaultTheme(exampleData)
  } else {
    try {
      const data = JSON.parse(store)
      try {
        return migrateStoredTheme(data)
      } catch (error) {
        return data
      }
    } catch (error) {
      return withDefaultTheme(exampleData)
    }
  }
}

// 存储思维导图数据
export const storeData = data => {
  try {
    let originData = null
    if (window.takeOverApp) {
      originData = window.takeOverAppMethods.getMindMapData()
    } else {
      originData = getData()
    }
    if (!originData) {
      originData = {}
    }
    originData = {
      ...originData,
      ...data
    }
    if (window.takeOverApp) {
      mindMapData = originData
      window.takeOverAppMethods.saveMindMapData(originData)
      return
    }
    globals.bus.$emit('write_local_file', originData)
    if (vuexStore.state.isHandleLocalFile) {
      return
    }
    localStorage.setItem(SIMPLE_MIND_MAP_DATA, JSON.stringify(originData))
    const currentId = localStorage.getItem('SIMPLE_MIND_MAP_CURRENT_ID')
    if (currentId) {
      browserFileLibrary().save(currentId, originData)
    }
  } catch (error) {
    console.log(error)
    if (isQuotaExceededError(error)) {
      globals.bus.$emit('localStorageExceeded')
    }
  }
}

// 获取思维导图配置数据
export const getConfig = () => {
  if (window.takeOverApp) {
    return window.takeOverAppMethods.getMindMapConfig()
  }
  let config = localStorage.getItem(SIMPLE_MIND_MAP_CONFIG)
  if (config) {
    return JSON.parse(config)
  }
  return null
}

// 存储思维导图配置数据
export const storeConfig = config => {
  try {
    if (window.takeOverApp) {
      window.takeOverAppMethods.saveMindMapConfig(config)
      return
    }
    localStorage.setItem(SIMPLE_MIND_MAP_CONFIG, JSON.stringify(config))
  } catch (error) {
    console.log(error)
  }
}

// 存储语言
export const storeLang = lang => {
  if (window.takeOverApp) {
    window.takeOverAppMethods.saveLanguage(lang)
    return
  }
  localStorage.setItem(SIMPLE_MIND_MAP_LANG, lang)
}

// 获取存储的语言
export const getLang = () => {
  if (window.takeOverApp) {
    return window.takeOverAppMethods.getLanguage() || 'zh'
  }
  let lang = localStorage.getItem(SIMPLE_MIND_MAP_LANG)
  if (lang) {
    return lang
  }
  storeLang('zh')
  return 'zh'
}

// 存储本地配置
export const storeLocalConfig = config => {
  if (window.takeOverApp) {
    return window.takeOverAppMethods.saveLocalConfig(config)
  }
  localStorage.setItem(SIMPLE_MIND_MAP_LOCAL_CONFIG, JSON.stringify(config))
}

// 获取本地配置
export const getLocalConfig = () => {
  if (window.takeOverApp) {
    return window.takeOverAppMethods.getLocalConfig()
  }
  let config = localStorage.getItem(SIMPLE_MIND_MAP_LOCAL_CONFIG)
  if (config) {
    return JSON.parse(config)
  }
  return null
}
