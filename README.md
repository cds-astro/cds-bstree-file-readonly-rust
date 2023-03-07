<meta charset="utf-8"/>

# `bstree-file`

About
-----

Immutable implicit naive Binary Search Tree structure stored in a file.

The tree structure (possibly larger than the available RAM) is created at once
using bulk-loading.
It is then possible to perform queries on the datastructure
(nn query, knn query, range query, ...), but not to update it.

It has been developed for (with a more general usage than) static astronomical catalogues.

The datastructure is implicit: it is basically a flat array of entries 
ordered in a pre-defined way depending on a few parameters like
the number of elements in the tree, the size of both the L1 and the disk caches.

The simple design inputs are:
* a metadata part followed by the data part
* data part as compact as possible, but without compression
* => hence the choice of an implicit structure with an unbalanced rightmost part of the tree

Remark: I do not claim this is the best possible structure, 
it is a quite **naive implementation by a non-expert**, any feedback welcome.

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
* plus a rightmost sub-tree recursivly consiting in
    - a main balanced tree
    - plus a rightmost sub-tree...
The tree has 0 unused byte.

Warning
-------

Main functionnalities are complete (building and queryig), but this is not 
necessarilly production ready: more testing is needed (please report any bug).

For performances purposes, the code makes a large use of monomorphization (no dynamic dispath at all!).
It leads to:
* very long compilation time (1min/10min in debug/release mode)
* large binaries:
    - `mkbst` (tree creation) is about 9/65 MB in release/debug mode
    - `qbst` (tree query) is about 29/116 MB in release/debug mode


Other tools
-----------

For a larger project that may fullfill the need (an more), see:
* [sled](https://github.com/spacejam/sled) and this [paper](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/bw-tree-icde2013-final.pdf)


Install
-------

The standard way to install the `mkbst`, `qbst` and `genfile` binaries is:
* install rust [see here](https://www.rust-lang.org/tools/install), possibly removing `--tlsv1.2` in the command line
* fork and dowbload this repository
* type `cargo install --path .` from the downloaded directory (can take ~10min!)
* WARNING: by default only a subset of (key, value) pair is available. For all posilibities, use
  `cargo install --path . --features "all"` See [Cargo.toml](Cargo.toml) for the list of features.


Example
-------

### Generate data (deprecated)

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

Remark: one can use the `shuf` command (first removing and then adding `val`) to shuffle the input lines.

### Generate data

The `genfile` tool generates simple files to test the BSTree code.
Example:
```bash
genfile 10000000000 randf64 | mkbst -h test.10b.randf64 --id-type u5 --val-type f4
```
Build a Binary Search Tree on 10 billion single precision floats having values in `[0.0, 1.0)`.

Generate a very simple sequential file containing integers with both the id and value columns:
```bash
genfile -o 10000000000 out.csv seqint
```

Remarks:
* the purpose of a same sequential number for both the identifier and the value is to test
that a value is associated with the correct identifier.
* one can use the `shuf` command (first removing and then adding `val`) to shuffle the input lines.

### Create a tree

Create a simple tree containing 2 billion entries consisting of tuples having the
same identifiers (a row number) and value (the row number):
```bash
./mkseq.bash 2000000000 | ./mkbst -h bigtest --id-type u4 --val-type u4
```
Both identifiers and values are stored on regular 32bit unsigned integers.

### Perform queries

Perform queries:
```bash
time qbst bigtest.bstree.bin get -v 256984
time qbst bigtest.bstree.bin knn -v 69853145 -k 10
time qbst bigtest.bstree.bin range -l 10000 -f 25639 -t 250000
time qbst bigtest.bstree.bin range -c -f 25639 -t 250000
```

Test on 10 million random f32 values
------------------------------------

Generate 10 million random f64 and create a bstree storing id on 32 bit integers and value on 32bit floats
```bash
genfile 10000000 randf64 | mkbst -h  test_10m --id-type u4 --val-type f4
```

Look at the nearest value from 0.5
```bash
time qbst test_10m.bstree.bin nn value 0.5
```

Look at the 10 nearest values from 0.2 (the result is ordered by distance to 0.2)
```bash
time qbst test_10m.bstree.bin knn -v 0.2 -k 10
```

Count the number of entries havig value in 0.4 and 0.6
```bash
time qbst test_10m.bstree.bin range -f 0.4 -t 0.6 -c
```

Priny the value in the range 0.49999 and 0.50001 (the result is ordered by increasing values)
```bash
time qbst test_10m.bstree.bin range -f 0.49999 -t 0.50001
```


Benchmark
---------

Test on my personal desktop (MVNe SSD, 16 GB RAM, AMD Ryzen) on a 20 GB file containing
sequential numbers (from 0 to 1999999999).
```bash
> time mkbst -h --input test.2billion.csv test2b --id-type u4 --val-type u4

real	9m49,775s
user	7m37,361s
sys	0m50,522s
```
It took less than 10min to build the 15 GB ouput file (ok, the input file is alreay sorted).

```bash
> qbst test2b.bstree.bin info

{
  "types": [
    "U32",
    "U32"
  ],
  "constants": {
    "n_entries": 2000000000,
    "entry_byte_size": 8,
    "n_entries_per_l1page": 4096,
    "n_l1page_per_ldpage": 255
  },
  "layout": {
    "depth": 2,
    "n_entries_root": 1914,
    "n_entries_main": 1999622790,
    "rigthmost_subtree": {
      "depth": 1,
      "n_entries_root": 92,
      "n_entries_main": 376924,
      "rigthmost_subtree": {
        "depth": 0,
        "n_entries_root": 286,
        "n_entries_main": 286,
        "rigthmost_subtree": null
      }
    }
  }
}
```

Simple exact value query:
```bash
> time qbst test2b.bstree.bin get value 1569874529

id,val
1569874529,1569874529

real	0m0,002s
user	0m0,000s
sys	0m0,002s
```

Nearest neighbour query
```bash
> time qbst test2b.bstree.bin nn -v 3000000000

distance,id,val
1000000001,1999999999,1999999999

real	0m0,002s (0m0,009s at the first execution)
user	0m0,002s
sys	0m0,000s
```

K nearest neighbours query
```bash
> time qbst test2b.bstree.bin knn -v 25684 -k 10

distance,id,val
0,25684,25684
1,25685,25685
1,25683,25683
2,25686,25686
2,25682,25682
3,25681,25681
3,25687,25687
4,25688,25688
4,25680,25680
5,25679,25679

real	0m0,002s (0m0,005s at the first execution)
user	0m0,002s
sys	0m0,000s
```

Range query
```bash
> time qbst test2b.bstree.bin range -l 10 -f 25698470 -t 25698570

id,val
25698470,25698470
25698471,25698471
25698472,25698472
25698473,25698473
25698474,25698474
25698475,25698475
25698476,25698476
25698477,25698477
25698478,25698478
25698479,25698479

real	0m0,002s
user	0m0,000s
sys	0m0,002s
```

At first execution, the limiting factor is the disk access.
At the second execution, the limiting factor is the time required by the OS to handle the process.
The 2ms incude the time needed to read the tree metadata.

Generate 100,000 random point in 0 - 2billion:
```bash
./genfile 2000000000 randint | head -100001 | tail -n +2 > toto.list
```

In release mode (third execution):
```bash
time qbst test2b.bstree.bin get list toto.list > toto.res.txt

real	0m0,732s
user	0m0,245s
sys	0m0,481s
```
i.e. a mean of less than 8 micro second per query (including parsing, conversion to string and writing the result in a file)!
(The mean is ~0.14 ms/query at the first execution)

```bash
> time qbst test2b.bstree.bin nn list toto.list > toto.res.txt

real	0m16,224s
user	0m0,482s
sys	0m3,222s

> time qbst test2b.bstree.bin nn list toto.list > toto.res.txt

real	0m1,651s
user	0m0,256s
sys	0m0,804s

> time qbst test2b.bstree.bin nn list toto.list > toto.res.txt

real	0m0,760s
user	0m0,251s
sys	0m0,509s
```

* First execution: 0.16 ms/query
* Thrid execution: 7.60 us/query

We redo those query sorting the random number:
```bash
./genfile 2000000000 randint | head -100001 | tail -n +2 | sort -n > toto.list
```
The results are similar.

We recall that the index file is 15 GB large, so 2nd excecution is faster since the data
is in the disk cache.


Bench on 10 billion f32 random values in `[0, 1)`
--------------------------------------------------

Generate the value and the index at once:
```bash
nohup sh -c "genfile 10000000000 randf64 | mkbst -h test.10b.randf64 --id-type u5 --val-type f4" &
```
Or in two steps
```bash
genfile 10000000000 randf64 > test_10b_randf64.csv
mkbst -h --input test_10b_randf64.csv test.10b.randf64 --id-type u5 --val-type f4
```

The size of the index is `~85 GB`.

The queries have been tested on 2 distinct hardware:
* Desktop computer
    + 1 TB, 7200 RPM, 32 MB cache HDD (HGST HTS721010A9E630)
    + Intel(R) Core(TM) i5-6600 CPU @ 3.30GHz (6 MB "smart cache")
    + 16 GB DDR4 2133 MHz
* Server
    + RAID of 5 SSDs (Samsung SATA SSDs)
    + 2x Intel(R) Xeon(R) CPU E5-2650 v3 @ 2.30GHz (25 MB "smart cache")
    + 64 GB DDR4 2133 MHz

The table return the "real" time provided by the "time" command.  
Each command starts by `time qbst test.10b.bstree.bin` plus the `Query params`.

Here the command to generate the list input file in the queries using `list`:
```bash
genfile 10000 randf64 | egrep -v "val" | sort -n > randf64_4q.csv
```

Query params | 1st or 2nd | Result | Desktop | Server
-------------|------------|--------|---------|--------
nn value 0.5 | 1 | | 0m0,071s | 0m0.013s
nn value 0.5 | 2 | | 0m0,004s | 0m0.003s
knn -v 0.8 -k 10 | 1 | | 0m0,034s | 0m0.007s
knn -v 0.8 -k 10 | 2 | | 0m0,004s | 0m0.003s
all -v 0.8 -c | 1/2 | 588 | 0m0,004s | 0m0.005s
all -v 0.2 -c | 1 | 148 | 0m0,026s | 0m0.011s
all -v 0.2 -c | 2 | 148 | 0m0,004s | 0m0.003s
range -f 0.4 -t 0.5 -c | 1/2 | 1000028688 | 1m5,450s | 1m57.212s
range -f 0.150 -t 0.149 -c | 1 | 157 | 0m0,028s | 0m0.025s
range -f 0.150 -t 0.149 -c | 2 | 157 | 0m0,004s | 0m0.003s
get list 1000.csv > res.csv | 1 | | 0m9,392s | 0m0.251s
get list 1000.csv > res.csv | 2 | | 0m0,026s | 0m0.034s
get list 10000.csv > res.csv | 1 | | 1m40,354s | 0m3.807s 
get list 10000.csv > res.csv | 2 | | 0m0,181s | 0m0.207s
get list 100000.csv > res.csv | 1 | | | 0m24.548s

In the last case (100000 queries, no data in the disk cache), 
the mean query time is about 0.24 ms (which is probably not far from the disk access time).

In the previous results, we clearly see the effect of the spinning vs SSD disk at the first execution.
On the query `range -f 0.4 -t 0.5 -c` we see that the server has a slower CPU.

The query `get list 10000.csv` is very interesting (a factor more than x20 in performances!):
* the mean time on a spinning disk is about 10 ms
* the mean time on a SSD disk is about 0.4 ms 

For the query `get list 100000.csv > res.csv`, the result time is about the 
same the input being sorted or not. 


Bench with Gaia DR2 data (1.6 Billion entries)
----------------------------------------------

### With this BSTree index

The use case is simple: from the [Gaia DR2](http://vizier.u-strasbg.fr/viz-bin/VizieR-3?-source=I/345/gaia2) 
`Source` unique identifier, I want to retrieve the associated position (I am using the formatted position
`%015.9%+015.9f` as a string made of 30 ASCII chars).  
Thus here the value I want to index is `Source` and the associated identifier is the position
(yes it is different from a HashMap in which `Source` would be the (unique) key and the position the associated value
because in a bs-tree the indexed value is not necessarily unique).

In input, the files looks like:
```bash
ra,dec,source
45.00431616421,0.02104503269,34361129088
44.99615368416,0.00561580621,4295806720
45.00497424498,0.01987700037,38655544960
44.96389532530,0.04359518482,343597448960
...
```
and contains 1,69,2919,136 rows (including the header line).

I build and exec the following script
```bash
#!/bin/bash

LC_NUMERIC=C LC_COLLATE=C; \
tail -n +2 gaia_dr2.idradec.csv | tr ',' ' ' |\
while read line; do printf "%015.11f%+015.11f,%d\n" $line; done |\
mkbst gaia_dr2_source --val 1 --id 0 --id-type t30 --val-type u8 
```
(the process is quite slow due to the bash `while` and `printf`).

I then have a 2.5 GB file `Gaia_source.txt` containing more 132,739,322 `Source`, looking like:
```bash
2448780173659609728
2448781208748235648
2448689605685695488
2448689777484387072
2448783991887042176
...
```

Two consecutive executions (slow 7200 RPM HDD) gives:
```bash
time qbst ../gaia_dr2_source.bstree.bin get list Gaia_source.txt > Gaia.test.csv

real	24m4,323s
user	5m8,834s
sys	4m52,898s
```
and
```bash
time qbst ../gaia_dr2_source.bstree.bin get list Gaia_source.txt > Gaia.test.csv

real	13m58,572s
user	4m2,162s
sys	3m32,534s
```
I guess that the second execution benefits from HDD cache.

Now sorting the input and querying again leads to:
```bash
time qbst ../gaia_dr2_source.bstree.bin get list Gaia_source.sorted.txt > Gaia.test.sorted.csv

real	5m53,569s
user	2m43,339s
sys	2m28,872s
```
The output file is 6.3 GB large.

### With PSQL10

I install PSQL10 on Ubuntu via `apt`, create a user and move the database out of the system disk:
```bash
sudo apt-get install postgresql-10
sudo -u postgres createuser --interactive
sudo -u postgres createdb fxtests
sudo systemctrl stop postgresql
sudo rsync -av /var/lib/postgresql /data-local/psql/ 
sudo vim /etc/postgresql/10/main/postgresql.conf
sudo systemctrl start postgresql
```

I create two tables (the index and the data tables) and copy data:
```bash
CREATE TABLE gaia2_idpos (
  source BIGINT PRIMARY KEY,
  pos CHAR(30) NOT NULL
);

COPY gaia2_idpos(pos, source)
FROM '/data-local/org/gaia_dr2.idradec.4index.csv'
DELIMITER ',';

```
and
```bash
CREATE TABLE gaia2_id (
  source BIGINT PRIMARY KEY
)

COPY gaia2_id(source)
FROM '/data-local/org/aas/Gaia_source.txt'
DELIMITER ','
```
And perform the query:
```bash
time psql -d fxtests -t -A -F"," -c "SELECT b.* FROM gaia2_id as a NATURAL JOIN gaia2_idpos as b" > output.csv

real	53m17,051s
user	1m4,109s
sys	0m50,723s
```

Remarks:
* I tested PSQL out of the box, without modifying any parameters
* In the PSQL test, using a PRIMARY KEY for both tables, the result is to be compared with the sorted input in the  BSTree test.

TODO list
---------

* [X] add the possibility to query by a list of target
* [X] make a simple test with PSQL
* [ ] replace memory map by pread/pwrite? (see e.g. [positioned-io](https://github.com/vasi/positioned-io) or [scroll](https://github.com/m4b/scroll))
* [ ] remove the code which is now obsolete (e.g. `get` overwritten by `get exact visitor`)
* [ ] add much more tests
* [ ] add benchmarks
* [ ] try to reduce the code redundancy (particularly in `SubTreeW` and `SubTreeR`)
* [ ] add support for NULL values (storing them separately, out of the tree structure)
* [ ] perform tests with [SQLx](https://github.com/launchbadge/sqlx) and PostgreSQL
      to have a reference time (would be nice if we are at least as fast)

Acknowledgements
----------------

If you use this code and work in a scientific public domain
(especially astronomy), please acknowledge its usage and the 
[CDS](https://en.wikipedia.org/wiki/Centre_de_donn%C3%A9es_astronomiques_de_Strasbourg)
who developed it. 
It may help us in promoting our work to our financiers.

Warning
-------

If the compilation fails with a message like
```bash
Caused by:
  process didn't exit successfully: `rustc --crate-name bstree_file [...]
  (signal: 9, SIGKILL: kill)
```
, try
```bash
dmesg | egrep -i 'killed process'
```
If the result looks like
```bash
[...] Out of memory: Killed process xxxxx (rustc)
```
it means that your machine was out of memory.


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

