import { createRequire } from 'node:module';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const req=createRequire(import.meta.url);
const {chromium}=req(resolve('web/node_modules/@playwright/test'));
const {marked}=await import(pathToFileURL(resolve(process.env.ANNOTAGENT_DOCS_NODE_MODULES || 'web/node_modules', 'marked/lib/marked.esm.js')));
const root=resolve('.'),out=resolve('docs/product/readme');
const locale=process.env.ANNOTAGENT_DOCS_LOCALE==='en'?'en':'zh';
const suffix=locale==='en'?'-en':'';
const body=marked.parse(readFileSync(locale==='en'?'README.en.md':'README.md','utf8'));
writeFileSync(out+'/preview'+suffix+'.html',`<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><base href="../../../"><title>README 本地近似预览</title><style>body{margin:0;background:#fff;color:#20221f;font:16px/1.65 -apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC",sans-serif}main{max-width:980px;margin:30px auto;padding:32px}h1,h2{line-height:1.35;border-bottom:1px solid #daddd5;padding-bottom:10px}h1{font-size:32px}h2{font-size:24px;margin-top:32px}img{max-width:100%;height:auto}a{color:#355e8c}pre{background:#f0f1ed;overflow:auto;padding:16px}code{font:14px/1.5 ui-monospace,monospace}blockquote{margin:20px 0;padding-left:16px;border-left:3px solid #daddd5;color:#666a64}table{border-collapse:collapse;width:100%;font-size:15px}td,th{border:1px solid #daddd5;padding:10px;text-align:left}li{margin:10px 0}@media(prefers-color-scheme:dark){body{background:#181a18;color:#edefe8}a{color:#a3bedc}pre{background:#202320}blockquote{color:#a5aba0}td,th{border-color:#484d44}}@media(max-width:500px){main{padding:18px;margin:0}table{font-size:14px}td,th{padding:7px}h1{font-size:28px}}</style><main>${body}</main></html>`);
const b=await chromium.launch();const results=[];
for(const [theme,width]of [['light',1280],['dark',1280],['light',390],['dark',390]]){
 const p=await b.newPage({viewport:{width,height:900},colorScheme:theme});await p.goto(pathToFileURL(out+'/preview'+suffix+'.html').href);await p.screenshot({path:`${out}/preview${suffix}-${theme}-${width}.png`,fullPage:true});results.push({theme,width,...await p.evaluate(()=>({scrollWidth:document.documentElement.scrollWidth,clientWidth:document.documentElement.clientWidth,brokenImages:[...document.images].filter(i=>!i.complete||!i.naturalWidth).map(i=>i.src)}))});await p.close();
}
for(const name of ['brand-light','brand-dark','flow-light','flow-dark']){const p=await b.newPage({viewport:{width:name.startsWith('brand')?1600:1200,height:name.startsWith('brand')?300:660}});await p.goto(pathToFileURL(`${out}/${name}.svg`).href);await p.screenshot({path:`${out}/${name}.png`});await p.close();}
await b.close();writeFileSync(out+'/preview'+suffix+'-checks.json',JSON.stringify(results,null,2));console.log(results);
