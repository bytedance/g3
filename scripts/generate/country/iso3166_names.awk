
BEGIN {
	print "const ALL_COUNTRY_NAMES: &[&str] = &[";
}

$1 ~ "^#.*" {
	next;
}

{
	print "    \""$5"\",";
}

END {
	print "];";
}

