
BEGIN {
	print "const ALL_ALPHA3_CODES: &[&str] = &[";
}

$1 ~ "^#.*" {
	next;
}

{
	print "    \""$2"\", /* "$5" */";
}

END {
	print "];";
}

