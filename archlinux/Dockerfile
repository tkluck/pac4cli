FROM archlinux/base

RUN useradd --create-home pkgbuilder
ADD . /home/pkgbuilder/pac4cli
RUN chown -R pkgbuilder:pkgbuilder /home/pkgbuilder/pac4cli

WORKDIR /home/pkgbuilder/pac4cli/archlinux

RUN pacman -Syu --noconfirm base-devel $( bash -c '. PKGBUILD && echo ${depends[@]} ${makedepends[@]}' ) && \
		sed -i -e \
		's@https://github.com/tkluck/pac4cli.git@file:///home/pkgbuilder/pac4cli@' PKGBUILD

USER pkgbuilder
RUN makepkg

USER root
RUN pacman --noconfirm -U pac4cli*pkg.tar.xz

ENTRYPOINT /bin/bash
