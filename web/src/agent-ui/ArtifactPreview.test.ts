import {it,expect} from "vitest";
import {artifactIdentity,artifactImageMatches} from "./ArtifactPreview";
import type {PipelineArtifact} from "../types";
it("requires explicit matching artifact image references without guessing",()=>{
  const artifact={kind:"detection_set",artifact:{image_id:"image-a",reference:{artifact_id:"a"},detections:[]}} as PipelineArtifact;
  expect(artifactIdentity(artifact)).toBe("a");expect(artifactImageMatches(artifact,"image-a")).toBe(true);
  expect(artifactImageMatches(artifact,"image-b")).toBe(false);
  expect(artifactImageMatches({...artifact,artifact:{}},"image-a")).toBe(false);
  expect(artifactIdentity({...artifact,artifact:{}})).toBeUndefined();
  expect(artifactImageMatches({...artifact,artifact:{...artifact.artifact,root_region:{x:0.2}}},"image-a")).toBe(false);
});
