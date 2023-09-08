
BEGIN {
	print "    pub fn continent(&self) -> ContinentCode {";
	print "        match self {";
}

$1 ~ "^#.*" {
	next;
}

{
	print "            ISO3166Alpha3CountryCode::"$2" => ContinentCode::"$9",";
}

END {
	print "        }";
	print "    }";
}

