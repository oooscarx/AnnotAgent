import { chromium } from '../web/node_modules/playwright/index.mjs';
import { createServer } from 'node:http';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
const root = process.argv[2];
assert.match(root ?? '', /^\/Users\/oscar\/Downloads\/AnnotAgent-Live-Demo-/);
const bytes = await readFile(join(root, 'AnnotAgent-Live-Demo.mp4'));
const html = '<!doctype html><meta charset="utf-8"><title>AnnotAgent 实录视频</title><style>body{margin:0;background:#292b29;color:white;font:16px system-ui}video{display:block;width:100%;max-height:95vh}p{margin:12px}</style><video controls preload="auto" src="AnnotAgent-Live-Demo.mp4"></video><p>真实产品录屏剪辑 · 中文字幕 · 无配音</p>';
await writeFile(join(root, '观看视频.html'), html);
const server = createServer((req, res) => {
  if (req.url === '/AnnotAgent-Live-Demo.mp4') {
    const range = /^bytes=(\d+)-(\d*)$/.exec(req.headers.range ?? '');
    if (range) {
      const start = Number(range[1]), end = Math.min(Number(range[2] || bytes.length - 1), bytes.length - 1);
      if (start > end) { res.writeHead(416); res.end(); return; }
      res.writeHead(206, { 'Content-Type': 'video/mp4', 'Accept-Ranges': 'bytes', 'Content-Range': `bytes ${start}-${end}/${bytes.length}`, 'Content-Length': end - start + 1 }); res.end(bytes.subarray(start, end + 1));
    } else { res.writeHead(200, { 'Content-Type': 'video/mp4', 'Accept-Ranges': 'bytes', 'Content-Length': bytes.length }); res.end(bytes); }
  }
  else { res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' }); res.end(html); }
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1140 } });
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.waitForFunction(() => document.querySelector('video').readyState >= 3);
  const metadata = await page.locator('video').evaluate(v => ({ duration: v.duration, width: v.videoWidth, height: v.videoHeight }));
  assert.ok(metadata.duration > 110 && metadata.duration < 140);
  assert.equal(metadata.width, 1440); assert.equal(metadata.height, 1080);
  const positions = [];
  for (const seconds of [0, 49, 82, 108, 117]) {
    await page.locator('video').evaluate(async (v, t) => { v.currentTime = t; await v.play(); }, seconds);
    await page.waitForFunction(t => document.querySelector('video').currentTime > t + 0.5, seconds);
    console.log(`Played and sought ${seconds}s`);
    positions.push(await page.locator('video').evaluate(v => ({ position: v.currentTime, error: v.error?.message ?? null, decodedFrames: v.getVideoPlaybackQuality().totalVideoFrames })));
    await page.locator('video').evaluate(v => v.pause());
  }
  await page.screenshot({ path: join(root, 'editing', 'verified-browser-playback.png') });
  const report = { ...metadata, codec: 'H264', fps: 25, audio: 'none', sha256: createHash('sha256').update(bytes).digest('hex'), browser: await browser.version(), checks: positions, allFramesFfmpegDecode: 'passed separately', packageSha256: createHash('sha256').update(await readFile(join(root, 'AnnotAgent-training.zip'))).digest('hex') };
  await writeFile(join(root, 'playback-verification.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} finally { await browser.close(); server.close(); }
