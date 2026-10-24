import { createServer } from 'node:http';
import { watch } from 'node:fs';
import { join } from 'node:path';
import { asset, respond } from './assets.mjs';
import { DEVTOOLS_SCRIPT } from './shortcut.mjs';

export async function htmlPreview(root) {
  const clients = new Set();
  const server = createServer(async (request, response) => {
    const path = new URL(request.url, 'http://127.0.0.1').pathname;
    if (path === '/__reload') {
      response.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-cache' });
      response.write(': ready\n\n');
      clients.add(response);
      request.on('close', () => clients.delete(response));
      return;
    }
    const file = await asset(join(root, 'src'), path === '/' ? 'index.html' : path.slice(1));
    if (file?.mime.startsWith('text/html')) file.data = Buffer.from(file.data.toString('utf8').replace(/<\/head>/i, `<script>new EventSource("/__reload").onmessage=()=>location.reload();${DEVTOOLS_SCRIPT}</script></head>`));
    respond(response, file);
  });
  const watcher = watch(join(root, 'src'), { recursive: true }, () => { for (const client of clients) client.write('data: reload\n\n'); });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  return {
    url: `http://127.0.0.1:${server.address().port}/`,
    close: async () => { watcher.close(); for (const client of clients) client.end(); await new Promise((resolve) => server.close(resolve)); },
  };
}
