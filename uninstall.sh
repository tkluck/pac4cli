#! /usr/bin/env bash

DESTDIR=$1

systemctl stop pac4cli.service
systemctl disable pac4cli.service
rm -f ${DESTDIR}lib/systemd/system/pac4cli.service
rm -f ${DESTDIR}etc/NetworkManager/dispatcher.d/trigger-pac4cli
rm -f ${DESTDIR}etc/profile.d/pac4cli.sh
