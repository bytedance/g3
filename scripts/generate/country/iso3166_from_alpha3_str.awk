
$1 ~ "^#.*" {
	next;
}

{
	print "                \""$2"\" | \""tolower($2)"\" => Ok(IsoCountryCode::"$1"),";
}

