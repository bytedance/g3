
BEGIN {
	print "    pub fn continent(&self) -> ContinentCode {";
	print "        match self {";
}

$1 ~ "^#.*" {
	next;
}

{
	print "            ISO3166Alpha2CountryCode::"$1" => ContinentCode::"$9",";
}

END {
	print "        }";
	print "    }";
}

