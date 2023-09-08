
BEGIN {
	print "    pub fn continent(&self) -> ContinentCode {";
	print "        match self {";
}

$1 ~ "^#.*" {
	next;
}

{
	print "            IsoCountryCode::"$1" => ContinentCode::"$9",";
}

END {
	print "        }";
	print "    }";
}

