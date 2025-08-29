
$1 ~ "^#.*" {
	next;
}

{
	print "            IsoCountryCode::"$1" => ContinentCode::"$9",";
}

