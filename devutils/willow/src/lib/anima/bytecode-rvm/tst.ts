import { OP_APPLY, OP_CALL_CC } from "../common";
import { Compiler } from "./compiler";
import { deepPrint } from "./utils";
import { AnimaVM, APPLY_PROC, CALLCC_PROC, Globals, IBUILTINS } from "./vm";

const stmts = [
  `(define saved-cont #f)
(define (test-cont)
  (display "A ")
  (call/cc
   (lambda (k)
     (set! saved-cont k) ; Captures state after "A "
     (display "B ")))
  (display "C "))`,
  `(test-cont)`,
  `(saved-cont '())`
]

/*const code = `
(define (test-shadowing)
  (let ((x 100))
    (let ((x 20))
      (let ((f (lambda () x)))
        (let ((x 50))
          (f)))))) ; f should still return 20, not 50 or 100

(test-shadowing)`*/
/*const code = `
(begin
                  (define (loop n)
                    (if (= n 0)
                        "survived!"
                        (loop (- n 1))))
                  (loop 15000))`*/

const GLOBALS = new Globals(new Map())
for(const builtin of IBUILTINS) {
  GLOBALS.data.set(builtin.name, builtin)
}
GLOBALS.data.set(OP_APPLY, APPLY_PROC)
GLOBALS.data.set(OP_CALL_CC, CALLCC_PROC)


const c = new Compiler()
for (const code of stmts) {
  const bc = c.compile(code)

  console.log(deepPrint(bc))
  const t1 = performance.now()
  const retv = new AnimaVM().evaluate(bc, GLOBALS)
  const t2 = performance.now()
  console.log(retv, t2-t1)
}