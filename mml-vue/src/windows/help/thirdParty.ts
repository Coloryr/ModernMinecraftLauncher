// 帮助页「开源许可」与「第三方服务与平台」的数据
//
// 只登记**会随安装包发给用户**的东西：内嵌进二进制的 Java 组件、编译进桌面壳的 vendored
// 插件、打进 dist/ 的前端库，以及静态链接的 Rust 依赖汇总；另加本项目会去调用的第三方服务。
//
// **许可全文不随 exe 分发**：这里只给上游地址与许可列表入口，由帮助页调系统浏览器打开。
//
// 文案约定：只放语言中立的数据（包名 / 版本 / 许可表达式 / 地址）；凡是要显示给人看的说明
// 都存 i18n 键，由帮助页 `t()` 出来，见 `note` 与 `purpose`。
//
// 更新时机：升级依赖后。前端各库的版本与许可取实装的 package.json；Rust 那一档取
// `cargo metadata` 的非 dev 依赖闭包（Windows 运行时图 475 个）盘点的结果。

/** 一个第三方组件 */
export interface ThirdPartyItem {
  /** 组件名，与上游仓库名一致 */
  name: string;
  /** 发行物里实际生效的版本 */
  version: string;
  /** 许可，取上游声明的 SPDX 表达式；双许可时注明本项目按哪一个履行 */
  license: string;
  /** 上游地址；空串表示没有可打开的地址，帮助页不显示按钮 */
  url: string;
  /** 补充说明的 i18n 键；没有时为空串 */
  note: string;
}

/** 组件分组，标题走 i18n 的 `winHelp.licenseGroup` 系列 */
export interface ThirdPartyGroup {
  /** 分组键，与 i18n 键一一对应 */
  id: "embedded" | "plugin" | "frontend";
  /** 该组的组件 */
  items: ThirdPartyItem[];
}

/** 本项目会去调用的第三方服务；与组件不同，它没有许可与版本，只有用途与地址 */
export interface ThirdPartyService {
  /** 服务名，专有名词，不翻译 */
  name: string;
  /** 用途文案的 i18n 键 */
  purpose: string;
  /** 服务地址，通常是官网或控制台；空串表示不显示按钮 */
  url: string;
  /** 附加链接，如 API 文档 */
  links: Array<{ label: string; url: string }>;
}

/** 本项目作者自己的作品，不算第三方；列出来是为了让清单完整 */
export const PROJECT_WORKS: ThirdPartyItem[] = [
  {
    name: "ColorASM",
    version: "1.1",
    license: "Apache-2.0",
    url: "https://github.com/Coloryr/ColorMC",
    note: "winHelp.noteColorasm",
  },
];

/** 随发行物分发的第三方组件 */
export const THIRD_PARTY_GROUPS: ThirdPartyGroup[] = [
  {
    id: "embedded",
    items: [
      {
        name: "ForgeWrapper",
        version: "prism-2025-12-07",
        license: "MIT",
        url: "https://github.com/ZekerZhayard/ForgeWrapper",
        note: "winHelp.noteEmbedded",
      },
      {
        name: "OptifineWrapper",
        version: "1.1",
        license: "MIT",
        url: "https://github.com/Coloryr/OptifineWrapper",
        note: "winHelp.noteEmbedded",
      },
    ],
  },
  {
    id: "plugin",
    items: [
      {
        name: "tauri-plugin-decoration",
        version: "3.0.5",
        license: "MIT",
        url: "https://github.com/oovz/tauri-plugin-decoration",
        note: "winHelp.noteLocalPatch",
      },
    ],
  },
  {
    id: "frontend",
    items: [
      {
        name: "vue",
        version: "3.5.43",
        license: "MIT",
        url: "https://github.com/vuejs/core",
        note: "",
      },
      {
        name: "@tauri-apps/api",
        version: "2.12.2",
        license: "Apache-2.0 OR MIT",
        url: "https://github.com/tauri-apps/tauri",
        note: "",
      },
      {
        name: "@tauri-apps/plugin-dialog",
        version: "2.8.1",
        license: "MIT OR Apache-2.0",
        url: "https://github.com/tauri-apps/plugins-workspace",
        note: "",
      },
      {
        name: "monaco-editor",
        version: "0.56.0",
        license: "MIT",
        url: "https://github.com/microsoft/monaco-editor",
        note: "winHelp.noteMonacoBundles",
      },
      {
        name: "vscode extensions/log",
        version: "main",
        license: "MIT",
        url: "https://github.com/microsoft/vscode/blob/main/extensions/log/syntaxes/log.tmLanguage.json",
        note: "winHelp.noteVscodeLog",
      },
      {
        name: "dompurify",
        version: "3.4.16",
        license: "Apache-2.0",
        url: "https://github.com/cure53/DOMPurify",
        note: "winHelp.noteDualLicense",
      },
      {
        name: "marked",
        version: "18.1.0",
        license: "MIT",
        url: "https://github.com/markedjs/marked",
        note: "",
      },
      {
        name: "three",
        version: "0.156.1",
        license: "MIT",
        url: "https://github.com/mrdoob/three.js",
        note: "winHelp.noteSkin3d",
      },
      {
        name: "skinview3d",
        version: "3.4.2",
        license: "MIT",
        url: "https://github.com/bs-community/skinview3d",
        note: "",
      },
      {
        name: "skinview-utils",
        version: "0.7.1",
        license: "MIT",
        url: "https://github.com/bs-community/skinview-utils",
        note: "",
      },
      {
        name: "@vue-flow/core",
        version: "1.48.2",
        license: "MIT",
        url: "https://github.com/bcakmakoglu/vue-flow",
        note: "",
      },
      {
        name: "@vue-flow/background",
        version: "1.3.2",
        license: "MIT",
        url: "https://github.com/bcakmakoglu/vue-flow",
        note: "",
      },
      {
        name: "@dagrejs/dagre",
        version: "3.1.1",
        license: "MIT",
        url: "https://github.com/dagrejs/dagre",
        note: "",
      },
    ],
  },
];

/** Rust 静态链接依赖总数，口径见文件头 */
export const RUST_TOTAL = 475;

/** 许可分布的一行 */
export interface LicenseCount {
  /** SPDX 表达式；聚合行留空，内容改由 `key` 给 */
  license: string;
  /** 该行的 crate 数 */
  count: number;
  /** 聚合行的 i18n 键；普通 SPDX 表达式行没有 */
  key?: string;
}

/** Rust 静态链接依赖按许可表达式的分布，尾部合成一行 */
export const RUST_LICENSE_COUNTS: LicenseCount[] = [
  { license: "MIT OR Apache-2.0", count: 233 },
  { license: "MIT", count: 82 },
  { license: "Apache-2.0 OR MIT", count: 39 },
  { license: "MIT/Apache-2.0", count: 18 },
  { license: "Unicode-3.0", count: 18 },
  { license: "Apache-2.0", count: 10 },
  { license: "Unlicense OR MIT", count: 10 },
  { license: "BSD-3-Clause", count: 8 },
  { license: "MPL-2.0", count: 6 },
  { license: "ISC", count: 5 },
  { license: "", count: 46, key: "winHelp.licenseOtherExprs" },
];

/**
 * 需要点名的 MPL-2.0 组件
 *
 * 文件级弱 copyleft：可以静态链接，但要以可执行形式分发就得告诉接收者去哪拿源码。
 * 本项目未修改这些 crate，所以给出 crates.io 地址即可。
 */
export const RUST_MPL: ThirdPartyItem[] = [
  { name: "cssparser", version: "0.37.0", license: "MPL-2.0", url: "https://crates.io/crates/cssparser", note: "" },
  {
    name: "cssparser-macros",
    version: "0.7.1",
    license: "MPL-2.0",
    url: "https://crates.io/crates/cssparser-macros",
    note: "",
  },
  { name: "dtoa-short", version: "0.3.5", license: "MPL-2.0", url: "https://crates.io/crates/dtoa-short", note: "" },
  { name: "dwrote", version: "0.11.5", license: "MPL-2.0", url: "https://crates.io/crates/dwrote", note: "" },
  { name: "option-ext", version: "0.2.0", license: "MPL-2.0", url: "https://crates.io/crates/option-ext", note: "" },
  { name: "selectors", version: "0.38.0", license: "MPL-2.0", url: "https://crates.io/crates/selectors", note: "" },
];

/** 许可原文入口：SPDX 官方许可列表 */
export const SPDX_LICENSE_LIST = "https://spdx.org/licenses/";

/**
 * 本项目会去调用的第三方服务
 *
 * 与组件不同，这些是别人运营的服务：本项目只是调用方，与它们没有隶属关系，
 * 用对应功能要走用户自己的账号。联机功能的后端接口见 mml-net 的 `openfrp_api`
 * 与 `sakurafrp_api`（通道列表 / 通道配置 / frpc 客户端下载）。
 */
export const THIRD_PARTY_SERVICES: ThirdPartyService[] = [
  {
    name: "Minecraft",
    purpose: "winHelp.serviceMinecraft",
    url: "https://www.minecraft.net/",
    links: [],
  },
  {
    name: "SakuraFrp",
    purpose: "winHelp.serviceFrp",
    url: "https://www.natfrp.com/user/",
    links: [],
  },
  {
    name: "OpenFrp",
    purpose: "winHelp.serviceFrp",
    url: "https://console.openfrp.net/home/",
    links: [{ label: "winHelp.serviceApiDoc", url: "https://github.com/ZGIT-Network/OPENFRP-APIDOC" }],
  },
];
