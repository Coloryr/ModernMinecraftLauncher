// Demo 和 SDK 来自同一份检出；先完整准备，再创建用户目录。
import { cp, lstat, mkdir, mkdtemp, readFile, readdir, realpath, rm, stat } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { basename, join, resolve, relative } from 'node:path';
import { StudioError } from './messages.mjs';
import { IGNORED, inside, writeJson } from './files.mjs';
import { environment, run } from './process.mjs';

export const TEMPLATES = { css: 'custom-ui-css-demo', page: 'custom-ui-demo', home: 'custom-main-demo', html: 'custom-ui-html-demo' };
export const REPOSITORY = 'https://github.com/Coloryr/ModernMinecraftLauncher.git';

export function validateName(name) {
  if (typeof name !== 'string' || !name.trim() || name.length > 80 || /[<>:"/\\|?*]/.test(name) || [...name].some((char) => char.charCodeAt(0) < 32) || /[. ]$/.test(name) || /^(con|prn|aux|nul|com[0-9]|lpt[0-9])(?:\.|$)/i.test(name) || name === '..') throw new StudioError('invalidName');
  return name;
}

async function copySources(source, target) {
  await cp(source, target, {
    recursive: true, dereference: false,
    filter: async (path) => {
      const parts = relative(source, path).split(/[\\/]/);
      if (parts.some((part) => IGNORED.has(part) || part.endsWith('.zip'))) return false;
      // 下载源中的符号链接不带入项目，防止源码目录外的文件混进来。
      return !(await lstat(path)).isSymbolicLink();
    },
  });
}

export async function createProject({ parent, name, template, source = '', cache, signal, log = () => {} }) {
  if (signal?.aborted) throw new StudioError('cancelled');
  validateName(name);
  if (!Object.hasOwn(TEMPLATES, template)) throw new StudioError('invalidProject');
  const destination = join(await realpath(parent), name);
  if (existsSync(destination)) throw new StudioError('exists', { path: destination });
  await mkdir(cache, { recursive: true });
  const stage = await mkdtemp(join(cache, 'demo-'));
  let claimed = false;
  try {
    const repo = source ? await realpath(source) : join(stage, 'repo');
    if (inside(repo, destination) && !source) throw new StudioError('outside', { path: destination });
    if (!source) {
      await run('git', ['-c', 'core.longpaths=true', 'clone', '--depth', '1', '--filter=blob:none', '--sparse', '--', REPOSITORY, repo], { signal, log });
      await run('git', ['-C', repo, 'sparse-checkout', 'set', `custom/${TEMPLATES[template]}`, 'mml-vue', 'mml-ipc'], { signal, log });
    }
    const demo = join(repo, 'custom', TEMPLATES[template]);
    for (const file of [join(demo, 'package.json'), join(demo, 'src/theme.json'), join(repo, 'mml-ipc/package.json'), join(repo, 'mml-vue/package.json')]) {
      if (!existsSync(file)) throw new StudioError('badSource', { path: file });
    }
    let revision = '';
    try { revision = await run('git', ['-C', repo, 'rev-parse', 'HEAD'], { signal }); } catch (error) { if (signal?.aborted) throw error; }
    const prepared = join(stage, 'project');
    await copySources(demo, prepared);
    if (template !== 'html') {
      for (const pkg of ['mml-ipc', 'mml-vue']) await copySources(join(repo, pkg), join(prepared, '.mml-deps', pkg));
      const packagePath = join(prepared, 'package.json');
      const pkg = JSON.parse(await readFile(packagePath, 'utf8'));
      pkg.dependencies = { ...pkg.dependencies, 'mml-ipc': 'file:.mml-deps/mml-ipc', 'mml-vue': 'file:.mml-deps/mml-vue' };
      // 工具已随 demo 固定了 SDK，安装时不再运行示例的仓库探测与二次克隆。
      delete pkg.scripts.preinstall;
      delete pkg.scripts.deps;
      await writeJson(packagePath, pkg);
    }
    await writeJson(join(prepared, '.mml-deps/studio.json'), { template, revision, source: source || REPOSITORY });
    if (signal?.aborted) throw new StudioError('cancelled');
    await mkdir(destination);
    claimed = true;
    await cp(prepared, destination, { recursive: true });
    if (signal?.aborted) throw new StudioError('cancelled');
    return await openProject(destination);
  } catch (error) {
    if (claimed && inside(await realpath(parent), destination)) await rm(destination, { recursive: true, force: true });
    throw error;
  } finally {
    // stage 是 mkdtemp 刚创建的路径，清理前仍检查它归属下载缓存。
    if (inside(resolve(cache), stage)) await rm(stage, { recursive: true, force: true });
  }
}

export async function openProject(root) {
  root = await realpath(root);
  let manifest;
  let pkg;
  try {
    manifest = JSON.parse(await readFile(join(root, 'src/theme.json'), 'utf8'));
    pkg = JSON.parse(await readFile(join(root, 'package.json'), 'utf8'));
  } catch { throw new StudioError('invalidProject'); }
  const template = manifest.type === 'page' && existsSync(join(root, 'src/index.html')) ? 'html' : manifest.type;
  if (!Object.hasOwn(TEMPLATES, template) || typeof pkg.scripts?.build !== 'string') throw new StudioError('invalidProject');
  let metadata = {};
  try { metadata = JSON.parse(await readFile(join(root, '.mml-deps/studio.json'), 'utf8')); } catch { /* 允许打开手工创建的项目。 */ }
  return { root, name: manifest.name || basename(root), template, revision: metadata.revision || '' };
}

export async function installProject(project, options) {
  const env = await environment();
  if (!existsSync(join(project.root, '.mml-deps/studio.json')) && existsSync(join(project.root, 'scripts/deps.mjs'))) {
    // npm 在 preinstall 之前就可能解析 file 依赖，必须先准备契约包。
    await run(env.node, ['scripts/deps.mjs'], { cwd: project.root, ...options });
  }
  await run(env.node, [env.npm, 'install', '--no-audit', '--no-fund'], { cwd: project.root, ...options });
}

export async function buildProject(project, options) {
  const env = await environment();
  const before = new Map();
  for (const name of await readdir(project.root)) {
    if (name.endsWith('.zip')) before.set(name, await stat(join(project.root, name)));
  }
  await run(env.node, [env.npm, 'run', 'build'], { cwd: project.root, ...options });
  const artifacts = [];
  for (const name of await readdir(project.root)) {
    if (!name.endsWith('.zip')) continue;
    const current = await stat(join(project.root, name));
    const previous = before.get(name);
    if (current.isFile() && (!previous || current.mtimeMs !== previous.mtimeMs || current.ctimeMs !== previous.ctimeMs)) artifacts.push(join(project.root, name));
  }
  if (!artifacts.length) throw new StudioError('missingBuild');
  return artifacts;
}
