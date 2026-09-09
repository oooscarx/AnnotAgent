// Assemble evidence, never generate application data or modify source screenshots.
import {readFileSync,writeFileSync,mkdirSync,copyFileSync,readdirSync} from "node:fs";
import {resolve,join,basename} from "node:path";
import {createHash} from "node:crypto";
const after=resolve(process.argv[2]);
const reference=resolve(process.argv[3]);
const base="http://127.0.0.1:8812";
const output=resolve("public/evidence/visual-finish");mkdirSync(join(output,"reference"),{recursive:true});
const hash=b=>createHash("sha256").update(b).digest("hex");
const refRecords=[];
for(const file of readdirSync(reference).filter(f=>f.endsWith(".png"))){copyFileSync(join(reference,file),join(output,"reference",file));refRecords.push({file,sha256:hash(readFileSync(join(reference,file))),source:"User-supplied first-version design reference; not application or inference evidence",sourceSHA:null,viewport:{width:1440,height:960},dpr:"not supplied",theme:file.includes("dark")?"dark":"light"});}
writeFileSync(join(output,"reference","manifest.json"),JSON.stringify(refRecords,null,2));
const proof=[];
for(const file of ["index.html",...readdirSync("dist/assets").map(f=>`assets/${f}`),"brand/core/ui-icons.svg","brand/core/annotagent-mark-paper.svg"]){
  const response=await fetch(`${base}/${file}`);const actual=Buffer.from(await response.arrayBuffer());const expected=readFileSync(`dist/${file}`);
  if(!response.ok||hash(actual)!==hash(expected))throw Error(`Service build mismatch: ${file}`);
  proof.push({url:`${base}/${file}`,status:response.status,sha256:hash(actual)});
}
writeFileSync(join(output,"build-proof.json"),JSON.stringify({checkedAt:new Date().toISOString(),service:base,backend:"3e9d8f51d89be21851c4920ddeda4ea06d2c7a43",files:proof},null,2));
const before="../agent-ui-integration/3e9d8f51d8-1788957154496";
const afterUrl=`../agent-ui-integration/${basename(after)}`;
const escape=s=>String(s).replaceAll("&","&amp;").replaceAll("<","&lt;").replaceAll('"',"&quot;");
const shot=(path,label,ref=false)=>`<figure><a class="frame ${ref?"reference":""}" href="${path}" target="_blank"><img loading="lazy" src="${path}" alt="${escape(label)}"></a><figcaption>${escape(label)}</figcaption></figure>`;
const rows=[["新任务","01-new-task-light.png","01-new-task.png"],["Plan 与批准","02-plan-light.png","02-plan-scope.png"],["人工修正 / 已保存框","03-human-review-light.png","04-bbox.png"],["深色画布","05-interrupted-dark.png","05-bbox-dark.png"],["模型选择","06-model-picker-light.png","08-model-picker.png"]];
const manifest=JSON.parse(readFileSync(join(after,"manifest.json"),"utf8")).records;
const fixture=JSON.parse(readFileSync(join(output,"fixture-scenes/manifest.json"),"utf8"));
writeFileSync(join(output,"index.html"),`<!doctype html><html lang="zh"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>AnnotAgent · 第一版视觉还原对照</title><style>*{box-sizing:border-box}body{margin:0;padding:32px;font:15px/1.65 system-ui;background:#f7f7f4;color:#20221f}header{max-width:980px}h1{font-size:28px}h2{margin-top:36px}a{color:#355e8c}figure{margin:0;min-width:0}img{width:100%;display:block}.frame{display:block;border:1px solid #daddd5;overflow:hidden}.reference img{margin-top:-2.3611%}.compare{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:16px}.gallery{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:24px}figcaption{padding:10px 0;color:#666a64;font-size:12px}details{margin:12px 0}pre{white-space:pre-wrap;overflow-wrap:anywhere;font-size:11px}@media(max-width:760px){body{padding:16px}.compare,.gallery{grid-template-columns:1fr}}</style><header><h1>第一版视觉还原 · 真实应用对照</h1><p>左：设计参考（仅排版参考，裁去顶部 34px 原型工具栏）；中：改版前集成 UI；右：改版后 HttpAdapter 应用。内容和图片不同，比较图标、间距、层级和画布，不比较检测准确率。</p><p><a href="${base}/projects">打开真实 HTTP TEST 应用</a> · <a href="http://127.0.0.1:5178/ui-preview?icons=1">交互 IconGallery（Fixture）</a> · <a href="build-proof.json">服务构建哈希核验</a></p><p>Main 仍因未提交用户文件阻塞，未合并、未 push。TEST 使用真实 HTTP / SQLite 与外部测试模型；不代表 Live 模型质量。原生 200% 尚未验证，CSS 200% 图标截图已明确区分。</p></header>${rows.map(([title,ref,file])=>`<h2>${title}</h2><div class="compare">${shot(`reference/${ref}`,"第一版参考",true)}${shot(`${before}/${file}`,"Before · 3e9d8f5 / TEST HTTP")}${shot(`${afterUrl}/${file}`,"After · 真实 TEST HTTP")}</div>`).join("")}<h2>完整真实 HTTP 页面与来源</h2><p><a href="${afterUrl}/manifest.json">逐图 SHA / URL / viewport / DPR / theme</a></p><div class="gallery">${manifest.map(r=>shot(`${afterUrl}/${r.file}`,r.title)).join("")}</div><h2>固定场景视觉与 IconGallery（Fixture，不是后端结果）</h2><p>包括审批、执行/排队、停止中、人工协助、六个 Settings 与手机；图库内的执行状态只用于视觉参考，不计入真实 HTTP 功能验收。</p><p><a href="fixture-scenes/manifest.json">Fixture 逐图来源</a> · <a href="reference/manifest.json">原始参考图来源</a></p><div class="gallery">${fixture.map(r=>shot(`fixture-scenes/${r.file}`,`${r.file} · ${r.theme} · CSS zoom ${r.cssZoom}`)).join("")}</div></html>`);
console.log(`${base}/evidence/visual-finish/index.html`);
