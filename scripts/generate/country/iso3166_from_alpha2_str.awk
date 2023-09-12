
$1 ~ "^#.*" {
	next;
}

{
	print "                \""$1"\" => Ok(IsoCountryCode::"$1"),";
}

