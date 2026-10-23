// 项目、构建任务与预览进程由桌面主进程持有，关闭工具时一并回收。
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { StudioError, text } from './messages.mjs';
import { createProject, openProject, installProject, buildProject } from './project.mjs';
import { listFiles, readText, saveText } from './files.mjs';
import { environment, run, stopChild } from './process.mjs';

const previewWorker = fileURLToPath(new URL('../preview/worker.mjs', import.meta.url));
const catalogWorker = fileURLToPath(new URL('../preview/catalog.mjs', import.meta.url));

export class Studio {
  constructor({ cache, emit = () => {}, locale = 'zh-CN' }) {
    this.cache = cache;
    this.emit = emit;
    this.locale = locale;
    this.project = null;
    this.task = null;
    this.controller = null;
    this.preview = null;
    this.previewUrl = '';
  }

  log = (value) => this.emit({ type: 'log', text: String(value).slice(-50000) });
  message(key, values) { this.log(text(this.locale, key, values) + '\n'); }

  async perform(work) {
    if (this.task) throw new StudioError('busy');
    this.controller = new AbortController();
    const options = { signal: this.controller.signal, log: this.log };
    this.task = Promise.resolve().then(() => work(options));
    this.emit({ type: 'busy', value: true });
    try { return await this.task; }
    finally { this.task = null; this.controller = null; this.emit({ type: 'busy', value: false }); }
  }

  async create(options) {
    return this.perform(async (processOptions) => {
      this.message('downloading');
      const project = await createProject({ ...options, cache: this.cache, ...processOptions });
      await this.select(project);
      this.message('created', { path: project.root });
      return project;
    });
  }

  async open(root) {
    if (this.task) throw new StudioError('busy');
    const project = await openProject(root);
    await this.select(project);
    return project;
  }

  async select(project) {
    await this.stopPreview();
    this.project = project;
    this.emit({ type: 'project', project });
  }

  current() {
    if (!this.project) throw new StudioError('invalidProject');
    return this.project;
  }

  files() { return listFiles(this.current().root); }
  read(path) { return readText(this.current().root, path); }
  async save(input) {
    const file = await saveText(this.current().root, input);
    this.message('fileSaved', { path: input.path });
    return file;
  }

  install() {
    return this.perform(async (options) => {
      await this.stopPreview();
      await installProject(this.current(), options);
      this.message('installed');
    });
  }

  build() {
    return this.perform(async (options) => {
      const artifacts = await buildProject(this.current(), options);
      for (const path of artifacts) this.message('built', { path });
      return artifacts;
    });
  }

  async catalog() {
    if (this.current().template === 'html') return { controls: [], variables: [] };
    const env = await environment();
    try { return JSON.parse(await run(env.node, [catalogWorker, this.current().root])); }
    catch { throw new StudioError('missingSdk'); }
  }

  startPreview() {
    return this.perform(async ({ signal }) => {
      await this.stopPreview();
      this.message('preparing');
      const env = await environment();
      const project = this.current();
      await mkdir(this.cache, { recursive: true });
      const cache = await mkdtemp(join(this.cache, 'preview-'));
      const child = spawn(env.node, [previewWorker, project.root, project.template, cache, this.locale], {
        cwd: project.root, windowsHide: true, detached: process.platform !== 'win32',
        stdio: ['ignore', 'pipe', 'pipe', 'ipc'],
      });
      this.preview = child;
      child.stdout.on('data', (value) => this.log(value));
      child.stderr.on('data', (value) => this.log(value));
      child.once('close', () => {
        if (this.preview === child) {
          this.preview = null;
          this.previewUrl = '';
          this.emit({ type: 'preview', url: '' });
        }
      });
      const cancel = () => { void stopChild(child); };
      signal.addEventListener('abort', cancel, { once: true });
      try {
        this.previewUrl = await new Promise((resolve, reject) => {
          const timeout = setTimeout(() => reject(new StudioError('previewFailed')), 120000);
          const done = (error, url) => { clearTimeout(timeout); if (error) reject(error); else resolve(url); };
          child.once('error', (error) => done(error));
          child.once('exit', () => done(new StudioError(signal.aborted ? 'cancelled' : 'previewFailed')));
          child.on('message', (message) => {
            if (message.type === 'ready') done(null, message.url);
            if (message.type === 'failed') done(new StudioError('previewFailed'));
          });
        });
        if (signal.aborted) throw new StudioError('cancelled');
        this.message('previewReady', { url: this.previewUrl });
        this.emit({ type: 'preview', url: this.previewUrl });
        return this.previewUrl;
      } catch (error) { await this.stopPreview(); throw error; }
      finally { signal.removeEventListener('abort', cancel); }
    });
  }

  async stopPreview() {
    const child = this.preview;
    this.preview = null;
    this.previewUrl = '';
    await stopChild(child);
    this.emit({ type: 'preview', url: '' });
  }

  async cancel() {
    this.controller?.abort();
    await this.task?.catch(() => {});
  }

  async close() { await this.cancel(); await this.stopPreview(); }
}
