
$0 ~ /^warning: Patch .* was not used in the crate graph.$/ {
    print substr($3,2)"/"substr($4,2);
}

$0 ~ /^Patch .* was not used in the crate graph.$/ {
    print substr($2,2)"/"substr($3,2);
}
