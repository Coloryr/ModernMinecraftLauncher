/* eslint-disable no-console, no-empty, no-undef, preserve-caught-error */
// 前端页面截图器：Vite 开发服务器 + Edge headless + CDP。
//
// 不引入 Playwright / Puppeteer：Node 自带 WebSocket，直接说 CDP。
// 每个场景新建一个 target，注入 mock.js（替换 Tauri 运行时）+ 假数据，再截图。
//
// 用法：
//   node scripts/screenshot/run.mjs                # 全部场景
//   node scripts/screenshot/run.mjs --only=main    # 只跑 id 含 main 的
//   node scripts/screenshot/run.mjs --list         # 只列场景
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { scenarios } from "./scenarios.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const VUE_DIR = resolve(HERE, "../..");
const ROOT = resolve(VUE_DIR, "..");
const MOCK = readFileSync(join(HERE, "mock.js"), "utf8");

const args = process.argv.slice(2);
const flag = (name) => args.some((a) => a === "--" + name);
const value = (name, fallback) => {
  const hit = args.find((a) => a.startsWith("--" + name + "="));
  return hit ? hit.slice(name.length + 3) : fallback;
};
const only = value("only", "");
const ids = value("ids", "").split(",").map((s) => s.trim()).filter(Boolean);
const OUT_DIR = resolve(ROOT, value("out", "target/temp/screens"));
const PORT = Number(value("port", "1420"));
const EDGE = value("edge", "C:\\\\Program Files (x86)\\\\Microsoft\\\\Edge\\\\Application\\\\msedge.exe");

if (flag("list")) {
  for (const s of scenarios) console.log(s.id);
  process.exit(0);
}

const chosen = ids.length ? scenarios.filter((s) => ids.includes(s.id)) : only ? scenarios.filter((s) => s.id.includes(only)) : scenarios;
if (!chosen.length) {
  console.error("没有匹配的场景:", only);
  process.exit(1);
}
if (!existsSync(EDGE)) {
  console.error("找不到 Edge:", EDGE);
  process.exit(1);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** 杀掉子进程及其派生进程：Vite / Edge 都是 shell 拉起来的，只杀 shell 会留下监听端口 */
function killTree(child) {
  if (!child || child.exitCode !== null) return;
  if (process.platform === "win32") {
    spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
  } else {
    child.kill();
  }
}

/** 等 Vite 起来；已经在跑就直接用 */
async function ensureVite() {
  const url = "http://127.0.0.1:" + PORT + "/";
  for (let i = 0; i < 20; i += 1) {
    try {
      const res = await fetch(url);
      if (res.ok) return null;
    } catch {}
    await sleep(300);
  }
  console.log("[shots] 启动 Vite …");
  const child = spawn("npm run dev -- --host 127.0.0.1 --port " + PORT, {
    cwd: VUE_DIR,
    shell: true,
    stdio: "ignore",
  });
  for (let i = 0; i < 120; i += 1) {
    await sleep(500);
    try {
      const res = await fetch(url);
      if (res.ok) return child;
    } catch {}
  }
  killTree(child);
  throw new Error("Vite 启动超时");
}

/** 一个很薄的 CDP 客户端 */
class Cdp {
  constructor(url) {
    this.url = url;
    this.id = 0;
    this.pending = new Map();
    this.handlers = new Map();
  }
  async open() {
    this.ws = new WebSocket(this.url);
    this.ws.onmessage = (event) => {
      const msg = JSON.parse(event.data);
      if (msg.id && this.pending.has(msg.id)) {
        const { resolve: ok, reject } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        if (msg.error) reject(new Error(msg.error.message));
        else ok(msg.result);
        return;
      }
      const key = msg.method;
      for (const cb of this.handlers.get(key) || []) cb(msg.params, msg.sessionId);
    };
    await new Promise((resolve, reject) => {
      this.ws.onopen = resolve;
      this.ws.onerror = reject;
    });
  }
  send(method, params = {}, sessionId) {
    const id = ++this.id;
    const payload = { id, method, params };
    if (sessionId) payload.sessionId = sessionId;
    this.ws.send(JSON.stringify(payload));
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      setTimeout(() => {
        if (this.pending.has(id)) {
          this.pending.delete(id);
          reject(new Error("CDP 超时: " + method));
        }
      }, 30000);
    });
  }
  on(method, cb) {
    if (!this.handlers.has(method)) this.handlers.set(method, []);
    this.handlers.get(method).push(cb);
  }
}

async function evalIn(cdp, sessionId, expression) {
  const res = await cdp.send(
    "Runtime.evaluate",
    { expression, returnByValue: true, awaitPromise: true, userGesture: true },
    sessionId,
  );
  if (res.exceptionDetails) {
    throw new Error(res.exceptionDetails.text + " " + (res.exceptionDetails.exception?.description || ""));
  }
  return res.result.value;
}

async function waitReady(cdp, sessionId, scenario) {
  const selector = JSON.stringify(scenario.ready || "#app");
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    try {
      const ok = await evalIn(
        cdp,
        sessionId,
        "(() => { const el = document.querySelector(" + selector + "); return !!(el && el.children.length && document.readyState === 'complete'); })()",
      );
      if (ok) {
        await sleep(700);
        await evalIn(cdp, sessionId, "document.documentElement.classList.add('no-anim')");
        await sleep(120);
        return;
      }
    } catch {}
    await sleep(120);
  }
  throw new Error("页面没有就绪: " + scenario.id);
}

function scenarioSource(scenario) {
  const payload = {
    commands: scenario.commands || {},
    label: scenario.label || "mml-" + scenario.kind,
    width: (scenario.size || [1280, 800])[0],
    height: (scenario.size || [1280, 800])[1],
    savePath: scenario.savePath || undefined,
    openPath: scenario.openPath || undefined,
  };
  return "window.__SHOT__ = " + JSON.stringify(payload) + ";\n" + MOCK;
}

async function shoot(cdp, browser, scenario, index) {
  const [width, height] = scenario.size || [1280, 800];
  const { targetId } = await browser.send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await browser.send("Target.attachToTarget", { targetId, flatten: true });
  const consoleErrors = [];
  cdp.on("Runtime.exceptionThrown", (params, sid) => {
    if (sid === sessionId) consoleErrors.push(params.exceptionDetails?.text || "exception");
  });
  cdp.on("Runtime.consoleAPICalled", (params, sid) => {
    if (sid === sessionId && (params.type === "error" || params.type === "warning")) {
      consoleErrors.push(
        params.type + ": " + params.args.map((a) => a.value ?? a.description ?? a.type).join(" "),
      );
    }
  });

  try {
    await cdp.send("Page.enable", {}, sessionId);
    await cdp.send("Runtime.enable", {}, sessionId);
    await cdp.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false }, sessionId);
    await cdp.send("Page.addScriptToEvaluateOnNewDocument", { source: scenarioSource(scenario) }, sessionId);
    const query = scenario.query ? "&" + scenario.query.replace(/^\?/, "") : "";
    const url = "http://127.0.0.1:" + PORT + "/?window=" + scenario.kind + query;
    await cdp.send("Page.navigate", { url }, sessionId);
    await waitReady(cdp, sessionId, scenario);

    for (const action of scenario.actions || []) {
      if (typeof action === "string") await evalIn(cdp, sessionId, action);
      else if (action.wait) await sleep(action.wait);
      else if (action.eval) await evalIn(cdp, sessionId, action.eval);
      await sleep(action.after ?? 220);
    }

    const shot = await cdp.send("Page.captureScreenshot", { format: "png", fromSurface: true }, sessionId);
    const file = join(OUT_DIR, (scenario.id || "shot-" + index) + ".png");
    writeFileSync(file, Buffer.from(shot.data, "base64"));

    const runtimeErrors = await evalIn(cdp, sessionId, "JSON.stringify(window.__SHOT__ ? window.__SHOT__.errors : [])");
    const unknown = await evalIn(cdp, sessionId, "JSON.stringify(window.__SHOT__ ? [...new Set(window.__SHOT__.unknown)] : [])");
    return {
      id: scenario.id,
      file,
      consoleErrors: [...new Set(consoleErrors)].slice(0, 20),
      runtimeErrors: JSON.parse(runtimeErrors || "[]").slice(0, 20),
      unknown: JSON.parse(unknown || "[]"),
    };
  } catch (error) {
    let diag = "";
    try {
      diag = await evalIn(
        cdp,
        sessionId,
        "JSON.stringify({ errors: window.__SHOT__?.errors ?? [], unknown: window.__SHOT__?.unknown ?? [], text: (document.body?.innerText ?? '').slice(0, 400) })",
      );
    } catch {}
    throw new Error(String(error) + " | " + diag);
  } finally {
    await cdp.send("Target.closeTarget", { targetId }).catch(() => {});
  }
}

async function launchEdge(port, profile) {
  if (existsSync(profile)) rmSync(profile, { recursive: true, force: true });
  mkdirSync(profile, { recursive: true });
  const child = spawn(
    EDGE,
    [
      "--headless=new",
      "--disable-gpu",
      "--hide-scrollbars",
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-background-networking",
      "--disable-component-update",
      "--disable-sync",
      "--mute-audio",
      "--remote-debugging-port=" + port,
      "--user-data-dir=" + profile,
      "--force-device-scale-factor=1",
      "--window-size=1400,900",
      "about:blank",
    ],
    { stdio: "ignore" },
  );
  for (let i = 0; i < 60; i += 1) {
    await sleep(250);
    try {
      const res = await fetch("http://127.0.0.1:" + port + "/json/version");
      if (res.ok) {
        const info = await res.json();
        return { child, wsUrl: info.webSocketDebuggerUrl };
      }
    } catch {}
  }
  killTree(child);
  throw new Error("Edge 调试端口没起来");
}


/** 生成一个可直接打开的索引页：所有截图按场景 id 排列，失败与未 mock 一并标出 */
function writeIndex(report) {
  const cards = report
    .map((item) => {
      const name = item.file ? item.file.split(/[\\/]/).pop() : "";
      const notes = [];
      if (item.error) notes.push("失败：" + item.error);
      for (const u of item.unknown || []) notes.push("未 mock：" + u);
      for (const e of item.runtimeErrors || []) notes.push("页面错误：" + e);
      const img = name ? '<a href="' + name + '"><img loading="lazy" src="' + name + '" alt="' + item.id + '"></a>' : "";
      return '<figure><figcaption>' + item.id + "</figcaption>" + img + (notes.length ? "<p>" + notes.join("<br>") + "</p>" : "") + "</figure>";
    })
    .join("\n");
  const html = [
    "<!doctype html>",
    '<meta charset="utf-8">',
    "<title>M²L 页面截图</title>",
    "<style>",
    "body{font-family:system-ui,sans-serif;background:#eef1f5;margin:20px;color:#1d2229}",
    "h1{font-size:18px}",
    ".grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(360px,1fr));gap:16px}",
    "figure{margin:0;background:#fff;border:1px solid #d8dce3;border-radius:10px;padding:8px}",
    "figcaption{font-size:12px;font-weight:600;margin-bottom:6px}",
    "img{width:100%;height:auto;border-radius:6px;display:block}",
    "p{font-size:11px;color:#6a7480;margin:6px 0 0}",
    "</style>",
    "<h1>M²L 页面截图 · " + report.length + " 个场景</h1>",
    '<div class="grid">' + cards + "</div>",
  ].join("\n");
  writeFileSync(join(OUT_DIR, "index.html"), html);
}

/** PID 对应的进程是否还活着；Windows 上已退出的 PID 会抛错 */
function isProcessAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** 清掉上次异常退出留下的 Edge 配置目录；只删进程已不存在的，避免误删并行截图 */
function cleanupStaleProfiles() {
  const tempDir = join(ROOT, "target", "temp");
  if (!existsSync(tempDir)) return;
  for (const name of readdirSync(tempDir)) {
    if (!name.startsWith("shot-edge-")) continue;
    const pid = Number(name.slice("shot-edge-".length));
    if (!Number.isInteger(pid) || pid <= 0 || pid === process.pid) continue;
    if (isProcessAlive(pid)) continue;
    try {
      rmSync(join(tempDir, name), {
        recursive: true,
        force: true,
        maxRetries: 5,
        retryDelay: 200,
      });
    } catch {
      // Edge 可能刚退出还在释放文件；下次运行再清
    }
  }
}

/** 等子进程退出，超时也继续清理；Windows 上先杀父进程，文件锁通常随后释放 */
function waitExit(child, timeout) {
  return new Promise((resolve) => {
    if (child.exitCode !== null) {
      resolve();
      return;
    }
    const timer = setTimeout(resolve, timeout);
    child.once("exit", () => {
      clearTimeout(timer);
      resolve();
    });
  });
}

/** 删 Edge 配置目录；被杀进程可能短暂占着文件，重试几次 */
async function removeProfile(profile) {
  for (let i = 0; i < 5; i += 1) {
    try {
      rmSync(profile, {
        recursive: true,
        force: true,
        maxRetries: 5,
        retryDelay: 200,
      });
      return;
    } catch {
      await sleep(300);
    }
  }
  console.warn("临时目录未能删除：", profile);
}

async function main() {
  cleanupStaleProfiles();
  if (existsSync(OUT_DIR)) {
    for (const name of readdirSync(OUT_DIR)) {
      if (name.endsWith(".png") || name === "report.json") rmSync(join(OUT_DIR, name), { force: true });
    }
  }
  mkdirSync(OUT_DIR, { recursive: true });
  const vite = await ensureVite();
  const port = 9300 + (process.pid % 400);
  const profile = join(ROOT, "target", "temp", "shot-edge-" + process.pid);
  const report = [];
  let edge = null;
  try {
    edge = await launchEdge(port, profile);
    const cdp = new Cdp(edge.wsUrl);
    await cdp.open();
    for (let i = 0; i < chosen.length; i += 1) {
      const scenario = chosen[i];
      process.stdout.write("[" + (i + 1) + "/" + chosen.length + "] " + scenario.id + " … ");
      try {
        const item = await shoot(cdp, cdp, scenario, i);
        report.push(item);
        console.log("ok");
      } catch (e) {
        report.push({ id: scenario.id, error: String(e) });
        console.log("失败:", String(e));
      }
    }
  } finally {
    writeFileSync(join(OUT_DIR, "report.json"), JSON.stringify(report, null, 2));
    writeIndex(report);
    if (edge) {
      killTree(edge.child);
      await waitExit(edge.child, 3000);
    }
    await removeProfile(profile);
    if (vite) killTree(vite);
  }
  const failed = report.filter((r) => r.error || (r.unknown && r.unknown.length) || (r.consoleErrors && r.consoleErrors.length));
  console.log("\n[done] 图片目录:", OUT_DIR);
  if (failed.length) {
    console.log("[有问题的场景]");
    for (const item of failed) {
      console.log("-", item.id);
      if (item.error) console.log("   ", item.error);
      for (const u of item.unknown || []) console.log("    未 mock:", u);
      for (const e of item.runtimeErrors || []) console.log("    页面错误:", e);
      for (const e of item.consoleErrors || []) console.log("    控制台:", e);
    }
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
