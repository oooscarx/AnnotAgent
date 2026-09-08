import { describe, expect, it } from "vitest";
import { parseWorkspaceRoute, projectWorkPath, routeFocusKey, withConversationReturn, conversationReturn, conversationSettingsPath } from "../navigation";

describe("conversation formal results context",()=>{
  it("preserves setup context only for the explicit owning Project",()=>{
    const origin=projectWorkPath("p",{conversationId:"c",taskId:"t",imageId:"i",draftId:"d",sampleTestId:"s"});
    const path=conversationSettingsPath("p","models",origin);
    const url=new URL(path,"http://localhost");
    expect(parseWorkspaceRoute(url.pathname,url.search)).toMatchObject({kind:"settings",section:"models",returnProjectId:"p",workspaceReturn:origin,canonicalPath:path});
    expect(parseWorkspaceRoute("/settings","?return_project=other&workspace_return="+encodeURIComponent(origin))).toMatchObject({workspaceReturn:undefined,canonicalPath:"/settings"});
    expect(parseWorkspaceRoute("/settings/plugins","?return_project=p&workspace_return=https://evil.example")).toMatchObject({workspaceReturn:undefined,canonicalPath:"/settings/plugins"});
  });
  it("preserves an owned workspace return through canonical Review and Export",()=>{
    const origin=projectWorkPath("p",{conversationId:"c",results:{batchId:"b",imageId:"i",annotationId:"a"}});
    for(const destination of ["/projects/p/review/a", "/projects/p/export"]){
      const url=new URL(withConversationReturn(destination,origin),"http://localhost");
      expect(parseWorkspaceRoute(url.pathname,url.search)).toMatchObject({workspaceReturn:origin,canonicalPath:url.pathname+url.search});
    }
    for(const invalid of ["https://example.com", "//example.com/projects/p/work", "/projects/other/work", "/projects/p/export", "/projects/p/work#fragment"]){
      expect(conversationReturn("p",invalid)).toBeUndefined();
      expect(withConversationReturn("/projects/p/review/a",invalid)).toBe("/projects/p/review/a");
    }
    expect(parseWorkspaceRoute("/projects/p/export","?workspace_return=https://example.com")).toMatchObject({workspaceReturn:undefined,canonicalPath:"/projects/p/export"});
  });
  it("keeps sample and formal result selection separate across refresh",()=>{
    const context={conversationId:"c",draftId:"d",sampleTestId:"test",imageId:"sample-image",taskId:"task",humanRequestId:"help",processingOperationId:"receipt",results:{batchId:"batch",imageId:"formal-image",status:"needs_review",annotationId:"candidate",canvasView:"original" as const}};
    const path=projectWorkPath("p",context);
    const url=new URL(path,"http://localhost");
    const route=parseWorkspaceRoute(url.pathname,url.search);
    expect(route).toMatchObject({kind:"conversation",projectId:"p",...context,canonicalPath:path});
    expect(routeFocusKey(route)).toBe("conversation:p");
    const back=projectWorkPath("p",{...context,results:undefined});
    expect(new URL(back,"http://localhost").searchParams.get("image")).toBe("sample-image");
    expect(back).not.toContain("result_image");
  });
  it("does not interpret detached result parameters or unknown views",()=>{
    expect(parseWorkspaceRoute("/projects/p/work","?result_image=i&result_view=debug")).toMatchObject({kind:"conversation",results:undefined});
    expect(parseWorkspaceRoute("/projects/p/work","?batch=b&result_view=debug")).toMatchObject({results:{batchId:"b",canvasView:undefined}});
  });
});
