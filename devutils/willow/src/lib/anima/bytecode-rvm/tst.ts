import { Compiler } from "./compiler";
import { deepPrint } from "./utils";
import { AnimaVM, Globals } from "./vm";

const c = new Compiler()
const bc = c.compile(`
(define (make-counter)
  (let ((count 0))
    (lambda ()
      (set! count (+ count 1))
      count)))

(let ((counter-a (make-counter))
      (counter-b (make-counter)))
  (counter-a) ; 1
  (counter-a) ; 2
  (counter-b) ; 1 (Should be completely independent)
  (counter-a))
`)

console.log(deepPrint(bc))
new AnimaVM().evaluate(bc, new Globals(new Map()))