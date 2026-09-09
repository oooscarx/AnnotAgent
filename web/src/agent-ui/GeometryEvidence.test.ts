import {expect,it} from "vitest";
import {checkedGeometryEvidence,geometryMeasure} from "./GeometryEvidence";
import type {GeometryCalibrationView,ProjectGeometryPolicy} from "../types";
it("keeps unknown quality distinct from measured zero",()=>{expect(geometryMeasure(undefined)).toBe("未测量");expect(geometryMeasure(NaN)).toBe("未测量");expect(geometryMeasure(0)).toBe("0.000");expect(geometryMeasure(0,true)).toBe("0.0%");});
it("keeps server-resolved Core identity separate from route slug and rejects mixed records",()=>{const p={project_id:"core-id"} as ProjectGeometryPolicy;const c={report:{key:{project_id:"other"}}} as GeometryCalibrationView;expect(()=>checkedGeometryEvidence("route-slug",[p],[c])).toThrow();expect(checkedGeometryEvidence("route-slug",[p],[]).policies).toEqual([p]);expect(checkedGeometryEvidence("p",[],[])).toEqual({policies:[],calibrations:[]});});
