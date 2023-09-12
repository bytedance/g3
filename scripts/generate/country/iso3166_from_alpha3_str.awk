
$1 ~ "^#.*" {
	next;
}

{
	print "                \""$2"\" => Ok(IsoCountryCode::"$1"),";
}

