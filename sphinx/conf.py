# File: docs/conf.py

extensions = [
    "multiproject",
]

# Define the projects that will share this configuration file.
multiproject_projects = {
    "project": {
        "path": "project",
    },
    "g3proxy": {
        "path": "g3proxy",
    },
    "g3tiles": {
        "path": "g3tiles",
    },
    "g3statsd": {
        "path": "g3statsd",
    },
    "g3keymess": {
        "path": "g3keymess",
    },
}
