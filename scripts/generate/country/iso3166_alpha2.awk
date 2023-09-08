
BEGIN {
	print "const ALL_ALPHA2_CODES: &[&str] = &[";
}

$1 ~ "^#.*" {
	next;
}

{
	print "    \""$1"\", /* "$5" */";
}

END {
	print "];";
}

