import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

export const tempRoot = fileURLToPath(new URL('../../target/temp/theme-studio-tests/', import.meta.url));
export async function temp(name) {
  await mkdir(tempRoot, { recursive: true });
  return mkdtemp(join(tempRoot, name + '-'));
}
export async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, JSON.stringify(value, null, 2) + '\n');
}
export async function fixture(root, type = 'page') {
  await writeJson(join(root, 'package.json'), { type: 'module', scripts: { build: 'node build.mjs' } });
  await writeJson(join(root, 'src/theme.json'), { name: 'Studio test', type, version: '1' });
  return root;
}
export async function until(check, label, timeout = 30000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    const value = await check();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error('Timeout: ' + label);
}
