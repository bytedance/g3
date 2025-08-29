
BEGIN {
    LAST_CODE="AN"
}

$1 ~ "^#.*" {
	next;
}

{
	LAST_CODE=$1;
}

END {
	print LAST_CODE;
}

