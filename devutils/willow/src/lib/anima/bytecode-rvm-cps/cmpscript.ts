import { Anima } from "./anima";
import { deepPrint } from "./utils";

const anima = new Anima()
const simpleProg = `(begin
                  (define (loop n)
                    (if (= n 0)
                        "survived!"
                        (loop (- n 1))))
                  (loop 15000))
`
const bc = anima.compileRaw(simpleProg)
deepPrint(bc)
const t1 = performance.now()
const res = anima.evaluateRaw(bc)
const t2 = performance.now()
console.log(res)
console.log(t2-t1, "ms")