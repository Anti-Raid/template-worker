import { Compiler } from "./compiler";
import { deepPrint } from "./utils";
import { AnimaVM, Globals } from "./vm";

const code = `
(begin
  (define (loop n)
    (if (= n 0)
        "survived!"
        (loop (- n 1))))
  (loop 15000))`

const c = new Compiler()
const bc = c.compile(code)

console.log(deepPrint(bc))
const t1 = performance.now()
const retv = new AnimaVM().evaluate(bc, new Globals(new Map()))
const t2 = performance.now()
console.log(retv, t2-t1)