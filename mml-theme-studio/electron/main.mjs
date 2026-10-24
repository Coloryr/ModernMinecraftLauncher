import { app } from 'electron';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createStudioWindow } from './application.mjs';

// 不在 ESM 顶层等待 ready，否则 Electron 会等模块加载完成而无法进入 ready。
app.whenReady().then(async () => {
  const dataRoot = app.getPath('userData');
  // 源码运行的中间产物集中在仓库 target/temp；独立分发时放工具自己的数据目录。
  const cache = app.isPackaged
    ? join(dataRoot, 'target/temp')
    : fileURLToPath(new URL('../../target/temp/theme-studio', import.meta.url));
  await createStudioWindow({ dataRoot, cache });
}).catch((error) => { console.error(error); app.exit(1); });
app.on('window-all-closed', () => app.quit());
