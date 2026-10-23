import { readFile, realpath, stat } from 'node:fs/promises';
import { extname, join } from 'node:path';
import { inside } from '../lib/files.mjs';

const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.jpg': 'image/jpeg', '.woff2': 'font/woff2' };

export async function asset(root, name) {
  let decoded;
  try { decoded = decodeURIComponent(name); } catch { return null; }
  if (decoded.includes('\\') || decoded.includes('\0') || decoded.split('/').some((part) => part === '..' || part.startsWith('.'))) return null;
  try {
    const base = await realpath(root);
    const path = await realpath(join(base, decoded));
    if (!inside(base, path) || !(await stat(path)).isFile()) return null;
    return { data: await readFile(path), mime: MIME[extname(path)] || 'application/octet-stream' };
  } catch { return null; }
}

export function respond(response, file) {
  if (!file) { response.writeHead(404); response.end(); return; }
  response.writeHead(200, { 'Content-Type': file.mime, 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' });
  response.end(file.data);
}
