import { test } from 'node:test';
import assert from 'node:assert/strict';
import { writeFile, symlink } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { htmlPreview } from '../preview/html.mjs';
import { asset } from '../preview/assets.mjs';
import { fixture, temp } from './helpers.mjs';

const READY = '@@mml-studio@@';

test('HTML 实时预览与静态资源不能越出 src', async () => {
  const root = await fixture(await temp('html'));
  await writeFile(join(root, 'src/index.html'), '<head></head><body>Preview</body>');
  await writeFile(join(root, 'src/theme.css'), 'body {}');
  await writeFile(join(root, 'secret.txt'), 'secret');
  await symlink(root, join(root, 'src/link'), process.platform === 'win32' ? 'junction' : 'dir');
  for (const name of ['../secret.txt', '%2e%2e/secret.txt', '%', 'link/secret.txt', '..\\secret.txt']) assert.equal(await asset(join(root, 'src'), name), null);
  const preview = await htmlPreview(root);
  try {
    const page = await (await fetch(preview.url)).text();
    assert.match(page, /EventSource/);
    assert.match(page, /Preview/);
    // 整页 HTML 不限定检查范围，元素树仍从 html 开始
    assert.doesNotMatch(page, /__mmlInspectRoot\s*=\s*["']/);
    assert.equal((await fetch(preview.url + 'theme.css')).status, 200);
    assert.equal((await fetch(preview.url + 'link/secret.txt')).status, 404);
    const controller = new AbortController();
    const stream = await fetch(preview.url + '__reload', { signal: controller.signal });
    const reader = stream.body.getReader();
    await reader.read();
    await writeFile(join(root, 'src/theme.css'), 'body { color: red }');
    const update = await reader.read();
    assert.match(new TextDecoder().decode(update.value), /data: reload/);
    controller.abort();

    // 自定义主页面整页就是内容区，检查范围收到 body
    const scoped = await htmlPreview(root, 'home-html');
    try {
      assert.match(await (await fetch(scoped.url)).text(), /__mmlInspectRoot="body"/);
    } finally { await scoped.close(); }
  } finally { await preview.close(); }
});

test('预览 worker 通过 stdout 报告就绪，stdin 关闭后退出', async () => {
  const root = await fixture(await temp('worker'));
  await writeFile(join(root, 'src/index.html'), '<html><head></head><body>Worker preview</body></html>');
  const worker = fileURLToPath(new URL('../preview/worker.mjs', import.meta.url));
  const child = spawn(process.execPath, [worker, root, 'html', join(root, 'cache')], { stdio: ['pipe', 'pipe', 'pipe'] });
  let stderr = '';
  child.stderr.on('data', (value) => { stderr += value; });
  try {
    const url = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('timeout: ' + stderr)), 30000);
      let buffer = '';
      const onData = (value) => {
        buffer += value.toString();
        for (;;) {
          const index = buffer.indexOf('\n');
          if (index < 0) return;
          const line = buffer.slice(0, index);
          buffer = buffer.slice(index + 1);
          if (!line.startsWith(READY)) continue;
          const message = JSON.parse(line.slice(READY.length));
          clearTimeout(timer);
          child.stdout.off('data', onData);
          if (message.type === 'ready') resolve(message.url);
          else reject(new Error(message.error));
          return;
        }
      };
      child.stdout.on('data', onData);
      child.once('exit', () => { clearTimeout(timer); reject(new Error('worker exited: ' + stderr)); });
    });
    const page = await (await fetch(url)).text();
    assert.match(page, /Worker preview/);
    assert.doesNotMatch(page, /__mmlInspectRoot\s*=\s*["']/);
  } finally {
    if (child.exitCode === null) {
      child.stdin.end();
      await once(child, 'exit');
    }
  }
});
