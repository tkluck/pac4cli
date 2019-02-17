#!/usr/bin/env bash

cd ..

docker build -t archlinux-pac4cli -f archlinux/Dockerfile .

RES=$?
if [[ RES -ne 0 ]]; then
	exit 1;
fi

docker run -it archlinux-pac4cli
