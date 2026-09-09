// Editorial assembly of recorded production UI; never calls AnnotAgent or a Provider.
import { chromium } from '../web/node_modules/playwright/index.mjs';
import { mkdir, readdir, writeFile, copyFile, readFile } from 'node:fs/promises';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const [workspace, output] = process.argv.slice(2);
if (!workspace?.startsWith('/tmp/AnnotAgent-LIVE-recording-') || !output?.startsWith('/Users/oscar/Downloads/AnnotAgent-Live-Demo-')) throw Error('Explicit owned recording directories required');
const work = path.join(output, 'editing');
await mkdir(work, { recursive: true });
await mkdir(path.join(output, 'raw'), { recursive: true });
const groups = {};
for (const [key, folder] of Object.entries({ opening: 'record-opening-2', processing: 'record-processing', delivery: 'record-delivery', evidence: 'record-evidence-verified', files: 'record-files' })) {
  const root = path.join(workspace, folder);
  const dirs = (await readdir(root, { withFileTypes: true })).filter(e => e.isDirectory());
  if (dirs.length !== 1) throw Error(`Ambiguous recording: ${root}`);
  groups[key] = path.join(root, dirs[0].name);
  await copyFile(path.join(groups[key], 'video.webm'), path.join(output, 'raw', `${key}.webm`));
}
const shots = [];
const clip = (group, from, to, speed, title, detail) => shots.push({ source: path.join(groups[group], 'video.webm'), from, to, speed, duration: (to - from) / speed, title, detail, kind: 'recording' });
const hold = (group, file, duration, title, detail) => shots.push({ source: path.join(groups[group], file), duration, title, detail, kind: 'recorded-frame' });
hold('opening', '01-empty-project.png', 4, 'AnnotAgent · 从原图到训练数据包', '真实应用与付费模型实录｜2 张 B-Human 原图｜字幕版，无配音');
clip('opening', 0.3, 6.3, 0.75, '01  上传原图，说明标注目标', '足球 + 机器人；原图没有预画标注。操作慢放 0.75×。');
hold('opening', '04-persisted-delivery-goal.png', 6, '02  保存类别与 YOLO 交付目标', '目标、标签、图片范围和训练/验证划分保存到当前任务。画面停留。');
hold('opening', '05-real-model-authorization.png', 6, '03  明确批准本次模型与样例范围', '真实 GLM 规划 + Qwen 视觉调用 + 本地 EfficientSAM；未知费用不当作免费。');
clip('opening', 9.5, 56.3, 4, '04  Agent 构造方案并试跑样例', '真实模型等待 4× 加速；没有替换响应、伪造进度或预录结果回填。');
hold('processing', '09-actual-plan-models.png', 6, '实际方案：寻找候选，再进行局部几何精修', '从真实 Draft 查看节点与模型；SAM 负责几何，不代替目标语义判断。');
clip('evidence', 0.5, 9.5, 1, '05  回看真实 VLM 候选与 SAM Mask', '已保存执行证据回看，不重复调用模型；同一张原图、可追溯 Artifact。');
hold('processing', '08-named-real-sample.png', 4, '06  样例结果仍然需要检查', '模型组合不代表自动准确；样例修正属于评估，不是正式数据集接受。');
clip('processing', 2.8, 6.2, 0.5, '07  在原任务中修正边界并提交', '真实表单与画布操作慢放 0.5×；刷新后读取已保存的人工请求处理结果。');
hold('processing', '12-formal-range-confirmation.png', 5, '08  单独确认正式处理范围', '固定已测试方案与实际图片；发布、处理和正式标注接受是不同边界。');
clip('processing', 6.4, 21.4, 4, '开始正式处理两张原图', '真实等待 4× 加速；离开或刷新不会再次触发收费请求。');
clip('delivery', 0.3, 3.45, 0.6, '09  逐对象检查，再确认整张图片', '接受一个框不等于确认全图无遗漏；操作慢放 0.6×。');
clip('delivery', 3.45, 10.1, 0.4, '修正机器人边界，并补上两个漏检目标', '真实人工操作慢放 0.4×；缺陷直接呈现，新增框实际保存，不伪装为模型输出。');
hold('delivery', 'color_548575.png', 4, '人工补漏与确认完成', '视频由实现者操作，并非真人新手测试；未测量 precision / recall。');
clip('delivery', 10.1, 15.5, 0.65, '10  生成并下载真实训练 ZIP', '2 张已确认图片、5 个对象；train / val 各 1 张。打包与下载操作慢放。');
clip('files', 0.4, 10.4, 1, '11  解压后查看 data.yaml 与校验报告', '下载包的只读文件查看器（不是产品界面）；所见内容与 ZIP 内文件逐字核验。');
hold('delivery', 'package-ready.png', 6, '交付完成：视频 + 原始录像 + 训练数据包', '两处独立解压及结构校验通过；未进行正式训练、官方加载器测试或准确率基准。');

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 120 }, deviceScaleFactor: 1 });
const esc = s => s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
function run(args) {
  const p = spawnSync('/opt/homebrew/bin/ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', ...args], { encoding: 'utf8', maxBuffer: 4e6 });
  if (p.status !== 0) throw Error(p.stderr || `ffmpeg failed ${p.status}`);
}
let elapsed = 0;
for (let i = 0; i < shots.length; i++) {
  const s = shots[i], id = String(i + 1).padStart(2, '0');
  const caption = path.join(work, `${id}-caption.png`), part = path.join(work, `${id}.mp4`);
  await page.setContent(`<style>*{box-sizing:border-box}body{margin:0;background:#f5f3ee;color:#292b29;font-family:Arial,'PingFang SC','Microsoft YaHei',sans-serif;padding:17px 32px;border-top:1px solid #ccc9c1}h1{font-size:29px;line-height:38px;margin:0 0 6px;font-weight:600}p{font-size:21px;line-height:28px;margin:0;color:#555952}aside{float:right;font-size:17px;color:#666;margin-top:10px}</style><aside>实录 · ${id}/${shots.length}</aside><h1>${esc(s.title)}</h1><p>${esc(s.detail)}</p>`);
  await page.screenshot({ path: caption });
  const inputs = s.kind === 'recorded-frame' ? ['-loop', '1', '-framerate', '25', '-i', s.source] : ['-ss', String(s.from), '-t', String(s.to - s.from), '-i', s.source];
  run([...inputs, '-loop', '1', '-framerate', '25', '-i', caption,
    '-filter_complex', `[0:v]setpts=(PTS-STARTPTS)/${s.speed ?? 1},fps=25,setsar=1,pad=1440:1080:0:0:color=0xf5f3ee[base];[base][1:v]overlay=0:960:shortest=1,format=yuv420p[v]`,
    '-map', '[v]', '-an', '-t', String(s.duration), '-c:v', 'libx264', '-preset', 'veryfast', '-crf', '19', '-r', '25', part]);
  s.start_seconds = elapsed; elapsed += s.duration;
  s.sha256 = createHash('sha256').update(await readFile(s.source)).digest('hex');
  console.log(`${id}/${shots.length}: ${s.title}`);
}
await browser.close();
const list = path.join(work, 'concat.txt');
await writeFile(list, shots.map((_, i) => `file '${path.join(work, `${String(i + 1).padStart(2, '0')}.mp4`)}'`).join('\n'));
const movie = path.join(output, 'AnnotAgent-Live-Demo.mp4');
run(['-f', 'concat', '-safe', '0', '-i', list, '-c', 'copy', '-movflags', '+faststart', movie]);
await writeFile(path.join(output, 'recording-manifest.json'), JSON.stringify({ source_ui: { opening: '2a82bc0', later: '7663a76' }, project: 'live-robocup-video', task: '545aa2dd-54a8-40a8-9e65-d0acdaf7501f', kind: 'LIVE production HTTP UI, editorial captions outside original viewport', viewport: [1440, 960], movie: [1440, 1080], fps: 25, audio: 'none', duration_seconds: elapsed, shots }, null, 2));
console.log(movie);
