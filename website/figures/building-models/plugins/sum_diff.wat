;; The map interface: the whole maps at once, two outputs. map(rows, cols, n_in, inputs, n_out, outputs) fills the
;; first output with the sum of the two inputs and the second with their difference, and returns 0 (ok).
;; Used with  interface = "map", args = ["a", "b"], outputs = ["sum", "diff"].
(module
  (memory (export "memory") 1)
  (global $top (mut i32) (i32.const 8))
  ;; alloc(bytes): a bump allocator that grows the memory (pages of 64 KiB) when the block does not fit; the host
  ;; asks for the maps' blocks one after the other, and writes to them
  (func (export "alloc") (param $bytes i32) (result i32)
    (local $p i32) (local $end i32) (local $have i32)
    (local.set $p (global.get $top))
    (local.set $end (i32.add (local.get $p) (local.get $bytes)))
    (local.set $have (i32.shl (memory.size) (i32.const 16)))
    (if (i32.gt_u (local.get $end) (local.get $have))
      (then
        (if (i32.lt_s
              (memory.grow (i32.add (i32.shr_u (i32.sub (local.get $end) (local.get $have)) (i32.const 16)) (i32.const 1)))
              (i32.const 0))
          (then (unreachable)))))
    (global.set $top (local.get $end))
    (local.get $p))

  ;; the address of the i-th f64 of the array at `base`
  (func $at (param $base i32) (param $i i32) (result i32)
    (i32.add (local.get $base) (i32.shl (local.get $i) (i32.const 3))))

  ;; `inputs` and `outputs` point at arrays of pointers (i32), each to rows * cols f64
  (func (export "map") (param $rows i32) (param $cols i32) (param $n_in i32) (param $inputs i32)
        (param $n_out i32) (param $outputs i32) (result i32)
    (local $i i32) (local $n i32) (local $a i32) (local $b i32) (local $sum i32) (local $diff i32)
    (local.set $a (i32.load (local.get $inputs)))
    (local.set $b (i32.load offset=4 (local.get $inputs)))
    (local.set $sum (i32.load (local.get $outputs)))
    (local.set $diff (i32.load offset=4 (local.get $outputs)))
    (local.set $n (i32.mul (local.get $rows) (local.get $cols)))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
        (f64.store (call $at (local.get $sum) (local.get $i))
          (f64.add (f64.load (call $at (local.get $a) (local.get $i)))
                   (f64.load (call $at (local.get $b) (local.get $i)))))
        (f64.store (call $at (local.get $diff) (local.get $i))
          (f64.sub (f64.load (call $at (local.get $a) (local.get $i)))
                   (f64.load (call $at (local.get $b) (local.get $i)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    ;; 0: all went well (any other number is an error, and Terraflux reports it)
    (i32.const 0))
)
