#!/bin/bash

declare -rx END=$1
declare -rx OUT=vals.csv

echo "val" > ${OUT}

for i in $(seq 1 $END); do 
  echo $i >> ${OUT}
done



