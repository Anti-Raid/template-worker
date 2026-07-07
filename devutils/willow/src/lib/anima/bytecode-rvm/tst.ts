import { OP_APPLY } from "../common";
import { publicScope } from "./bootstrap";
import { Compiler } from "./compiler";
import { deepPrint } from "./utils";
import { AnimaVM, APPLY_PROC, Globals, IBUILTINS } from "./vm";

const stmts = [`(begin
                  (define (loop n)
                    (if (= n 0)
                        "survived!"
                        (loop (- n 1))))
                  (loop 15000))`]

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

const GLOBALS = publicScope.nestWith({})

const c = new Compiler()
for (const code of stmts) {
  const bc = c.compileRaw(code)

  console.log(deepPrint(bc))
  const t1 = performance.now()
  const retv = new AnimaVM().evaluateRaw(bc, GLOBALS)
  const t2 = performance.now()
  console.log(retv, t2-t1)
}