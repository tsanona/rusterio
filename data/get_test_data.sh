#!/bin/bash

re="_T([[:digit:]]{2})([[:upper:]])([[:upper:]]{2})"
for tile in S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342 S2B_MSIL2A_20241206T093309_N0511_R136_T33PTM_20241206T115919
do
[[ $tile =~ $re ]]
gcloud storage cp -r gs://gcp-public-data-sentinel-2/L2/tiles/${BASH_REMATCH[1]}/${BASH_REMATCH[2]}/${BASH_REMATCH[3]}/${tile}.SAFE "$(dirname "$(readlink -f "$0")")"
zip -r "$(dirname "$(readlink -f "$0")")/${tile}.SAFE.zip" "$(dirname "$(readlink -f "$0")")/${tile}.SAFE"
rm -rf "$(dirname "$(readlink -f "$0")")/${tile}.SAFE"
done