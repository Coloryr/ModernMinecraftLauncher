import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, readFile, symlink, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { listFiles, readText, saveText, safePath } from '../lib/files.mjs';
import { temp } from './helpers.mjs';

test('保存支持中文与空格路径，清除 BOM 并保留外部编辑', async () => {
  const root = await temp('files');
  const initial = await saveText(root, { path: 'src/中文 文件.css', text: '\uFEFFa { color: red }', expectedHash: null });
  assert.equal(initial.text, 'a { color: red }');
  assert.notEqual((await readFile(join(root, initial.path)))[0], 0xef);
  const saved = await saveText(root, { path: initial.path, text: 'b {}', expectedHash: initial.hash });
  assert.notEqual(saved.hash, initial.hash);
  await writeFile(join(root, initial.path), 'external');
  await assert.rejects(saveText(root, { path: initial.path, text: 'overwrite', expectedHash: saved.hash }), { key: 'conflict' });
  assert.equal((await readText(root, initial.path)).text, 'external');
  await assert.rejects(saveText(root, { path: initial.path, text: '', expectedHash: null }), { key: 'exists' });
});

test('拒绝越界、依赖目录、符号链接以及二进制和大文件', async () => {
  const root = await temp('paths');
  for (const path of ['../secret.txt', 'src/../../secret.txt', 'C:/secret.txt', '/secret.txt', 'src\\secret.txt', 'node_modules/a.js', '.mml-deps/a.js', 'a:stream.txt', 'NUL.txt', 'src /file.txt', 'CON/test.css']) {
    await assert.rejects(safePath(root, path, { create: true }), { key: 'outside' }, path);
  }
  const outside = await temp('outside');
  await symlink(outside, join(root, 'link'), process.platform === 'win32' ? 'junction' : 'dir');
  await assert.rejects(saveText(root, { path: 'link/new.txt', text: 'x', expectedHash: null }), { key: 'outside' });
  await writeFile(join(root, 'binary.txt'), Buffer.from([1, 0, 2]));
  await assert.rejects(readText(root, 'binary.txt'), { key: 'invalidFile' });
  await writeFile(join(root, 'huge.txt'), 'x'.repeat(2 * 1024 * 1024 + 1));
  await assert.rejects(readText(root, 'huge.txt'), { key: 'invalidFile' });
});

test('文件树排除依赖、归档与目录链接', async () => {
  const root = await temp('tree');
  await mkdir(join(root, 'node_modules'), { recursive: true });
  await writeFile(join(root, 'node_modules/secret.js'), 'hidden');
  await writeFile(join(root, 'theme.zip'), 'zip');
  await writeFile(join(root, 'package-lock.json'), '{}');
  await saveText(root, { path: 'src/test.vue', text: '<template />', expectedHash: null });
  assert.deepEqual(await listFiles(root), ['src/test.vue']);
});
