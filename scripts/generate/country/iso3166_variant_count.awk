
BEGIN {
    LAST_CODE="AN"
}

$1 ~ "^#.*" {
	next;
}

{
	LAST_CODE=$1;
}

END {
	print "    pub fn variant_count() -> usize {";
	print "        Self::"LAST_CODE" as usize";
	print "    }";
}

