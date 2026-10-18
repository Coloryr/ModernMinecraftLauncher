// 日志语法：Monaco 自带的语言定义里没有 log（0.56 的 languages/definitions 下 81 种里没有），
// 这里把 VS Code 内置扩展 extensions/log 的语法**逐条转写**成 Monarch（Monaco 只吃 Monarch，
// 上游那份是 TextMate，需要 vscode-textmate + oniguruma 才能跑，这个仓库没有那两个运行时）。
//
// 来源：https://github.com/microsoft/vscode/blob/main/extensions/log/syntaxes/log.tmLanguage.json
// （MIT；上游文件由 emilast/vscode-logfile-highlighter 转换而来，scopeName 是 text.log）
// 取下来的副本留在 target/temp/log-grammar/ 供比对。
//
// 不自己设计规则，30 条 pattern 全保留、顺序不变；与上游的差异只有四处，都是原写法在
// Monaco 里表达不出来：
// 1. (?i) 内联大小写开关 JS 没有，Monarch 的 ignoreCase 又是整份 tokenizer 级别的，所以把上游
//    同一级别的三条（词形 / [xx] 形态 / 行首单字母）里的大小写变体写全；
// 2. 上游 [\s\d\p] 里的 \p 在 JS 里只是字面量 p，写成 [\s\d]；
// 3. 栈帧上游用 begin/end 区域，Monarch 直接一条整行规则；
// 4. 上游一个 pattern 挂多个 scope（如 "string.regexp, strong log.error"），这里取其中
//    log.* 那个当 token 名，颜色由 lib/monaco 的两套控制台主题给。
import { monaco } from "./monaco";

/** 语言 id，日志窗口的编辑器用它 */
export const LOG_LANGUAGE = "log";

/** 是否已注册：组件 setup 每次挂载都会调到，注册必须幂等 */
let registered = false;

/** 注册日志语法：语言本体 + Monarch 着色器 */
export function registerLog() {
  if (registered) return;
  registered = true;

  monaco.languages.register({ id: LOG_LANGUAGE, extensions: [".log"], aliases: ["Log"] });

  monaco.languages.setMonarchTokensProvider(LOG_LANGUAGE, {
    defaultToken: "",
    tokenPostfix: ".log",
    tokenizer: {
      root: [
        // ── verbose / trace（上游 1–3）──
        [/\b(?:TRACE|Trace|trace|VERBOSE|Verbose|verbose)\b:?/, "log.verbose"],
        [/\[(?:verbose|verb|vrb|vb|v)\]/, "log.verbose"],
        [/(?<=^[\s\d]*)\bV\b/, "log.verbose"],

        // ── debug（上游 4–6）──
        [/\b(?:DEBUG|Debug|debug)\b:?/, "log.debug"],
        [/\[(?:debug|dbug|dbg|de|d)\]/, "log.debug"],
        [/(?<=^[\s\d]*)\bD\b/, "log.debug"],

        // ── info（上游 7–9）──
        [/\b(?:HINT|INFO|INFORMATION|Info|NOTICE|II|info|information)\b:?/, "log.info"],
        [/\[(?:information|info|inf|in|i)\]/, "log.info"],
        [/(?<=^[\s\d]*)\bI\b/, "log.info"],

        // ── warning（上游 10–12）──
        [/\b(?:WARNING|WARN|Warn|warning|warn)\b:?/, "log.warning"],
        [/\[(?:warning|warn|wrn|wn|w)\]/, "log.warning"],
        [/(?<=^[\s\d]*)\bW\b/, "log.warning"],

        // ── error（上游 13–15）──
        [/\b(?:ALERT|CRITICAL|EMERGENCY|ERROR|FAILURE|FAIL|Fatal|FATAL|Error|error)\b:?/, "log.error"],
        [/\[(?:error|eror|err|er|e|fatal|fatl|ftl|fa|f)\]/, "log.error"],
        [/(?<=^[\s\d]*)\bE\b/, "log.error"],

        // ── 日期时间（上游 16–19）──
        [/\b\d{4}-\d{2}-\d{2}(?=T|\b)/, "log.date"],
        [/(?<=(?:^|\s))\d{2}[^\w\s]\d{2}[^\w\s]\d{4}\b/, "log.date"],
        [/T?\d{1,2}:\d{2}(?::\d{2}(?:[.,]\d+)?)?(?:Z| ?[+-]\d{1,2}:\d{2})?\b/, "log.date"],
        [/T\d{2}\d{2}(?:\d{2}(?:[.,]\d+)?)?(?:Z| ?[+-]\d{1,2}\d{2})?\b/, "log.date"],

        // ── 常量：哈希 / UUID / 十六进制串 / 数字 / 0x（上游 20–24）──
        [/\b(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{10}|[0-9a-fA-F]{7})\b/, "log.constant"],
        [/\b[0-9a-fA-F]{8}-?(?:[0-9a-fA-F]{4}-?){3}[0-9a-fA-F]{12}\b/, "log.constant"],
        [/\b(?:[0-9a-fA-F]{2,}[:-])+[0-9a-fA-F]{2,}\b/, "log.constant"],
        [/\b(?:[0-9]+|true|false|null)\b/, "log.constant"],
        [/\b0x[a-fA-F0-9]+\b/, "log.constant"],

        // ── 字符串（上游 25–26）──
        [/"([^"]*)"/, "log.string"],
        [/(?<![\w])'[^']*'/, "log.string"],

        // ── 异常类型名（上游 27）──
        [/\b[a-zA-Z.]*Exception\b/, "log.exceptiontype"],

        // ── 栈帧整行（上游 28 的 begin/end）──
        [/^[\t ]*at[\t ].*$/, "log.exception"],

        // ── URL（上游 29）──
        [/\b[a-z]+:\/\/\S+\b\/?/, "log.constant"],

        // ── 点分名字（上游 30）──
        [/(?<![\w/\\])(?:[\w-]+\.)+[\w-]+(?![\w/\\])/, "log.constant"],
      ],
    },
  });
}
