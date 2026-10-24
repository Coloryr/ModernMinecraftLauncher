import { test } from 'node:test';
import assert from 'node:assert/strict';
import { writeFile, symlink } from 'node:fs/promises';
import { join } from 'node:path';
import { htmlPreview } from '../preview/html.mjs';
import { asset } from '../preview/assets.mjs';
import { Studio } from '../lib/studio.mjs';
import { run } from '../lib/process.mjs';
import { fixture, temp } from './helpers.mjs';

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
  } finally { await preview.close(); }
});

test('执行错误可见、任务取消释放忙状态、关闭回收预览进程', async () => {
  const root = await fixture(await temp('lifecycle'));
  await writeFile(join(root, 'src/index.html'), '<head></head>');
  const events = [];
  const studio = new Studio({ cache: join(root, 'cache'), emit: (event) => events.push(event) });
  await studio.open(root);
  await assert.rejects(run(process.execPath, ['-e', 'process.exit(5)']), { key: 'commandFailed' });
  const task = studio.perform(({ signal }) => run(process.execPath, ['-e', 'setInterval(()=>{},1000)'], { signal }));
  await assert.rejects(studio.perform(() => {}), { key: 'busy' });
  await studio.cancel();
  await assert.rejects(task, { key: 'cancelled' });
  assert.equal(studio.task, null);
  const url = await studio.startPreview();
  assert.equal((await fetch(url)).status, 200);
  await studio.close();
  assert.equal(studio.preview, null);
  assert.equal(events.at(-1).url, '');
  await assert.rejects(fetch(url));
});
