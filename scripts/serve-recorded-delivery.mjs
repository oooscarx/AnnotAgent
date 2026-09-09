// Read-only viewer for the actual browser-downloaded dataset files. Not product UI.
import {createServer} from 'node:http';
import {readFileSync,statSync,readdirSync,realpathSync} from 'node:fs';
import {resolve,join,sep} from 'node:path';
import assert from 'node:assert/strict';
const root=realpathSync(process.argv[2]);assert.match(root,/\/AnnotAgent-Live-Demo-20260910-[^/]+\/dataset$/);
const escape=s=>s.replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('"','&quot;');
const server=createServer((req,res)=>{
  if(req.method!=='GET'){res.writeHead(405);res.end();return;}
  try{
    const url=new URL(req.url,'http://localhost'),path=realpathSync(resolve(root,'.'+decodeURIComponent(url.pathname)));
    assert.ok(path===root||path.startsWith(root+sep));
    res.setHeader('Cache-Control','no-store');
    if(statSync(path).isDirectory()){
      res.setHeader('Content-Type','text/html; charset=utf-8');
      res.end('<!doctype html><meta charset="utf-8"><title>Downloaded dataset files</title><style>body{font:22px system-ui;margin:56px;line-height:1.8;color:#262a25;background:#f7f8f4}a{color:#365e47}li{margin:10px}</style><h1>下载后解压的真实文件</h1><p>只读文件查看器 · 非 AnnotAgent 产品界面 · 不调用模型</p><ul>'+readdirSync(path).map(name=>'<li><a href="'+escape(url.pathname.replace(/\/$/,'')+'/'+encodeURIComponent(name))+(statSync(join(path,name)).isDirectory()?'/':'')+'">'+escape(name)+'</a></li>').join('')+'</ul>');
    }else if(path.endsWith('.png')||url.searchParams.has('raw')){res.setHeader('Content-Type',path.endsWith('.png')?'image/png':'text/plain; charset=utf-8');res.end(readFileSync(path));}
    else{res.setHeader('Content-Type','text/html; charset=utf-8');res.end('<!doctype html><meta charset="utf-8"><title>'+escape(url.pathname)+'</title><style>body{font:20px system-ui;margin:44px;color:#262a25;background:#f7f8f4}pre{font:24px/1.6 ui-monospace,monospace;white-space:pre-wrap;overflow-wrap:anywhere}a{color:#365e47}</style><p>下载包的真实文件 · 只读查看器（非产品界面）</p><h1>'+escape(url.pathname)+'</h1><a href="?raw=1">原始文件</a><pre>'+escape(readFileSync(path,'utf8'))+'</pre>');}
  }catch{res.writeHead(404);res.end('Not found');}
});
server.listen(0,'127.0.0.1',()=>console.log('Read-only extracted dataset: http://127.0.0.1:'+server.address().port));
