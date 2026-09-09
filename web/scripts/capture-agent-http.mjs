// Actual application screenshots. Never uses the prototype or Fixture Adapter.
import { chromium } from "playwright";
import { readFileSync, writeFileSync, mkdirSync, readdirSync, statSync, copyFileSync } from "node:fs";
import { dirname, resolve, join } from "node:path";
import { execFileSync } from "node:child_process";
const manifestPath=resolve(process.argv[2] || "missing-test-manifest");
const manifest=JSON.parse(readFileSync(manifestPath,"utf8"));
if(readFileSync(join(dirname(manifestPath),"FIXTURE_ONLY"),"utf8")!=="AnnotAgent HTTP integration fixture\n")throw Error("Not a marked test workspace");
const base=new URL(manifest.base_url);
if(base.hostname!=="127.0.0.1" || base.port==="8787")throw Error("Only isolated loopback test services may be captured");
const sha=execFileSync("git",["rev-parse","HEAD"],{encoding:"utf8"}).trim();
const id=`${sha.slice(0,10)}-${Date.now()}`;
const directory=resolve("public/evidence/agent-ui-integration",id);
mkdirSync(directory,{recursive:true});
const browser=await chromium.launch();
const context=await browser.newContext({viewport:{width:1440,height:960}});
const health=await context.request.get(`${base.origin}/api/health`);
if(health.headers()["x-annotagent-fixture"]!=="external-model-only")throw Error("Not a TEST HTTP backend");
const page=await context.newPage();
const records=[];
const work=(project,task,extra="")=>`${base.origin}/projects/${encodeURIComponent(project)}/work?task=${encodeURIComponent(task)}${extra}`;
async function open(url) {
  await page.goto(url);
  await page.locator('.ui-app[data-adapter="http"] .preview-chip').filter({hasText:"TEST"}).waitFor();
  if(!url.includes("settings=") && !url.includes("task=new"))await page.locator(".user-message").first().waitFor();
}
async function shot(name,title,note="") {
  await page.locator("img").evaluateAll(images=>Promise.all(images.map(image=>image.decode().catch(()=>{}))));
  const file=`${name}.png`;
  await page.screenshot({path:join(directory,file),animations:"disabled"});
  records.push({file,title,note,sha,url:page.url(),viewport:page.viewportSize(),theme:await page.locator("html").getAttribute("data-aa-theme"),state:"TEST / real HTTP+SQLite; external model fixture; not Live accuracy",capturedAt:new Date().toISOString()});
}
try {
  await open(work(manifest.project,`new:${manifest.project}`));
  await shot("01-new-task","新任务与项目树");
  await open(work(manifest.project,manifest.plan_task_id));
  await page.getByRole("button",{name:"查看规划授权",exact:true}).click();
  await page.getByRole("button",{name:"查看并确认授权",exact:true}).waitFor();
  await shot("02-plan-scope","真实规划授权范围","Only GET preview; no approval or new model call during capture.");
  await open(work(manifest.project,manifest.task_id,"&pane=image"));
  await page.locator("svg image").waitFor();
  await shot("03-sample","样例分类与真实图片","Synthetic TEST pixels, not a model accuracy demonstration.");
  await open(work(manifest.bbox.project,manifest.bbox.task_id,`&pane=image&image=${manifest.bbox.image_id}`));
  await page.locator("svg rect").first().waitFor();
  await shot("04-bbox","已保存的框修正","Sandbox feedback, not accepted dataset annotations.");
  await page.emulateMedia({colorScheme:"dark"});
  await shot("05-bbox-dark","深色画布");
  await page.emulateMedia({colorScheme:"light"});
  const interrupted=manifest.controls.interrupted;
  await open(work(interrupted.project,interrupted.task_id));
  await page.getByText("停止回执已确认",{exact:true}).waitFor();
  await shot("06-interrupted","已中断，不虚构继续能力");
  const scenes=readdirSync(dirname(manifestPath)).filter(file=>/^UIAPI-003_STOP_.*\.json$/.test(file)).map(file=>join(dirname(manifestPath),file)).sort((a,b)=>statSync(b).mtimeMs-statSync(a).mtimeMs);
  if(scenes.length) {
    const scene=JSON.parse(readFileSync(scenes[0],"utf8")).scene;
    await open(work(manifest.project,scene.task_id));
    await page.getByText(/远端结果未知/).waitFor();
    await shot("07-unknown","停止后的未知结果","The initial stopping state is evidenced by the real HTTP response; it is not artificially held for a screenshot.");
  }
  await open(work(manifest.project,`new:${manifest.project}`));
  await page.getByRole("button",{name:/模型⌄/}).click();
  await shot("08-model-picker","真实 Registry 模型选择");
  for(const [key,title] of [["general","通用"],["providers","Providers"],["agent","Agent 模型"],["vision","视觉模型与插件"],["privacy","数据与隐私"],["usage","用量与预算"]]) {
    await open(work(manifest.project,manifest.task_id,`&settings=${key}`));
    await page.locator(".settings-content h1").waitFor();
    await shot(`settings-${key}`,title);
  }
  await page.setViewportSize({width:390,height:844});
  await open(work(manifest.bbox.project,manifest.bbox.task_id,`&pane=image&image=${manifest.bbox.image_id}`));
  await page.locator("svg image").waitFor();
  await shot("09-mobile","手机查看结果","390×844 viewport, not native 200% browser zoom.");
  const runShot=resolve("test-results/agent-ui-http-d-actual-sto-b7d0c-unknown-without-fake-resume/running-queue.png");
  try {
    const stat=statSync(runShot);
    if(stat.mtimeMs>=Date.now()-30*60*1000) {
      copyFileSync(runShot,join(directory,"running-queue.png"));
      records.push({file:"running-queue.png",title:"真实执行与一条排队输入",sha,viewport:{width:1440,height:960},theme:"light",state:"TEST / real HTTP E2E",note:"Captured by the HTTP browser test while the server call was reserved. See its real-test-scene and initial-stop-receipt attachments."});
    }
  } catch { /* Missing actual evidence is never substituted by a fabricated frame. */ }
  const escape=s=>String(s).replaceAll("&","&amp;").replaceAll("<","&lt;").replaceAll('"',"&quot;");
  writeFileSync(join(directory,"manifest.json"),JSON.stringify({sha,backendSha:manifest.backend_sha,approvedUI:"f0bbbc692904aef9f89392bd4cbb1ba3cd79172a",records},null,2));
  writeFileSync(join(directory,"index.html"),`<!doctype html><html lang="zh"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Agent UI HTTP 验收</title><style>body{font:16px system-ui,sans-serif;background:#f5f5f1;color:#232723;max-width:1440px;margin:32px auto;padding:0 24px}img{width:100%;height:auto;border:1px solid #dedfd7}section{margin:40px 0}small,p{overflow-wrap:anywhere}a{color:inherit}</style><h1>Agent UI · HTTP 集成实拍</h1><p>真实 React + HTTP + SQLite；隔离 TEST 数据和测试模型，不是 Live 准确率证明，不是原型 HTML。批准 UI 保持项目树、Agent、Composer 与按需画布。</p><p>UI source: ${sha}</p><a href="${base.origin}/projects">打开隔离应用</a>${records.map(r=>`<section><h2>${escape(r.title)}</h2><p>${escape(r.note||"")}</p><small>${escape(r.url||"HTTP E2E scene attachment")} · ${r.viewport.width}×${r.viewport.height} · ${escape(r.theme)} · ${escape(r.state)}</small><img loading="lazy" alt="${escape(r.title)}" src="${r.file}"></section>`).join("")}</html>`);
  console.log(`${base.origin}/evidence/agent-ui-integration/${id}/index.html`);
  console.log(directory);
} finally { await browser.close(); }
