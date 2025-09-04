
BEGIN {
    CHARS["25"]="Z"
    CHARS["24"]="Y"
    CHARS["23"]="X"
    CHARS["22"]="W"
    CHARS["21"]="V"
    CHARS["20"]="U"
    CHARS["19"]="T"
    CHARS["18"]="S"
    CHARS["17"]="R"
    CHARS["16"]="Q"
    CHARS["15"]="P"
    CHARS["14"]="O"
    CHARS["13"]="N"
    CHARS["12"]="M"
    CHARS["11"]="L"
    CHARS["10"]="K"
    CHARS["9"]="J"
    CHARS["8"]="I"
    CHARS["7"]="H"
    CHARS["6"]="G"
    CHARS["5"]="F"
    CHARS["4"]="E"
    CHARS["3"]="D"
    CHARS["2"]="C"
    CHARS["1"]="B"
    CHARS["0"]="A"
    LAST_CODE="AN"
}

$1 ~ "^#.*" {
	next;
}

{
	CODE_MAP[$1] = "IsoCountryCode::"$1;
	LAST_CODE=$1;
}

END {
    for (i = 0; i < 26; i++) {
        for (j = 0; j < 26; j++) {
            c = CHARS[i]""CHARS[j];
            id = (i * 26) + j;
            if (c in CODE_MAP) {
                print "    "CODE_MAP[c]", // "id" - "c;
            } else {
                print "    "CODE_MAP[LAST_CODE]", // "id
            }
        }
    }
}
