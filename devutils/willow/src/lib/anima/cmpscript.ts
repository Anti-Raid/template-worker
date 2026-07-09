import { Anima } from "./anima";
import { impl } from "./bytecode-rvm/meta";

const anima = new Anima(impl)
const simpleProg = `(begin
                  (define (loop n)
                    (if (= n 0)
                        "survived!"
                        (loop (- n 1))))
                  (loop 15000))
`
const bc = anima.compileRaw(simpleProg)
impl.deepPrint(bc)
const t1 = performance.now()
const res = anima.evaluateRaw(bc)
const t2 = performance.now()
console.log(res)
console.log(t2-t1, "ms")