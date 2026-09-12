import { useEffect, useState } from "react";
import { DeliveryPackage } from "./DeliveryPackage";
import { assertDemoDeliveryPanelRead, demoSourceModeLabel } from "./demoDeliveryPresentation";
import type { DemoDeliveryPanelRead, DemoDeliveryPanelService, DeliveryPackageRead } from "./deliveryService";

export type DemoDeliveryPanelProps = {
  projectId:string;
  taskId:string;
  deliveryId?:string;
  onReady?:(receipt:DeliveryPackageRead)=>void;
  onDownload?:(packageId:string)=>void;
};

type HostProps = DemoDeliveryPanelProps & {service:DemoDeliveryPanelService};

/** Loads only the frozen delivery scope; admission and ZIP creation remain server-owned. */
export function DemoDeliveryPanel({service,projectId,taskId,deliveryId,onReady,onDownload}:HostProps) {
  const [view,setView]=useState<DemoDeliveryPanelRead>();
  const [error,setError]=useState("");
  const [reload,setReload]=useState(0);

  useEffect(()=>{
    const controller=new AbortController();
    setError("");
    void service.demoDeliveryPanel(projectId,taskId,deliveryId,controller.signal)
      .then(next=>{if(!controller.signal.aborted)setView(assertDemoDeliveryPanelRead(next,projectId,taskId,deliveryId));})
      .catch((cause:Error)=>{if(!controller.signal.aborted)setError(cause.message);});
    return()=>controller.abort();
  },[deliveryId,projectId,reload,service,taskId]);

  if(error)return <section aria-label="Demo 训练包"><p role="alert">{error}</p><button type="button" onClick={()=>setReload(value=>value+1)}>重新读取交付范围</button></section>;
  if(!view)return <section aria-label="Demo 训练包"><p role="status">读取正式审核范围与数据包状态…</p></section>;

  return <section className="demo-delivery-panel" aria-label="Demo 训练包">
    <header>
      <h3>生成真实训练数据包</h3>
      <p>{demoSourceModeLabel(view.demo)}</p>
      <p>ZIP 由 Rust 使用当前正式审核快照生成；修改框后需等待新的服务端数据包与 hash，旧包不会被覆盖。</p>
    </header>
    <DeliveryPackage
      service={service}
      project={projectId}
      task={taskId}
      scope={view.scope}
      initialPackageId={deliveryId||view.delivery_id||undefined}
      onReady={onReady}
      onDownload={onDownload}
      expectedDemo={view.demo}
      onInspect={()=>{}}
    />
  </section>;
}

export const createDemoDeliveryPanel = (service:DemoDeliveryPanelService) => (props:DemoDeliveryPanelProps) => <DemoDeliveryPanel {...props} service={service}/>;
