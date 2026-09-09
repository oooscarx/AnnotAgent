import {Disclosure} from "./Disclosure";
function record(value:unknown):Record<string,unknown>{return value!==null&&typeof value==="object"&&!Array.isArray(value)?value as Record<string,unknown>:{};}
function scalar(value:unknown){return typeof value==="string"||typeof value==="number"?String(value):"";}
export function TraceRecord({value,index}:{value:unknown;index:number}){
  const raw=record(value);const operation=Object.keys(record(raw.operation)).length?record(raw.operation):raw;const session=record(raw.session);
  const title=scalar(operation.tool_name||operation.title||operation.kind||operation.remote_model_id)||`记录 ${index+1}`;
  const status=scalar(operation.status||operation.state||session.status);const time=scalar(operation.created_at||operation.started_at||session.created_at);
  const steps=Array.isArray(session.steps)?session.steps:[];
  const message=record(raw.input);const userText=scalar(message.text);const modelCalls=Array.isArray(session.model_calls)?session.model_calls:[];
  return <article className="task-trace-record"><header><strong>{title}</strong>{status&&<span>{status}</span>}{time&&<time>{time}</time>}</header>
    {!!(operation.error||session.stop_reason||session.builder_stop_reason)&&<p>{scalar(operation.error||session.stop_reason||session.builder_stop_reason)}</p>}
    {scalar(session.phase)&&<p>构建阶段：{scalar(session.phase)}</p>}
    {userText&&<p style={{whiteSpace:"pre-wrap"}}>{userText}</p>}
    {modelCalls.length>0&&<Disclosure title={`实际模型调用 · ${modelCalls.length}`}>{modelCalls.map((v,i)=>{const call=record(v);return <p key={i}>{scalar(call.remote_model_id)} · {call.succeeded===true?"成功":call.succeeded===false?"失败":"状态未记录"} · {scalar(call.duration_ms)||"未知"} ms {scalar(call.safe_error)}</p>;})}</Disclosure>}
    {steps.length>0&&<ol aria-label="实际工具执行步骤">{steps.map((value,i)=>{const step=record(value);return <li key={scalar(step.call_id)||i}><strong>{scalar(step.tool_name)||"工具调用"}</strong><span> · {step.success===true?"成功":step.success===false?"失败":"状态未记录"}</span><Disclosure title="输入与结果"><h4>输入</h4><pre>{JSON.stringify(step.arguments,null,2)}</pre><h4>结果</h4><pre>{JSON.stringify(step.result,null,2)}</pre></Disclosure></li>;})}</ol>}
    <Disclosure title="原始记录与引用"><pre>{JSON.stringify(value,null,2)}</pre></Disclosure>
  </article>;
}
