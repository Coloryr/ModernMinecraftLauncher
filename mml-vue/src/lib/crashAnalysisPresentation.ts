// 只负责展示后端结论；不在前端根据日志重做原因判断。
import type { CrashFindingDto } from "./bindings";

/** 当前界面的翻译函数。 */
type Translate = (key: string, params?: Record<string, string | number>) => string;

/** 一条结论在卡片中的解释、建议与证据说明。 */
export interface FindingPresentation {
  /** 本地化标题，未知类型保留原 id。 */
  title: string;
  /** 已有证据能说明的现象。 */
  reason: string;
  /** 下一步可执行的排查建议。 */
  advice: string;
  /** 后端给出的阶段、位置与其他数据。 */
  details: string[];
  /** 有额外证据支持的场景提示。 */
  hints: string[];
}

/** 后端细节键到界面文案的对应关系。 */
const DETAIL_KEYS: Record<string, string> = {
  mixin_config: "mixinConfig",
  mixin_class: "mixinClass",
  requires: "requires",
  required_version: "requiredVersion",
  actual_version: "actualVersion",
  memory_max_mb: "memory",
  class_file_version: "classFile",
  required_java: "requiredJava",
  loader: "loader",
  stage: "stage",
  location: "location",
  related_location: "relatedLocation",
  bytecode_location: "bytecodeLocation",
  repeat_frame: "repeatFrame",
  dimension: "dimension",
};

/**
 * 把机器可读结论转为当前语言的展示内容。
 *
 * - `finding`: 后端结论，未知类型也能展示
 * - `t`: 当前语言的翻译函数
 *
 * # 返回值
 * 标题、解释、建议、细节与场景提示。
 */
export function presentFinding(finding: CrashFindingDto, t: Translate): FindingPresentation {
  const titleKey = "analysis.finding." + finding.id;
  const title = t(titleKey);
  const reasonKey = "analysis.reason." + finding.id;
  const adviceKey = "analysis.advice." + finding.id;
  const reason = t(reasonKey);
  const advice = t(adviceKey);
  const details: string[] = [];
  const hints: string[] = [];
  for (const line of finding.details) {
    const at = line.indexOf("=");
    if (at < 0) {
      details.push(line);
      continue;
    }
    const key = line.slice(0, at);
    const value = line.slice(at + 1);
    if (key === "hint") {
      const hintKey = "analysis.hint." + value;
      const text = t(hintKey);
      if (text !== hintKey && !hints.includes(text)) hints.push(text);
    } else {
      const label = DETAIL_KEYS[key];
      details.push(label ? t("analysis.detail." + label, { value }) : line);
    }
  }
  return {
    title: title === titleKey ? finding.id : title,
    reason: reason === reasonKey ? t("analysis.reason.unclassified") : reason,
    advice: advice === adviceKey ? t("analysis.advice.unclassified") : advice,
    details,
    hints,
  };
}
