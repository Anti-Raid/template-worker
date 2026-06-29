import { OP_APPLY } from "../common";
import { Compiler } from "./compiler";
import { deepPrint } from "./utils";
import { AnimaVM, APPLY_PROC, Globals, IBUILTINS } from "./vm";

const code = `
;(define (my-foo x y z) (list (+ x y z)))
;(define f (apply my-foo 1 (list 2 3)))

;(apply my-foo 1 (list 2 3))
;(apply my-foo (list 1 2 3))
(+ (apply + (list 1 2 3)) 123)
;(list 1 2 3)
;(+ 1 2 3)
`

const GLOBALS = new Globals(new Map())
for(const builtin of IBUILTINS) {
  GLOBALS.data.set(builtin.name, builtin)
}
GLOBALS.data.set(OP_APPLY, APPLY_PROC)

const c = new Compiler()
const bc = c.compile(code)

console.log(deepPrint(bc))
const t1 = performance.now()
const retv = new AnimaVM().evaluate(bc, GLOBALS)
const t2 = performance.now()
console.log(retv, t2-t1)