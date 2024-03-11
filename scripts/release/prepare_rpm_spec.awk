
BEGIN {
	OLD_VERSION = "";
}

$1 == "Version:" {
	OLD_VERSION = $2;
	# escape '+' in version string
	sub("[+]", "[+]", OLD_VERSION)
	sub(OLD_VERSION, VERSION, $0)
	print $0;
	next;
}

$1 == "%changelog" {
	exit 0;
}

{
	print $0;
}

