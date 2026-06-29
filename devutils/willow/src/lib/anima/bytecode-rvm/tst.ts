import { Compiler } from "./compiler";
import { deepPrint } from "./utils";
import { AnimaVM, Globals } from "./vm";

const code = `
(define (make-account initial)
  (let ((balance initial))
    (define (withdraw amount)
      (set! balance (- balance amount))
      balance)
    (define (deposit amount)
      (set! balance (+ balance amount))
      balance)
    (withdraw 10)
    (deposit 50)))

(make-account 100)
`

const c = new Compiler()
const bc = c.compile(code)

console.log(deepPrint(bc))
const t1 = performance.now()
const retv = new AnimaVM().evaluate(bc, new Globals(new Map()))
const t2 = performance.now()
console.log(retv, t2-t1)