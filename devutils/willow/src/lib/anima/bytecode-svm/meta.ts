import type { AnimaMeta } from "../anima";
import { Compiler } from "./compiler";
import { deepPrint } from "./utils";
import { AnimaVM, ByteCode } from "./vm";

export const impl: AnimaMeta = {
    id: "rvm",
    vm: (maxSteps: number) => new AnimaVM(0, maxSteps),
    compiler: () => new Compiler(),
    deepPrint: (bc) => deepPrint(bc as ByteCode)
}