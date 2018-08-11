FROM base/archlinux
RUN pacman -Sy --noconfirm base-devel && \
    useradd --create-home pkgbuilder
ADD . /home/pkgbuilder/pac4cli
RUN chown -R pkgbuilder:pkgbuilder /home/pkgbuilder/pac4cli
WORKDIR /home/pkgbuilder/pac4cli
RUN pacman -Sy --noconfirm $( bash -c '. PKGBUILD && echo ${depends[@]} ${makedepends[@]}' )
USER pkgbuilder
RUN makepkg
USER root
RUN pacman --noconfirm -U pac4cli*pkg.tar.xz
EXPOSE 3128
CMD pac4cli -p 3128
