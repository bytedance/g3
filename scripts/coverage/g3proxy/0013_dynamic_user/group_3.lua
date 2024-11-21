
local script_dir = __file__:match("(.*/)")
local file = io.open(string.format("%s%s", script_dir, "group_1.json"), "r")
local content = file:read "*a"
file:close()
-- return the json encoded string
return content
