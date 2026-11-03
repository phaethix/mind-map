// 文档自己带了 view 就用那一份。树本身的更新（比如大纲编辑）没有 view 字段，
// 不该把镜头打回原点。显式的 view: null 才表示回到默认缩放。
export const shouldResetView = data => {
  return Boolean(
    data && Object.prototype.hasOwnProperty.call(data, 'view') && !data.view
  )
}
