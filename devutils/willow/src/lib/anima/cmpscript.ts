import { Anima } from "./anima";
import { impl } from "./bytecode-svm/meta";

const anima = new Anima(impl)

console.log("\n\n")
const simpleProg = `(+ (* 1 2) (- 1 1) (- 1 2))`
const bc = anima.compileRaw(simpleProg)
impl.deepPrint(bc)
const t1 = performance.now()
const res = anima.evaluateRaw(bc)
const t2 = performance.now()
console.log(res)
console.log(t2-t1, "ms")