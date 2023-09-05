
BEGIN {
	print "pub enum ISO3166Alpha2CountryCode {";
}

$1 ~ "^#.*" {
	next;
}

{
	print "    "$1",";
}

END {
	print "}";
}

