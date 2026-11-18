const idx = ['https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/ta/ur/tauri-plugin-log', 'https://index.crates.io/ta/ur/tauri-plugin-log'];
let t = null;
for (const u of idx) {
  try { t = await (await fetch(u, { signal: AbortSignal.timeout(15000) })).text(); break; } catch (e) { console.log('fail', u, e.message); }
}
const lines = t.split('\n').filter(Boolean).map((l) => JSON.parse(l));
const v = lines.filter((l) => l.vers === '2.10.2')[0];
for (const d of v.deps) console.log(`${d.name} ${d.req} kind=${d.kind} target=${d.target ?? '-'} optional=${d.optional} features=${JSON.stringify(d.features)}`);
console.log('--- features ---');
console.log(JSON.stringify(v.features, null, 1));
