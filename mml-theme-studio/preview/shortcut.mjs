// 预览页里的 F12 / Ctrl+Shift+I 只通知父页面；不直接碰编辑器，也不依赖 Tauri IPC。
export const DEVTOOLS_MESSAGE = 'mml-studio-devtools';

/**
 * 注入预览页的快捷键监听
 *
 * 必须挂在捕获阶段：预览应用自己也可能用 F12 / Ctrl+Shift+I。
 */
export const DEVTOOLS_SCRIPT = `addEventListener('keydown', (event) => {
  const shortcut = (event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === 'i';
  if (event.key !== 'F12' && !shortcut) return;
  event.preventDefault();
  event.stopPropagation();
  parent.postMessage({ type: '${DEVTOOLS_MESSAGE}' }, '*');
}, true);`;
