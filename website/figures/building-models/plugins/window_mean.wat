;; The window interface: the mean of the (2r+1) x (2r+1) cells around each cell, the missing ones left out.
;; plugin.wasm writes the window of every input in the module's memory, and calls window(ptr, side, n_inputs, row, col).
(module
  ;; the host needs the module's memory and an allocator (a bump allocator is enough: the host asks for one block)
  (memory (export "memory") 1)
  (global $top (mut i32) (i32.const 8))
  (func (export "alloc") (param $bytes i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $top))
    (global.set $top (i32.add (local.get $p) (local.get $bytes)))
    (local.get $p))

  ;; window(ptr, side, n_inputs, row, col): the first input's window starts at `ptr`, `side` x `side` f64 row by row
  (func (export "window") (param $ptr i32) (param $side i32) (param $n i32) (param $row i32) (param $col i32)
        (result f64)
    (local $i i32) (local $count i32) (local $sum f64) (local $x f64)
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (i32.mul (local.get $side) (local.get $side))))
        (local.set $x (f64.load (i32.add (local.get $ptr) (i32.shl (local.get $i) (i32.const 3)))))
        ;; NaN (outside the map, or a missing cell) is not equal to itself
        (if (f64.eq (local.get $x) (local.get $x))
          (then
            (local.set $sum (f64.add (local.get $sum) (local.get $x)))
            (local.set $count (i32.add (local.get $count) (i32.const 1)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (f64.div (local.get $sum) (f64.convert_i32_s (local.get $count))))
)
