// 根节点删掉会清空整棵树。概要节点不走这条删除。
export const canDeleteHoveredNode = node => {
  return Boolean(node && !node.isRoot && !node.isGeneralization)
}
