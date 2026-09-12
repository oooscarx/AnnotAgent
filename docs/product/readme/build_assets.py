from pathlib import Path
from html import escape
import re
P=Path(__file__).parent
ROOT=P.parents[2]
mark=(ROOT/'web/public/brand/core/annotagent-mark-ink.svg').read_text()
body=re.sub(r'^.*?<title>.*?</title>','',mark).removesuffix('</svg>')
font='-apple-system, BlinkMacSystemFont, Segoe UI, PingFang SC, Microsoft YaHei, sans-serif'
for theme,bg,fg,muted,accent in [('light','#F7F7F4','#20221F','#666A64','#355E8C'),('dark','#181A18','#EDEFE8','#A5ABA0','#A3BEDC')]:
 def start(w,h):return [f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}" fill="{bg}"/><g font-family="{font}" fill="{fg}">']
 def txt(s,x,y,text,size=28,color=None):s.append(f'<text x="{x}" y="{y}" font-size="{size}" fill="{color or fg}">{escape(text)}</text>')
 s=start(1600,300);s.append(f'<g transform="translate(80 65) scale(1.2)" color="{fg}">{body}</g>');txt(s,280,132,'AnnotAgent',58);txt(s,282,197,'从原始图片到训练数据包的视觉数据 Agent',32,muted);s.append('</g></svg>');(P/f'brand-{theme}.svg').write_text(''.join(s))
 s=start(1200,660);txt(s,40,60,'从任务目标到数据交付',32);txt(s,40,104,'概念分工：实际步骤由任务与可用模型决定',22,muted)
 blocks=[(40,170,260,110,'任务目标','图片 · 标注目标 · 训练用途'),(380,170,260,110,'LLM 规划','选择模型与处理步骤'),(720,170,420,110,'Rust 执行','检查数据契约、来源与执行状态'),(720,360,420,100,'视觉模型与工具','检测 · 裁剪 · 分类 · 分割'),(380,500,260,100,'人工修订','样例反馈 · 正式审核'),(720,500,420,100,'确定性导出','标注文件 · 支持的训练数据包')]
 for x,y,w,h,a,b in blocks:
  s.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="none" stroke="{muted}"/>');txt(s,x+20,y+40,a,27);txt(s,x+20,y+77,b,18,muted)
 s.append(f'<defs><marker id="a" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M0 0L8 4L0 8" fill="{accent}"/></marker></defs>')
 for d in ['M300 225H374','M640 225H714','M930 280V354','M720 250H680V550H646','M640 550H714','M1080 460V494']:
  s.append(f'<path d="{d}" fill="none" stroke="{accent}" stroke-width="2" marker-end="url(#a)"/>')
 s.append('</g></svg>');(P/f'flow-{theme}.svg').write_text(''.join(s))

 # Narrow-screen version preserves readable labels without cropping the product screenshot.
 s=start(560,1120);txt(s,30,55,'职责示意',32);txt(s,30,97,'按任务与可用模型组合执行',24,muted)
 mobile=[('任务目标','图片范围 · 标注目标 · 训练用途'),('LLM 规划','选择模型与处理步骤'),('Rust 执行','验证契约、来源与状态'),('视觉模型与工具','检测 · 裁剪 · 分类 · 分割'),('人工修订','样例反馈与正式审核'),('确定性导出','标注文件与支持的训练包')]
 for i,(a,b) in enumerate(mobile):
  y=140+i*155
  s.append(f'<rect x="30" y="{y}" width="500" height="120" fill="none" stroke="{muted}"/>');txt(s,55,y+45,a,32);txt(s,55,y+87,b,25,muted)
  if i<5:s.append(f'<path d="M280 {y+120}V{y+150}m-6-7 6 7 6-7" fill="none" stroke="{accent}" stroke-width="2"/>')
 s.append('</g></svg>');(P/f'flow-mobile-{theme}.svg').write_text(''.join(s))
