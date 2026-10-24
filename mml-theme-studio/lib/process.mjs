// 进程只接受参数数组；取消时回收本工具创建的进程树。
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { existsSync, realpathSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { StudioError } from './messages.mjs';
const exec = promisify(execFile);

export async function environment() {
  let node;
  try {
    const result = await exec('node', ['-p', 'JSON.stringify({path:process.execPath,version:process.versions.node})'], { windowsHide: true });
    node = JSON.parse(result.stdout.trim());
  } catch { throw new StudioError('missingTool', { tool: 'Node.js' }); }
  if (Number(node.version.split('.')[0]) < 24) throw new StudioError('nodeVersion', { version: node.version });
  const candidates = [join(dirname(node.path), 'node_modules/npm/bin/npm-cli.js'), join(dirname(node.path), '../lib/node_modules/npm/bin/npm-cli.js')];
  if (process.platform !== 'win32') {
    try { candidates.push(realpathSync((await exec('which', ['npm'])).stdout.trim())); } catch { /* 下面统一报告。 */ }
  }
  const npm = candidates.find(existsSync);
  if (!npm) throw new StudioError('missingTool', { tool: 'npm' });
  let git;
  try { git = (await exec('git', ['--version'], { windowsHide: true })).stdout.trim(); }
  catch { throw new StudioError('missingTool', { tool: 'Git' }); }
  return { node: node.path, nodeVersion: node.version, npm, git };
}

export function run(command, args, { cwd, log = () => {}, signal, env = {} } = {}) {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) return reject(new StudioError('cancelled'));
    const child = spawn(command, args, {
      cwd, env: { ...process.env, ...env }, windowsHide: true,
      detached: process.platform !== 'win32', stdio: ['ignore', 'pipe', 'pipe'],
    });
    let output = '';
    const receive = (chunk) => { const value = chunk.toString(); output = (output + value).slice(-200000); log(value); };
    child.stdout.on('data', receive);
    child.stderr.on('data', receive);
    const cancel = () => { void stopChild(child); };
    signal?.addEventListener('abort', cancel, { once: true });
    child.once('error', (error) => { signal?.removeEventListener('abort', cancel); reject(error); });
    child.once('close', (code) => {
      signal?.removeEventListener('abort', cancel);
      if (signal?.aborted) reject(new StudioError('cancelled'));
      else if (code !== 0) reject(new StudioError('commandFailed', { command: [command, ...args].join(' '), code }));
      else resolve(output.trim());
    });
  });
}

export async function stopChild(child) {
  if (!child?.pid || child.exitCode !== null || child.signalCode !== null) return;
  if (process.platform === 'win32') {
    try { await exec('taskkill', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true }); } catch { /* 已退出。 */ }
  } else {
    try { process.kill(-child.pid, 'SIGTERM'); } catch { /* 已退出。 */ }
  }
}
