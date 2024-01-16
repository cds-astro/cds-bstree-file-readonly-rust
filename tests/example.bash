#!/bin/bash

# To generate random integers instead of random floats
# genfile 10000000 seqint | mkbst -h test.10m.d3 --id-type u8 --val-type u8

echo "# Generate 10 million random f64 and create a bstree storing"
echo "#    * ids (sequential row number) on 32 bit integers"
echo "#    * values (random numbers)     on 32 bit floats"
echo "# (the longest part is the random generation)"
time genfile 10000000 randf64 | mkbst -h test.10m.d3.bstree --fill-factor 0.8 --id-type u4 --val-type f4
#echo "# Using options '--l1 1' and '--disk 8' we:"
#echo "#    * assume a L1 cache of only 1kB"
#echo "#    * assume a disk cache of only 8kB"
#echo "#    * => it leads to a tree of depth=3 (instead of depth=1 with default options)"
#time genfile 10000000 randf64 | mkbst -h --l1 1 --disk 8 test.10m.d3.bstree --id-type u4 --val-type f4

echo "# Look at the nearest value from 0.5"
time qbst test.10m.d3.bstree nn value 0.5
echo "# Look at the 5 nearest values from 0.2 (the result is ordered by distance to 0.2)"
time qbst test.10m.d3.bstree knn --value 0.2 -k 5
echo "# Count the number of entries havig value in 0.4 and 0.6"
time qbst test.10m.d3.bstree range --from 0.4 --to 0.6 --count
echo "# Print the values in the range 0.499999 and 0.500001 (the result is ordered by increasing values)"
time qbst test.10m.d3.bstree range --from 0.499999 --to 0.500001

