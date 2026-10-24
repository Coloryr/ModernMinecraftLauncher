// 在隐藏的自有窗口内验证配色与配置恢复，不操作用户桌面。
import { app, nativeTheme } from 'electron';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { createStudioWindow } from '../electron/application.mjs';
import { writeJson } from '../lib/files.mjs';
import { fixture, temp, until } from './helpers.mjs';

const root = await temp('appearance');
app.setPath('userData', join(root, 'electron-data'));
app.on('window-all-closed', () => {});
app.whenReady().then(async () => {
  let host;
  let success = false;
  try {
    const project = await fixture(join(root, 'project'));
    const configPath = join(root, 'config/studio.json');
    await writeJson(configPath, { locale: 'en-US', recent: [project], Theme: 'invalid' });
    const options = { dataRoot: join(root, 'config'), cache: join(root, 'cache'), show: false };
    const dom = (code) => host.window.webContents.executeJavaScript(code);
    const openRecent = async () => {
      await until(() => dom('!!document.querySelector(".recent-entry")'), 'recent project');
      await dom('document.querySelector(".recent-entry").click()');
      await until(() => dom('!!document.querySelector(".monaco-editor") && !document.querySelector("#studio-theme").disabled'), 'editor ready');
    };
    const assertTheme = async (theme) => {
      const light = theme === 'Light';
      await until(() => dom(`document.documentElement.dataset.theme === ${JSON.stringify(theme)} && !!document.querySelector('.monaco-editor.${light ? 'vs' : 'vs-dark'}') && !document.querySelector('#studio-theme').disabled`), 'appearance ' + theme);
      assert.equal(await dom('getComputedStyle(document.body).backgroundColor'), light ? 'rgb(238, 241, 245)' : 'rgb(20, 22, 26)');
      assert.equal(await dom('getComputedStyle(document.documentElement).colorScheme'), light ? 'light' : 'dark');
      assert.equal(nativeTheme.themeSource, light ? 'light' : 'dark');
      assert.equal(await dom('document.querySelector(".preview-options select:nth-child(2)").value'), 'Dark');
    };
    const switchTheme = async (theme) => {
      await dom(`(() => { const select = document.querySelector('#studio-theme'); select.value = ${JSON.stringify(theme)}; select.dispatchEvent(new Event('change', {bubbles:true})); })()`);
      await assertTheme(theme);
      const saved = JSON.parse(await readFile(configPath, 'utf8'));
      assert.equal(saved.Theme, theme);
      assert.equal(saved.Locale, 'en-US');
      assert.deepEqual(saved.Recent, [project]);
    };

    host = await createStudioWindow(options);
    await openRecent();
    await assertTheme('Dark');
    const source = await dom('document.querySelector(".view-lines").textContent');
    for (const theme of ['Light', 'Dark', 'Light']) {
      await switchTheme(theme);
      assert.equal(await dom('document.querySelector(".view-lines").textContent'), source);
    }
    assert.equal((await dom('window.studio.call("theme", "invalid")')).ok, false);
    await assertTheme('Light');
    await host.dispose();
    host = await createStudioWindow(options);
    assert.equal(new URL(host.window.webContents.getURL()).searchParams.get('theme'), 'Light');
    await openRecent();
    await assertTheme('Light');
    console.log('PASS: light/dark UI, Monaco, native appearance, saved preference, restart recovery, legacy config and independent preview');
    success = true;
  } catch (error) { console.error(error); }
  finally { await host?.dispose(); app.exit(success ? 0 : 1); }
});
