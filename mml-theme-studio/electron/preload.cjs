// 只有编辑器窗口有这座桥；预览页面没有 preload，也没有 Node.js 权限。
const { contextBridge, ipcRenderer } = require('electron');
const actions = new Set(['state', 'pick', 'create', 'open', 'files', 'read', 'save', 'install', 'build', 'preview', 'stop', 'reload', 'devtools', 'bounds', 'catalog', 'reveal', 'locale', 'dirty', 'cancel', 'previewOptions']);
contextBridge.exposeInMainWorld('studio', {
  call: (action, input) => {
    if (!actions.has(action)) return Promise.reject(new Error('Unknown action'));
    return ipcRenderer.invoke('studio:call', action, input);
  },
  onEvent: (callback) => {
    const listener = (_event, value) => callback(value);
    ipcRenderer.on('studio:event', listener);
    return () => ipcRenderer.removeListener('studio:event', listener);
  },
});
