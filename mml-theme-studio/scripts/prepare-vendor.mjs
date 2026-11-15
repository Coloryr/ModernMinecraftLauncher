// 首次构建前还原 tauri-plugin-decoration 源码
//
// 仓库只存 patch，Cargo 在解析 path 依赖前必须已经有源码；
// 这里调仓库根的 build/vendor 脚本，已存在就跳过。
import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '..', '..');
if (existsSync(join(repo, 'target/vendor/tauri-plugin-decoration/Cargo.toml'))) process.exit(0);

const dir = join(repo, 'build/vendor/tauri-plugin-decoration');
const result = process.platform === 'win32'
  ? spawnSync('pwsh', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', join(dir, 'prepare.ps1')], { stdio: 'inherit' })
  : spawnSync('bash', [join(dir, 'prepare.sh')], { stdio: 'inherit' });
process.exit(result.status ?? 1);
