import { chromium } from 'playwright';
const base='http://127.0.0.1:4173';
// wait for webServer from playwright - or start our own
import { spawn } from 'node:child_process';
const dist='/home/raju/repositories/positive-intentions/git/git-gallery/target/dx/git-gallery/debug/web/public';
const srv=spawn('npx',['--yes','serve@14',dist,'-l','4199','-s'],{stdio:['ignore','pipe','pipe']});
await new Promise(r=>setTimeout(r,2500));
const browser=await chromium.launch({headless:true});
const page=await browser.newPage();
for (const path of ['/','/demo/gui/git/clone']) {
  const errs=[];
  page.removeAllListeners('pageerror');
  page.on('pageerror',e=>errs.push(String(e)));
  await page.goto('http://127.0.0.1:4199'+path,{waitUntil:'domcontentloaded',timeout:30000});
  await page.waitForTimeout(4000);
  const body=(await page.locator('body').innerText()).slice(0,350);
  const monacoH=await page.locator('#git-gallery-monaco').evaluate(el=>el.clientHeight).catch(()=>null);
  const hasHost=await page.evaluate(()=>!!globalThis.MonacoHost);
  const banner=body.includes('Monaco mount failed');
  console.log(path,'monacoH',monacoH,'MonacoHost',hasHost,'mountFailed',banner);
  console.log('body',JSON.stringify(body));
  console.log('pageErrors',errs);
}
await browser.close();
srv.kill();
