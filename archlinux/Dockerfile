FROM base/devel

#systemd needs to know it is running in a container
ENV container docker

#For systemd to correctly teminate services with `docker stop`
STOPSIGNAL SIGRTMIN+3

RUN pacman -Syu --noconfirm && useradd --create-home pkgbuilder
ADD . /home/pkgbuilder/pac4cli
RUN chown -R pkgbuilder:pkgbuilder /home/pkgbuilder/pac4cli

WORKDIR /home/pkgbuilder/pac4cli/archlinux
#fake a new release locally & use local repo
RUN pacman -Sy --noconfirm $( bash -c '. PKGBUILD && echo ${depends[@]} ${makedepends[@]}' ) && \
		git tag v100 && \
		sed -i -e \
		's@https://github.com/tkluck/pac4cli.git@file:///home/pkgbuilder/pac4cli@' PKGBUILD

USER pkgbuilder
RUN makepkg

USER root
RUN pacman --noconfirm -U pac4cli*pkg.tar.xz

#EXPOSE 3128
#CMD pac4cli --bind 0.0.0.0 -p 3128


#enable NetworkManager & pac4cli
RUN ln -s /usr/lib/systemd/system/NetworkManager.service \
    	/etc/systemd/system/multi-user.target.wants/NetworkManager.service && \
		ln -s /usr/lib/systemd/system/pac4cli.service	\
			/etc/systemd/system/multi-user.target.wants/pac4cli.service

ENTRYPOINT /usr/bin/init
