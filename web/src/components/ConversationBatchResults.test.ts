import { describe, expect, it } from "vitest";
import { parseWorkspaceRoute, projectWorkPath, routeFocusKey } from "../navigation";

describe("conversation formal results context",()=>{
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
