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
console.log(deepPrint(bc))