
BEGIN {
	print "impl FromStr for ISO3166Alpha3CountryCode {";
	print "    type Err = ();";
	print "";
	print "    fn from_str(s: &str) -> Result<Self, Self::Err> {";
	print "        match s {";
}

$1 ~ "^#.*" {
	next;
}

{
	print "            \""$2"\" | \""tolower($2)"\" => Ok(ISO3166Alpha3CountryCode::"$2"),";
}

END {
	print "            _ => Err(()),";
	print "        }";
	print "    }";
	print "}";
}

