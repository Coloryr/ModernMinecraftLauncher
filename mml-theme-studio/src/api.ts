export type Locale = 'zh-CN' | 'en-US';
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
  recent: string[];
  environment: { nodeVersion?: string; git?: string; error?: string };
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
  reload: { input: undefined; output: undefined };
  devtools: { input: undefined; output: undefined };
  bounds: { input: { x: number; y: number; width: number; height: number; visible: boolean }; output: undefined };
  previewOptions: { input: { kind: string; theme: string }; output: undefined };
  locale: { input: Locale; output: undefined };
  dirty: { input: boolean; output: undefined };
  reveal: { input: undefined; output: string };
}

declare global {
  interface Window {
    studio: {
      call(action: string, input?: unknown): Promise<{ ok: boolean; value?: unknown; error?: string }>;
      onEvent(callback: (event: StudioEvent) => void): () => void;
    };
  }
}

export async function call<K extends keyof Actions>(action: K, ...args: undefined extends Actions[K]['input'] ? [input?: Actions[K]['input']] : [input: Actions[K]['input']]): Promise<Actions[K]['output']> {
  const result = await window.studio.call(action, args[0]);
  if (!result.ok) throw new Error(result.error);
  return result.value as Actions[K]['output'];
}
