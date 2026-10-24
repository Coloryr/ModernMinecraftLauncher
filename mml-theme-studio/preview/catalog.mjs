import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
const catalog = await import(pathToFileURL(join(process.argv[2], '.mml-deps/mml-vue/src/generated/controls.ts')).href);
process.stdout.write(JSON.stringify({ controls: catalog.CONTROLS, variables: catalog.CSS_VARIABLES }));
