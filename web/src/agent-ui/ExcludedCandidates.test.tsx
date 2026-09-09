import {expect,it} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import {ExcludedCandidates} from "./ExcludedCandidates";
import type {ExcludedSampleCandidate} from "../sampleFeedbackOverlay";
it("keeps excluded candidate canvases unmounted until explicitly opened",()=>{const item={annotation:{id:"one",label:"TEST"},revision:{reason:"exclude_target"}} as ExcludedSampleCandidate;const html=renderToStaticMarkup(<ExcludedCandidates excluded={[item]} imageUrl="/TEST-image"/>);expect(html).toContain("已排除的样例候选 · 1");expect(html).not.toContain("<image");expect(html).not.toContain("<select");expect(renderToStaticMarkup(<ExcludedCandidates excluded={[]} imageUrl="/TEST-image"/>)).toBe("");});
