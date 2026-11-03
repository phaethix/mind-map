// 只在汉字和英文/数字交界处加一点空隙，不要拉开英文单词内部。
const HAN = /\p{Script=Han}/u
const LATIN = /\p{Script=Latin}/u
const DIGIT = /[0-9]/
const GAP = '\u2009'
const INLINE = new Set([
  'SPAN',
  'STRONG',
  'EM',
  'B',
  'I',
  'U',
  'A',
  'FONT',
  'S',
  'SUB',
  'SUP',
  'CODE',
  'MARK'
])

const isHan = char => HAN.test(char)
const isLatinOrDigit = char => LATIN.test(char) || DIGIT.test(char)

const needsGap = (prev, next) => {
  return (
    (isHan(prev) && isLatinOrDigit(next)) ||
    (isLatinOrDigit(prev) && isHan(next))
  )
}

const charsOf = text => Array.from(String(text))

export const spaceCjkLatin = text => {
  if (text == null || text === '') return text
  const chars = charsOf(text)
  let out = ''
  for (let index = 0; index < chars.length; index++) {
    const char = chars[index]
    const prev = index > 0 ? chars[index - 1] : ''
    if (prev && needsGap(prev, char)) {
      out += GAP
    }
    out += char
  }
  return out
}

const tagName = tag => {
  const match = tag.match(/^<\/?\s*([a-z0-9]+)/i)
  return match ? match[1].toUpperCase() : ''
}

// 换行和块级标签会把两侧文字拆开，行内标签不会
const separatesText = tag => {
  const name = tagName(tag)
  if (!name || name === 'BR') return true
  return !INLINE.has(name)
}

const lastChar = text => {
  const chars = charsOf(text)
  return chars.length ? chars[chars.length - 1] : ''
}

const firstChar = text => {
  const chars = charsOf(text)
  return chars.length ? chars[0] : ''
}

export const spaceCjkLatinHtml = html => {
  if (html == null || html === '') return html
  const parts = String(html).split(/(<[^>]*>)/g)
  let lastTextIndex = -1
  let blocked = false
  for (let index = 0; index < parts.length; index++) {
    const part = parts[index]
    if (part.startsWith('<')) {
      if (separatesText(part)) blocked = true
      continue
    }
    let text = spaceCjkLatin(part)
    const prev = lastTextIndex >= 0 ? lastChar(parts[lastTextIndex]) : ''
    const next = firstChar(text)
    if (
      !blocked &&
      prev &&
      next &&
      needsGap(prev, next) &&
      !parts[lastTextIndex].endsWith(GAP) &&
      !text.startsWith(GAP)
    ) {
      text = GAP + text
    }
    parts[index] = text
    if (text) {
      lastTextIndex = index
      blocked = false
    }
  }
  return parts.join('')
}
