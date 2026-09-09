import {expect,it} from "vitest";
import {calibrationRuns} from "./GeometryCalibration";
it("requires explicit bounded run references without guessing or duplicates",()=>{const id="12345678-1234-1234-1234-123456789abc";expect(calibrationRuns(`${id},\n${id}`)).toEqual([id]);for(const input of ["","latest","wrong owner","1"])expect(()=>calibrationRuns(input)).toThrow();});
