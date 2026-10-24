// 编辑器只访问项目内的文本文件；保存前比对版本，避免盖掉外部编辑。
import { createHash } from 'node:crypto';
import { lstat, readdir, readFile, realpath, writeFile, mkdir } from 'node:fs/promises';
import { dirname, extname, isAbsolute, join, relative, resolve } from 'node:path';
import { StudioError } from './messages.mjs';

export const IGNORED = new Set(['.git', '.mml-deps', 'node_modules', 'dist', 'dist-theme', 'target']);
const TEXT = new Set(['.vue', '.ts', '.js', '.mjs', '.cjs', '.json', '.css', '.html', '.md', '.svg', '.txt', '.gitignore']);
const LIMIT = 2 * 1024 * 1024;
export const hash = (data) => createHash('sha256').update(data).digest('hex');
export const inside = (root, path) => { const rel = relative(root, path); return rel === '' || (!rel.startsWith('..') && !isAbsolute(rel)); };

export async function safePath(root, name, { create = false } = {}) {
  if (typeof name !== 'string' || !name || isAbsolute(name) || /[\\:<>"|?*]/.test(name) || [...name].some((char) => char.charCodeAt(0) < 32) || name.split('/').some((part) => !part || /[. ]$/.test(part) || /^(con|prn|aux|nul|com[0-9]|lpt[0-9])(?:\.|$)/i.test(part) || IGNORED.has(part))) {
    throw new StudioError('outside', { path: name });
  }
  const base = await realpath(root);
  const target = resolve(base, name);
  if (!inside(base, target)) throw new StudioError('outside', { path: name });
  let current = base;
  const parts = name.split('/');
  for (let index = 0; index < parts.length; index++) {
    current = join(current, parts[index]);
    try {
      const info = await lstat(current);
      if (info.isSymbolicLink() || !inside(base, await realpath(current))) throw new StudioError('outside', { path: name });
    } catch (error) {
      if (error.code !== 'ENOENT' || !create) throw error;
      if (index < parts.length - 1) await mkdir(current);
    }
  }
  return target;
}

export async function listFiles(root) {
  const result = [];
  async function visit(dir, prefix = '') {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      if (IGNORED.has(entry.name) || entry.isSymbolicLink() || entry.name.endsWith('.zip') || entry.name === 'package-lock.json') continue;
      const name = prefix + entry.name;
      if (entry.isDirectory()) await visit(join(dir, entry.name), name + '/');
      else if (isText(name)) result.push(name);
      if (result.length > 5000) throw new StudioError('invalidProject');
    }
  }
  await visit(root);
  return result.sort();
}

function isText(name) { return TEXT.has(extname(name)) || TEXT.has(name.split('/').at(-1)); }

export async function readText(root, name) {
  const target = await safePath(root, name);
  if (!isText(name) || (await lstat(target)).size > LIMIT) throw new StudioError('invalidFile', { path: name });
  const buffer = await readFile(target);
  if (buffer.includes(0)) throw new StudioError('invalidFile', { path: name });
  return { path: name, text: buffer.toString('utf8'), hash: hash(buffer) };
}

export async function saveText(root, { path, text, expectedHash }) {
  if (typeof text !== 'string' || Buffer.byteLength(text) > LIMIT || !isText(path)) throw new StudioError('invalidFile', { path });
  const target = await safePath(root, path, { create: expectedHash === null });
  if (expectedHash !== null && (await readText(root, path)).hash !== expectedHash) throw new StudioError('conflict', { path });
  try { await writeFile(target, text.replace(/^\uFEFF/, ''), { encoding: 'utf8', flag: expectedHash === null ? 'wx' : 'w' }); }
  catch (error) { if (error.code === 'EEXIST') throw new StudioError('exists', { path }); throw error; }
  return readText(root, path);
}

export async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, JSON.stringify(value, null, 2) + '\n', 'utf8');
}
