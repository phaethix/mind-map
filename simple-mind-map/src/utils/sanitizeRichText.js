const decodeEntities = value => {
  return value
    .replace(/&#x([0-9a-f]+);?/gi, (_, hex) => {
      const code = Number.parseInt(hex, 16)
      return Number.isFinite(code) ? String.fromCodePoint(code) : ''
    })
    .replace(/&#(\d+);?/g, (_, num) => {
      const code = Number(num)
      return Number.isFinite(code) ? String.fromCodePoint(code) : ''
    })
    .replace(/&colon;/gi, ':')
    .replace(/&amp;/gi, '&')
}

const safeUrl = value => {
  const decoded = decodeEntities(value).replace(/[\u0000-\u0020]+/g, '')
  if (/^javascript:/i.test(decoded)) return false
  // 图片的 data:image 要留下，其它 data: 可以当成页面
  if (/^data:(?!image\/)/i.test(decoded)) return false
  return true
}

// 只改标签，不改标签外面的文字。javascript: 出现在正文里要原样留下。
const sanitizeTag = tag => {
  let cleaned = tag.replace(
    /\son[a-z]+\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)/gi,
    ''
  )
  cleaned = cleaned.replace(
    /(\s(?:href|src|xlink:href)\s*=\s*)(?:"([^"]*)"|'([^']*)'|([^\s>]+))/gi,
    (match, prefix, doubleQuoted, singleQuoted, bare) => {
      const quote = doubleQuoted != null ? '"' : singleQuoted != null ? "'" : ''
      const raw = doubleQuoted ?? singleQuoted ?? bare ?? ''
      if (safeUrl(raw)) return match
      return `${prefix}${quote}${quote}`
    }
  )
  return cleaned
}

export const sanitizeRichText = value => {
  if (typeof value !== 'string' || !value) return ''
  let html = value.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, '')
  html = html.replace(/<iframe\b[^>]*>[\s\S]*?<\/iframe>/gi, '')
  html = html.replace(/<\/?(?:script|iframe|object|embed|link|meta)\b[^>]*>/gi, '')
  html = html.replace(/<[^>]+>/g, sanitizeTag)
  return html
}
