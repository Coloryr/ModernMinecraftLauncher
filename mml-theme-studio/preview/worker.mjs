// 预览构建在外部 Node.js 中运行，桌面壳不执行项目脚本，也不承载 Vite。
import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { mkdir, readFile, readdir } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { htmlPreview } from './html.mjs';
import { asset, respond } from './assets.mjs';
import { themeStatus, homeStatus } from './status.mjs';
import { DEVTOOLS_SCRIPT } from './shortcut.mjs';
import { text } from '../lib/messages.mjs';

const READY = '@@mml-studio@@';
const [root, template, cache, locale = 'zh-CN'] = process.argv.slice(2);
let close = async () => {};

async function vuePreview() {
  const require = createRequire(join(root, 'package.json'));
  const vite = await import(pathToFileURL(require.resolve('vite')).href);
  const vue = (await import(pathToFileURL(require.resolve('@vitejs/plugin-vue')).href)).default;
  const sdk = join(root, '.mml-deps/mml-vue');
  if (!existsSync(join(sdk, 'src/main.ts'))) throw new Error(text(locale, 'missingSdk'));
  const output = join(cache, 'bundle');
  const config = join(root, template === 'css' ? 'vite.config.ts' : 'vite.theme.config.ts');
  const tauri = dirname(require.resolve('@tauri-apps/api/package.json'));
  const modulePath = (name) => JSON.stringify(join(tauri, name + '.js').replaceAll('\\', '/'));
  const events = await readFile(new URL('./events.mjs', import.meta.url), 'utf8');
  let server;
  let timer;
  let running = false;
  let pending = false;
  let closed = false;
  async function compile() {
    if (running) { pending = true; return; }
    running = true;
    try {
      do {
        pending = false;
        await vite.build({ root, configFile: config, build: { outDir: output, emptyOutDir: true }, logLevel: 'warn' });
      } while (pending && !closed);
      server?.ws.send({ type: 'full-reload' });
    } catch (error) {
      if (!server) throw error;
      console.error(error);
      server.ws.send({ type: 'error', err: { message: String(error), stack: error.stack || '' } });
    } finally { running = false; }
  }
  await mkdir(cache, { recursive: true });
  await compile();
  const shims = { core: '\0mml-studio-core', event: '\0mml-studio-event', window: '\0mml-studio-window' };
  server = await vite.createServer({
    configFile: false, root: sdk, cacheDir: join(cache, 'vite'), plugins: [vue(), {
      name: 'mml-studio-preview', enforce: 'pre',
      transformIndexHtml(html) { return html.replace('src="/src/main.ts"', 'src="/__studio/boot.js"'); },
      resolveId(source) { if (source.startsWith('@tauri-apps/api/')) return shims[source.slice('@tauri-apps/api/'.length)]; },
      load(id) {
        if (id === shims.event) return `export * from ${modulePath('event')};\n${events}`;
        if (id === shims.window) return `export * from ${modulePath('window')};
          import { Window } from ${modulePath('window')};
          export function getCurrentWindow() {
            return new Window('mml-' + (new URLSearchParams(location.search).get('window') || 'main'), {skip:true});
          }`;
        if (id !== shims.core) return;
        return `export * from ${modulePath('core')};
          export async function invoke(command) {
            if(command==='custom_ui_status') return ${JSON.stringify(themeStatus(template))};
            if(command==='custom_home_status') return ${JSON.stringify(homeStatus(template))};
            if(command==='main_load_state') return {ok:true,error:null};
            if(command==='main_get_instances') return [];
            throw new Error(${JSON.stringify(text(locale, 'unsupported', { command: '{command}' }))}.replace('{command}',command));
          }`;
      },
      configureServer(instance) {
        instance.middlewares.use(async (request, response, next) => {
          try {
          const path = new URL(request.url, 'http://127.0.0.1').pathname;
          if (path === '/__studio/boot.js') {
            respond(response, { mime: 'text/javascript', data: `import {KEYS,writeString} from '/src/lib/storage.ts';
              const color=new URLSearchParams(location.search).get('theme');
              if(color==='Dark'||color==='Light') writeString(KEYS.theme,color);
              ${template === 'home' ? "writeString(KEYS.startupPage,'Home');" : ''}
              ${DEVTOOLS_SCRIPT}
              await import('/src/main.ts');` });
            return;
          }
          if (!path.startsWith('/__theme/')) return next();
          const name = path.slice('/__theme/'.length);
          if (name === 'custom.css') {
            const cssFiles = (await readdir(output)).filter((file) => file.endsWith('.css'));
            const parts = await Promise.all(cssFiles.map((file) => readFile(join(output, file), 'utf8')));
            if (template === 'css') parts.unshift(await readFile(join(root, 'custom.css'), 'utf8'));
            respond(response, { data: parts.join('\n'), mime: 'text/css; charset=utf-8' });
          } else respond(response, await asset(output, name) || await asset(root, name));
          } catch (error) { next(error); }
        });
      },
    }],
    resolve: { dedupe: ['vue'], alias: { 'mml-ipc': join(root, '.mml-deps/mml-ipc/src/index.ts') } },
    optimizeDeps: { exclude: ['@tauri-apps/api/core', '@tauri-apps/api/event', '@tauri-apps/api/window'] },
    server: { host: '127.0.0.1', port: 0, strictPort: true, cors: false, fs: { allow: [root, sdk, resolve(cache)] } },
  });
  server.watcher.add([join(root, 'src'), join(root, 'custom.css'), config]);
  server.watcher.on('all', (_event, file) => {
    const path = file.replaceAll('\\', '/');
    const project = root.replaceAll('\\', '/');
    if (!(path.startsWith(project + '/src/') || path === project + '/custom.css' || path === config.replaceAll('\\', '/'))) return;
    clearTimeout(timer);
    timer = setTimeout(() => { void compile(); }, 200);
  });
  await server.listen();
  return {
    url: `http://127.0.0.1:${server.httpServer.address().port}/?window=main`,
    close: async () => { closed = true; clearTimeout(timer); await server.close(); },
  };
}

// 父进程通过 stdin 生命周期感知：Rust 侧退出时管道断开，这里顺手收掉 Vite。
let closing = false;
async function shutdown() {
  if (closing) return;
  closing = true;
  try { await close(); } finally { process.exit(0); }
}
process.stdin.resume();
process.stdin.on('end', shutdown);
process.stdin.on('close', shutdown);
process.on('SIGTERM', shutdown);

try {
  const preview = template === 'html' ? await htmlPreview(root) : await vuePreview();
  close = preview.close;
  process.stdout.write(READY + JSON.stringify({ type: 'ready', url: preview.url }) + '\n');
} catch (error) {
  console.error(error);
  process.stdout.write(READY + JSON.stringify({ type: 'failed', error: String(error) }) + '\n', () => process.exit(1));
}
