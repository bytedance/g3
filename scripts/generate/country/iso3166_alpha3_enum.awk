
BEGIN {}

$1 ~ "^#.*" {
	next;
}

{
	print "    "$2", /* "$5" */";
}

END {}

