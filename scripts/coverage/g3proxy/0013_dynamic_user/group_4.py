
from pathlib import Path


def fetch_users():
    script_dir = Path(__file__).parent
    json_file = script_dir.joinpath('group_1.json')
    content = json_file.read_text()
    return content

def report_ok():
    # optional, takes no argument
    pass

def report_err(errmsg):
    # optional, takes one positional argument, which is the error message string
    pass


if __name__ == '__main__':
    print(fetch_users())
