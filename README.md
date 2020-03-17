<meta charset="utf-8"/>

# `bstree-file`

About
-----

Immutable implicit naive Binary Search Tree structure stored in a file.

The tree structure (possibly larger than the available RAM) is created at once
using bulk-loading.
It is then possible to perform queries on the datastructure
(nn query, knn query, range query, ...), but not to update it.

It has been developed for static astronomical catalogues.

The datastructure is implicit: it is basically a flat array of entries 
ordered in a pre-defined way depending on a few parameters like
the number of elements in the tree, the size of both the L1 and the disk caches.

The implementation is probably a naive implementation by a non-expert,
any feedback welcome.

Purpose
-------

Perform fast queries on a single catalogue column.
The binary-search tree basically stores both values and OIDs (row indices).

In input, the tools takes an identifiers (which can be an implicit sequential number)
and a value.
The indexation is made on the values.
Queries returns entries which basically are tuples containing an identifier and value couple.


Creation algorithm
------------------

Although the first step is an external k-way merge sort, 
the final file is not ordered sequentially.
It consists in a sequence of binary search tree blocks.
There is two levels of blocks: 
* blocks fitting in the L1 cache
* groups of blocks fitting in the disk cache 
The full tree is not balanced:
* it is made of a main balance tree
* plus a sub-tree recursivly consiting in
    - a main balanced tree
    - plus a sub-tree...
The tree has 0 unused byte.

Warning
-------

For performances purposes, the code make a large use of monomorphization (no dynamic dispath at all!).
It leads to:
* very long compilation time (several minutes, especially in release mode)
* large binaries:
    - `mkbst` (tree creation) is about 9/65 MB in release/debug mode
    - `qbst` (tree query) is about 29/116 MB in release/debug mode

Install
-------

The standard way to install both `mkbst` and `qbst` binaries is:
* fork this repository
* install rust [see here](https://www.rust-lang.org/tools/install), possibly removing `--tlsv1.2` in the command line
* type `cargo install --path .` from the forked directory

Example
-------

Bash script [mkseq.bash](resources/test/mkseq.bash) to generate a simple sequence of value from 0 to `n`:
```bash
#!/bin/bash

declare -rx END=$1

echo "val"

COUNTER=0
until [ $COUNTER -ge ${END} ]; do
  echo $COUNTER
  let COUNTER+=1
done
``` 
Create a simple tree containing 2 billion entries consisting of tuples having the
same identifiers (a row number) and value (the row number):
```bash
./mkseq.bash 2000000000 | ./mkbst -h bigtest --id-type u4 --val-type u4
```
Both identifiers and values are stored on regular 32bit unsigned integers.

Perform queries:
```bash
time ./qbst bigtest.bstree.bin get -v 256984
time ./qbst bigtest.bstree.bin knn -v 69853145 -k 10
time ./qbst bigtest.bstree.bin range -l 10000 -f 25639 -t 250000
time ./qbst bigtest.bstree.bin range -c -f 25639 -t 250000
```


Benchmark on magnitudes
-----------------------

TBD


TODO list
---------

* [ ] remove the code which is now obsolete (`get` overwritten by `get exact visitor` 
* [ ] add much more tests
* [ ] add benchmarks
* [ ] try to reduce the code redundance (particularly in `SubTreeW` and `SubTreeR`)

Acknowledgements
----------------

If you use this code and work in a scientific public domain
(especially astronomy), please acknowledge its usage and the 
[CDS](https://en.wikipedia.org/wiki/Centre_de_donn%C3%A9es_astronomiques_de_Strasbourg)
who developped it. 
It may help us in promoting our work to our financiers.


License
-------

Like most projects in Rust, this project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.


Contribution
------------

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

