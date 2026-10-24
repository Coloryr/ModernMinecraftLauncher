// 使用隐藏的自有 Electron 实例验证 DOM 和 IPC，不操作桌面或捕获窗口图像。
import { app, BrowserWindow } from 'electron';
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createStudioWindow } from '../electron/application.mjs';
import { writeJson } from '../lib/files.mjs';
import { fixture, temp, until } from './helpers.mjs';

const root = await temp('electron');
app.setPath('userData', join(root, 'electron-data'));
app.setPath('cache', join(root, 'electron-cache'));
app.whenReady().then(async () => {
let host;
let inspector;
let success = false;
try {
  const project = await fixture(join(root, 'html'));
  await writeFile(join(project, 'src/index.html'), '<html><head></head><body><h1>Studio preview test</h1></body></html>');
  await writeJson(join(root, 'config/studio.json'), { locale: 'zh-CN', recent: [project] });
  host = await createStudioWindow({ dataRoot: join(root, 'config'), cache: join(root, 'cache'), show: false });
  const { window, view, studio } = host;
  const editorErrors = [];
  const previewErrors = [];
  window.webContents.on('console-message', (_event, ...details) => { editorErrors.push(details); });
  view.webContents.on('console-message', (_event, ...details) => {
    if (details[0] >= 3) { previewErrors.push(details); console.log('Preview console:', ...details); }
  });
  view.webContents.on('did-fail-load', (_event, code, message, url) => console.log('Preview load:', code, message, url));
  const dom = (code) => window.webContents.executeJavaScript(code);
  const previewDom = (code) => view.webContents.executeJavaScript(code);
  const invoke = async (action, input) => {
    const result = await dom(`window.studio.call(${JSON.stringify(action)}, ${JSON.stringify(input) ?? 'undefined'})`);
    assert.equal(result.ok, true, result.error);
    return result.value;
  };
  await until(() => dom('!!document.querySelector(".recent-entry")'), 'recent project');
  await dom('document.querySelector(".recent-entry").click()');
  await until(() => dom('!!document.querySelector(".monaco-editor") && !!document.querySelector(".file-entry.selected")'), 'Monaco editor');
  assert.equal(await dom('typeof require'), 'undefined');
  const file = await invoke('read', 'src/index.html');
  const changed = await invoke('save', { path: file.path, text: file.text.replace('test', 'saved'), expectedHash: file.hash });
  assert.match(changed.text, /saved/);
  await invoke('preview');
  await until(() => previewDom('document.body?.textContent.includes("Studio preview saved")'), 'HTML preview');
  assert.equal(await previewDom('typeof window.studio'), 'undefined');
  assert.equal(await previewDom('typeof require'), 'undefined');
  inspector = new BrowserWindow({ show: false, webPreferences: { nodeIntegration: false, sandbox: true } });
  view.webContents.setDevToolsWebContents(inspector.webContents);
  await invoke('devtools');
  await until(() => inspector.webContents.getURL().startsWith('devtools:') && !inspector.webContents.isLoading(), 'full Chromium DevTools');
  await until(() => inspector.webContents.executeJavaScript(`(() => {
    function collect(node) { return node.textContent + [...node.querySelectorAll('*')].map(el => el.shadowRoot ? collect(el.shadowRoot) : '').join(''); }
    const text = collect(document.body);
    return ['Elements', 'Console', 'Network', 'Sources'].every(label => text.includes(label));
  })()`), 'DevTools Elements, Console, Network and Sources');
  assert.match(inspector.webContents.getURL(), /^devtools:/);
  view.webContents.closeDevTools();
  await invoke('save', { path: changed.path, text: changed.text.replace('saved', 'reloaded'), expectedHash: changed.hash });
  await until(() => previewDom('document.body?.textContent.includes("Studio preview reloaded")'), 'HTML live reload');
  console.log('PASS: editor window, Monaco, IPC save, HTML preview, hot reload, isolated preview, full DevTools');

  for (const path of process.argv.slice(2)) {
    const current = await invoke('open', resolve(path));
    console.log('Preview start:', current.template, current.root);
    await invoke('preview');
    const marker = current.template === 'css' ? 'window.__mmlCustomCss' : current.template === 'home' ? 'window.__mmlHomePages' : 'window.__mmlWindowPages';
    await until(() => previewDom('document.querySelector("#app")?.children.length > 0'), 'Vue mount ' + current.template, 60000);
    assert.ok(await previewDom('!!(' + marker + ')'), current.template + ' registry');
    assert.ok(await previewDom(`!!document.querySelector('link[href*="/__theme/custom.css"]')`), current.template + ' CSS loaded');
    if (current.template === 'page') await until(() => previewDom('!!document.querySelector(".theme-fab")'), 'custom window component');
    if (current.template === 'home') await until(() => previewDom('!!document.querySelector(".mml-home")'), 'home component');
    await invoke('previewOptions', { kind: 'main', theme: 'Light' });
    await until(() => previewDom('document.documentElement.dataset.theme === "Light"'), 'preview light theme');
    const entry = current.template === 'css' ? 'custom.css' : current.template === 'home' ? 'src/entry.ts' : 'src/replace.ts';
    const original = await invoke('read', entry);
    let updated;
    try {
      const extra = current.template === 'css' ? '\n:root { --studio-smoke: ready; }\n' : '\ndocument.documentElement.dataset.studioSmoke = "ready";\n';
      updated = await invoke('save', { path: entry, text: original.text + extra, expectedHash: original.hash });
      await until(() => previewDom(current.template === 'css' ? 'getComputedStyle(document.documentElement).getPropertyValue("--studio-smoke").trim() === "ready"' : 'document.documentElement.dataset.studioSmoke === "ready"'), 'saved source live rebuild ' + current.template, 60000);
    } finally {
      await invoke('stop');
      if (updated) await invoke('save', { path: entry, text: original.text, expectedHash: updated.hash });
    }
    assert.equal(studio.preview, null);
    console.log('PASS:', current.template, 'actual launcher host, theme load, light mode, source rebuild, process cleanup');
  }
  assert.equal(editorErrors.length, 0, JSON.stringify(editorErrors));
  assert.equal(previewErrors.length, 0, JSON.stringify(previewErrors));
  success = true;
} catch (error) {
  console.error(error);
  if (host) console.error('Studio output:', await host.window.webContents.executeJavaScript('document.querySelector(".output-log")?.textContent').catch(String));
  if (host) console.error('Preview DOM:', await host.view.webContents.executeJavaScript('document.body.innerHTML.slice(0, 3000)').catch(String));
} finally {
  inspector?.destroy();
  await host?.dispose();
  app.exit(success ? 0 : 1);
}
});
