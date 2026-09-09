import {renderToStaticMarkup} from "react-dom/server";
import {expect,it} from "vitest";
import {ImageBrowser} from "./ImageBrowser";
const assets=Array.from({length:60},(_,id)=>({id,name:`image ${id}`,src:`/original/${id}`,thumbnail:`/thumb/${id}`,width:100,height:100}));
it("bounds native thumbnails to the selected page without replacing original resources",()=>{
  const html=renderToStaticMarkup(<ImageBrowser assets={assets} image={30} onSelect={()=>{}}/>);
  expect(html.match(/<img /g)).toHaveLength(24);expect(html).toContain('src="/thumb/30"');expect(html).not.toContain('src="/thumb/0"');expect(html).not.toContain('/original/');expect(assets[30].src).toBe('/original/30');
});
it("falls back to the original when the server has no thumbnail",()=>{
  const html=renderToStaticMarkup(<ImageBrowser assets={[{...assets[0],thumbnail:undefined}]} image={0} onSelect={()=>{}}/>);expect(html).toContain('src="/original/0"');expect(html).not.toContain("下一页图片");
});
