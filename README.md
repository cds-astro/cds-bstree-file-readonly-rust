
<meta charset="utf-8"/>

# `bstree-file`

About
-----

Immutable implicit naive Binary Search Tree structure stored in a file.

The tree structure (possibly larger than the available RAM) is created at once
using bulk-loading.
It is then possible to perform queries on the datastructure
(nn query, knn query, range query), but not to update it.

It has been developped for static astronomical catalogues.

The datastructure is implicit: it is basically a flat array of entries 
ordered in a pre-defined way depending on a few parameters like
the number of elements in the tree and the node size.

The implementation is a naive implementation by a non-expert,
any feedback welcome.

Purpose
-------

Perform fast queries on a single catalogue column.
The binary-search tree basically stores both values and OIDs (row indices).


Creation algorithm
------------------

Althought the first step is an external merge sort, 
the final file is not ordered sequentially.
It consists in a sequence of binary search tree blocks.
The first block in the file contains the root of the tree.

Example of root block containg the 3 first tree layers (depth 0 to 2).

| d0  | d1       | d2                 |
|-----|----------|--------------------|
| n/2 | n/4 3n/4 | n/8 2n/8 3n/8 4n/8 |

Values `n/2`, ... are the indices in the (virtual) ordered array containing
all entries.


Benchmark on magnitudes
-----------------------

TBD


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

