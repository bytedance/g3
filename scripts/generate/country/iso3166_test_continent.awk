$1 ~ "^#.*" {
	next;
}

{
	print "        assert_eq!(IsoCountryCode::"$1".continent(), ContinentCode::"$9");";
}
