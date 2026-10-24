// Electron 的宿主环境可能带有 Node 模式开关，启动桌面工具时只清理子进程的那份。
import { spawn } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const cache = fileURLToPath(new URL('../../target/temp/theme-studio/electron/', import.meta.url));
await mkdir(cache, { recursive: true });
process.env.electron_config_cache = cache;
process.env.TEMP = cache;
process.env.TMP = cache;
process.env.TMPDIR = cache;
const { default: electron } = await import('electron');
const env = { ...process.env };
delete env.ELECTRON_RUN_AS_NODE;
const child = spawn(electron, process.argv.slice(2).length ? process.argv.slice(2) : ['.'], {
  env, stdio: 'inherit', windowsHide: true,
});
child.on('error', (error) => { console.error(error); process.exitCode = 1; });
child.on('exit', (code) => { process.exitCode = code ?? 1; });
