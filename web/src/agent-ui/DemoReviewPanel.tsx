import { useEffect, useState } from "react";
import { DeliveryReview } from "./DeliveryReview";
import { assertDemoReviewPanelRead, demoOriginLabel, demoReviewStateLabel, demoSourceModeLabel } from "./demoDeliveryPresentation";
import type { DemoReviewPanelRead, DemoReviewPanelService } from "./deliveryService";

export type DemoReviewPanelProps = {
  projectId:string;
  taskId:string;
  reviewId?:string;
  onOpenArtifact?:(artifactId:string)=>void;
  onReady?:(view:DemoReviewPanelRead)=>void;
};

type HostProps = DemoReviewPanelProps & {service:DemoReviewPanelService};

/** A thin server-read container around the existing review editor. */
export function DemoReviewPanel({service,projectId,taskId,reviewId,onOpenArtifact,onReady}:HostProps) {
  const [view,setView]=useState<DemoReviewPanelRead>();
  const [error,setError]=useState("");
  const [reload,setReload]=useState(0);

  useEffect(()=>{
    const controller=new AbortController();
    setError("");
    void service.demoReviewPanel(projectId,taskId,reviewId,controller.signal)
      .then(next=>{
        if(controller.signal.aborted)return;
        const checked=assertDemoReviewPanelRead(next,projectId,taskId,reviewId);
        setView(checked);
        onReady?.(checked);
      })
      .catch((cause:Error)=>{if(!controller.signal.aborted)setError(cause.message);});
    return()=>controller.abort();
  },[onReady,projectId,reload,reviewId,service,taskId]);

  const openImage=(imageId:string)=>{
    const url=new URL(location.href);
    url.searchParams.set("delivery_view","formal");
    url.searchParams.set("delivery_image",imageId);
    history.pushState(history.state,"",url);
    window.dispatchEvent(new PopStateEvent("popstate"));
  };

  if(error)return <section aria-label="Demo 图片审核"><p role="alert">{error}</p><button type="button" onClick={()=>setReload(value=>value+1)}>重新读取审核</button></section>;
  if(!view)return <section aria-label="Demo 图片审核"><p role="status">读取六图结果与正式审核状态…</p></section>;

  const origins=Object.fromEntries(view.images.map(image=>[image.image_id,image.annotation_origins]));
  return <section className="demo-review-panel" aria-label="Demo 图片审核">
    <header>
      <h3>{view.images.length} 张示例图片</h3>
      <p>{demoSourceModeLabel(view.demo)}</p>
      <p>开始体验只导入候选，不代表你已审核。对象接受与整图确认会分别保存。</p>
    </header>
    <div className="delivery-review-summary-items" aria-label="示例图片状态">
      {view.images.map(image=><article key={image.image_id}>
        <button type="button" onClick={()=>openImage(image.image_id)}>
          <img src={image.thumbnail_url||image.url} alt="" loading="lazy" />
          <span>{image.name}</span>
          <span>{demoReviewStateLabel(image.state)}</span>
        </button>
        {Object.values(image.annotation_origins).map(origin=><small key={origin.source_id}>{demoOriginLabel(origin)}</small>)}
        {image.source_artifact_id&&onOpenArtifact&&<button type="button" onClick={()=>onOpenArtifact(image.source_artifact_id!)}>查看来源 Artifact</button>}
      </article>)}
    </div>
    <DeliveryReview
      service={service}
      project={projectId}
      task={taskId}
      images={view.images.map(image=>({id:image.image_id,name:image.name,src:image.url}))}
      labels={view.labels}
      sampleResult={view.sample_result}
      formalResult={view.formal_result}
      annotationOrigins={origins}
    />
  </section>;
}

export const createDemoReviewPanel = (service:DemoReviewPanelService) => (props:DemoReviewPanelProps) => <DemoReviewPanel {...props} service={service}/>;
