#!/bin/bash
printf "This script will download Sentinel-2 test data from the Copernicus OData api.\n"
printf "Please provide Copernicus credentials:\n"
read -rp 'username: ' username
read -rsp 'password: ' password
printf "\n"

access_token=$(curl -s -d 'client_id=cdse-public' -d username=${username} -d password=${password} -d 'grant_type=password' 'https://identity.dataspace.copernicus.eu/auth/realms/CDSE/protocol/openid-connect/token' | python3 -m json.tool | grep "access_token" | awk -F\" '{print $4}')

for tile_name in S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342 S2B_MSIL2A_20241206T093309_N0511_R136_T33PTM_20241206T115919; do
    if [ ! -f "$(dirname "$(readlink -f "$0")")/${tile_name}.SAFE.zip" ]; then 
        tile_id=$(curl -s "https://catalogue.dataspace.copernicus.eu/odata/v1/Products?\$filter=Name%20eq%20%27${tile_name}.SAFE%27" | python3 -m json.tool | grep "Id" | awk -F\" '{print $4}')
        wget  --header "Authorization: Bearer $access_token" "https://download.dataspace.copernicus.eu/odata/v1/Products(${tile_id})/\$value" -O "$(dirname "$(readlink -f "$0")")/${tile_name}.SAFE.zip"
    fi
done