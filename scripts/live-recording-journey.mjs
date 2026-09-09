// Explicitly authorized LIVE rehearsal through normal Rust HTTP services.
// Never synthesizes model output, edits SQL annotations or retries paid POSTs.
import {readFileSync,writeFileSync,existsSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {randomUUID as uuid} from 'node:crypto';
import assert from 'node:assert/strict';
const [workspace,stage]=process.argv.slice(2);
assert.match(workspace,/^\/tmp\/AnnotAgent-LIVE-recording-/);
assert.equal(JSON.parse(readFileSync(join(workspace,'LIVE_RECORDING.json'))).synthetic_model,false);
const base='http://127.0.0.1:62883', record=join(workspace,'journey.json');
let state=existsSync(record)?JSON.parse(readFileSync(record)): {base,live:true,trace:[]};
let cookie='',csrf='';
async function request(method,path,data){
  const headers={cookie};if(method!=='GET')headers['x-annotagent-csrf']=csrf;
  if(data)headers['content-type']=Buffer.isBuffer(data)?'image/png':'application/json';
  const response=await fetch(base+path,{method,headers,body:data?(Buffer.isBuffer(data)?data:JSON.stringify(data)):undefined,signal:AbortSignal.timeout(1_200_000)});
  if(response.headers.getSetCookie().length)cookie=response.headers.getSetCookie().map(s=>s.split(';')[0]).join('; ');
  const text=await response.text();let value;try{value=JSON.parse(text);}catch{value={body:text.slice(0,500)};}
  if(path!=='/api/session'){state.trace.push({at:new Date().toISOString(),method,path,status:response.status,response:value});save();}
  assert.ok(response.ok,`${method} ${path}: ${response.status} ${JSON.stringify(value).slice(0,1600)}`);return value;
}
function save(){writeFileSync(record,JSON.stringify(state,null,2)+'\n',{mode:0o600});}
csrf=(await request('GET','/api/session')).csrf_token;
const get=p=>request('GET',p),post=(p,b)=>request('POST',p,b);
if(stage==='video-project'){
  assert.ok(!state.video_project,'Do not duplicate the recording project');
  state.video_project='live-robocup-video';save();
  const p='/api/projects/'+state.video_project;
  await post('/api/projects',{id:state.video_project,yaml:'version: 1\nproject:\n  name: 足球与机器人 · 实录\ndataset:\n  root: images\nruntime:\n  max_parallel_images: 1\ntasks: []\nreview:\n  auto_accept_confidence: 0.99\n  force_review_below: 0.95\nexport:\n  formats: [native]\n'});
  await request('PUT',p+'/model-bindings',{bindings:[{capability:'vision_language',role:'detection',match_kind:'capability',model_profile_id:state.vision,locked:false}]});
  const cr=p+'/conversations/'+(await post(p+'/conversations')).conversation_id;
  const preference=await get(cr+'/agent-model');await post(cr+'/agent-model',{request_id:uuid(),expected_revision:preference.revision,model_profile_id:state.planner});
  console.log('Empty recording Project configured. No image upload, model call, task or annotation created.');
}else if(stage==='prepare'){
  assert.ok(!state.project,'Existing recording project: inspect it instead of creating duplicates.');
  state.project='live-robocup-delivery';save();const p='/api/projects/'+state.project;
  await post('/api/projects',{id:state.project,yaml:'version: 1\nproject:\n  name: 足球与机器人 · 训练数据交付\ndataset:\n  root: images\nruntime:\n  max_parallel_images: 1\ntasks: []\nreview:\n  auto_accept_confidence: 0.99\n  force_review_below: 0.95\nexport:\n  formats: [native]\n'});
  state.planner='982f93e2-a9a3-4337-a2f0-cb6b0d5f60b9';state.vision='b9c5bbe8-e21a-5784-9c52-cade259b434f';save();
  await request('PUT',p+'/model-bindings',{bindings:[{capability:'vision_language',role:'detection',match_kind:'capability',model_profile_id:state.vision,locked:false}]});
  const source='/Users/oscar/Documents/my_workspace/AnnotAgent/workspace/robocup-ball/images';
  for(const name of ['color_1001525.png','color_759909.png','color_548575.png','color_289771.png','20260705_11v11_FirstHalf_Bert/color_2271701.png'])await request('POST',p+'/image-upload?name='+encodeURIComponent(name.split('/').at(-1)),readFileSync(join(source,name)));
  state.images=(await get(p+'/images')).images;
  state.conversation_id=(await post(p+'/conversations')).conversation_id;save();
  const cr=p+'/conversations/'+state.conversation_id;
  const preference=await get(cr+'/agent-model');await post(cr+'/agent-model',{request_id:uuid(),expected_revision:preference.revision,model_profile_id:state.planner});
  const sent=await post(cr+'/send',{message:{id:uuid(),text:'标注这五张 B-Human 原图中的足球和机器人，排除人和场地线。我要用 Ultralytics YOLO 训练目标检测，每个目标一个紧贴边界的框。先测试最多三张；VLM 粗定位可能不准，请按 Registry 实际能力组合局部检查及必要的 SAM 边界精修，不确定时请我确认。最后交付原图、标签、划分、data.yaml 和检查报告。',image:null},schema_revision:(await get(p+'/goal')).revision,mode:'plan'});
  state.task_id=sent.task_id;state.task_root=cr+'/tasks/'+sent.task_id;save();
  state.delivery=(await post(state.task_root+'/delivery-intent',{command_id:uuid(),expected_revision:0,image_ids:state.images.map(i=>i.image_id),label_spec:[{stable_id:'ball',display_name:'足球',aliases:['football','soccer ball'],include:'场地上的足球，每个球单独框出',exclude:'人、场线、白色杂物'},{stable_id:'robot',display_name:'机器人',aliases:['humanoid robot'],include:'可见人形足球机器人，每个机器人一个框',exclude:'人、裁判、观众'}],training_target:{annotation_kind:'bounding_box',framework:'ultralytics',export_profile:'ultralytics_yolo_detection',profile_revision:1},split_policy:{train_percent:80,seed:7,preserve_existing:true,keep_known_groups_together:true}})).saved;
  state.schema=await post(state.task_root+'/delivery-schema',{command_id:uuid(),expected_revision:state.delivery.revision,expected_sha256:state.delivery.content_sha256});save();
  console.log(JSON.stringify({project:state.project,task:state.task_id,images:state.images.length,url:base+'/projects/'+state.project+'/work?task='+state.task_id}));
}else if(stage==='builder'||stage==='replan'){
  if(stage==='replan'){
    assert.equal(state.builder?.status,'completed','Unknown/active operations must be inspected, not replaced.');
    (state.previous_builders??=[]).push(state.builder);
    delete state.builder_operation;delete state.builder;save();
  }
  assert.ok(state.schema&&!state.builder_operation,'Requires prepared scope and no previous builder submission.');
  state.builder_operation=uuid();save();
  const preview=await get(state.task_root+'/builder-preview?'+new URLSearchParams({operation_id:state.builder_operation,schema_id:state.schema.id,schema_revision:state.schema.revision,model_id:state.planner}));
  state.builder_preview=preview;save();
  console.log('Submitting one explicitly authorized LIVE planning operation; no automatic retry.');
  state.builder=await post(state.task_root+'/builder-operations',{...Object.fromEntries(['selection','repair','previous_grant_id','scope_hash','expires_at'].map(k=>[k,preview[k]])),allow_unknown_cost:true});save();console.log(JSON.stringify(state.builder));
}else if(stage==='sample'||stage==='retry-rejected-sample'){
  assert.equal(state.builder?.status,'completed');
  if(stage==='retry-rejected-sample'){
    const last=state.trace.filter(t=>t.method==='POST'&&t.path.endsWith('/sample-operations')).at(-1);
    assert.equal(last?.status,400);assert.match(last.response.error,/No inference was started/);
    assert.ok(state.sample_request&&!state.sample,'Only retry the confirmed pre-inference rejection using its original command.');
  }else assert.ok(!state.sample_request,'Do not duplicate an uncertain sample POST.');
  const draft=state.builder.evidence.draft_id;
  state.sample_request??=uuid();save();
  const preview=await get(state.task_root+'/sample-preview?'+new URLSearchParams({draft_id:draft,request_id:state.sample_request}));
  state.sample_preview=preview;save();
  const budget=preview.conversation_budget;
  state.sample=await post('/api/projects/'+state.project+'/sample-operations',{request_id:preview.request_id,draft_id:draft,expected_revision:preview.revision,image_indices:[0,1,2],authorization_fingerprint:preview.authorization_fingerprint,conversation:{conversation_id:state.conversation_id,task_id:state.task_id,...Object.fromEntries(['previous_grant_id','scope_hash','expires_at'].map(k=>[k,budget[k]])),allow_unknown_cost:true,human_review:true}});save();console.log(JSON.stringify(state.sample));
}else if(stage==='process'){
  assert.equal(state.sample_status?.status,'succeeded');assert.ok(!state.processing_request,'Inspect original processing command before any retry.');
  const selection={draft_id:state.builder.evidence.draft_id,sample_test_id:state.sample.id,limit:state.images.length};
  const preview=await get('/api/projects/'+state.project+'/processing-preview?'+new URLSearchParams(selection));
  state.processing_request=uuid();state.processing_preview=preview;save();
  state.processing=await post('/api/projects/'+state.project+'/processing-operations',{request_id:state.processing_request,selection,expected_revision:preview.revision,authorization_fingerprint:preview.authorization_fingerprint});save();console.log(JSON.stringify(state.processing));
}else if(stage==='inspect-formal'){
  assert.ok(state.processing);
  state.batch=await get('/api/batches/'+state.processing.batch_id);save();
  for(const image of state.images){
    const run=state.batch.batch.images.find(i=>i.image_id===image.image_id)?.child_run_id;
    if(!run)continue;
    const view=await get(state.task_root+'/delivery-images/'+image.image_id+'?source_run_id='+run);
    console.log(JSON.stringify({image:image.name||image.file_name||image.image_id,run,annotations:view.snapshot.annotations.map(a=>({id:a.id,label:a.label,status:a.review_status,value:a.value}))}));
  }
}else if(stage==='status'){
  const task=await get(state.task_root+'/workspace');console.log(JSON.stringify({status:task.task?.status,sample_operations:task.sample_operations}));
  if(state.sample){state.sample_status=await get('/api/projects/'+state.project+'/sample-operations/'+state.sample.id);save();console.log(JSON.stringify(state.sample_status));}
  if(state.processing){state.batch=await get('/api/batches/'+state.processing.batch_id);save();console.log(JSON.stringify(state.batch));}
}else throw new Error('Use prepare, builder, replan, sample or status. No paid operation is automatically retried.');
