// npm 构建入口按宿主平台选用已有的依赖还原脚本。
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const windows = process.platform === "win32";
const script = fileURLToPath(new URL(windows ? "prepare.ps1" : "prepare.sh", import.meta.url));
const args = windows
  ? ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script]
  : [script];
const result = spawnSync(windows ? "powershell.exe" : "bash", [...args, ...process.argv.slice(2)], {
  stdio: "inherit",
  windowsHide: true,
});
if (result.error) console.error(result.error.message);
process.exitCode = result.status ?? 1;
