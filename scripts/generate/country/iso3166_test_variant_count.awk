$1 ~ "^#.*" {
	next;
}

{
	last_country = $1;
}

END {
	print last_country;
}
