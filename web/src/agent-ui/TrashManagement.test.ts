import {expect,it,vi} from "vitest";
import {ownedTrashTargets} from "./TrashManagement";
import type {TrashEntry,DatasetBatchSummary} from "../types";
const entries:TrashEntry[]=[{project_id:"p",object:{kind:"batch",id:"b",expected_revision:2}},{project_id:"p",object:{kind:"run",id:"child",expected_revision:1}},{project_id:"p",object:{kind:"run",id:"independent",expected_revision:1}}] as TrashEntry[];
const batch={id:"b",project_id:"p",lifecycle_revision:2,in_trash:true,child_run_ids:["child"]} as DatasetBatchSummary;
it("removes only children proven to belong to the exact selected owned Batch",async()=>{
  const read=vi.fn().mockResolvedValue({batch});expect(await ownedTrashTargets(entries,"p",{batch:read})).toEqual([entries[0].object,entries[2].object]);expect(read).toHaveBeenCalledWith("b");
  read.mockClear();expect(await ownedTrashTargets(entries.slice(1),"p",{batch:read})).toEqual(entries.slice(1).map(e=>e.object));expect(read).not.toHaveBeenCalled();
});
it("rejects foreign or stale Batch evidence without broadening management scope",async()=>{
  for(const change of [{project_id:"foreign"},{id:"wrong"},{lifecycle_revision:3},{in_trash:false}])await expect(ownedTrashTargets(entries,"p",{batch:vi.fn().mockResolvedValue({batch:{...batch,...change}})})).rejects.toThrow();
  const read=vi.fn();await expect(ownedTrashTargets(entries,"other",{batch:read})).rejects.toThrow();expect(read).not.toHaveBeenCalled();
});
