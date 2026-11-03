// 只有配额错误才该提示“存储空间不足”。其它异常不是磁盘满了。
export const isQuotaExceededError = error => {
  if (!error || typeof error !== 'object') return false
  const name = error.name || ''
  if (name === 'QuotaExceededError' || name === 'NS_ERROR_DOM_QUOTA_REACHED') {
    return true
  }
  return error.code === 22 || error.code === 1014
}
