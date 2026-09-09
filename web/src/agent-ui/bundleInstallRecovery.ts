export interface PendingBundleInstall {
  catalog_id: string; bundle_id: string; bundle_version: string; plugin_id: string; plugin_version: string;
}
export function bundleInstallKey(workspace: string, plugin: string, version: string): string {
  return `annotagent.bundle-install.pending:${JSON.stringify([workspace, plugin, version])}`;
}
/** This record prevents retries. It is not proof that a server operation exists. */
export function preserveBundleInstall(storage: Pick<Storage,"getItem"|"setItem">, key:string, request:PendingBundleInstall) {
  if(storage.getItem(key)!==null)throw new Error("已有未核实的模型安装请求，不能重新提交。");
  const value=JSON.stringify(request);storage.setItem(key,value);
  if(storage.getItem(key)!==value)throw new Error("无法保存安装恢复记录，未启动下载。");
}
