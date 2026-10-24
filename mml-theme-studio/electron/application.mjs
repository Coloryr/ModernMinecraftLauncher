import { BrowserWindow, WebContentsView, ipcMain, dialog, shell, Menu, nativeTheme } from 'electron';
import { mkdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { once } from 'node:events';
import { Studio } from '../lib/studio.mjs';
import { text, StudioError } from '../lib/messages.mjs';
import { writeJson } from '../lib/files.mjs';
import { environment } from '../lib/process.mjs';

const root = fileURLToPath(new URL('..', import.meta.url));
const backgrounds = { Dark: '#14161a', Light: '#eef1f5' };

export async function createStudioWindow({ dataRoot, cache, show = true }) {
  await mkdir(dataRoot, { recursive: true });
  const configPath = join(dataRoot, 'studio.json');
  const config = { locale: 'zh-CN', recent: [], theme: 'Dark' };
  try {
    const saved = JSON.parse(await readFile(configPath, 'utf8'));
    // 兼容首版的小驼峰键，后续落盘统一使用项目约定的大驼峰键。
    const locale = saved?.Locale ?? saved?.locale;
    const recent = saved?.Recent ?? saved?.recent;
    const theme = saved?.Theme ?? saved?.theme;
    if (['zh-CN', 'en-US'].includes(locale)) config.locale = locale;
    if (Array.isArray(recent)) config.recent = recent.filter((path) => typeof path === 'string').slice(0, 8);
    if (['Dark', 'Light'].includes(theme)) config.theme = theme;
  } catch { /* 首次启动使用默认值。 */ }
  const saveConfig = (value = config) => writeJson(configPath, { Locale: value.locale, Recent: value.recent, Theme: value.theme });
  nativeTheme.themeSource = config.theme === 'Light' ? 'light' : 'dark';
  const window = new BrowserWindow({
    width: 1440, height: 920, minWidth: 1000, minHeight: 640, show: false,
    title: text(config.locale, 'title'),
    backgroundColor: backgrounds[config.theme],
    webPreferences: { preload: join(root, 'electron/preload.cjs'), contextIsolation: true, sandbox: true, nodeIntegration: false },
  });
  const view = new WebContentsView({
    webPreferences: { contextIsolation: true, sandbox: true, nodeIntegration: false, partition: `studio-preview-${window.id}` },
  });
  window.contentView.addChildView(view);
  view.setVisible(false);
  view.webContents.session.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  view.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  view.webContents.on('will-navigate', (event, url) => {
    if (!studio.previewUrl || new URL(url).origin !== new URL(studio.previewUrl).origin) event.preventDefault();
  });
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.on('will-navigate', (event) => event.preventDefault());
  let dirty = false;
  let closing = false;
  let asking = false;
  let bounds = null;
  let previewOptions = { kind: 'main', theme: 'Dark' };
  function previewAddress(url) {
    const address = new URL(url);
    address.searchParams.set('window', previewOptions.kind);
    address.searchParams.set('theme', previewOptions.theme);
    return address.href;
  }
  const send = (event) => { if (!window.isDestroyed()) window.webContents.send('studio:event', event); };
  const studio = new Studio({ cache, locale: config.locale, emit: (event) => {
    if (closing) return;
    if (event.type === 'preview') {
      if (!event.url) { view.setVisible(false); if (!view.webContents.isDestroyed()) void view.webContents.loadURL('about:blank').catch(() => {}); }
      else void view.webContents.loadURL(previewAddress(event.url)).catch((error) => studio.log(String(error)));
    }
    send(event);
  } });

  function resizePreview(input) {
    bounds = input;
    const size = window.getContentBounds();
    const valid = studio.previewUrl && input?.visible && ['x', 'y', 'width', 'height'].every((key) => Number.isFinite(input[key]));
    if (!valid) { view.setVisible(false); return; }
    const x = Math.max(0, Math.min(size.width, Math.round(input.x)));
    const y = Math.max(0, Math.min(size.height, Math.round(input.y)));
    view.setBounds({ x, y, width: Math.max(0, Math.min(size.width - x, Math.round(input.width))), height: Math.max(0, Math.min(size.height - y, Math.round(input.height))) });
    view.setVisible(true);
  }
  view.webContents.on('did-finish-load', () => { if (bounds) resizePreview(bounds); });

  function devtools() {
    if (!studio.previewUrl) return;
    view.webContents.openDevTools({ mode: 'detach' });
  }
  for (const contents of [window.webContents, view.webContents]) {
    contents.on('before-input-event', (event, input) => {
      if (input.type !== 'keyDown') return;
      const shortcut = (input.control || input.meta) && input.shift;
      if (input.key === 'F12' || (shortcut && input.key.toLowerCase() === 'i')) {
        event.preventDefault(); devtools();
      }
    });
  }
  Menu.setApplicationMenu(null);

  async function remember(project) {
    config.recent = [project.root, ...config.recent.filter((path) => path !== project.root)].slice(0, 8);
    await saveConfig();
    return project;
  }

  const handler = async (event, action, input) => {
    if (event.sender !== window.webContents || event.senderFrame !== window.webContents.mainFrame) throw new Error('Invalid sender');
    try {
      let value;
      switch (action) {
        case 'state': value = { project: studio.project, locale: studio.locale, recent: config.recent, theme: config.theme, environment: await environment().catch((error) => ({ error: text(studio.locale, error.key, error.values) })) }; break;
        case 'pick': {
          const key = input === 'source' ? 'chooseRepo' : input === 'parent' ? 'chooseParent' : 'chooseProject';
          const result = await dialog.showOpenDialog(window, { title: text(studio.locale, key), properties: ['openDirectory', 'createDirectory'] });
          value = result.canceled ? null : result.filePaths[0]; break;
        }
        case 'create': value = await remember(await studio.create(input)); break;
        case 'open': value = await remember(await studio.open(input)); break;
        case 'files': value = await studio.files(); break;
        case 'read': value = await studio.read(input); break;
        case 'save': value = await studio.save(input); break;
        case 'install': value = await studio.install(); break;
        case 'build': value = await studio.build(); break;
        case 'preview': value = await studio.startPreview(); break;
        case 'previewOptions':
          if (!['main', 'settings', 'account', 'add', 'resource', 'download', 'log'].includes(input?.kind) || !['Dark', 'Light'].includes(input?.theme)) throw new StudioError('invalidProject');
          previewOptions = { kind: input.kind, theme: input.theme };
          if (studio.previewUrl) await view.webContents.loadURL(previewAddress(studio.previewUrl));
          break;
        case 'stop': value = await studio.stopPreview(); break;
        case 'cancel': value = await studio.cancel(); break;
        case 'catalog': value = await studio.catalog(); break;
        case 'reload': if (studio.previewUrl) view.webContents.reload(); break;
        case 'devtools': devtools(); break;
        case 'bounds': resizePreview(input); break;
        case 'dirty': dirty = input === true; break;
        case 'reveal': value = await shell.openPath(studio.current().root); break;
        case 'locale':
          if (!['zh-CN', 'en-US'].includes(input)) throw new StudioError('invalidProject');
          studio.locale = input; config.locale = input;
          await saveConfig(); break;
        case 'theme':
          if (!['Dark', 'Light'].includes(input)) throw new StudioError('invalidTheme');
          await saveConfig({ ...config, theme: input });
          config.theme = input;
          nativeTheme.themeSource = input === 'Light' ? 'light' : 'dark';
          window.setBackgroundColor(backgrounds[input]);
          value = input; break;
        default: throw new StudioError('invalidProject');
      }
      return { ok: true, value };
    } catch (error) {
      const message = error instanceof StudioError ? text(studio.locale, error.key, error.values) : String(error);
      studio.log(message + '\n');
      return { ok: false, error: message };
    }
  };
  ipcMain.handle('studio:call', handler);
  let disposal;
  function dispose() {
    if (!disposal) disposal = (async () => {
      closing = true;
      await studio.close();
      // 子视图异步销毁，必须先等它结束，再销毁宿主以免退出卡住。
      if (!view.webContents.isDestroyed()) {
        const closed = once(view.webContents, 'destroyed');
        view.webContents.close();
        await closed;
      }
      if (!window.isDestroyed()) window.destroy();
    })();
    return disposal;
  }
  window.on('close', (event) => {
    if (closing) return;
    event.preventDefault();
    if (asking) return;
    asking = true;
    void (async () => {
      if (dirty || studio.task) {
        const result = await dialog.showMessageBox(window, { type: 'question', message: text(studio.locale, dirty ? 'confirmDiscard' : 'confirmClose'), buttons: [text(studio.locale, 'cancel'), text(studio.locale, 'close')], defaultId: 0, cancelId: 0 });
        if (result.response !== 1) { asking = false; return; }
      }
      await dispose();
    })();
  });
  window.on('closed', () => ipcMain.removeHandler('studio:call'));
  await window.loadFile(join(root, 'dist/index.html'), { query: { theme: config.theme } });
  if (show) window.show();
  return { window, view, studio, dispose };
}
