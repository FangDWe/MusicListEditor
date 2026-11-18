import { createHash } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';

const INDEXES = [
  'https://mirrors.tuna.tsinghua.edu.cn/crates.io-index',
  'https://index.crates.io',
];
const DL = [
  (n, v) => `https://mirrors.tuna.tsinghua.edu.cn/crates.io/api/v1/crates/${n}/${v}/download`,
  (n, v) => `https://static.crates.io/crates/${n}/${n}-${v}.crate`,
];
const targets = [
  ['ta/ur/tauri-plugin-log', '2.10.2'],
  ['fe/rn/fern', '0.7.1'],
];

async function tryFetch(url) {
  for (let i = 0; i < 4; i += 1) {
    try {
      const res = await fetch(url, { signal: AbortSignal.timeout(45000) });
      if (res.ok) return res;
      console.log(`  ${url} -> HTTP ${res.status}`);
      if (res.status === 404) return null;
    } catch (e) {
      console.log(`  ${url} -> ${e.cause?.code ?? e.message} (attempt ${i + 1})`);
    }
  }
  return null;
}

await mkdir('.tmp/crates', { recursive: true });

for (const [path, vers] of targets) {
  const name = path.split('/').pop();
  let meta = null;
  for (const base of INDEXES) {
    const res = await tryFetch(`${base}/${path}`);
    if (!res) continue;
    const lines = (await res.text()).split('\n').filter(Boolean).map((l) => JSON.parse(l));
    meta = lines.find((l) => l.vers === vers);
    if (meta) break;
  }
  if (!meta) throw new Error(`index entry missing for ${name}@${vers}`);

  let buf = null;
  for (const make of DL) {
    const res = await tryFetch(make(name, vers));
    if (!res) continue;
    buf = Buffer.from(await res.arrayBuffer());
    break;
  }
  if (!buf) throw new Error(`download failed for ${name}@${vers}`);
  const digest = createHash('sha256').update(buf).digest('hex');
  const out = `.tmp/crates/${name}-${vers}.crate`;
  await writeFile(out, buf);
  console.log(`${name}-${vers} ${buf.length}B sha256 ${digest === meta.cksum ? 'MATCH' : 'MISMATCH'} -> ${out}`);
}
