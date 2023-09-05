
BEGIN {
	print "impl ISO3166Alpha2CountryCode {";
	print "    pub fn name(&self) -> &'static str {";
	print "        match self {";
}

$1 ~ "^#.*" {
	next;
}

{
	print "            ISO3166Alpha2CountryCode::"$1" => \""$5"\",";
}

END {
	print "        }";
	print "    }";
	print "}";
}

