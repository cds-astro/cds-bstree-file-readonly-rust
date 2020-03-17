#!/bin/bash

declare -rx END=$1

echo "val"

COUNTER=0
until [ $COUNTER -ge ${END} ]; do
  echo $COUNTER
  let COUNTER+=1
done

