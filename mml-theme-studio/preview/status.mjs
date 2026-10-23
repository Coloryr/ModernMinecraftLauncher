// 提供当前预览的主题状态，不接触启动器配置与账户。
export function themeStatus(template) {
  const base = '/__theme/';
  return {
    cssOn: template === 'css', css: template === 'css' ? [{ id: 'studio', name: 'Studio', enabled: true, url: base + 'custom.css', scriptUrl: base + 'custom.js' }] : [],
    pageOn: template === 'page', pageInstalled: template === 'page', kind: 'Page',
    name: 'Studio', version: '1', declared: true, pageEntryUrl: '',
    pageScriptUrl: template === 'page' ? base + 'custom.js' : '',
    pageStyleUrl: template === 'page' ? base + 'custom.css' : '', pageVersion: '1', pageFileCount: 0,
  };
}

export function homeStatus(template) {
  return {
    installed: template === 'home', enabled: template === 'home', kind: 'Home', name: 'Studio',
    version: '1', declared: true, entryUrl: '', scriptUrl: template === 'home' ? '/__theme/custom.js' : '',
    styleUrl: template === 'home' ? '/__theme/custom.css' : '', contentVersion: '1', fileCount: 0,
  };
}
