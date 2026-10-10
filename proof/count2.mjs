import { chromium } from 'playwright';
import http from 'node:http'; import fs from 'node:fs'; import path from 'node:path';
const [,, root, port, label] = process.argv;
const types={'.html':'text/html','.js':'text/javascript','.wasm':'application/wasm','.css':'text/css','.json':'application/json','.webmanifest':'application/manifest+json','.png':'image/png'};
const srv=http.createServer((q,r)=>{let p=path.join(root,decodeURIComponent(q.url.split('?')[0]));if(p.endsWith('/'))p+='index.html';fs.readFile(p,(e,b)=>{if(e){r.writeHead(404);return r.end()}r.writeHead(200,{'content-type':types[path.extname(p)]||'application/octet-stream'});r.end(b)})}).listen(+port);
const b=await chromium.launch(); const pg=await b.newPage({viewport:{width:1280,height:800}});
await pg.goto(`http://localhost:${port}/`);
// wait for boot sequence to finish: canvas present and status text stable/non-empty
await pg.waitForFunction(()=>document.getElementById('status')?.textContent.trim().length>0,null,{timeout:30000});
await pg.waitForTimeout(8000);
const demoOn=()=>pg.evaluate(()=>document.getElementById('demo').checked);
await pg.click('#demo'); await pg.waitForTimeout(500);
if(!(await demoOn())){ await pg.locator('canvas').first().focus().catch(()=>{}); await pg.keyboard.press('d'); await pg.waitForTimeout(500); }
const onBefore=await demoOn(); if(!onBefore) throw new Error('Demo did not turn on');
await pg.evaluate(()=>{window.__c={};for(const id of ['status','status-announce']){const el=document.getElementById(id);if(!el)continue;window.__c[id]=0;new MutationObserver(m=>{window.__c[id]+=m.length}).observe(el,{childList:true,characterData:true,subtree:true});}});
await pg.waitForTimeout(4000); await pg.screenshot({path:`/tmp/uap/${label}-demo.png`});
await pg.waitForTimeout(4000);
const counts=await pg.evaluate(()=>window.__c); const onAfter=await demoOn();
if(!onAfter) throw new Error('Demo turned off during count');
const live=await pg.evaluate(()=>[...document.querySelectorAll('[aria-live],[role=status]')].map(e=>`#${e.id}`));
console.log(JSON.stringify({label,onBefore,onAfter,live,counts,status:await pg.evaluate(()=>document.getElementById('status').textContent)}));
await b.close(); srv.close();
