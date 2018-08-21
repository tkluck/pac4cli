#!/usr/bin/env bash

SRC_DIR=$1

if [[ "x${SRC_DIR}" == "x" ]]; then
	echo "Usage:"
	echo "  test-arch-package <project_src_dir>"
	exit 1
fi

cd $SRC_DIR

docker build -t pac4cli -f archlinux/Dockerfile .

#systemd expects:
# - /run to be a tmpfs
# - /sys/fs/cgroup mounted (read only is enough)
# - Not sure about the `--tmpfs /tmp`. But without it systemd is unable to mount
# to /tmp. It seems that systemd needs /tmp to be a tmpfs
#
# TODO: Currently this is a kind of noop container. I need to figure out how to
# start it with a command (test script).
docker run -d --tmpfs /tmp --tmpfs /run -v /sys/fs/cgroup:/sys/fs/cgroup:ro pac4cli
