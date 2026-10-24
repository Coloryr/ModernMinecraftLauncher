import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type Locale = 'zh-CN' | 'en-US';
export type Theme = 'Dark' | 'Light';
export type Template = 'css' | 'page' | 'home' | 'html';
export interface Project { root: string; name: string; template: Template; revision: string }
export interface SourceFile { path: string; text: string; hash: string }
export interface CreateProject { parent: string; name: string; template: Template; source: string }
export interface Catalog {
  controls: { className: string; category: string; sources: string[] }[];
  variables: { name: string; description: string }[];
}
export interface StudioState {
  project: Project | null;
  locale: Locale;
  theme: Theme;
  recent: string[];
  environment: { nodeVersion?: string; git?: string; error?: string };
  windowMinSizes: Record<string, { width: number; height: number }>;
}
export type StudioEvent =
  | { type: 'log'; text: string }
  | { type: 'busy'; value: boolean }
  | { type: 'preview'; url: string }
  | { type: 'project'; project: Project };

interface Actions {
  state: { input: undefined; output: StudioState };
  pick: { input: 'source' | 'parent' | 'project'; output: string | null };
  create: { input: CreateProject; output: Project };
  open: { input: string; output: Project };
  files: { input: undefined; output: string[] };
  read: { input: string; output: SourceFile };
  save: { input: { path: string; text: string; expectedHash: string | null }; output: SourceFile };
  catalog: { input: undefined; output: Catalog };
  install: { input: undefined; output: undefined };
  build: { input: undefined; output: string[] };
  preview: { input: undefined; output: string };
  stop: { input: undefined; output: undefined };
  cancel: { input: undefined; output: undefined };
  devtools: { input: undefined; output: undefined };
  locale: { input: Locale; output: undefined };
  theme: { input: Theme; output: Theme };
  dirty: { input: boolean; output: undefined };
  reveal: { input: undefined; output: string };
}

// 动作名 = 命令名去掉 studio_ 前缀；多词动作按小驼峰转下划线。
const command = (action: string) => 'studio_' + action.replace(/[A-Z]/g, (letter) => '_' + letter.toLowerCase());

/**
 * 调用主进程动作
 *
 * - `action`: 动作名，与 Rust 命令一一对应
 * - `input`: 动作参数，无参数时省略
 *
 * # 返回值
 *
 * 动作结果；失败时抛出主进程给的本地化文案
 */
export async function call<K extends keyof Actions>(action: K, ...args: undefined extends Actions[K]['input'] ? [input?: Actions[K]['input']] : [input: Actions[K]['input']]): Promise<Actions[K]['output']> {
  const input = args[0];
  return invoke<Actions[K]['output']>(command(action), input === undefined ? {} : { input });
}

let bootstrapping: Promise<StudioState> | undefined;

/**
 * 首次启动状态，只请求一次
 *
 * # 返回值
 *
 * 项目、语言、配色、最近项目与外部环境
 */
export function bootstrap(): Promise<StudioState> {
  bootstrapping ??= invoke<StudioState>('studio_state');
  return bootstrapping;
}

/**
 * 订阅主进程事件
 *
 * - `callback`: 日志、忙状态、预览地址回调
 *
 * # 返回值
 *
 * 取消订阅函数
 */
export async function onEvent(callback: (event: StudioEvent) => void): Promise<() => void> {
  const unlisten = await Promise.all([
    listen<string>('studio-log', (event) => callback({ type: 'log', text: event.payload })),
    listen<boolean>('studio-busy', (event) => callback({ type: 'busy', value: event.payload })),
    listen<string>('studio-preview', (event) => callback({ type: 'preview', url: event.payload })),
  ]);
  return () => { for (const stop of unlisten) stop(); };
}
