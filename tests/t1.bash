#!/bin/bash

echo "Test a tree with a depth = 3"

##########################################################
# Test a tree with a depth = 4 by:                       #
# * limiting the L1 cache to 1k and the disk cache to 8k #
# * using long integers to store both the id and value   #
##########################################################
# genfile 10000000 seqint | mkbst -h --l1 1 --disk 8  test.10m.d3 --id-type u8 --val-type u8

# Generate 10 million random f64 and
# create a bstree storing id on 32 bit integers and value on 32bit floats
genfile 10000000 randf64 | mkbst -h  test.10m.d3 --id-type u4 --val-type f4

# Look at the nearest value from 0.5
time qbst test.10m.bstree.bin nn value 0.5
# Look at the 10 nearest values from 0.2 (the result is ordered by distance to 0.2)
time qbst test.10m.bstree.bin knn -v 0.2 -k 10
# Count the number of entries havig value in 0.4 and 0.6
time qbst test.10m.bstree.bin range -f 0.4 -t 0.6 -c
# Print the value in the range 0.49999 and 0.50001 (the result is ordered by increasing values)
time qbst test.10m.bstree.bin range -f 0.49999 -t 0.50001

