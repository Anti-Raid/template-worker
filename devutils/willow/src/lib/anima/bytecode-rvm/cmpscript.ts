import { Anima } from "./anima";
import { deepPrint, dumpFull, readFull } from "./utils";

const anima = new Anima()
const simpleProg = `((define (safe-mul a b) (* a b))
                (define (safe-add a b) (+ a b))
                (define (crash) (error "Core Meltdown"))

                (define (test)
                    (let ((x (try safe-add 10 20 '()))) ;; Sync builtin success
                        (let ((y (try (lambda () (safe-mul x 2)) '()))) ;; Async closure success
                            (if (= y 60)
                                (crash) ;; Outer try must catch this!
                                "Math failed"))))

                (error-message (try test '()))
`
const bc = anima.compileRaw(simpleProg)
console.log(deepPrint(bc))
const dumped = dumpFull(bc)
console.log(dumped)
console.log(readFull(dumped), bc)