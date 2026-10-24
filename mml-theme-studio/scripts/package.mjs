// 分发目录只装桌面壳与编译后的编辑器，Vite 在用户项目自己的 Node.js 中运行。
import { packager } from '@electron/packager';
import { mkdir, readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const root = fileURLToPath(new URL('..', import.meta.url));
const output = fileURLToPath(new URL('../../target/temp/theme-studio-package/', import.meta.url));
const temporary = join(output, 'staging');
await mkdir(temporary, { recursive: true });
const pkg = JSON.parse(await readFile(join(root, 'package.json'), 'utf8'));
const electronCache = fileURLToPath(new URL('../../target/temp/theme-studio/electron/', import.meta.url));
const cachedZip = join(electronCache, `electron-v${pkg.devDependencies.electron}-${process.platform}-${process.arch}.zip`);
const included = new Set(['electron', 'lib', 'preview', 'dist', 'package.json', 'README.md']);
const paths = await packager({
  dir: root, name: 'MML-Theme-Studio', executableName: 'MML-Theme-Studio',
  out: output, tmpdir: temporary, overwrite: true, asar: false, prune: false,
  electronVersion: pkg.devDependencies.electron,
  electronZipDir: existsSync(cachedZip) ? electronCache : undefined,
  download: { cacheRoot: electronCache },
  ignore: (path) => {
    const relative = path.replaceAll('\\', '/').replace(/^\//, '');
    return !!relative && !included.has(relative.split('/')[0]);
  },
});
for (const path of paths) console.log(path);
