import { isolatedEvidencePath } from "./evidence";
import {resolve} from "node:path";
import {readFileSync} from "node:fs";
import {expect,test} from "./fixtures";

test("new image project and inventory entry use the same conversation workspace",async({page,request})=>{
  const name=`TEST entry ${Date.now()}`;
  await page.goto("/projects?new=1");
  await page.getByLabel("Choose images",{exact:true}).setInputFiles({name:`${name}.png`,mimeType:"image/png",buffer:readFileSync(resolve("../examples/robocup/images/synthetic-robocup.png"))});
  await page.getByRole("button",{name:"Continue",exact:true}).click();
  await expect(page).toHaveURL(/\/projects\/[^/]+\/work$/);
  const project=new URL(page.url()).pathname.split("/")[2];
  await expect(page.getByRole("region",{name:"Annotation workspace",exact:true})).toBeVisible();
  await expect(page.getByRole("button",{name:"Save goal and prepare labels",exact:true})).toBeVisible();
  await expect(page.locator(".conversation-image img")).toBeVisible();
  expect((await (await request.get(`/api/projects/${project}/images`)).json()).images).toHaveLength(1);
  const writes:string[]=[];
  page.on("request",req=>{if(!["GET","HEAD"].includes(req.method()))writes.push(req.url());});
  await page.reload();
  await expect(page.getByRole("region",{name:"Annotation workspace",exact:true})).toBeVisible();
  await page.getByRole("button",{name:"Back to project",exact:true}).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${project}$`));
  await expect(page.getByRole("navigation",{name:`${name} workspace`,exact:true})).toBeVisible();
  await page.goto("/projects");
  let releaseImages!:()=>void;
  const imagesReady=new Promise<void>(resolve=>{releaseImages=resolve;});
  await page.route(`**/api/projects/${project}/images`,async route=>{await imagesReady;await route.continue();},{times:1});
  await page.locator(".project-row").filter({hasText:name}).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${project}/work$`));
  try{
    await expect(page.getByText("Loading saved images…",{exact:true})).toBeVisible();
    await expect(page.getByRole("heading",{name:"Start with your own images",exact:true})).toHaveCount(0);
  }finally{releaseImages();}
  await expect(page.locator(".conversation-image img")).toBeVisible();
  await expect(page.getByText("Loading saved workspace…",{exact:true})).toHaveCount(0);
  await expect(page.locator(".sidebar")).toHaveCount(0);
  expect(writes).toEqual([]);
  await page.screenshot({path:isolatedEvidencePath(resolve(process.env.ANNOTAGENT_E2E_EVIDENCE_DIR ?? "../docs/execution/conversational-workspace","default-project-entry.png")),fullPage:true,animations:"disabled"});
});

test("project deep links resolve their owner outside the dashboard page",async({page,request})=>{
  const id=`TEST-unlisted-${Date.now()}`;
  expect((await request.post("/api/projects",{data:{id,yaml:"version: 1\nproject:\n  name: TEST paginated owner\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n"}})).ok()).toBe(true);
  // Simulate a bounded index excluding this real Project, not a fake owner summary.
  await page.route("**/api/projects",async route=>{
    const response=await route.fetch();const data=await response.json();
    await route.fulfill({response,json:{...data,projects:data.projects.filter((project:any)=>project.id!==id)}});
  });
  let release!:()=>void;
  const held=new Promise<void>(resolve=>{release=resolve;});
  await page.route(`**/api/projects/${id}/summary`,async route=>{await held;await route.continue();},{times:1});
  await page.goto(`/projects/${id}/task/goal`);
  await expect(page.getByText("Loading workspace state…",{exact:true})).toBeVisible();
  await expect(page.getByRole("heading",{name:"This page does not exist",exact:true})).toHaveCount(0);
  release();
  await expect(page.getByRole("region",{name:"Annotation goal",exact:true})).toBeVisible();
  await expect(page.getByText("TEST paginated owner",{exact:true})).toBeVisible();
  await page.reload();
  await expect(page.getByRole("region",{name:"Annotation goal",exact:true})).toBeVisible();
  await page.goto(`/projects/${id}/work`);
  await expect(page.getByRole("region",{name:"Annotation workspace",exact:true})).toBeVisible();
});
