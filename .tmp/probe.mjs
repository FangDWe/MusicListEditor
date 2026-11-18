const urls = [
  'https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/ta/ur/tauri-plugin-log',
  'https://index.crates.io/ta/ur/tauri-plugin-log',
];
for (const u of urls) {
  try {
    const r = await fetch(u, { signal: AbortSignal.timeout(10000) });
    const t = await r.text();
    const lines = t.split('\n').filter(Boolean);
    const vers = lines.map((l) => JSON.parse(l).vers).slice(-4);
    console.log('OK', u, 'status', r.status, 'lines', lines.length, 'last', vers.join(','));
  } catch (e) {
    console.log('ERR', u, e.name, e.message);
  }
}
