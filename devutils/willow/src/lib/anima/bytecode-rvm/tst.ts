import { Compiler } from "./compiler";
import { deepPrint } from "./utils";
import { AnimaVM, Globals } from "./vm";

const code = `
(define union
    (lambda (a b)
        (define (in a rst) 
        (cond 
            [(empty? rst) #f]
            [(equal? a (car rst)) #t]
            [else (in a (cdr rst))]))

        (cond
        ; if either set is empty, the other one if the union
        [(empty? a) b]
        [(empty? b) a]
        ; if b is in a, skip it
        [(in (car b) a) (union a (cdr b))]
        [else (cons (car b) (union a (cdr b)))])))
        
    (list (equal? (union '(a b d e f h j) '(f c e g a)) '(c g a b d e f h j)))
`

const c = new Compiler()
const bc = c.compile(code)

console.log(deepPrint(bc))
const t1 = performance.now()
const retv = new AnimaVM().evaluate(bc, new Globals(new Map()))
const t2 = performance.now()
console.log(retv, t2-t1)