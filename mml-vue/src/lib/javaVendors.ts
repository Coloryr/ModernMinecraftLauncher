// Java 发行商分组的固定顺序
//
// 分组数据来自 core 的 vendor 字段：版本行只说明启动器叫 openjdk 还是 java，真正的发行商
// 在运行时行里（Temurin / Zulu / Semeru…），见 mml-jvms 的 test_java。
//
// 顺序固定：具名发行商按下面的列表排，OpenJDK 与 Java 永远最后；认不出的发行商排在
// 已知发行商之后、OpenJDK 之前，再按名称排。

/** 具名发行商的固定顺序 */
const VENDOR_ORDER = [
  "Oracle",
  "Zulu",
  "Temurin",
  "Semeru",
  "Microsoft",
  "Amazon Corretto",
  "Liberica",
  "GraalVM",
  "Dragonwell",
  "SapMachine",
  "Tencent Kona",
  "Huawei Bisheng",
  "JetBrains Runtime",
];

/** 通用 OpenJDK 构建，排在具名发行商之后 */
const GENERIC_OPENJDK = "OpenJDK";
/** 认不出发行商的 Java，排在最后 */
const GENERIC_JAVA = "Java";

/**
 * 规范发行商名：空值按 Java 处理，避免多出一个空标题分组
 *
 * - `vendor`: 后端给的发行商
 */
export function normalizeVendor(vendor: string | null | undefined): string {
  const value = (vendor ?? "").trim();
  return value || GENERIC_JAVA;
}

/**
 * 分组排序权重
 *
 * - `vendor`: 发行商
 *
 * # 返回值
 *
 * 越小越靠前；OpenJDK / Java 固定为最后两档
 */
export function vendorRank(vendor: string): number {
  const value = normalizeVendor(vendor);
  if (value === GENERIC_OPENJDK) return 1000;
  if (value === GENERIC_JAVA) return 1001;
  const index = VENDOR_ORDER.indexOf(value);
  return index < 0 ? 900 : index;
}

/**
 * 比较两个发行商分组
 *
 * - `a`: 发行商 A
 * - `b`: 发行商 B
 */
export function compareVendors(a: string, b: string): number {
  const left = normalizeVendor(a);
  const right = normalizeVendor(b);
  return vendorRank(left) - vendorRank(right) || left.localeCompare(right);
}
