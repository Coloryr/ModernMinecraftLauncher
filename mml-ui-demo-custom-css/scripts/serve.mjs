// 零依赖静态服务器：把示例 CSS 用 http 暴露出来，方便启动器用 ?customCss= 加载
//
// 只给开发预览用：绑定 127.0.0.1，禁止目录穿越，不做缓存。
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, resolve, sep } from "node:path";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const port = Number(process.env.PORT ?? 1521);

const TYPES = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
};

createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", `http://127.0.0.1:${port}`);
  const rel = decodeURIComponent(url.pathname).replace(/^\/+/, "");
  const file = join(root, rel === "" ? "index.html" : rel);

  // 路径必须落在本目录内：../ 之类直接 403
  if (file !== root && !file.startsWith(root + sep)) {
    res.writeHead(403, { "content-type": "text/plain; charset=utf-8" });
    res.end("forbidden");
    return;
  }

  try {
    const body = await readFile(file);
    res.writeHead(200, {
      "content-type": TYPES[extname(file)] ?? "application/octet-stream",
      "access-control-allow-origin": "*",
      "cache-control": "no-cache",
    });
    res.end(body);
  } catch {
    res.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
    res.end("not found");
  }
}).listen(port, "127.0.0.1", () => {
  console.log("");
  console.log(`自定义 CSS 示例已启动：http://127.0.0.1:${port}/`);
  console.log("启动器前端跑起来后打开（端口按需改）：");
  console.log(
    `  http://localhost:1420/?window=main&customCss=http://127.0.0.1:${port}/custom.css`,
  );
  console.log("");
});
