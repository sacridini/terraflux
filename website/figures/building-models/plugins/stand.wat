;; A forest stand's biomass, cell by cell: logistic growth year after year, and a harvest.
;; Every function takes and gives f64 (WebAssembly's 64-bit numbers), as plugin.wasm calls them.
(module
  ;; grow(r, b, years): the biomass (t/ha) after `years` years of logistic growth from `b`, at the rate `r`
  ;; per year, towards a carrying capacity of 300 t/ha:  b <- b + r b (1 - b / 300)
  (func $grow (export "grow") (param $r f64) (param $b f64) (param $years f64) (result f64)
    (local $t f64)
    ;; a missing cell (NaN, which is not equal to itself) stays missing
    (if (f64.ne (local.get $r) (local.get $r)) (then (return (local.get $r))))
    (if (f64.ne (local.get $b) (local.get $b)) (then (return (local.get $b))))
    (block $done
      (loop $year
        (br_if $done (f64.ge (local.get $t) (local.get $years)))
        (local.set $b
          (f64.add (local.get $b)
            (f64.mul (f64.mul (local.get $r) (local.get $b))
              (f64.sub (f64.const 1) (f64.div (local.get $b) (f64.const 300))))))
        (local.set $t (f64.add (local.get $t) (f64.const 1)))
        (br $year)))
    (local.get $b))

  ;; harvested(r, b, years, cycle): the same growth, but every `cycle` years a stand above 150 t/ha loses 60 % of
  ;; its biomass (a felling); the biomass at the end
  (func (export "harvested") (param $r f64) (param $b f64) (param $years f64) (param $cycle f64) (result f64)
    (local $t f64)
    (if (f64.ne (local.get $r) (local.get $r)) (then (return (local.get $r))))
    (if (f64.ne (local.get $b) (local.get $b)) (then (return (local.get $b))))
    (block $done
      (loop $year
        (br_if $done (f64.ge (local.get $t) (local.get $years)))
        (local.set $b (call $grow (local.get $r) (local.get $b) (f64.const 1)))
        (local.set $t (f64.add (local.get $t) (f64.const 1)))
        ;; a felling year: t a multiple of cycle, and the stand large enough
        (if (i32.and
              (f64.eq (f64.sub (local.get $t) (f64.mul (f64.floor (f64.div (local.get $t) (local.get $cycle))) (local.get $cycle))) (f64.const 0))
              (f64.gt (local.get $b) (f64.const 150)))
          (then (local.set $b (f64.mul (local.get $b) (f64.const 0.4)))))
        (br $year)))
    (local.get $b))

  ;; forever(x): a function that never ends, to see the sandbox stop it
  (func (export "forever") (param $x f64) (result f64)
    (loop $again (br $again))
    (local.get $x))
)
