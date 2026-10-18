// 配置文件窗口要用的 Monaco 语言包与编辑器贡献
//
// 日志视图是纯文本、不注册任何语言；这里按"配置文件里真的会出现的类型"挑：
// json / json5 / yaml / xml / ini / javascript。**toml 包里没有**，由 monacoToml 自己注册。
//
// json 在 Monaco 里只有语言服务那一份（没有无 worker 的着色器），连它的 worker 一起接上，
// 顺带把 json5 的合法写法放进校验白名单（注释、尾逗号），免得 json5 文件满屏红线。
//
// 下面那批"编辑器行为"的贡献与语言无关，为的是对齐官网示例的手感：查找（Ctrl+F）、折叠、
// 括号配对、补全、悬停、选中词高亮、自动缩进。官网示例直接上 `editor.main`，而它连
// **全部 160 个语言定义**和 css / html / ts 三个语言服务一起吞（实测 +8.8 MB，其中
// ts.worker 6.8 MB）；只挑这几个贡献实测只 +4 KB —— 它们本来就在 editor.api 的依赖图里，
// 只是没人注册，所以这里按需点一下即可，别再引 editor.main。
// Ctrl+F 的查找框本体在 contrib/find，features/find 只是它依赖的共享查找特性：
// 两个都要引（editor.main 也是这么引的），少引一个就只剩命令、没有界面
import "monaco-editor/features/find/register.js";
import "monaco-editor/editor/contrib/find/browser/findController.js";
import "monaco-editor/editor/contrib/folding/browser/folding.js";
import "monaco-editor/editor/contrib/bracketMatching/browser/bracketMatching.js";
import "monaco-editor/editor/contrib/suggest/browser/suggestController.js";
import "monaco-editor/editor/contrib/hover/browser/hoverContribution.js";
import "monaco-editor/editor/contrib/wordHighlighter/browser/wordHighlighter.js";
import "monaco-editor/editor/contrib/indentation/browser/indentation.js";
// 右键菜单的剪切 / 复制 / 粘贴走 `editor.action.clipboard*` 三个动作，它们在剪贴板贡献里注册；
// 原生 Ctrl+C / V 走输入层的 DOM 事件，不引这个也有，缺的只是那三个动作 id
import "monaco-editor/editor/contrib/clipboard/browser/clipboard.js";
// 格式化文档（Shift+Alt+F，右键菜单里那一项）：动作在这里注册，格式化器本身由语言包给 ——
// 当前只有 json 带了（JSON worker 里就是 VS Code 用的那套 vscode-json-languageservice），
// 其余语言包（toml / yaml / xml / ini / js）都只有着色，没有格式化器
import "monaco-editor/editor/contrib/format/browser/formatActions.js";

import "monaco-editor/languages/definitions/ini/register.js";
import "monaco-editor/languages/definitions/javascript/register.js";
import "monaco-editor/languages/definitions/xml/register.js";
import "monaco-editor/languages/definitions/yaml/register.js";
import { jsonDefaults } from "monaco-editor/language/json/monaco.contribution.js";

import { registerToml } from "./monacoToml";

// toml 是自注册的，必须在编辑器建模型之前跑；模块加载时注册一次即可
registerToml();

jsonDefaults.setDiagnosticsOptions({
  validate: true,
  allowComments: true,
  trailingCommas: "ignore",
});

export { applyCodeTheme, monaco } from "./monaco";
