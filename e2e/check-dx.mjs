import { chromium } from 'playwright';
const base = process.argv[2] || 'http://127.0.0.1:38785';
const urls = [`${base}/`, `${base}/demo/gui/git/clone`];
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage();
for (const url of urls) {
  const consoleMsgs = [], pageErrors = [];
  page.removeAllListeners('console');
  page.removeAllListeners('pageerror');
  page.on('console', m => consoleMsgs.push({ type: m.type(), text: m.text() }));
  page.on('pageerror', e => pageErrors.push(String(e)));
  page.on('requestfailed', r => consoleMsgs.push({ type: 'requestfailed', text: `${r.url()} ${r.failure()?.errorText}` }));
  await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30000 }).catch(e => console.log('GOTO', e.message));
  await page.waitForTimeout(6000);
  const bodyText = await page.locator('body').innerText().catch(() => '');
  const mainLen = await page.locator('#main').innerHTML().then(h => h.length).catch(() => -1);
  const title = await page.title();
  const hasMonaco = await page.evaluate(() => !!globalThis.MonacoHost);
  const mainHTML = await page.locator('#main').innerHTML().catch(() => 'NO');
  console.log('===', url);
  console.log('title', title, 'mainLen', mainLen, 'MonacoHost', hasMonaco);
  console.log('body', JSON.stringify(bodyText.slice(0, 500)));
  console.log('mainHTML', JSON.stringify(mainHTML.slice(0, 300)));
  console.log('pageErrors', pageErrors.slice(0, 10));
  console.log('errs', consoleMsgs.filter(m => m.type==='error' || m.type==='requestfailed').slice(0, 25));
}
await browser.close();
