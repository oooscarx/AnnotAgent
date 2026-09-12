import type { MainlineCapabilitySetupRequest } from "./modelPreparation";
import "./capability-setup-card.css";

const capabilityLabels: Record<string, string> = {
  text_generation: "生成并整理标注方案",
  vision_language: "理解图片并定位目标",
  image_classification: "给图片或局部目标分类",
  object_detection: "框出图片中的目标",
  open_vocabulary_detection: "按文字描述框出目标",
  phrase_grounding: "把文字描述定位到图片区域",
  semantic_segmentation: "描出目标区域",
  prompted_segmentation: "根据提示精修目标边界",
  instance_segmentation: "分别描出每个目标",
  keypoint_detection: "定位目标关键点",
};

export type CapabilitySetupCardProps = {
  request: MainlineCapabilitySetupRequest;
  busy?: boolean;
  onOpen: () => void;
};

/**
 * The one task-level model blocker shown in the Thread. It opens the existing
 * task-scoped Setup flow; it does not probe, install, authorize or execute.
 */
export function CapabilitySetupCard({ request, busy = false, onOpen }: CapabilitySetupCardProps) {
  if (request.status !== "required") return null;
  const needs = request.required_capabilities.map(
    (capability) => capabilityLabels[capability] ?? "完成当前标注步骤",
  );
  return (
    <section className="capability-setup-card" aria-labelledby={`capability-setup-${request.id}`}>
      <div>
        <h3 id={`capability-setup-${request.id}`}>继续前需要连接模型</h3>
        <p>{[...new Set(needs)].join("、")}。</p>
        <p className="capability-setup-note">
          {request.compatible_model_ids.length > 0
            ? `找到 ${request.compatible_model_ids.length} 个兼容候选；可用性会在设置页按真实 Registry 状态重新检查。`
            : "当前没有兼容候选；设置页会说明缺少的能力与可行替代。"}
        </p>
      </div>
      <button type="button" className="primary" disabled={busy} onClick={onOpen}>
        {busy ? "正在读取模型状态…" : "准备所需模型"}
      </button>
    </section>
  );
}
