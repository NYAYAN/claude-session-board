// xterm.js tarayıcı paketlerini src/vendor'a kopyalar. Ön yüz paketleyicisiz (vanilla) olduğu ve
// CSP yalnız 'self' betiklerine izin verdiği için kütüphaneler uygulamayla birlikte gömülür.
// Her `tauri build` / `tauri dev` öncesi otomatik çalışır (tauri.conf.json → beforeBuildCommand /
// beforeDevCommand). npm'in prebuild kancası DEĞİL: CI'daki tauri-action `npm run tauri build`
// çağırır ve o yolda prebuild çalışmaz → xterm dosyaları eksik kalırdı.
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'src', 'vendor');
mkdirSync(out, { recursive: true });

const files = [
  ['node_modules/@xterm/xterm/lib/xterm.js', 'xterm.js'],
  ['node_modules/@xterm/xterm/css/xterm.css', 'xterm.css'],
  ['node_modules/@xterm/addon-fit/lib/addon-fit.js', 'addon-fit.js'],
];
for (const [from, to] of files) copyFileSync(join(root, from), join(out, to));
console.log(`vendor: ${files.length} dosya → src/vendor`);
