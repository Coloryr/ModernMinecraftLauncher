import * as monaco from 'monaco-editor/editor/editor.api';
import type { Catalog } from './api';

/** 一条补全建议，替换范围由调用处按当前位置补 */
interface Draft {
  /** 列表里显示的名字 */
  label: string;
  /** 确认后插入的文本 */
  insertText: string;
  /** 列表右侧的说明，取清单里的用途 / 分类 */
  detail: string;
  /** 图标种类 */
  kind: monaco.languages.CompletionItemKind;
}

/** 当前光标处要替换的前缀，以及清单推出来的建议 */
interface Drafts {
  /** 前缀长度，单位字符 */
  length: number;
  /** 候选建议 */
  items: Draft[];
}

/** 控件类名统一从清单取，避免在补全里另抄一份 */
function controls(catalog: Catalog, prefix: string, dotted: boolean): Draft[] {
  return catalog.controls
    .filter((entry) => (dotted ? '.' + entry.className : entry.className).toLowerCase().startsWith(prefix.toLowerCase()))
    .map((entry) => ({
      label: dotted ? '.' + entry.className : entry.className,
      insertText: dotted ? '.' + entry.className : entry.className,
      detail: entry.category,
      kind: monaco.languages.CompletionItemKind.Class,
    }));
}

/**
 * 判断当前是不是在写自定义属性，值里的负号不弹变量
 *
 * - `before`: 光标处这段前缀之前的内容
 * - `typed`: 已经敲出来的前缀
 */
function variableContext(before: string, typed: string): boolean {
  // 敲了两个减号就是要写变量，不再看上下文
  if (typed.startsWith('--')) return true;
  const trimmed = before.trimEnd();
  return trimmed === '' || trimmed.endsWith('{') || trimmed.endsWith(';') || trimmed.endsWith('var(');
}

/**
 * CSS 补全：`--` 开头的主题变量、选择器里的控件类名
 *
 * - `catalog`: 当前项目的 CSS 清单
 * - `line`: 光标所在行、光标之前的内容
 *
 * # 返回值
 *
 * 没有可补的内容时返回 `undefined`
 */
function cssDrafts(catalog: Catalog, line: string): Drafts | undefined {
  const variable = /-{1,2}[-\w]*$/.exec(line);
  if (variable && variableContext(line.slice(0, variable.index), variable[0])) {
    return {
      length: variable[0].length,
      items: catalog.variables
        .filter((entry) => entry.name.toLowerCase().startsWith(variable[0].toLowerCase()))
        .map((entry) => ({ label: entry.name, insertText: entry.name, detail: entry.description, kind: monaco.languages.CompletionItemKind.Variable })),
    };
  }
  // 点号前面必须是行首、空白或组合符，才能把 0.5em、url(a.b) 这类值排除掉
  const selector = /(?:^|[\s,>+~])(\.[-\w]*)$/.exec(line);
  if (!selector) return undefined;
  return { length: selector[1].length, items: controls(catalog, selector[1], true) };
}

/**
 * HTML 补全：`class` 属性值里的控件类名
 *
 * - `catalog`: 当前项目的 CSS 清单
 * - `line`: 光标所在行、光标之前的内容
 *
 * # 返回值
 *
 * 没有可补的内容时返回 `undefined`
 */
function htmlDrafts(catalog: Catalog, line: string): Drafts | undefined {
  const attribute = /(?:^|[\s"'])class\s*=\s*("[^"]*|'[^']*|[^\s>]*)$/i.exec(line);
  if (!attribute) return undefined;
  const value = attribute[1].replace(/^["']/, '');
  const word = /[-\w]*$/.exec(value)?.[0] ?? '';
  return { length: word.length, items: controls(catalog, word, false) };
}

/**
 * 注册 mml 清单补全，覆盖 CSS 变量 / 控件类名与 HTML 的 class 值
 *
 * 语言自带的补全（CSS 属性、HTML 标签、JS 符号）由 Monaco 的语言服务负责，这里只补清单里才有的名字。
 *
 * - `catalog`: 读取当前项目的 CSS 清单；换项目后自动跟着变
 *
 * # 返回值
 *
 * 取消注册的句柄，编辑器卸载时必须调用
 */
export function registerCompletions(catalog: () => Catalog): monaco.IDisposable {
  const provider = (drafts: (catalog: Catalog, line: string) => Drafts | undefined): monaco.languages.CompletionItemProvider => ({
    provideCompletionItems(model, position) {
      const line = model.getLineContent(position.lineNumber).slice(0, position.column - 1);
      const found = drafts(catalog(), line);
      if (!found) return { suggestions: [] };
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: position.column - found.length,
        endColumn: position.column,
      };
      return { suggestions: found.items.map((item) => ({ ...item, range, documentation: item.detail })) };
    },
  });
  const css = monaco.languages.registerCompletionItemProvider('css', { triggerCharacters: ['-', '.'], ...provider(cssDrafts) });
  const html = monaco.languages.registerCompletionItemProvider('html', { triggerCharacters: ['"', "'"], ...provider(htmlDrafts) });
  return { dispose: () => { css.dispose(); html.dispose(); } };
}
