// 独立工具不加载启动器运行时；文案在这里同时维护中英两套。
export const messages = {
  'zh-CN': {
    title: 'M²L 主题开发工具', new: '新建项目', open: '打开项目', save: '保存', newFile: '新建文件',
    install: '安装依赖', preview: '启动预览', stop: '停止预览', reload: '刷新', build: '构建 ZIP',
    devtools: 'F12 开发者工具', reveal: '打开目录', files: '项目文件', output: '输出',
    empty: '选择 demo 创建项目，或打开已有主题项目', chooseFile: '选择文件开始编辑',
    name: '项目目录名', template: 'Demo 类型', source: 'Demo 来源', download: '官方仓库自动下载',
    local: '本地仓库', pickSource: '选择仓库', create: '下载并创建', cancel: '取消',
    css: 'CSS 样式包', page: 'Vue 整页主题', home: 'Vue 主页组件', html: 'HTML 整页主题',
    previewHint: '预览使用独立浏览器环境。启动游戏、登录与文件操作需在启动器中验证。',
    previewEmpty: '启动预览后在这里显示内容', language: '语言', theme: '预览配色',
    studioTheme: '界面配色', dark: '暗色', light: '亮色', window: '窗口', controls: 'CSS 控件', variables: 'CSS 变量',
    invalidTheme: '请选择亮色或暗色',
    search: '筛选控件或变量', insert: '插入到光标处', path: '相对路径，例如 src/Card.vue',
    confirmDiscard: '有未保存修改。放弃这些修改并继续？', unsaved: '未保存',
    ready: '就绪', working: '正在处理…', installed: '依赖安装完成', built: '构建完成：{path}',
    created: '项目已创建：{path}', downloading: '正在下载 demo 和同一版本的开发契约…',
    preparing: '正在准备预览…', previewReady: '预览已启动：{url}', stopped: '预览已停止',
    fileSaved: '文件已保存：{path}', conflict: '文件已被外部修改，请重新打开后再保存：{path}',
    exists: '目标已存在，不能覆盖：{path}', invalidName: '项目名不能包含路径分隔符或 Windows 保留名称',
    outside: '路径必须位于当前项目内：{path}', invalidFile: '只支持编辑不超过 2 MiB 的文本文件：{path}',
    invalidProject: '不是受支持的主题项目，需要 package.json 和 src/theme.json',
    missingSdk: '项目缺少开发契约，请先安装依赖或重新从 demo 创建',
    missingTool: '找不到 {tool}。请安装 Node.js 24+ 与 Git，并重新打开工具',
    nodeVersion: '需要 Node.js 24 或更新版本，当前版本：{version}',
    commandFailed: '命令执行失败：{command}，退出码 {code}', busy: '上一个操作尚未结束',
    cancelled: '操作已取消', missingBuild: '构建没有生成 ZIP，请查看输出日志',
    badSource: '所选仓库没有完整的 demo 或开发契约：{path}',
    unsupported: '此操作仅能在启动器中验证：{command}',
    previewFailed: '预览启动失败，请查看输出日志', unsafeNavigation: '预览不能跳转到外部页面',
    saveBeforeBuild: '请先保存编辑内容', recent: '最近项目', error: '操作失败',
    inspectorHint: 'F12、Ctrl+Shift+I 打开预览的完整 Chromium 开发者工具，可在其中选择并检查元素。',
    chooseParent: '选择项目的父目录', chooseProject: '选择主题项目', chooseRepo: '选择启动器源码仓库',
    confirmClose: '关闭工具会停止预览和正在执行的任务。', close: '关闭',
    refreshFiles: '刷新文件', environment: '环境', revision: 'Demo 版本',
    createHint: '自动准备开发契约、安装依赖并打开预览。项目保存在所选目录。',
    contentPreview: '内容预览', splitRatio: '代码与预览的宽度比例', reopenFile: '从磁盘重新打开', discard: '放弃修改',
  },
  'en-US': {
    title: 'M²L Theme Studio', new: 'New project', open: 'Open project', save: 'Save', newFile: 'New file',
    install: 'Install dependencies', preview: 'Start preview', stop: 'Stop preview', reload: 'Reload', build: 'Build ZIP',
    devtools: 'F12 Developer tools', reveal: 'Open folder', files: 'Project files', output: 'Output',
    empty: 'Create a project from a demo, or open an existing theme project', chooseFile: 'Select a file to edit',
    name: 'Project folder name', template: 'Demo type', source: 'Demo source', download: 'Download from official repository',
    local: 'Local repository', pickSource: 'Choose repository', create: 'Download and create', cancel: 'Cancel',
    css: 'CSS style pack', page: 'Vue window theme', home: 'Vue home component', html: 'HTML window theme',
    previewHint: 'Preview runs in an isolated browser. Verify game launch, login and file operations in the launcher.',
    previewEmpty: 'Start preview to display content here', language: 'Language', theme: 'Preview colors',
    studioTheme: 'Appearance', dark: 'Dark', light: 'Light', window: 'Window', controls: 'CSS controls', variables: 'CSS variables',
    invalidTheme: 'Choose Light or Dark',
    search: 'Filter controls or variables', insert: 'Insert at cursor', path: 'Relative path, e.g. src/Card.vue',
    confirmDiscard: 'Discard unsaved changes and continue?', unsaved: 'Unsaved',
    ready: 'Ready', working: 'Working…', installed: 'Dependencies installed', built: 'Build complete: {path}',
    created: 'Project created: {path}', downloading: 'Downloading the demo and matching development contracts…',
    preparing: 'Preparing preview…', previewReady: 'Preview started: {url}', stopped: 'Preview stopped',
    fileSaved: 'File saved: {path}', conflict: 'File changed outside the editor. Reopen it before saving: {path}',
    exists: 'The destination already exists and cannot be overwritten: {path}', invalidName: 'Project names cannot contain path separators or Windows reserved names',
    outside: 'Path must stay inside the current project: {path}', invalidFile: 'Only text files up to 2 MiB can be edited: {path}',
    invalidProject: 'Unsupported theme project: package.json and src/theme.json are required',
    missingSdk: 'Development contracts are missing. Install dependencies or create the project from a demo again',
    missingTool: '{tool} was not found. Install Node.js 24+ and Git, then restart the tool',
    nodeVersion: 'Node.js 24 or newer is required. Current version: {version}',
    commandFailed: 'Command failed: {command}, exit code {code}', busy: 'The previous operation is still running',
    cancelled: 'Operation cancelled', missingBuild: 'The build did not produce a ZIP. Check the output log',
    badSource: 'The repository does not contain the complete demo or development contracts: {path}',
    unsupported: 'Verify this operation in the launcher: {command}',
    previewFailed: 'Preview failed to start. Check the output log', unsafeNavigation: 'Preview cannot navigate to external pages',
    saveBeforeBuild: 'Save your changes first', recent: 'Recent projects', error: 'Operation failed',
    inspectorHint: 'F12 or Ctrl+Shift+I opens the full Chromium developer tools, including the element inspector.',
    chooseParent: 'Choose a parent folder for the project', chooseProject: 'Choose a theme project', chooseRepo: 'Choose the launcher source repository',
    confirmClose: 'Closing the tool stops the preview and any running tasks.', close: 'Close',
    refreshFiles: 'Refresh files', environment: 'Environment', revision: 'Demo revision',
    createHint: 'Prepares development contracts, installs dependencies and starts preview. The project stays in your chosen folder.',
    contentPreview: 'Content preview', splitRatio: 'Editor and preview width ratio', reopenFile: 'Reopen from disk', discard: 'Discard changes',
  },
};

export function text(locale, key, values = {}) {
  return (messages[locale]?.[key] ?? messages['en-US'][key] ?? key)
    .replace(/\{(\w+)\}/g, (_, name) => String(values[name] ?? ''));
}

export class StudioError extends Error {
  constructor(key, values = {}) {
    super(key);
    this.key = key;
    this.values = values;
  }
}
