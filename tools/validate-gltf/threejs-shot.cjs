const { chromium } = require('playwright');
(async () => {
  const http = require('http'), fs = require('fs'), path = require('path');
  const srv = http.createServer((q, r) => { const p = path.join(__dirname, decodeURIComponent(q.url.split('?')[0])); fs.readFile(p, (e, d) => { if (e) { r.writeHead(404); r.end(); return; } r.writeHead(200, { 'Content-Type': p.endsWith('.js') ? 'text/javascript' : p.endsWith('.html') ? 'text/html' : 'application/octet-stream' }); r.end(d); }); }).listen(8765);
  const b = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader'] });
  const p = await b.newPage({ viewport: { width: 1600, height: 560 } });
  p.on('console', m => console.log('page:', m.text())); p.on('pageerror', e => console.log('error:', e.message));
  await p.goto(`http://localhost:8765/view.html?files=${process.argv[2]}`);
  await p.waitForFunction('window.DONE === true', null, { timeout: 120000 });
  console.log(JSON.stringify(await p.evaluate('window.REPORT')));
  await p.screenshot({ path: process.argv[3] });
  await b.close(); srv.close();
})();
