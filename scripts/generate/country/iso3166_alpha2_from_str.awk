
BEGIN {
	print "impl FromStr for ISO3166Alpha2CountryCode {";
	print "    type Err = ();";
	print "";
	print "    fn from_str(s: &str) -> Result<Self, Self::Err> {";
	print "        match s {";
}

$1 ~ "^#.*" {
	next;
}

{
	print "            \""$1"\" | \""tolower($1)"\" => Ok(ISO3166Alpha2CountryCode::"$1"),";
}

END {
	print "            _ => Err(()),";
	print "        }";
	print "    }";
	print "}";
}

