// 库侧栏的 GitHub 状态文案。和同步对话框同一套判断：
// 有令牌才算已登录；用户名只决定要不要带上 @login。
// 用户名缺失时仍显示已登录，不能落到「未登录」。
export function githubSidebarLabel(status) {
  if (status && status.signedIn && status.login) {
    return {
      key: 'github.statusOn',
      params: { login: status.login }
    }
  }
  if (status && status.signedIn) {
    return { key: 'github.signedInUnknown' }
  }
  return { key: 'github.statusOff' }
}
