import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { createProject, openProject, validateName, TEMPLATES, buildProject } from '../lib/project.mjs';
import { writeJson } from '../lib/files.mjs';
import { fixture, temp } from './helpers.mjs';

test('项目名拒绝保留名称与路径注入', () => {
  for (const name of ['', '..', '.', 'CON', 'nul.css', 'LPT1', 'a/b', 'a\\b', 'name.', 'name ', '\0bad']) assert.throws(() => validateName(name), { key: 'invalidName' });
  assert.equal(validateName('主题 demo'), '主题 demo');
});

test('四类模板完整复制，锁定 SDK，已有目标不覆盖', async () => {
  const root = await temp('projects');
  const source = join(root, 'repo');
  for (const pkg of ['mml-ipc', 'mml-vue']) await writeJson(join(source, pkg, 'package.json'), { name: pkg });
  for (const [type, name] of Object.entries(TEMPLATES)) {
    const demo = await fixture(join(source, 'custom', name), type === 'html' ? 'page' : type);
    if (type === 'html') await writeFile(join(demo, 'src/index.html'), '<html></html>');
    await writeFile(join(demo, 'old.zip'), 'do not copy');
    await mkdir(join(demo, 'node_modules'));
    await writeFile(join(demo, 'node_modules/secret.txt'), 'do not copy');
    const options = { parent: root, source, name: type, template: type, cache: join(root, 'cache') };
    const project = await createProject(options);
    assert.equal(project.template, type);
    assert.equal((await openProject(project.root)).name, 'Studio test');
    assert.equal((await readdir(project.root)).includes('old.zip'), false);
    assert.equal((await readdir(project.root)).includes('node_modules'), false);
    if (type !== 'html') {
      const pkg = JSON.parse(await readFile(join(project.root, 'package.json'), 'utf8'));
      assert.equal(pkg.dependencies['mml-vue'], 'file:.mml-deps/mml-vue');
      assert.equal(pkg.scripts.preinstall, undefined);
    }
    await assert.rejects(createProject(options), { key: 'exists' });
    assert.equal((await openProject(project.root)).template, type);
  }
  assert.deepEqual(await readdir(join(root, 'cache')), []);
});

test('模板损坏或取消时不留下半成品目标', async () => {
  const root = await temp('invalid-project');
  const options = { parent: root, source: root, name: 'new', template: 'css', cache: join(root, 'cache') };
  await assert.rejects(createProject(options), { key: 'badSource' });
  assert.equal((await readdir(root)).includes('new'), false);
  assert.deepEqual(await readdir(join(root, 'cache')), []);
  await assert.rejects(openProject(root), { key: 'invalidProject' });
  await assert.rejects(createProject({ ...options, signal: AbortSignal.abort() }), { key: 'cancelled' });
  assert.equal((await readdir(root)).includes('new'), false);
});

test('构建失败不能被旧 ZIP 掩盖，成功返回本次新产物', async () => {
  const root = await fixture(await temp('build'), 'css');
  await writeFile(join(root, 'old.zip'), 'old');
  await writeFile(join(root, 'build.mjs'), 'process.exit(0);');
  const project = await openProject(root);
  await assert.rejects(buildProject(project), { key: 'missingBuild' });
  await writeFile(join(root, 'build.mjs'), "import {writeFileSync} from 'node:fs'; writeFileSync('new.zip','new');");
  assert.deepEqual(await buildProject(project), [join(root, 'new.zip')]);
  await writeFile(join(root, 'build.mjs'), 'process.exit(3);');
  await assert.rejects(buildProject(project), { key: 'commandFailed' });
});
