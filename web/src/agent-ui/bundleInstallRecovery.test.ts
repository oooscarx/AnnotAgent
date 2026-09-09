import {it,expect} from "vitest";
import {bundleInstallKey,preserveBundleInstall} from "./bundleInstallRecovery";
const request={catalog_id:"c",bundle_id:"b",bundle_version:"1",plugin_id:"p",plugin_version:"1"};
it("isolates workspace and plugin identities",()=>{
  expect(bundleInstallKey("a","p","1")).not.toBe(bundleInstallKey("b","p","1"));
  expect(bundleInstallKey("a","p","1")).not.toBe(bundleInstallKey("a","p","2"));
});
it("retains the frozen request across reload and refuses a second submission",()=>{
  const values=new Map<string,string>();const store={getItem:(k:string)=>values.get(k)??null,setItem:(k:string,v:string)=>{values.set(k,v);}};
  const key=bundleInstallKey("w","p","1");preserveBundleInstall(store,key,request);
  expect(JSON.parse(store.getItem(key)!)).toEqual(request);
  expect(()=>preserveBundleInstall(store,key,{...request,bundle_id:"different"})).toThrow();
  expect(JSON.parse(store.getItem(key)!)).toEqual(request);
});
it("fails closed if local recovery cannot be persisted",()=>{
  expect(()=>preserveBundleInstall({getItem:()=>null,setItem:()=>{}},"k",request)).toThrow();
});
