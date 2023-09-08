
$1 ~ "^#.*" {
	next;
}

{
	print "                \""$1"\" | \""tolower($1)"\" => Ok(IsoCountryCode::"$1"),";
}

