import {afterEach, describe, expect, it, vi} from "vitest";
import type {APIRequestContext, APIResponse} from "@playwright/test";
import {protectedRequestContext} from "../e2e/fixtures";

// TEST transport only: fake time and in-memory responses, no HTTP or credentials.
function response(status:number,body:unknown={}):APIResponse{
  return {status:()=>status,ok:()=>status>=200&&status<300,json:async()=>body,text:async()=>JSON.stringify(body)} as APIResponse;
}
const limited=()=>response(429,{code:"mutation_rate_limited"});
const path="/api/providers/TEST-provider/active-probe";
function transport(send:(headers:Record<string,string>)=>Promise<APIResponse>,confirm?:()=>Promise<APIResponse>){
  let sequence=0;
  const request={
    get:vi.fn(async()=>response(200,{csrf_token:"TEST-csrf"})),
    post:vi.fn(async()=>confirm?confirm():response(200,{confirmation_token:`TEST-nonce-${++sequence}`})),
    fetch:vi.fn(async(_path:string,options:{headers:Record<string,string>})=>send({...options.headers})),
  };
  return {request,protected:protectedRequestContext(request as unknown as APIRequestContext)};
}
afterEach(()=>{vi.useRealTimers();});

describe("isolated API mutation pacing",()=>{
  it("renews a privileged nonce only after the exact pre-execution 429",async()=>{
    vi.useFakeTimers();
    const sent:string[]=[];
    const fixture=transport(async headers=>{
      sent.push(headers["x-annotagent-privileged-confirmation"]);
      return sent.length<3?limited():response(200);
    });
    const pending=fixture.protected.post(path,{data:{confirmed_billable:true}});
    await vi.advanceTimersByTimeAsync(2_000);
    expect((await pending).status()).toBe(200);
    expect(sent).toEqual(["TEST-nonce-1","TEST-nonce-2","TEST-nonce-3"]);
    expect(fixture.request.post).toHaveBeenCalledTimes(3);
    expect(fixture.request.post).toHaveBeenLastCalledWith("/api/session/privileged-confirmation",{
      headers:{"x-annotagent-csrf":"TEST-csrf"},data:{action:`POST ${path}`,confirmed:true},
    });
  });

  it("waits through a 31-second rate window without sending an expired nonce",async()=>{
    vi.useFakeTimers();vi.setSystemTime(0);
    const issued=new Map<string,number>();let confirmations=0;
    const sent:Array<{time:number;token:string}>=[];
    const fixture=transport(async headers=>{
      const token=headers["x-annotagent-privileged-confirmation"];
      sent.push({time:Date.now(),token});
      if(Date.now()<31_000)return limited();
      return response(Date.now()-(issued.get(token)??-Infinity)<30_000?200:403);
    },async()=>{
      if(confirmations>0&&Date.now()<31_000)return limited();
      const token=`TEST-nonce-${++confirmations}`;issued.set(token,Date.now());
      return response(200,{confirmation_token:token});
    });
    const pending=fixture.protected.post(path,{data:{confirmed_billable:true}});
    await vi.advanceTimersByTimeAsync(31_000);
    expect((await pending).status()).toBe(200);
    expect(sent).toEqual([{time:0,token:"TEST-nonce-1"},{time:31_000,token:"TEST-nonce-2"}]);
  });

  for(const [status,body] of [
    [403,{code:"privileged_confirmation_required"}],
    [502,{error:"TEST Provider outcome unknown"}],
    [429,{code:"expensive_action_concurrency_limited"}],
    [429,{}],
  ] as const){
    it(`does not retry ${status} ${JSON.stringify(body)}`,async()=>{
      vi.useFakeTimers();
      const fixture=transport(async()=>response(status,body));
      expect((await fixture.protected.post(path)).status()).toBe(status);
      expect(fixture.request.fetch).toHaveBeenCalledTimes(1);
      expect(fixture.request.post).toHaveBeenCalledTimes(1);
      expect(vi.getTimerCount()).toBe(0);
    });
  }

  it("does not retry an unknown transport failure",async()=>{
    vi.useFakeTimers();
    const fixture=transport(async()=>{throw new Error("TEST lost acknowledgement");});
    await expect(fixture.protected.post(path)).rejects.toThrow("TEST lost acknowledgement");
    expect(fixture.request.fetch).toHaveBeenCalledTimes(1);
    expect(fixture.request.post).toHaveBeenCalledTimes(1);
  });

  it("stops if the requested nonce renewal is refused",async()=>{
    vi.useFakeTimers();let confirmations=0;
    const fixture=transport(async()=>limited(),async()=>++confirmations===1?response(200,{confirmation_token:"TEST-initial"}):response(403,{code:"TEST-confirmation-refused"}));
    const pending=fixture.protected.post(path);
    await vi.advanceTimersByTimeAsync(1_000);
    expect((await pending).status()).toBe(403);
    expect(fixture.request.fetch).toHaveBeenCalledTimes(1);
    expect(fixture.request.post).toHaveBeenCalledTimes(2);
  });

  it("shares the 65-second deadline with nonce renewal instead of extending it",async()=>{
    vi.useFakeTimers();vi.setSystemTime(0);let confirmations=0;
    const fixture=transport(async()=>limited(),async()=>++confirmations===1?response(200,{confirmation_token:"TEST-initial"}):limited());
    const pending=fixture.protected.post(path);
    await vi.advanceTimersByTimeAsync(65_000);
    expect((await pending).status()).toBe(429);
    expect(fixture.request.fetch).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("paces ordinary mutations without issuing a privileged nonce",async()=>{
    vi.useFakeTimers();let calls=0;
    const fixture=transport(async()=>++calls===1?limited():response(200));
    const pending=fixture.protected.post("/api/projects/TEST/conversations/TEST/messages");
    await vi.advanceTimersByTimeAsync(1_000);
    expect((await pending).status()).toBe(200);
    expect(fixture.request.post).not.toHaveBeenCalled();expect(fixture.request.fetch).toHaveBeenCalledTimes(2);
  });
});
