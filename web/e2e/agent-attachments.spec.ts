import {createHash} from "node:crypto";
import {readFileSync} from "node:fs";
import {resolve} from "node:path";
import {expect,test,fetchWithinMutationLimit} from "./fixtures";

test("Composer attaches the uploaded content identity and retains it through lost Send acknowledgement",async({page,request})=>{
  const project=`TEST-composer-attachment-${Date.now()}`;
  expect((await request.post("/api/projects",{data:{id:project,yaml:"version: 1\nproject:\n  name: TEST attachments\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBe(true);
  const original=readFileSync(resolve("../examples/robocup/images/synthetic-robocup.png"));
  // Same TEST pixels/name but distinct encoded content: do not attach by name,
  // first item, or import count. Bytes following PNG IEND are not image pixels.
  const bytes=Buffer.concat([original,Buffer.from("TEST encoded-content identity")]);
  const hash=createHash("sha256").update(bytes).digest("hex");
  expect((await request.post(`/api/projects/${project}/image-upload?name=same.png`,{data:original,headers:{"Content-Type":"image/png"}})).ok()).toBe(true);
  await page.goto(`/projects/${project}/work?pane=thread`);
  const composer=page.getByRole("form",{name:"Agent composer",exact:true});
  const file=composer.getByLabel("Attach image to message",{exact:true});
  const writes:string[]=[];
  page.on("request",req=>{if(req.method()!=="GET")writes.push(new URL(req.url()).pathname);});
  await file.setInputFiles({name:"same.png",mimeType:"image/png",buffer:bytes});
  await expect(page.getByRole("status").filter({hasText:"attached to your unsent message"})).toBeVisible();
  const dataset=await (await request.get(`/api/projects/${project}/images`)).json();
  expect(dataset.images).toHaveLength(2);
  const attached=dataset.images.find((image:{content_hash:string})=>image.content_hash===hash);
  expect(attached).toBeTruthy();
  await expect(composer.getByRole("img",{name:`Attached image: ${attached.name}`,exact:true})).toBeVisible();
  expect(writes.every(path=>path.endsWith("/image-upload"))).toBe(true);
  // Retrying an identical upload reuses the existing source identity.
  await file.setInputFiles({name:"same.png",mimeType:"image/png",buffer:bytes});
  await expect(file).toBeEnabled();
  expect((await (await request.get(`/api/projects/${project}/images`)).json()).images).toHaveLength(2);
  await file.setInputFiles({name:"invalid.png",mimeType:"image/png",buffer:Buffer.from("TEST not an image")});
  await expect(page.getByRole("alert").last()).toBeVisible();
  await expect(composer.getByRole("img",{name:`Attached image: ${attached.name}`,exact:true})).toBeVisible();
  await expect(file).toBeEnabled();
  await page.getByRole("button",{name:"Open data and results",exact:true}).click();
  const other=dataset.images.find((image:{content_hash:string})=>image.content_hash!==hash);
  await page.getByRole("navigation",{name:"Select image",exact:true}).getByRole("button",{name:other.name,exact:true}).click();
  await expect.poll(()=>new URL(page.url()).searchParams.get("image")).toBe(other.image_id);
  await expect(composer.getByRole("img",{name:`Attached image: ${attached.name}`,exact:true})).toBeVisible();
  await page.getByRole("button",{name:"Close data and results",exact:true}).click();
  await page.setViewportSize({width:390,height:844});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await composer.screenshot({path:"/tmp/annotagent-composer-attachment-390.png"});
  await page.setViewportSize({width:1440,height:900});
  let guarded=false;
  page.once("dialog",async dialog=>{guarded=true;expect(dialog.type()).toBe("confirm");await dialog.dismiss();});
  await page.getByRole("button",{name:"← Projects",exact:true}).click();
  expect(guarded).toBe(true);
  expect(new URL(page.url()).pathname).toBe(`/projects/${project}/work`);
  const sends:unknown[]=[];
  await page.route(`**/projects/${project}/conversations/*/send`,async route=>{
    sends.push(route.request().postDataJSON());
    if(sends.length===1){expect((await fetchWithinMutationLimit(route)).ok()).toBe(true);await route.abort("failed");}
    else await route.continue();
  });
  await composer.getByRole("textbox",{name:"Your message",exact:true}).fill("TEST describe this image after authorization");
  await composer.getByRole("button",{name:"Send",exact:true}).click();
  await expect(composer.getByRole("button",{name:"Retry same send",exact:true})).toBeEnabled();
  await expect(file).toBeDisabled();
  await expect(composer.getByRole("button",{name:"Remove image reference",exact:true})).toBeDisabled();
  expect((sends[0] as {message:{image:unknown}}).message.image).toEqual({image_id:attached.image_id,sha256:hash});
  await composer.getByRole("button",{name:"Retry same send",exact:true}).click();
  await expect(composer.getByRole("textbox",{name:"Your message",exact:true})).toHaveValue("");
  expect(sends[1]).toEqual(sends[0]);
  const conversation=(await (await request.get(`/api/projects/${project}/conversations`)).json()).conversation_id;
  const root=`/api/projects/${project}/conversations/${conversation}`;
  const messages=await (await request.get(`${root}/messages`)).json();
  expect(messages).toHaveLength(1);
  expect(messages[0].input.image).toEqual({image_id:attached.image_id,sha256:hash});
  const tasks=await (await request.get(`${root}/tasks`)).json();
  expect((await (await request.get(`${root}/tasks/${tasks[0].input.id}/budget`)).json()).total_reserved_calls).toBe(0);
  await page.reload();
  await expect(page.getByRole("list",{name:"Saved messages",exact:true})).toContainText(attached.name);
  expect(sends).toHaveLength(2);
});
